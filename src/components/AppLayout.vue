<template>
  <div class="app-layout" @keydown="handleGlobalKey" tabindex="-1" ref="layoutRef">
    <!-- Tab Bar -->
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

    <!-- Toolbar -->
    <Toolbar v-if="store.activeTab" />

    <!-- Main Area -->
    <div class="main-area" v-if="store.activeTab">
      <!-- File Tree -->
      <FileTree class="sidebar-left" />

      <!-- Content Area -->
      <div class="content-area">
        <GridView v-if="store.activeTab.viewMode === 'grid'" />
        <CullView v-else-if="store.activeTab.viewMode === 'cull'" />
      </div>

      <!-- Metadata Panel -->
      <MetaPanel class="sidebar-right" v-if="!metaPanelCollapsed" />
      <button
        class="meta-toggle"
        @click="metaPanelCollapsed = !metaPanelCollapsed"
        :title="metaPanelCollapsed ? '显示元数据面板' : '隐藏元数据面板'"
      >{{ metaPanelCollapsed ? '◀' : '▶' }}</button>
    </div>

    <!-- Empty state -->
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

    <!-- Status Bar -->
    <StatusBar v-if="store.activeTab" />

    <!-- Lightbox -->
    <LightboxView v-if="lightboxPhoto" :photo="lightboxPhoto" @close="lightboxPhoto = null" />

    <!-- Help Panel -->
    <HelpPanel v-if="showHelp" @close="showHelp = false" />

    <!-- Settings Panel -->
    <SettingsPanel v-if="showSettings" @close="showSettings = false" />

    <!-- Export Dialog -->
    <ExportDialog
      v-if="showExport"
      :photoIds="store.selectedPhotos.map(p => p.id)"
      :totalCount="store.activeTab?.photos.length ?? 0"
      @close="showExport = false"
    />

    <!-- Notification area -->
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
import { ref, onMounted, provide } from 'vue'
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

const metaPanelCollapsed = ref(false)
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

function addNotification(msg: string, type: Notification['type'] = 'info', action?: Notification['action']) {
  const id = ++notifIdCounter
  notifications.value.push({ id, message: msg, type, action })
  setTimeout(() => removeNotification(id), 8000)
}

function removeNotification(id: number) {
  const i = notifications.value.findIndex(n => n.id === id)
  if (i >= 0) notifications.value.splice(i, 1)
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
    e.preventDefault(); openFolder(); return
  }
  if (kbStore.matchesAction(e, 'close_workspace') && e.ctrlKey) {
    e.preventDefault(); store.closeTab(store.activeTabIndex); return
  }
  if (kbStore.matchesAction(e, 'open_settings') && e.ctrlKey) {
    e.preventDefault(); showSettings.value = true; return
  }
  if (e.key === '?') {
    showHelp.value = true; return
  }
  if (e.key === 'Escape' && lightboxPhoto.value) {
    lightboxPhoto.value = null; return
  }
  if (e.key === 'Escape' && showHelp.value) {
    showHelp.value = false; return
  }
  if (e.key === 'Escape' && showSettings.value) {
    showSettings.value = false; return
  }
}

onMounted(async () => {
  layoutRef.value?.focus()
  recentWorkspaces.value = await invoke('get_recent_workspaces')

  // File-created event
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
        }
      })
    }
  })

  // File removed - show warning
  await listen<{ workspace_id: number }>('file-removed', (event) => {
    const tab = store.tabs.find(t => t.workspace.id === event.payload.workspace_id)
    if (tab) {
      addNotification('检测到照片文件已删除', 'warning')
    }
  })
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
  background: none; border: none; color: #666; cursor: pointer;
  font-size: 14px; width: 16px; height: 16px; display: flex;
  align-items: center; justify-content: center; border-radius: 3px;
}
.tab-close:hover { background: #c0392b; color: #fff; }
.tab-add {
  background: none; border: 1px dashed #444; color: #888;
  padding: 3px 10px; border-radius: 4px; cursor: pointer;
  font-size: 12px; white-space: nowrap; margin-left: 4px;
}
.tab-add:hover { border-color: #4F8EF7; color: #4F8EF7; }

.main-area {
  display: flex;
  flex: 1;
  min-height: 0;
  position: relative;
}
.sidebar-left { width: 220px; flex-shrink: 0; border-right: 1px solid #2a2a2a; }
.content-area { flex: 1; min-width: 0; }
.sidebar-right { width: 240px; flex-shrink: 0; border-left: 1px solid #2a2a2a; }
.meta-toggle {
  position: absolute;
  right: 0;
  top: 50%;
  transform: translateY(-50%);
  background: #2a2a2a;
  border: 1px solid #333;
  color: #888;
  width: 16px;
  height: 40px;
  cursor: pointer;
  font-size: 10px;
  z-index: 5;
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
  background: #4F8EF7; color: #fff; border: none;
  padding: 10px 24px; border-radius: 6px; cursor: pointer;
  font-size: 14px; font-weight: 500;
}
.btn-primary:hover { background: #6BA3F9; }

.recent-list { margin-top: 16px; width: 400px; }
.recent-title { font-size: 12px; color: #555; margin-bottom: 8px; }
.recent-item {
  display: flex; align-items: center; gap: 10px;
  padding: 8px 12px; border-radius: 6px; cursor: pointer;
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
  display: flex; align-items: center; gap: 8px;
  background: #2a2a2a; border: 1px solid #444;
  padding: 10px 14px; border-radius: 6px;
  font-size: 13px; max-width: 320px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.4);
}
.notification.warning { border-color: #e67e22; }
.notification.error { border-color: #c0392b; }
.notif-action {
  background: #4F8EF7; color: #fff; border: none;
  padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 12px;
}
.notif-close {
  background: none; border: none; color: #666; cursor: pointer;
  font-size: 16px; margin-left: auto;
}

@keyframes spin { to { transform: rotate(360deg); } }
</style>
