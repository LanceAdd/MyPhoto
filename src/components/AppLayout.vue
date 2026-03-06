<template>
  <div class="app-layout" @keydown="handleGlobalKey" tabindex="-1" ref="layoutRef">
    <div class="tab-bar">
      <div
        v-for="(tab, i) in store.tabs"
        :key="tab.workspace.id"
        class="tab"
        :class="{ active: i === store.activeTabIndex }"
        @click="store.activeTabIndex = i"
      >
        <span class="tab-icon">📁</span>
        <span class="tab-name">{{ tab.workspace.name }}</span>
        <span v-if="tab.scanning" class="tab-scanning">⟳</span>
        <span class="tab-count" v-if="!tab.scanning">{{ tab.photos.length }}</span>
        <button class="tab-close" @click.stop="store.closeTab(i)">×</button>
      </div>
      <button class="tab-add" @click="openFolder" title="打开文件夹 (Ctrl+O)">+ 新建工作区</button>
    </div>

    <Toolbar v-if="store.activeTab" />

    <div class="main-area" v-if="store.activeTab">
      <div
        class="pane pane-left"
        :class="{ collapsed: leftPanelCollapsed }"
        :style="{ width: leftPanelCollapsed ? '34px' : `${leftPanelWidth}px` }"
      >
        <div class="pane-controls">
          <button
            class="pane-toggle"
            @click="toggleLeftPanel"
            :title="leftPanelCollapsed ? 'Expand file tree' : 'Collapse file tree'"
          >{{ leftPanelCollapsed ? '>' : '<' }}</button>
        </div>
        <FileTree v-if="!leftPanelCollapsed" class="sidebar-left" />
      </div>

      <div
        v-if="!leftPanelCollapsed"
        class="pane-resizer"
        @mousedown.prevent="startResize('left', $event)"
      />

      <div class="content-area">
        <GridView v-if="store.activeTab.viewMode === 'grid'" />
        <CullView v-else-if="store.activeTab.viewMode === 'cull'" />
      </div>

      <div
        v-if="!rightPanelCollapsed"
        class="pane-resizer"
        @mousedown.prevent="startResize('right', $event)"
      />

      <div
        class="pane pane-right"
        :class="{ collapsed: rightPanelCollapsed }"
        :style="{ width: rightPanelCollapsed ? '34px' : `${rightPanelWidth}px` }"
      >
        <div class="pane-controls pane-controls-right">
          <button
            class="pane-toggle"
            @click="toggleRightPanel"
            :title="rightPanelCollapsed ? 'Expand metadata panel' : 'Collapse metadata panel'"
          >{{ rightPanelCollapsed ? '<' : '>' }}</button>
        </div>
        <MetaPanel v-if="!rightPanelCollapsed" class="sidebar-right" />
      </div>
    </div>

    <div class="empty-state" v-else>
      <div class="empty-icon">🖼️</div>
      <h2>MyPhoto</h2>
      <p>打开一个文件夹开始管理你的照片</p>
      <button class="btn-primary" @click="openFolder">打开文件夹</button>
      <div class="recent-list" v-if="recentWorkspaces.length">
        <p class="recent-title">最近打开</p>
        <div
          v-for="ws in recentWorkspaces"
          :key="ws.id"
          class="recent-item"
          @click="store.openWorkspace(ws.path)"
        >
          <span class="recent-icon">📁</span>
          <div>
            <div class="recent-name">{{ ws.name }}</div>
            <div class="recent-path">{{ ws.path }}</div>
          </div>
          <span class="recent-count">{{ ws.photo_count }} 张</span>
        </div>
      </div>
    </div>

    <StatusBar v-if="store.activeTab" />

    <LightboxView v-if="lightboxPhoto" :photo="lightboxPhoto" @close="lightboxPhoto = null" />
    <HelpPanel v-if="showHelp" @close="showHelp = false" />
    <SettingsPanel v-if="showSettings" @close="showSettings = false" />

    <ExportDialog
      v-if="showExport"
      :photoIds="store.selectedPhotos.map(p => p.id)"
      :totalCount="store.activeTab?.photos.length ?? 0"
      @close="showExport = false"
    />

    <div class="notifications">
      <div
        v-for="n in notifications"
        :key="n.id"
        class="notification"
        :class="n.type"
      >
        {{ n.message }}
        <button v-if="n.action" @click="n.action.fn" class="notif-action">{{ n.action.label }}</button>
        <button @click="removeNotification(n.id)" class="notif-close">×</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, provide, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useWorkspaceStore, type Workspace, type Photo } from '../stores/workspace'
