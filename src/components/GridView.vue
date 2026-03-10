<template>
  <div
    class="grid-view"
    ref="containerRef"
    @keydown="handleKey"
    @scroll="onScroll"
    tabindex="0"
  >
    <div v-if="tab" class="grid-actions">
      <n-select
        v-model:value="sortBy"
        :options="sortOptions"
        size="small"
        class="grid-sort-select"
      />
      <button
        class="grid-action-btn"
        @click="toggleSortDir"
        :title="sortDesc ? '当前降序，点击切换升序' : '当前升序，点击切换降序'"
      >{{ sortDesc ? '降序' : '升序' }}</button>

      <button
        class="grid-action-btn"
        :disabled="photos.length === 0"
        @click="store.selectAll()"
        title="全选当前筛选结果"
      >全选</button>
      <button
        class="grid-action-btn"
        :disabled="selectedCount === 0"
        @click="store.clearSelection()"
        title="取消当前选择"
      >取消全选</button>
      <button
        class="grid-action-btn"
        @click="toggleAdvancedFilters"
        :title="showAdvancedFilters ? '收起高级筛选' : '展开高级筛选'"
      >{{ showAdvancedFilters ? '收起筛选' : '高级筛选' }}</button>
      <span class="grid-selected-hint">已选 {{ selectedCount }}</span>
      <span v-if="activeFilterCount > 0" class="grid-filter-hint">筛选 {{ activeFilterCount }}</span>
    </div>

    <div v-if="tab && showAdvancedFilters" class="grid-filters-row">
      <div class="filter-group filter-section">
        <span class="filter-label">星级</span>
        <button
          class="star-filter-btn"
          :class="{ active: starFilter === 0 }"
          @click="setStarFilter(0)"
          title="全部"
        >全部</button>
        <button
          class="star-filter-btn"
          :class="{ active: starFilter === 'none' }"
          @click="setStarFilter('none')"
          title="无星级"
        >无星</button>
        <button
          v-for="n in [1, 2, 3, 4, 5]"
          :key="`star-${n}`"
          class="star-filter-btn"
          :class="{ active: starFilter === n }"
          @click="setStarFilter(n as 1 | 2 | 3 | 4 | 5)"
          :title="`${n} 星`"
        >{{ '★'.repeat(n) }}</button>
        <button
          v-if="starFilter !== 0"
          class="icon-btn"
          @click="clearStarFilter"
          title="清除星级筛选"
        >×</button>
      </div>

      <div class="filter-group filter-section">
        <span class="filter-label">颜色</span>
        <button
          class="none-filter-btn"
          :class="{ active: colorNone }"
          @click="toggleNoColorFilter"
          title="无颜色"
        >无色</button>
        <button
          v-for="color in colorOptions"
          :key="`color-${color.value}`"
          class="color-filter-btn"
          :class="{ active: activeColors.includes(color.value) }"
          :style="{ '--color': color.hex }"
          @click="toggleColorFilter(color.value)"
          :title="color.label"
        />
        <button
          v-if="colorNone || activeColors.length"
          class="icon-btn"
          @click="clearColorFilter"
          title="清除颜色筛选"
        >×</button>
      </div>
    </div>

    <!-- Scanning indicator -->
    <div v-if="tab?.scanning" class="scanning-banner">
      <span class="spin">⟳</span> 正在扫描照片...
    </div>

    <!-- Empty -->
    <div v-if="photos.length === 0" class="empty">
      <span>📷</span>
      <p v-if="tab?.scanning">正在扫描照片...</p>
      <p v-else-if="activeFilterCount > 0">当前筛选无结果，请调整筛选条件</p>
      <p v-else>此文件夹中没有照片</p>
    </div>

    <!-- Virtual grid -->
    <div
      v-else
      class="grid-container"
      :style="{ height: totalHeight + 'px' }"
    >
      <div
        class="grid-offset"
        :style="{ transform: `translateY(${offsetY}px)` }"
      >
        <div
          v-for="entry in visibleRows"
          :key="entry.rowIndex"
          class="grid-row"
          :style="rowStyle(entry.items)"
        >
          <PhotoCard
            v-for="(photo, colIndex) in entry.items"
            :key="photo.id"
            :photo="photo"
            :size="cellSize"
            :layout="tab?.gridLayout ?? 'fit'"
            :selected="tab?.selectedIds.has(photo.id)"
            :active="tab?.activePhotoId === photo.id"
            :workspace-path="tab?.workspace.path ?? ''"
            :grid-index="entry.rowIndex * cols + colIndex"
            :visible-start="visibleStart"
            :visible-end="visibleEnd"
            :prefetch-start="prefetchStart"
            :prefetch-end="prefetchEnd"
            @click="onPhotoClick($event, photo)"
            @dblclick="openLightbox(photo)"
            @contextmenu.prevent="onContextMenu($event, photo)"
          />
        </div>
      </div>
    </div>

    <!-- Context Menu -->
    <n-dropdown
      trigger="manual"
      :x="ctxX" :y="ctxY"
      :options="contextMenuOptions"
      :show="showContextMenu"
      @clickoutside="showContextMenu = false"
      @select="onContextMenuSelect"
    />

  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, inject } from 'vue'
