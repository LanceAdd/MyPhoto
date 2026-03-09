import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface Workspace {
  id: number
  path: string
  name: string
  last_opened_at: string | null
  settings_json: string
  photo_count: number
}

export interface Photo {
  id: number
  workspace_id: number
  relative_path: string
  filename: string
  file_size: number | null
  width: number | null
  height: number | null
  taken_at: string | null
  camera_make: string | null
  camera_model: string | null
  lens_model: string | null
  shutter_speed: string | null
  aperture: number | null
  iso: number | null
  focal_length: number | null
  file_modified_at: string | null
  is_missing: boolean
  star_rating: number
  color_label: string
  notes: string
}

export interface WorkspaceFile {
  relative_path: string
  filename: string
}

interface PhotoMetaEntry {
  photo_id: number
  star_rating: number
  color_label: string
  notes: string
}

export interface PhotoFilter {
  subfolder?: string
  star_min?: number
  star_none?: boolean
  color_labels?: string[]
  color_none?: boolean
  sort_by?: string
  sort_desc?: boolean
  include_missing?: boolean
}

export interface WorkspaceTab {
  workspace: Workspace
  photos: Photo[]
  subfolders: string[]
  treeFiles: WorkspaceFile[]
  filter: PhotoFilter
  selectedIds: Set<number>
  activePhotoId: number | null
  viewMode: 'grid' | 'cull'
  gridLayout: 'fit' | 'flow'
  thumbnailSize: number
  scrollTop: number
  scanning: boolean
  cullIndex: number // current index in cull mode
}