import { useKeybindingStore } from '../stores/keybinding'
import Toolbar from './Toolbar.vue'
import FileTree from './FileTree.vue'
import GridView from './GridView.vue'
import CullView from './CullView.vue'
import MetaPanel from './MetaPanel.vue'
import StatusBar from './StatusBar.vue'
import LightboxView from './LightboxView.vue'
import HelpPanel from './HelpPanel.vue'
import SettingsPanel from './SettingsPanel.vue'
import ExportDialog from './ExportDialog.vue'

const store = useWorkspaceStore()
const kbStore = useKeybindingStore()
const layoutRef = ref<HTMLElement>()

const LEFT_MIN = 180
const LEFT_MAX = 520
const RIGHT_MIN = 220
const RIGHT_MAX = 560

const leftPanelWidth = ref(readNumber('layout.left.width', 260, LEFT_MIN, LEFT_MAX))
const rightPanelWidth = ref(readNumber('layout.right.width', 280, RIGHT_MIN, RIGHT_MAX))
const leftPanelCollapsed = ref(readBool('layout.left.collapsed', false))
const rightPanelCollapsed = ref(readBool('layout.right.collapsed', false))

type ResizeSide = 'left' | 'right'
const resizingSide = ref<ResizeSide | null>(null)
const resizeStartX = ref(0)
const resizeStartWidth = ref(0)

const showHelp = ref(false)
const showSettings = ref(false)
const showExport = ref(false)
const lightboxPhoto = ref<Photo | null>(null)
const recentWorkspaces = ref<Workspace[]>([])

interface Notification {
  id: number
  message: string
  type: 'info' | 'warning' | 'error'
  action?: { label: string; fn: () => void }
}

const notifications = ref<Notification[]>([])
let notifIdCounter = 0

watch(leftPanelWidth, (v) => writeStorage('layout.left.width', String(v)))
watch(rightPanelWidth, (v) => writeStorage('layout.right.width', String(v)))
watch(leftPanelCollapsed, (v) => writeStorage('layout.left.collapsed', String(v)))
watch(rightPanelCollapsed, (v) => writeStorage('layout.right.collapsed', String(v)))

function addNotification(msg: string, type: Notification['type'] = 'info', action?: Notification['action']) {
  const id = ++notifIdCounter
  notifications.value.push({ id, message: msg, type, action })
  setTimeout(() => removeNotification(id), 8000)
}

function removeNotification(id: number) {
  const i = notifications.value.findIndex(n => n.id === id)
  if (i >= 0) notifications.value.splice(i, 1)
}

function clamp(n: number, min: number, max: number) {
  return Math.max(min, Math.min(max, n))
}

function readNumber(key: string, fallback: number, min: number, max: number) {
  const raw = localStorage.getItem(key)
  if (!raw) return fallback
  const n = Number(raw)
  if (!Number.isFinite(n)) return fallback
  return clamp(n, min, max)
}

function readBool(key: string, fallback: boolean) {
  const raw = localStorage.getItem(key)
  if (raw == null) return fallback
  return raw === 'true'
}

function writeStorage(key: string, value: string) {
  localStorage.setItem(key, value)
}

function toggleLeftPanel() {
  leftPanelCollapsed.value = !leftPanelCollapsed.value
}

function toggleRightPanel() {
  rightPanelCollapsed.value = !rightPanelCollapsed.value
}