import { NDropdown, NSelect, useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useWorkspaceStore, type Photo } from '../stores/workspace'
import PhotoCard from './PhotoCard.vue'
import { useGridRowAlignMode } from '../utils/grid-row-settings'

type StarFilterValue = 0 | 1 | 2 | 3 | 4 | 5 | 'none'
const ADVANCED_FILTERS_EXPANDED_KEY = 'grid.filters.advanced.expanded'

const store = useWorkspaceStore()
const message = useMessage()
const openLightboxFn = inject<(p: Photo) => void>('openLightbox')!
const showExportFn = inject<() => void>('showExport')!

const tab = computed(() => store.activeTab)
const photos = computed(() => tab.value?.photos ?? [])
const selectedCount = computed(() => tab.value?.selectedIds.size ?? 0)
const gridRowAlignMode = useGridRowAlignMode()
const sortBy = computed({
  get: () => tab.value?.filter.sort_by ?? 'taken_at',
  set: (val: string) => store.setFilter({ sort_by: val }),
})
const sortDesc = computed(() => tab.value?.filter.sort_desc === true)
const sortOptions = [
  { label: '拍摄时间', value: 'taken_at' },
  { label: '文件名', value: 'filename' },
  { label: '文件大小', value: 'file_size' },
  { label: '星级', value: 'star_rating' },
]
const starFilter = computed<StarFilterValue>(() => {
  if (tab.value?.filter.star_none === true) return 'none'
  const min = tab.value?.filter.star_min ?? 0
  return [0, 1, 2, 3, 4, 5].includes(min) ? (min as StarFilterValue) : 0
})
const activeColors = computed(() => tab.value?.filter.color_labels ?? [])
const colorNone = computed(() => tab.value?.filter.color_none === true)
const colorOptions = [
  { value: 'red', label: '红色', hex: '#e74c3c' },
  { value: 'orange', label: '橙色', hex: '#e67e22' },
  { value: 'yellow', label: '黄色', hex: '#f1c40f' },
  { value: 'green', label: '绿色', hex: '#2ecc71' },
  { value: 'blue', label: '蓝色', hex: '#3498db' },
  { value: 'purple', label: '紫色', hex: '#9b59b6' },
]
const showAdvancedFilters = ref(readStoredBool(ADVANCED_FILTERS_EXPANDED_KEY, false))
const activeFilterCount = computed(() => {
  let count = 0
  if (starFilter.value === 'none' || starFilter.value > 0) count += 1
  if (colorNone.value) count += 1
  if (activeColors.value.length > 0) count += 1
  return count
})

const containerRef = ref<HTMLElement>()
const containerWidth = ref(800)
const scrollTop = ref(0)
const gap = 4

const cellSize = computed(() => tab.value?.thumbnailSize ?? 140)
const cols = computed(() => Math.max(1, Math.floor((containerWidth.value + gap) / (cellSize.value + gap))))
const rows = computed(() => {
  const result: Photo[][] = []
  for (let i = 0; i < photos.value.length; i += cols.value) {
    result.push(photos.value.slice(i, i + cols.value))
  }
  return result
})
const rowHeight = computed(() => cellSize.value + gap)
const totalHeight = computed(() => rows.value.length * rowHeight.value)