export const useWorkspaceStore = defineStore('workspace', () => {
  const tabs = ref<WorkspaceTab[]>([])
  const activeTabIndex = ref(0)
  const metaCacheByWorkspace = new Map<number, Map<number, PhotoMetaEntry>>()
  const removedMetaCacheByWorkspace = new Map<number, Map<number, PhotoMetaEntry>>()
  const metaHydratedWorkspace = new Set<number>()
  const metaHydrationInFlight = new Set<number>()
  const metaPresenceSyncInFlight = new Set<number>()
  const pendingCreatedPathsByWorkspace = new Map<number, Set<string>>()
  const pendingRemovedPathsByWorkspace = new Map<number, Set<string>>()
  const fileEventFlushTimerByWorkspace = new Map<number, number>()

  const activeTab = computed(() => tabs.value[activeTabIndex.value] ?? null)
  const activePhotos = computed(() => activeTab.value?.photos ?? [])
  const selectedPhotos = computed(() =>
    activePhotos.value.filter(p => activeTab.value?.selectedIds.has(p.id))
  )

  async function openWorkspace(path: string) {
    // Check if already open
    const existing = tabs.value.findIndex(t => t.workspace.path === path)
    if (existing >= 0) {
      activeTabIndex.value = existing
      return
    }

    const ws: Workspace = await invoke('open_workspace', { path })
    const settings = tryParseSettings(ws.settings_json)

    const tab: WorkspaceTab = {
      workspace: ws,
      photos: [],
      subfolders: [],
      treeFiles: [],
      filter: settings.filter ?? {},
      selectedIds: new Set(),
      activePhotoId: null,
      viewMode: settings.viewMode ?? 'grid',
      gridLayout: settings.gridLayout ?? 'fit',
      thumbnailSize: settings.thumbnailSize ?? 140,
      scrollTop: settings.scrollTop ?? 0,
      scanning: true,
      cullIndex: 0,
    }

    tabs.value.push(tab)
    activeTabIndex.value = tabs.value.length - 1

    const tabIndex = activeTabIndex.value
    void loadPhotos(tabIndex, { refreshMeta: true })
      .then(() => {
        const target = tabs.value[tabIndex]
        if (!target || target.workspace.id !== ws.id) return
        target.subfolders = deriveSubfoldersFromPhotos(target.photos)
        target.treeFiles = deriveTreeFilesFromPhotos(target.photos)
      })
      .catch(() => {})
  }

  async function closeTab(index: number) {
    const tab = tabs.value[index]
    if (tab) {
      const wsId = tab.workspace.id
      await saveTabSettings(tab)
      await invoke('close_workspace', { workspaceId: wsId })
      tabs.value.splice(index, 1)
      if (!tabs.value.some(t => t.workspace.id === wsId)) {
        metaCacheByWorkspace.delete(wsId)
        removedMetaCacheByWorkspace.delete(wsId)
        metaHydratedWorkspace.delete(wsId)
        metaHydrationInFlight.delete(wsId)
        metaPresenceSyncInFlight.delete(wsId)
        pendingCreatedPathsByWorkspace.delete(wsId)
        pendingRemovedPathsByWorkspace.delete(wsId)
        const timer = fileEventFlushTimerByWorkspace.get(wsId)
        if (timer != null) {
          window.clearTimeout(timer)
          fileEventFlushTimerByWorkspace.delete(wsId)
        }
      }
      if (activeTabIndex.value >= tabs.value.length) {
        activeTabIndex.value = Math.max(0, tabs.value.length - 1)
      }
    }
  }

  function filterNeedsMetaQuery(filter: PhotoFilter): boolean {
    return filter.star_none === true ||
      (filter.star_min ?? 0) > 0 ||
      filter.color_none === true ||
      (filter.color_labels?.length ?? 0) > 0 ||
      filter.sort_by === 'star_rating'
  }

  function getWorkspaceMetaCache(workspaceId: number): Map<number, PhotoMetaEntry> {
    let cache = metaCacheByWorkspace.get(workspaceId)
    if (!cache) {
      cache = new Map<number, PhotoMetaEntry>()
      metaCacheByWorkspace.set(workspaceId, cache)
    }
    return cache
  }

  function getWorkspaceRemovedMetaCache(workspaceId: number): Map<number, PhotoMetaEntry> {
    let cache = removedMetaCacheByWorkspace.get(workspaceId)
    if (!cache) {
      cache = new Map<number, PhotoMetaEntry>()
      removedMetaCacheByWorkspace.set(workspaceId, cache)
    }
    return cache
  }

  function applyMetaEntry(photo: Photo, meta: PhotoMetaEntry) {
    photo.star_rating = meta.star_rating
    photo.color_label = meta.color_label
    photo.notes = meta.notes
  }

  function applyCachedMetaToTab(tab: WorkspaceTab) {
    const cache = metaCacheByWorkspace.get(tab.workspace.id)
    if (!cache) return
    for (const photo of tab.photos) {
      const meta = cache.get(photo.id)
      if (meta) applyMetaEntry(photo, meta)
    }
  }

  function seedMetaCacheFromPhotos(workspaceId: number, photos: Photo[]) {
    const cache = getWorkspaceMetaCache(workspaceId)
    const removed = getWorkspaceRemovedMetaCache(workspaceId)
    for (const photo of photos) {
      const fromRemoved = removed.get(photo.id)
      cache.set(photo.id, fromRemoved ?? {
        photo_id: photo.id,
        star_rating: photo.star_rating ?? 0,
        color_label: photo.color_label ?? '',
        notes: photo.notes ?? '',
      })
      removed.delete(photo.id)
    }
  }

  async function applyMetaCacheToWorkspaceTabs(workspaceId: number) {
    const cache = metaCacheByWorkspace.get(workspaceId)
    if (!cache) return
    const relevantTabs = tabs.value.filter(t => t.workspace.id === workspaceId)
    const chunkSize = 400

    for (const tab of relevantTabs) {
      for (let i = 0; i < tab.photos.length; i += chunkSize) {
        const end = Math.min(i + chunkSize, tab.photos.length)
        for (let j = i; j < end; j++) {
          const photo = tab.photos[j]
          const meta = cache.get(photo.id)
          if (meta) applyMetaEntry(photo, meta)
        }
        if (end < tab.photos.length) {
          await new Promise(resolve => setTimeout(resolve, 0))
        }
      }
    }
  }

  async function hydrateWorkspaceMeta(workspaceId: number, force = false) {
    if (!force && metaHydratedWorkspace.has(workspaceId)) return
    if (metaHydrationInFlight.has(workspaceId)) return

    metaHydrationInFlight.add(workspaceId)
    try {
      const rows: PhotoMetaEntry[] = await invoke('get_workspace_photo_meta', { workspaceId })
      const cache = getWorkspaceMetaCache(workspaceId)
      for (const row of rows) {
        cache.set(row.photo_id, row)
      }
      await syncRemovedMetaCache(workspaceId)
      await applyMetaCacheToWorkspaceTabs(workspaceId)
      metaHydratedWorkspace.add(workspaceId)
    } catch {
      // Ignore metadata hydration failures; UI still has basic photo data.
    } finally {
      metaHydrationInFlight.delete(workspaceId)
    }
  }

  function updateMetaCacheEntry(workspaceId: number, photoId: number, starRating: number, colorLabel: string, notes: string) {
    const cache = getWorkspaceMetaCache(workspaceId)
    const removed = getWorkspaceRemovedMetaCache(workspaceId)
    cache.set(photoId, {
      photo_id: photoId,
      star_rating: starRating,
      color_label: colorLabel,
      notes,
    })
    removed.delete(photoId)
  }

  function restoreVisiblePhotosFromRemovedCache(workspaceId: number, photos: Photo[]) {
    const cache = getWorkspaceMetaCache(workspaceId)
    const removed = getWorkspaceRemovedMetaCache(workspaceId)
    for (const photo of photos) {
      if (cache.has(photo.id)) continue
      const meta = removed.get(photo.id)
      if (!meta) continue
      removed.delete(photo.id)
      cache.set(photo.id, meta)
    }
  }

  function markPhotoRemovedInCache(workspaceId: number, photoId: number) {
    const cache = getWorkspaceMetaCache(workspaceId)
    const removed = getWorkspaceRemovedMetaCache(workspaceId)
    const meta = cache.get(photoId)
    if (!meta) return
    cache.delete(photoId)
    removed.set(photoId, meta)
  }

  async function syncRemovedMetaCache(workspaceId: number) {
    if (metaPresenceSyncInFlight.has(workspaceId)) return
    metaPresenceSyncInFlight.add(workspaceId)
    try {
      const presentIds: number[] = await invoke('get_workspace_present_photo_ids', { workspaceId })
      const presentSet = new Set(presentIds)
      const active = getWorkspaceMetaCache(workspaceId)
      const removed = getWorkspaceRemovedMetaCache(workspaceId)

      for (const [photoId, meta] of [...active.entries()]) {
        if (presentSet.has(photoId)) continue
        active.delete(photoId)
        removed.set(photoId, meta)
      }
      for (const [photoId, meta] of [...removed.entries()]) {
        if (!presentSet.has(photoId)) continue
        removed.delete(photoId)
        if (!active.has(photoId)) {
          active.set(photoId, meta)
        }
      }
    } catch {
      // Ignore presence sync failures; cache will recover on next successful sync.
    } finally {
      metaPresenceSyncInFlight.delete(workspaceId)
    }
  }

  function addPendingPaths(container: Map<number, Set<string>>, workspaceId: number, paths: string[]) {
    let set = container.get(workspaceId)
    if (!set) {
      set = new Set<string>()
      container.set(workspaceId, set)
    }
    for (const path of paths) {
      if (path) set.add(path)
    }
  }

  function scheduleFileEventFlush(workspaceId: number) {
    if (fileEventFlushTimerByWorkspace.has(workspaceId)) return
    const timer = window.setTimeout(() => {
      fileEventFlushTimerByWorkspace.delete(workspaceId)
      void flushFileEvents(workspaceId)
    }, 220)
    fileEventFlushTimerByWorkspace.set(workspaceId, timer)
  }

  async function flushFileEvents(workspaceId: number) {
    const tab = tabs.value.find(t => t.workspace.id === workspaceId)
    if (!tab) {
      pendingCreatedPathsByWorkspace.delete(workspaceId)
      pendingRemovedPathsByWorkspace.delete(workspaceId)
      return
    }

    const created = [...(pendingCreatedPathsByWorkspace.get(workspaceId) ?? new Set<string>())]
    const removed = [...(pendingRemovedPathsByWorkspace.get(workspaceId) ?? new Set<string>())]
    pendingCreatedPathsByWorkspace.delete(workspaceId)
    pendingRemovedPathsByWorkspace.delete(workspaceId)

    if (created.length === 0 && removed.length === 0) return

    try {
      if (removed.length > 0) {
        await invoke('sync_removed_files', {
          workspaceId,
          workspacePath: tab.workspace.path,
          paths: removed,
        })
      }
      if (created.length > 0) {
        await invoke('sync_created_files', {
          workspaceId,
          workspacePath: tab.workspace.path,
          paths: created,
        })
      }
    } catch {
      // If partial sync fails, keep eventual consistency via scan-complete/rescan flow.
    }

    const tabIndex = tabs.value.findIndex(t => t.workspace.id === workspaceId)
    if (tabIndex >= 0) {
      await loadPhotos(tabIndex, { refreshMeta: true })
      await syncRemovedMetaCache(workspaceId)
      await refreshSubfolders(tabs.value[tabIndex])
      await refreshTreeFiles(tabs.value[tabIndex])
    }
  }

  async function loadPhotos(tabIndex?: number, options?: { refreshMeta?: boolean }) {
    const i = tabIndex ?? activeTabIndex.value
    const tab = tabs.value[i]
    if (!tab) return

    const useMetaQuery = filterNeedsMetaQuery(tab.filter)
    const command = useMetaQuery ? 'get_photos' : 'get_photos_basic'
    const photos: Photo[] = await invoke(command, {
      workspaceId: tab.workspace.id,
      filter: tab.filter,
    })
    tab.photos = photos

    // Keep selection valid
    tab.selectedIds = new Set([...tab.selectedIds].filter(id => photos.some(p => p.id === id)))
    restoreVisiblePhotosFromRemovedCache(tab.workspace.id, photos)

    if (useMetaQuery) {
      seedMetaCacheFromPhotos(tab.workspace.id, photos)
      if (options?.refreshMeta) {
        void hydrateWorkspaceMeta(tab.workspace.id, true)
      }
      return
    }

    applyCachedMetaToTab(tab)
    const shouldRefreshMeta = options?.refreshMeta === true || !metaHydratedWorkspace.has(tab.workspace.id)
    if (shouldRefreshMeta) {
      void hydrateWorkspaceMeta(tab.workspace.id, options?.refreshMeta === true)
    }
  }

  function deriveSubfoldersFromPhotos(photos: Photo[]): string[] {
    const folderSet = new Set<string>()
    for (const photo of photos) {
      const normalized = photo.relative_path.replace(/\\/g, '/')
      const segments = normalized.split('/').filter(Boolean)
      if (segments.length <= 1) continue
      let current = ''
      for (let i = 0; i < segments.length - 1; i++) {
        current = current ? `${current}/${segments[i]}` : segments[i]
        folderSet.add(current)
      }
    }
    return [...folderSet].sort((a, b) => a.localeCompare(b))
  }

  function deriveTreeFilesFromPhotos(photos: Photo[]): WorkspaceFile[] {
    return photos
      .map(photo => ({
        relative_path: photo.relative_path.replace(/\\/g, '/'),
        filename: photo.filename,
      }))
      .sort((a, b) => a.relative_path.localeCompare(b.relative_path))
  }

  async function refreshSubfolders(tab: WorkspaceTab) {
    try {
      const subfolders: string[] = await invoke('get_subfolders', {
        workspaceId: tab.workspace.id,
        rootPath: tab.workspace.path,
      })
      tab.subfolders = subfolders.length > 0 ? subfolders : deriveSubfoldersFromPhotos(tab.photos)
    } catch {
      tab.subfolders = deriveSubfoldersFromPhotos(tab.photos)
    }
  }

  async function refreshTreeFiles(tab: WorkspaceTab) {
    try {
      const files: WorkspaceFile[] = await invoke('get_workspace_files', {
        rootPath: tab.workspace.path,
      })
      tab.treeFiles = files
    } catch {
      tab.treeFiles = []
    }
  }

  function setFilter(filter: Partial<PhotoFilter>) {
    const tab = activeTab.value
    if (!tab) return
    Object.assign(tab.filter, filter)
    loadPhotos(undefined, { refreshMeta: false })
  }

  function clearFilter() {
    const tab = activeTab.value
    if (!tab) return
    tab.filter = {}
    loadPhotos(undefined, { refreshMeta: false })
  }

  function selectPhoto(photoId: number, mode: 'single' | 'add' | 'range') {
    const tab = activeTab.value
    if (!tab) return

    if (mode === 'single') {
      tab.selectedIds = new Set([photoId])
    } else if (mode === 'add') {
      if (tab.selectedIds.has(photoId)) {
        tab.selectedIds.delete(photoId)
      } else {
        tab.selectedIds.add(photoId)
      }
    } else if (mode === 'range') {
      if (tab.selectedIds.size === 0) {
        tab.selectedIds = new Set([photoId])
      } else {
        const photos = tab.photos
        const lastSelected = [...tab.selectedIds].pop()!
        const lastIdx = photos.findIndex(p => p.id === lastSelected)
        const targetIdx = photos.findIndex(p => p.id === photoId)
        const start = Math.min(lastIdx, targetIdx)
        const end = Math.max(lastIdx, targetIdx)
        for (let i = start; i <= end; i++) {
          tab.selectedIds.add(photos[i].id)
        }
      }
    }
    tab.activePhotoId = photoId
  }

  function selectAll() {
    const tab = activeTab.value
    if (!tab) return
    tab.selectedIds = new Set(tab.photos.map(p => p.id))
  }

  function clearSelection() {
    const tab = activeTab.value
    if (!tab) return
    tab.selectedIds = new Set()
    tab.activePhotoId = null
  }

  async function updateMeta(photoId: number, starRating: number, colorLabel: string, notes: string) {
    await invoke('update_photo_meta', {
      photoId,
      starRating,
      colorLabel,
      notes,
    })
    const tab = activeTab.value
    if (!tab) return
    const photo = tab.photos.find(p => p.id === photoId)
    if (photo) {
      photo.star_rating = starRating
      photo.color_label = colorLabel
      photo.notes = notes
    }
    updateMetaCacheEntry(tab.workspace.id, photoId, starRating, colorLabel, notes)
  }

  async function updateSelectedMeta(starRating?: number, colorLabel?: string) {
    const tab = activeTab.value
    if (!tab) return
    const updates = []
    for (const id of tab.selectedIds) {
      const photo = tab.photos.find(p => p.id === id)
      if (!photo) continue
      const newStar = starRating !== undefined ? starRating : photo.star_rating
      const newColor = colorLabel !== undefined ? colorLabel : photo.color_label
      updates.push({ photo_id: id, star_rating: newStar, color_label: newColor, notes: photo.notes })
      photo.star_rating = newStar
      photo.color_label = newColor
      updateMetaCacheEntry(tab.workspace.id, id, newStar, newColor, photo.notes)
    }
    if (updates.length > 0) {
      await invoke('batch_update_meta', { updates })
    }
  }

  function setViewMode(mode: 'grid' | 'cull') {
    const tab = activeTab.value
    if (!tab) return
    tab.viewMode = mode
    if (mode === 'cull' && tab.activePhotoId) {
      const idx = tab.photos.findIndex(p => p.id === tab.activePhotoId)
      tab.cullIndex = idx >= 0 ? idx : 0
    }
  }

  function setCullIndex(index: number) {
    const tab = activeTab.value
    if (!tab) return
    const clamped = Math.max(0, Math.min(index, tab.photos.length - 1))
    tab.cullIndex = clamped
    if (tab.photos[clamped]) {
      tab.activePhotoId = tab.photos[clamped].id
      tab.selectedIds = new Set([tab.photos[clamped].id])
    }
  }

  async function saveTabSettings(tab: WorkspaceTab) {
    const settings = {
      filter: tab.filter,
      viewMode: tab.viewMode,
      gridLayout: tab.gridLayout,
      thumbnailSize: tab.thumbnailSize,
      scrollTop: tab.scrollTop,
    }
    await invoke('save_workspace_settings', {
      workspaceId: tab.workspace.id,
      settingsJson: JSON.stringify(settings),
    }).catch(() => {})
  }

  // Listen for scan complete events
  async function setupListeners() {
    await listen<{ workspace_id: number; count: number }>('scan-complete', async (event) => {
      const tab = tabs.value.find(t => t.workspace.id === event.payload.workspace_id)
      if (tab) {
        tab.scanning = false
        tab.workspace.photo_count = event.payload.count
        await loadPhotos(tabs.value.indexOf(tab), { refreshMeta: true })
        await syncRemovedMetaCache(tab.workspace.id)
        await refreshSubfolders(tab)
        await refreshTreeFiles(tab)
      }
    })

    await listen<{ workspace_id: number; paths: string[] }>('file-created', async (event) => {
      const tab = tabs.value.find(t => t.workspace.id === event.payload.workspace_id)
      if (tab) {
        addPendingPaths(pendingCreatedPathsByWorkspace, tab.workspace.id, event.payload.paths)
        scheduleFileEventFlush(tab.workspace.id)
      }
    })

    await listen<{ workspace_id: number; paths: string[] }>('file-removed', async (event) => {
      const tab = tabs.value.find(t => t.workspace.id === event.payload.workspace_id)
      if (tab) {
        // Mark photos as missing
        for (const photo of tab.photos) {
          const fullPath = tab.workspace.path + '/' + photo.relative_path
          if (event.payload.paths.some(p => p.replace(/\\/g, '/') === fullPath.replace(/\\/g, '/'))) {
            photo.is_missing = true
            markPhotoRemovedInCache(tab.workspace.id, photo.id)
          }
        }
        addPendingPaths(pendingRemovedPathsByWorkspace, tab.workspace.id, event.payload.paths)
        scheduleFileEventFlush(tab.workspace.id)
      }
    })
  }

  function tryParseSettings(json: string): any {
    try { return JSON.parse(json) } catch { return {} }
  }

  return {
    tabs,
    activeTabIndex,
    activeTab,
    activePhotos,
    selectedPhotos,
    openWorkspace,
    closeTab,
    loadPhotos,
    setFilter,
    clearFilter,
    selectPhoto,
    selectAll,
    clearSelection,
    updateMeta,
    updateSelectedMeta,
    setViewMode,
    setCullIndex,
    saveTabSettings,
    setupListeners,
  }
})