function startResize(side: ResizeSide, e: MouseEvent) {
  if ((side === 'left' && leftPanelCollapsed.value) || (side === 'right' && rightPanelCollapsed.value)) return
  resizingSide.value = side
  resizeStartX.value = e.clientX
  resizeStartWidth.value = side === 'left' ? leftPanelWidth.value : rightPanelWidth.value
  document.body.classList.add('is-resizing')
  window.addEventListener('mousemove', onResizeMove)
  window.addEventListener('mouseup', stopResize)
}

function onResizeMove(e: MouseEvent) {
  if (!resizingSide.value) return
  const delta = e.clientX - resizeStartX.value
  if (resizingSide.value === 'left') {
    leftPanelWidth.value = clamp(resizeStartWidth.value + delta, LEFT_MIN, LEFT_MAX)
    return
  }
  rightPanelWidth.value = clamp(resizeStartWidth.value - delta, RIGHT_MIN, RIGHT_MAX)
}

function stopResize() {
  resizingSide.value = null
  document.body.classList.remove('is-resizing')
  window.removeEventListener('mousemove', onResizeMove)
  window.removeEventListener('mouseup', stopResize)
}

provide('openLightbox', (photo: Photo) => { lightboxPhoto.value = photo })
provide('showExport', () => { showExport.value = true })
provide('addNotification', addNotification)

async function openFolder() {
  const selected = await open({ directory: true, multiple: false })
  if (selected && typeof selected === 'string') {
    await store.openWorkspace(selected)
    layoutRef.value?.focus()
  }
}

function handleGlobalKey(e: KeyboardEvent) {
  const tag = (e.target as HTMLElement).tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA') return

  if (kbStore.matchesAction(e, 'open_workspace') && e.ctrlKey) {
    e.preventDefault()
    openFolder()
    return
  }
  if (kbStore.matchesAction(e, 'close_workspace') && e.ctrlKey) {
    e.preventDefault()
    store.closeTab(store.activeTabIndex)
    return
  }
  if (kbStore.matchesAction(e, 'open_settings') && e.ctrlKey) {
    e.preventDefault()
    showSettings.value = true
    return
  }
  if (e.key === '?') {
    showHelp.value = true
    return
  }
  if (e.key === 'Escape' && lightboxPhoto.value) {
    lightboxPhoto.value = null
    return
  }
  if (e.key === 'Escape' && showHelp.value) {
    showHelp.value = false
    return
  }
  if (e.key === 'Escape' && showSettings.value) {
    showSettings.value = false
  }
}

onMounted(async () => {
  layoutRef.value?.focus()
  recentWorkspaces.value = await invoke('get_recent_workspaces')

  await listen<{ workspace_id: number; paths: string[] }>('file-created', (event) => {
    const tab = store.tabs.find(t => t.workspace.id === event.payload.workspace_id)
    if (tab) {
      const count = event.payload.paths.length
      addNotification(`发现 ${count} 张新照片`, 'info', {
        label: '立即添加',
        fn: () => {
          invoke('rescan_workspace', {
            workspaceId: tab.workspace.id,
            workspacePath: tab.workspace.path,
          })
        },
      })
    }
  })

  await listen<{ workspace_id: number }>('file-removed', (event) => {
    const tab = store.tabs.find(t => t.workspace.id === event.payload.workspace_id)
    if (tab) {
      addNotification('检测到照片文件已删除', 'warning')
    }
  })
})