// Virtual scroll
const viewportHeight = ref(600)
const viewportStartRow = computed(() => Math.max(0, Math.floor(scrollTop.value / rowHeight.value)))
const viewportRowCount = computed(() => Math.max(1, Math.ceil(viewportHeight.value / rowHeight.value)))
const viewportEndRow = computed(() => Math.min(rows.value.length, viewportStartRow.value + viewportRowCount.value))

const visibleStart = computed(() => viewportStartRow.value * cols.value)
const visibleEnd = computed(() => Math.min(photos.value.length, viewportEndRow.value * cols.value))
const prefetchStart = computed(() => Math.max(0, (viewportStartRow.value - viewportRowCount.value) * cols.value))
const prefetchEnd = computed(() => Math.min(
  photos.value.length,
  (viewportEndRow.value + viewportRowCount.value) * cols.value,
))

const startRow = computed(() => Math.max(0, viewportStartRow.value - 2))
const endRow = computed(() => Math.min(rows.value.length, viewportEndRow.value + 2))
const visibleRows = computed(() => {
  const items: Array<{ rowIndex: number; items: Photo[] }> = []
  for (let rowIndex = startRow.value; rowIndex < endRow.value; rowIndex++) {
    items.push({
      rowIndex,
      items: rows.value[rowIndex] ?? [],
    })
  }
  return items
})
const offsetY = computed(() => startRow.value * rowHeight.value)

function rowStyle(row: Photo[]) {
  const style: Record<string, string> = {
    height: `${cellSize.value}px`,
    marginBottom: `${gap}px`,
    gap: `${gap}px`,
    justifyContent: 'center',
  }
  const isFullRow = row.length === cols.value

  // Keep partial rows from being centered; in center mode align to the full-row
  // first column start so visual column spacing stays consistent.
  if (!isFullRow) {
    style.justifyContent = 'flex-start'
    if (gridRowAlignMode.value === 'center' && cols.value > 1) {
      const fullRowWidth = cols.value * cellSize.value + (cols.value - 1) * gap
      const leading = Math.max(0, (containerWidth.value - fullRowWidth) / 2)
      style.paddingLeft = `${leading}px`
    }
    return style
  }

  if (gridRowAlignMode.value !== 'stretch') {
    return style
  }

  if (row.length <= 1) {
    return style
  }

  const available = Math.max(0, containerWidth.value - row.length * cellSize.value)
  const stretchedGap = available / (row.length - 1)
  style.justifyContent = 'flex-start'
  style.gap = `${stretchedGap}px`
  return style
}

function onScroll(e: Event) {
  scrollTop.value = (e.target as HTMLElement).scrollTop
  if (tab.value) tab.value.scrollTop = scrollTop.value
}

function toggleSortDir() {
  store.setFilter({ sort_desc: !sortDesc.value })
}

function setStarFilter(val: StarFilterValue) {
  if (val === 'none') {
    store.setFilter({
      star_none: true,
      star_min: undefined,
    })
    return
  }
  store.setFilter({
    star_none: undefined,
    star_min: val > 0 ? val : undefined,
  })
}

function clearStarFilter() {
  setStarFilter(0)
}

function applyColorFilter(colors: string[], none: boolean) {
  store.setFilter({
    color_labels: colors.length ? [...colors] : undefined,
    color_none: none ? true : undefined,
  })
}

function toggleColorFilter(color: string) {
  const colors = [...activeColors.value]
  const idx = colors.indexOf(color)
  if (idx >= 0) colors.splice(idx, 1)
  else colors.push(color)
  applyColorFilter(colors, colorNone.value)
}

function toggleNoColorFilter() {
  applyColorFilter(activeColors.value, !colorNone.value)
}

function clearColorFilter() {
  applyColorFilter([], false)
}

function toggleAdvancedFilters() {
  showAdvancedFilters.value = !showAdvancedFilters.value
  try {
    localStorage.setItem(ADVANCED_FILTERS_EXPANDED_KEY, String(showAdvancedFilters.value))
  } catch {
    // ignore storage errors
  }
}

