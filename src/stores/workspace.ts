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

    await refreshSubfolders(tab)
  }

  async function closeTab(index: number) {
    const tab = tabs.value[index]
    if (tab) {
      await saveTabSettings(tab)
      await invoke('close_workspace', { workspaceId: tab.workspace.id })
      tabs.value.splice(index, 1)
      if (activeTabIndex.value >= tabs.value.length) {
        activeTabIndex.value = Math.max(0, tabs.value.length - 1)
      }
    }
  }

  async function loadPhotos(tabIndex?: number) {
    const i = tabIndex ?? activeTabIndex.value
    const tab = tabs.value[i]
    if (!tab) return

    const photos: Photo[] = await invoke('get_photos', {
      workspaceId: tab.workspace.id,
      filter: tab.filter,
    })
    tab.photos = photos

    // Keep selection valid
    tab.selectedIds = new Set([...tab.selectedIds].filter(id => photos.some(p => p.id === id)))
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

  function setFilter(filter: Partial<PhotoFilter>) {
    const tab = activeTab.value
    if (!tab) return
    Object.assign(tab.filter, filter)
    loadPhotos()
  }

  function clearFilter() {
    const tab = activeTab.value
    if (!tab) return
    tab.filter = {}
    loadPhotos()
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
        await loadPhotos(tabs.value.indexOf(tab))
        await refreshSubfolders(tab)
      }
    })

    await listen<{ workspace_id: number; paths: string[] }>('file-created', async (event) => {
      const tab = tabs.value.find(t => t.workspace.id === event.payload.workspace_id)
      if (tab) {
        await refreshSubfolders(tab)
      }
    })

    await listen<{ workspace_id: number; paths: string[] }>('file-removed', (event) => {
      const tab = tabs.value.find(t => t.workspace.id === event.payload.workspace_id)
      if (tab) {
        // Mark photos as missing
        for (const photo of tab.photos) {
          const fullPath = tab.workspace.path + '/' + photo.relative_path
          if (event.payload.paths.some(p => p.replace(/\\/g, '/') === fullPath.replace(/\\/g, '/'))) {
            photo.is_missing = true
          }
        }
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