onUnmounted(() => {
  stopResize()
})
</script>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #1a1a1a;
  outline: none;
}
.tab-bar {
  display: flex;
  align-items: center;
  height: 36px;
  background: #111;
  border-bottom: 1px solid #333;
  overflow-x: auto;
  flex-shrink: 0;
  gap: 2px;
  padding: 0 4px;
}
.tab {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 10px;
  height: 30px;
  background: #222;
  border-radius: 6px 6px 0 0;
  cursor: pointer;
  font-size: 13px;
  white-space: nowrap;
  border: 1px solid transparent;
  transition: background 0.15s;
  color: #aaa;
}
.tab:hover { background: #2a2a2a; color: #ddd; }
.tab.active { background: #1a1a1a; border-color: #333; border-bottom-color: #1a1a1a; color: #fff; }
.tab-icon { font-size: 12px; }
.tab-name { max-width: 120px; overflow: hidden; text-overflow: ellipsis; }
.tab-scanning { animation: spin 1s linear infinite; font-size: 12px; color: #4F8EF7; }
.tab-count { font-size: 11px; color: #666; background: #333; padding: 1px 5px; border-radius: 8px; }
.tab-close {
  background: none;
  border: none;
  color: #666;
  cursor: pointer;
  font-size: 14px;
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 3px;
}
.tab-close:hover { background: #c0392b; color: #fff; }
.tab-add {
  background: none;
  border: 1px dashed #444;
  color: #888;
  padding: 3px 10px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  white-space: nowrap;
  margin-left: 4px;
}
.tab-add:hover { border-color: #4F8EF7; color: #4F8EF7; }

.main-area {
  display: flex;
  flex: 1;
  min-height: 0;
}
.pane {
  display: flex;
  min-height: 0;
  flex-shrink: 0;
  position: relative;
  background: #1c1c1c;
}
.pane.collapsed { background: #171717; }
.pane-left { border-right: 1px solid #2a2a2a; }
.pane-right { border-left: 1px solid #2a2a2a; }

.sidebar-left,
.sidebar-right {
  width: 100%;
  min-width: 0;
  min-height: 0;
}

.pane-controls {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  right: 6px;
  z-index: 10;
}
.pane-controls-right {
  left: 6px;
  right: auto;
}
.pane-toggle {
  width: 30px;
  height: 30px;
  border: 1px solid #3b3b3b;
  border-radius: 6px;
  background: #252525;
  color: #bbb;
  cursor: pointer;
  font-weight: 700;
  font-size: 16px;
  line-height: 1;
}
.pane-toggle:hover {
  border-color: #4F8EF7;
  color: #4F8EF7;
}

.pane-resizer {
  width: 6px;
  flex-shrink: 0;
  cursor: col-resize;
  position: relative;
  background: transparent;
}
.pane-resizer::after {
  content: '';
  position: absolute;
  left: 2px;
  top: 0;
  bottom: 0;
  width: 1px;
  background: #2d2d2d;
  transition: background 0.12s;
}
.pane-resizer:hover::after { background: #4F8EF7; }

.content-area {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 16px;
  color: #666;
}
.empty-icon { font-size: 64px; }
.empty-state h2 { font-size: 24px; color: #888; }
.empty-state p { font-size: 14px; }
.btn-primary {
  background: #4F8EF7;
  color: #fff;
  border: none;
  padding: 10px 24px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
}
.btn-primary:hover { background: #6BA3F9; }

.recent-list { margin-top: 16px; width: 400px; }
.recent-title { font-size: 12px; color: #555; margin-bottom: 8px; }
.recent-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.15s;
}
.recent-item:hover { background: #242424; }
.recent-icon { font-size: 18px; }
.recent-name { font-size: 13px; color: #ccc; }
.recent-path { font-size: 11px; color: #555; }
.recent-count { margin-left: auto; font-size: 11px; color: #666; }

.notifications {
  position: fixed;
  bottom: 40px;
  right: 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: 1000;
}
.notification {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #2a2a2a;
  border: 1px solid #444;
  padding: 10px 14px;
  border-radius: 6px;
  font-size: 13px;
  max-width: 320px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.4);
}
.notification.warning { border-color: #e67e22; }
.notification.error { border-color: #c0392b; }
.notif-action {
  background: #4F8EF7;
  color: #fff;
  border: none;
  padding: 3px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}
.notif-close {
  background: none;
  border: none;
  color: #666;
  cursor: pointer;
  font-size: 16px;
  margin-left: auto;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

:global(body.is-resizing) {
  cursor: col-resize !important;
  user-select: none !important;
}
</style>