function readStoredBool(key: string, fallback: boolean) {
  try {
    const raw = localStorage.getItem(key)
    if (raw == null) return fallback
    return raw === 'true'
  } catch {
    return fallback
  }
}

// Context menu
const ctxX = ref(0), ctxY = ref(0)
const showContextMenu = ref(false)
const ctxPhoto = ref<Photo | null>(null)
const copyDestPath = ref('')
const copyMode = ref<'copy' | 'move'>('copy')

const contextMenuOptions = computed(() => [
  { label: '用系统查看器打开', key: 'open' },
  { label: '在文件管理器中显示', key: 'explorer' },
  { label: '复制文件路径', key: 'copy_path' },
  { type: 'divider', key: 'd0' },
  {
    label: '设置星级', key: 'star', children: [
      { label: '无星级', key: 'star_0' },
      { label: '★ 1星', key: 'star_1' },
      { label: '★★ 2星', key: 'star_2' },
      { label: '★★★ 3星', key: 'star_3' },
      { label: '★★★★ 4星', key: 'star_4' },
      { label: '★★★★★ 5星', key: 'star_5' },
    ]
  },
  {
    label: '设置颜色标签', key: 'color', children: [
      { label: '🔴 红色', key: 'color_red' },
      { label: '🟠 橙色', key: 'color_orange' },
      { label: '🟡 黄色', key: 'color_yellow' },
      { label: '🟢 绿色', key: 'color_green' },
      { label: '🔵 蓝色', key: 'color_blue' },
      { label: '🟣 紫色', key: 'color_purple' },
      { label: '无颜色', key: 'color_clear' },
    ]
  },
  { type: 'divider', key: 'd1' },
  { label: '导出选中照片...', key: 'export' },
  { label: '复制到文件夹...', key: 'copy_to' },
  { label: '移动到文件夹...', key: 'move_to' },
  { type: 'divider', key: 'd2' },
  { label: '查看 EXIF 信息', key: 'exif' },
  { type: 'divider', key: 'd3' },
  { label: '移入回收站', key: 'delete' },
])

function onContextMenu(e: MouseEvent, photo: Photo) {
  // If right-clicked photo is not selected, select it
  if (!tab.value?.selectedIds.has(photo.id)) {
    store.selectPhoto(photo.id, 'single')
  }
  ctxPhoto.value = photo
  ctxX.value = e.clientX
  ctxY.value = e.clientY
  showContextMenu.value = true
}

async function onContextMenuSelect(key: string) {
  showContextMenu.value = false
  const t = tab.value
  if (!t) return
  const photo = ctxPhoto.value
  if (!photo) return
  const selectedIds = [...t.selectedIds]
  const targetIds = selectedIds.includes(photo.id) ? selectedIds : [photo.id]
  const fullPath = `${t.workspace.path}/${photo.relative_path}`

  if (key === 'open') {
    await invoke('open_with_default_app', { path: fullPath })
  } else if (key === 'explorer') {
    await invoke('open_in_explorer', { path: fullPath })
  } else if (key === 'copy_path') {
    await navigator.clipboard.writeText(fullPath)
    message.success('路径已复制')
  } else if (key.startsWith('star_')) {
    const n = parseInt(key.replace('star_', ''))
    await store.updateSelectedMeta(n, undefined)
  } else if (key.startsWith('color_')) {
    const c = key === 'color_clear' ? '' : key.replace('color_', '')
    await store.updateSelectedMeta(undefined, c)
  } else if (key === 'export') {
    showExportFn()
  } else if (key === 'copy_to') {
    copyMode.value = 'copy'
    await pickDestFolderAndRun()
  } else if (key === 'move_to') {
    copyMode.value = 'move'
    await pickDestFolderAndRun()
  } else if (key === 'exif') {
    openLightboxFn(photo)
  } else if (key === 'delete') {
    const deleted: number[] = await invoke('delete_photos', {
      photoIds: targetIds,
      workspacePath: t.workspace.path,
    })
    t.photos = t.photos.filter(p => !deleted.includes(p.id))
    for (const id of deleted) t.selectedIds.delete(id)
    if (t.activePhotoId != null && deleted.includes(t.activePhotoId)) {
      t.activePhotoId = null
    }
    message.success(`已删除 ${deleted.length} 张照片`)
  }
}

