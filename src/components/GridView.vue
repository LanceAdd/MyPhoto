<template>
  <div
    class="grid-view"
    ref="containerRef"
    @keydown="handleKey"
    @scroll="onScroll"
    tabindex="0"
  >
    <!-- Scanning indicator -->
    <div v-if="tab?.scanning" class="scanning-banner">
      <span class="spin">⟳</span> 正在扫描照片...
    </div>

    <!-- Empty -->
    <div v-if="photos.length === 0" class="empty">
      <span>📷</span>
      <p>{{ tab?.scanning ? '正在扫描照片...' : '此文件夹中没有照片' }}</p>
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
          v-for="row in visibleRows"
          :key="row[0]?.id"
          class="grid-row"
          :style="{ height: cellSize + 'px', gap: gap + 'px', marginBottom: gap + 'px' }"
        >
          <PhotoCard
            v-for="photo in row"
            :key="photo.id"
            :photo="photo"
            :size="cellSize"
            :layout="tab?.gridLayout ?? 'fit'"
            :selected="tab?.selectedIds.has(photo.id)"
            :active="tab?.activePhotoId === photo.id"
            :workspace-path="tab?.workspace.path ?? ''"
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

    <!-- Copy to folder dialog -->
    <n-modal v-model:show="showCopyDialog">
      <n-card title="复制到文件夹" style="width:400px">
        <n-input v-model:value="copyDestPath" placeholder="目标文件夹路径" />
        <template #footer>
          <div style="display:flex;gap:8px;justify-content:flex-end">
            <n-button @click="showCopyDialog = false">取消</n-button>
            <n-button type="primary" @click="doCopyPhotos">确认</n-button>
          </div>
        </template>
      </n-card>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, inject } from 'vue'
import { NDropdown, NModal, NCard, NInput, NButton, useMessage } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { useWorkspaceStore, type Photo } from '../stores/workspace'
import PhotoCard from './PhotoCard.vue'

const store = useWorkspaceStore()
const message = useMessage()
const openLightboxFn = inject<(p: Photo) => void>('openLightbox')!
const showExportFn = inject<() => void>('showExport')!

const tab = computed(() => store.activeTab)
const photos = computed(() => tab.value?.photos ?? [])

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
const startRow = computed(() => Math.max(0, Math.floor(scrollTop.value / rowHeight.value) - 2))
const endRow = computed(() => Math.min(rows.value.length, startRow.value + Math.ceil(viewportHeight.value / rowHeight.value) + 4))
const visibleRows = computed(() => rows.value.slice(startRow.value, endRow.value))
const offsetY = computed(() => startRow.value * rowHeight.value)

function onScroll(e: Event) {
  scrollTop.value = (e.target as HTMLElement).scrollTop
  if (tab.value) tab.value.scrollTop = scrollTop.value
}

// Context menu
const ctxX = ref(0), ctxY = ref(0)
const showContextMenu = ref(false)
const ctxPhoto = ref<Photo | null>(null)
const showCopyDialog = ref(false)
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
    copyDestPath.value = ''
    showCopyDialog.value = true
  } else if (key === 'move_to') {
    copyMode.value = 'move'
    copyDestPath.value = ''
    showCopyDialog.value = true
  } else if (key === 'exif') {
    openLightboxFn(photo)
  } else if (key === 'delete') {
    const deleted: number[] = await invoke('delete_photos', {
      photoIds: targetIds,
      workspacePath: t.workspace.path,
    })
    t.photos = t.photos.filter(p => !deleted.includes(p.id))
    message.success(`已删除 ${deleted.length} 张照片`)
  }
}

async function doCopyPhotos() {
  const t = tab.value
  if (!t) return
  const ids = [...t.selectedIds]
  const cmd = copyMode.value === 'copy' ? 'copy_photos' : 'move_photos'
  const count: number = await invoke(cmd, {
    photoIds: ids,
    workspacePath: t.workspace.path,
    destFolder: copyDestPath.value,
  })
  showCopyDialog.value = false
  message.success(`已${copyMode.value === 'copy' ? '复制' : '移动'} ${count} 张`)
  if (copyMode.value === 'move') await store.loadPhotos()
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


