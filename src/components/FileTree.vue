<template>
  <div class="file-tree" @contextmenu.prevent="onTreeContextMenu($event, null)">
    <div class="tree-header">
      <span>{{ tab?.workspace.name }}</span>
    </div>

    <!-- Root (All photos) -->
    <div
      class="tree-item"
      :class="{ active: !activeFolder }"
      @click="selectFolder(null)"
      @contextmenu.stop.prevent="onTreeContextMenu($event, null)"
    >
      <span class="tree-icon">📁</span>
      <span class="tree-name">全部照片</span>
      <span class="tree-count">{{ tab?.photos.length }}</span>
    </div>

    <!-- Subfolders -->
    <div
      v-for="folder in tab?.subfolders"
      :key="folder"
      class="tree-item"
      :class="{ active: activeFolder === folder }"
      :style="{ paddingLeft: `${(folder.split('/').length) * 12 + 8}px` }"
      @click="selectFolder(folder)"
      @contextmenu.stop.prevent="onTreeContextMenu($event, folder)"
    >
      <span class="tree-icon">📁</span>
      <span class="tree-name">{{ folderName(folder) }}</span>
    </div>

    <!-- Context Menu -->
    <n-dropdown
      trigger="manual"
      :x="ctxX"
      :y="ctxY"
      :options="contextMenuOptions"
      :show="showContextMenu"
      @clickoutside="showContextMenu = false"
      @select="onContextMenuSelect"
    />

    <!-- Rename Modal -->
    <n-modal v-model:show="showRename">
      <n-card title="重命名文件夹" style="width: 360px">
        <n-input v-model:value="renameValue" placeholder="输入新名称" @keyup.enter="doRename" />
        <template #footer>
          <div style="display:flex;gap:8px;justify-content:flex-end">
            <n-button @click="showRename = false">取消</n-button>
            <n-button type="primary" @click="doRename">确认</n-button>
          </div>
        </template>
      </n-card>
    </n-modal>

    <!-- New Folder Modal -->
    <n-modal v-model:show="showNewFolder">
      <n-card title="新建文件夹" style="width: 360px">
        <n-input v-model:value="newFolderName" placeholder="文件夹名称" @keyup.enter="doCreateFolder" />
        <template #footer>
          <div style="display:flex;gap:8px;justify-content:flex-end">
            <n-button @click="showNewFolder = false">取消</n-button>
            <n-button type="primary" @click="doCreateFolder">创建</n-button>
          </div>
        </template>
      </n-card>
    </n-modal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NDropdown, NModal, NCard, NInput, NButton, useMessage } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'

const store = useWorkspaceStore()
const message = useMessage()
const tab = computed(() => store.activeTab)

const activeFolder = ref<string | null>(null)
const ctxX = ref(0)
const ctxY = ref(0)
const showContextMenu = ref(false)
const ctxTarget = ref<string | null>(null)

const showRename = ref(false)
const renameValue = ref('')
const showNewFolder = ref(false)
const newFolderName = ref('')

function folderName(path: string) {
  return path.split('/').pop() ?? path
}

function selectFolder(folder: string | null) {
  activeFolder.value = folder
  store.setFilter({ subfolder: folder ?? undefined })
}

function onTreeContextMenu(e: MouseEvent, folder: string | null) {
  ctxX.value = e.clientX
  ctxY.value = e.clientY
  ctxTarget.value = folder
  showContextMenu.value = true
}

const contextMenuOptions = computed(() => {
  const items = []
  if (ctxTarget.value !== null) {
    items.push(
      { label: '在文件管理器中打开', key: 'explorer' },
      { label: '复制路径', key: 'copy_path' },
      { type: 'divider', key: 'd1' },
      { label: '重命名', key: 'rename' },
    )
  }
  items.push(
    { label: '新建文件夹', key: 'new_folder' },
  )
  if (ctxTarget.value !== null) {
    items.push(
      { type: 'divider', key: 'd2' },
      { label: '移入回收站', key: 'delete' },
    )
  }
  return items
})

async function onContextMenuSelect(key: string) {
  showContextMenu.value = false
  const t = tab.value
  if (!t) return

  const folderPath = ctxTarget.value
    ? `${t.workspace.path}/${ctxTarget.value}`
    : t.workspace.path

  if (key === 'explorer') {
    await invoke('open_in_explorer', { path: folderPath })
  } else if (key === 'copy_path') {
    await navigator.clipboard.writeText(folderPath)
    message.success('路径已复制')
  } else if (key === 'rename') {
    renameValue.value = folderName(ctxTarget.value ?? t.workspace.name)
    showRename.value = true
  } else if (key === 'new_folder') {
    newFolderName.value = ''
    showNewFolder.value = true
  } else if (key === 'delete') {
    // Simple confirmation via message
    message.warning('文件夹删除功能请在文件管理器中操作')
  }
}

async function doRename() {
  const t = tab.value
  if (!t || ctxTarget.value === null) return
  const fullPath = `${t.workspace.path}/${ctxTarget.value}`
  await invoke('rename_folder', { path: fullPath, newName: renameValue.value })
  showRename.value = false
  // Refresh subfolders
  const subfolders: string[] = await invoke('get_subfolders', {
    workspaceId: t.workspace.id,
    rootPath: t.workspace.path,
  })
  t.subfolders = subfolders
  message.success('重命名成功')
}

async function doCreateFolder() {
  const t = tab.value
  if (!t) return
  const parent = ctxTarget.value
    ? `${t.workspace.path}/${ctxTarget.value}`
    : t.workspace.path
  await invoke('create_folder', { parentPath: parent, name: newFolderName.value })
  showNewFolder.value = false
  const subfolders: string[] = await invoke('get_subfolders', {
    workspaceId: t.workspace.id,
    rootPath: t.workspace.path,
  })
  t.subfolders = subfolders
  message.success('文件夹已创建')
}
</script>

<style scoped>
.file-tree {
  height: 100%;
  overflow-y: auto;
  background: #1c1c1c;
  padding: 8px 0;
}
.tree-header {
  padding: 4px 12px 8px;
  font-size: 11px;
  color: #555;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px 5px 8px;
  cursor: pointer;
  font-size: 13px;
  color: #aaa;
  border-radius: 4px;
  margin: 1px 4px;
  transition: background 0.12s;
}
.tree-item:hover { background: #2a2a2a; color: #ddd; }
.tree-item.active { background: #1e3a5f; color: #4F8EF7; }
.tree-icon { font-size: 13px; flex-shrink: 0; }
.tree-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree-count { font-size: 10px; color: #555; background: #2a2a2a; padding: 1px 5px; border-radius: 8px; }
.tree-item.active .tree-count { background: #1a3050; color: #4F8EF7; }
</style>