async function pickDestFolderAndRun() {
  const selected = await open({
    directory: true,
    multiple: false,
    title: copyMode.value === 'copy' ? '选择复制目标文件夹' : '选择移动目标文件夹',
  })
  if (!selected || typeof selected !== 'string') return
  copyDestPath.value = selected
  await doCopyPhotos()
}

async function doCopyPhotos() {
  const t = tab.value
  if (!t) return
  const ids = [...t.selectedIds]
  if (ids.length === 0) {
    message.warning('请先选择要处理的照片')
    return
  }
  if (!copyDestPath.value) {
    message.warning('请选择目标文件夹')
    return
  }
  const cmd = copyMode.value === 'copy' ? 'copy_photos' : 'move_photos'
  try {
    const count: number = await invoke(cmd, {
      photoIds: ids,
      workspacePath: t.workspace.path,
      destFolder: copyDestPath.value,
    })
    message.success(`已${copyMode.value === 'copy' ? '复制' : '移动'} ${count} 张`)
    if (copyMode.value === 'move') await store.loadPhotos()
  } catch (error) {
    message.error(`执行失败: ${String(error)}`)
  }
}

function onPhotoClick(e: MouseEvent, photo: Photo) {
  const mode = e.ctrlKey ? 'add' : e.shiftKey ? 'range' : 'single'
  store.selectPhoto(photo.id, mode)
}

function openLightbox(photo: Photo) {
  openLightboxFn(photo)
}

// Keyboard navigation
function handleKey(e: KeyboardEvent) {
  const t = tab.value
  if (!t || photos.value.length === 0) return

  const tag = (e.target as HTMLElement).tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA') return

  // Toggle cull mode
  if (e.key === 'Tab') {
    e.preventDefault()
    store.setViewMode('cull')
    return
  }

  // Star shortcuts
  if (['1','2','3','4','5'].includes(e.key)) {
    store.updateSelectedMeta(parseInt(e.key), undefined)
    return
  }
  if (e.key === '6') { store.updateSelectedMeta(undefined, 'red'); return }
  if (e.key === '7') { store.updateSelectedMeta(undefined, 'orange'); return }
  if (e.key === '8') { store.updateSelectedMeta(undefined, 'yellow'); return }
  if (e.key === '9') { store.updateSelectedMeta(undefined, 'green'); return }
  if (e.key === '0') { store.updateSelectedMeta(0, ''); return }

  const activeId = t.activePhotoId
  const activeIdx = activeId ? photos.value.findIndex(p => p.id === activeId) : -1
  const c = cols.value

  if (e.key === 'Enter' && activeIdx >= 0) {
    e.preventDefault()
    openLightboxFn(photos.value[activeIdx])
    return
  }

  let nextIdx = activeIdx
  if (e.key === 'ArrowLeft') nextIdx = Math.max(0, activeIdx - 1)
  else if (e.key === 'ArrowRight') nextIdx = Math.min(photos.value.length - 1, activeIdx + 1)
  else if (e.key === 'ArrowUp') nextIdx = Math.max(0, activeIdx - c)
  else if (e.key === 'ArrowDown') nextIdx = Math.min(photos.value.length - 1, activeIdx + c)
  else return

  e.preventDefault()
  const next = photos.value[nextIdx]
  if (next) {
    store.selectPhoto(next.id, e.shiftKey ? 'range' : 'single')
    // Scroll into view
    const rowIdx = Math.floor(nextIdx / c)
    const rowTop = rowIdx * rowHeight.value
    const rowBottom = rowTop + cellSize.value
    const el = containerRef.value
    if (el) {
      if (rowTop < el.scrollTop) el.scrollTop = rowTop - gap
      else if (rowBottom > el.scrollTop + el.clientHeight) el.scrollTop = rowBottom - el.clientHeight + gap
    }
  }
}

// Resize observer
let resizeObserver: ResizeObserver
onMounted(() => {
  const el = containerRef.value
  if (!el) return
  containerWidth.value = el.clientWidth
  viewportHeight.value = el.clientHeight
  resizeObserver = new ResizeObserver(entries => {
    for (const e of entries) {
      containerWidth.value = e.contentRect.width
      viewportHeight.value = e.contentRect.height
    }
  })
  resizeObserver.observe(el)
  el.focus()

  // Restore scroll position
  if (tab.value?.scrollTop) {
    el.scrollTop = tab.value.scrollTop
    scrollTop.value = tab.value.scrollTop
  }
})
onUnmounted(() => resizeObserver?.disconnect())
</script>

<style scoped>
.grid-view {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  background: #1a1a1a;
  outline: none;
  position: relative;
  padding: 8px;
}
.grid-container { position: relative; width: 100%; }
.grid-offset { position: absolute; top: 0; left: 0; right: 0; }
.grid-row { display: flex; }
.grid-actions {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  justify-content: flex-start;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  padding: 4px 0;
  background: linear-gradient(180deg, rgba(26, 26, 26, 0.96), rgba(26, 26, 26, 0.75), rgba(26, 26, 26, 0));
}
.grid-filters-row {
  position: sticky;
  top: 32px;
  z-index: 2;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  padding: 4px 0;
  background: linear-gradient(180deg, rgba(26, 26, 26, 0.94), rgba(26, 26, 26, 0.68), rgba(26, 26, 26, 0));
}
.grid-sort-select {
  width: 132px;
}
.filter-group {
  display: flex;
  align-items: center;
  gap: 4px;
}
.filter-section {
  padding: 2px 6px;
  border: 1px solid #2f2f2f;
  border-radius: 7px;
  background: #181818;
}
.filter-label {
  font-size: 11px;
  font-weight: 700;
  color: #b8c7dc;
  letter-spacing: 0.03em;
  background: #253a57;
  border: 1px solid #355a85;
  border-radius: 999px;
  padding: 1px 7px;
  margin-right: 2px;
  white-space: nowrap;
}
.grid-action-btn {
  background: #202020;
  border: 1px solid #343434;
  color: #bbb;
  border-radius: 5px;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
}
.grid-action-btn:hover:not(:disabled) {
  border-color: #4f8ef7;
  color: #4f8ef7;
}
.grid-action-btn:disabled {
  opacity: 0.45;
  cursor: default;
}
.star-filter-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 2px 6px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  transition: all 0.15s;
}
.star-filter-btn.active {
  background: #f39c12;
  border-color: #f39c12;
  color: #1a1a1a;
}
.star-filter-btn:hover:not(.active) {
  border-color: #f39c12;
  color: #f39c12;
}
.none-filter-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 2px 6px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  transition: all 0.15s;
}
.none-filter-btn.active {
  background: #555;
  border-color: #888;
  color: #fff;
}
.none-filter-btn:hover:not(.active) {
  border-color: #aaa;
  color: #ddd;
}
.color-filter-btn {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--color);
  border: 2px solid transparent;
  cursor: pointer;
  transition: border-color 0.15s;
}
.color-filter-btn.active { border-color: #fff; }
.color-filter-btn:hover { border-color: rgba(255, 255, 255, 0.6); }
.icon-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 2px 7px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}
.icon-btn:hover { border-color: #4F8EF7; color: #4F8EF7; }
.grid-selected-hint {
  font-size: 12px;
  color: #7f90ad;
  min-width: 54px;
  text-align: right;
}
.grid-filter-hint {
  font-size: 12px;
  color: #9fb6dd;
  min-width: 56px;
}
.scanning-banner {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 12px; background: #1e3a5f; border-radius: 4px;
  font-size: 13px; color: #4F8EF7; margin-bottom: 8px;
}
.spin { animation: spin 1s linear infinite; display: inline-block; }
.empty {
  display: flex; flex-direction: column; align-items: center;
  justify-content: center; height: 200px; color: #555; gap: 8px;
  font-size: 32px;
}
.empty p { font-size: 14px; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
