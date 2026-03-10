<template>
  <div class="file-tree" @contextmenu.prevent="onTreeContextMenu($event, null, 'root')">
    <div class="tree-header">
      <span>{{ tab?.workspace.name }}</span>
    </div>

    <div
      class="tree-item"
      :class="{ active: !activeFolder && !activeFile }"
      @click="selectFolder(null)"
      @contextmenu.stop.prevent="onTreeContextMenu($event, null, 'root')"
    >
      <span class="tree-icon icon-root">📔</span>
      <span class="tree-name" title="全部照片">全部照片</span>
      <span class="tree-count">{{ tab?.treeFiles.length ?? 0 }}</span>
    </div>

    <div
      v-for="node in treeRows"
      :key="`${node.kind}:${node.path}`"
      class="tree-item"
      :class="{
        active: (node.kind === 'folder' && activeFolder === node.path && !activeFile) ||
          (node.kind === 'file' && activeFile === node.path),
        'is-file': node.kind === 'file',
      }"
      :style="{ paddingLeft: `${node.depth * 12 + 8}px` }"
      @click="node.kind === 'folder' ? selectFolder(node.path) : selectFile(node.path)"
      @contextmenu.stop.prevent="onTreeContextMenu($event, node.path, node.kind)"
    >
      <span class="tree-icon" :class="node.kind === 'folder' ? 'icon-folder' : 'icon-file'">
        {{ node.kind === 'folder' ? '📁' : '🖼️' }}
      </span>
      <span class="tree-name" :title="node.name">{{ shortenName(node.name, node.kind) }}</span>
      <span v-if="node.kind === 'folder'" class="tree-count">
        {{ folderFileCounts.get(node.path) ?? 0 }}
      </span>
    </div>

    <n-dropdown
      trigger="manual"
      :x="ctxX"
      :y="ctxY"
      :options="contextMenuOptions"
      :show="showContextMenu"
      @clickoutside="showContextMenu = false"
      @select="onContextMenuSelect"
    />

    <n-modal v-model:show="showRename">
      <n-card :title="renameTitle" style="width: 360px">
        <n-input v-model:value="renameValue" placeholder="请输入新名称" @keyup.enter="doRename" />
        <template #footer>
          <div style="display:flex;gap:8px;justify-content:flex-end">
            <n-button @click="showRename = false">取消</n-button>
            <n-button type="primary" @click="doRename">确认</n-button>
          </div>
        </template>
      </n-card>
    </n-modal>

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
import { useWorkspaceStore, type WorkspaceFile } from '../stores/workspace'
import { clearGridThumbCaches } from '../utils/thumb-loader'

type ContextTargetType = 'root' | 'folder' | 'file'

interface FlatTreeNode {
  kind: 'folder' | 'file'
  path: string
  name: string
  depth: number
}

interface BatchRenameSummary {
  total: number
  renamed: number
  skipped_conflict: number
  skipped_missing: number
  failed: number
}

const store = useWorkspaceStore()
const message = useMessage()
const tab = computed(() => store.activeTab)

const activeFolder = ref<string | null>(null)
const activeFile = ref<string | null>(null)
const fileSelectToken = ref(0)

const ctxX = ref(0)
const ctxY = ref(0)
const showContextMenu = ref(false)
const ctxTargetPath = ref<string | null>(null)
const ctxTargetType = ref<ContextTargetType>('root')

const showRename = ref(false)
const renameValue = ref('')
const showNewFolder = ref(false)
const newFolderName = ref('')
const batchRenamePrefix = ref('IMG_')
const batchRenaming = ref(false)

const renameTitle = computed(() => ctxTargetType.value === 'file' ? '重命名文件' : '重命名文件夹')

function normalizePath(path: string) {
  return path.replace(/\\/g, '/').replace(/^\/+|\/+$/g, '')
}

function pathName(path: string) {
  const p = normalizePath(path)
  return p.split('/').pop() ?? p
}

function shortenName(name: string, kind: 'folder' | 'file') {
  const max = kind === 'folder' ? 28 : 32
  if (name.length <= max) return name
  if (kind === 'folder') {
    const head = name.slice(0, 14)
    const tail = name.slice(-10)
    return `${head}...${tail}`
  }
  const dot = name.lastIndexOf('.')
  if (dot > 0 && dot < name.length - 1) {
    const ext = name.slice(dot)
    const base = name.slice(0, dot)
    const keep = Math.max(8, max - ext.length - 3)
    if (base.length <= keep) return name
    return `${base.slice(0, keep)}...${ext}`
  }
  return `${name.slice(0, max - 3)}...`
}

function parentFolder(path: string) {
  const p = normalizePath(path)
  const idx = p.lastIndexOf('/')
  return idx >= 0 ? p.slice(0, idx) : null
}

function toRelativePath(absPath: string, rootPath: string) {
  const abs = normalizePath(absPath)
  const root = normalizePath(rootPath)
  if (abs === root) return ''
  if (abs.startsWith(`${root}/`)) return abs.slice(root.length + 1)
  return abs
}

function buildTreeRows(files: WorkspaceFile[]) {
  const folderChildren = new Map<string, Set<string>>()
  const filesByFolder = new Map<string, string[]>()
  const folderCounts = new Map<string, number>()

  for (const file of files) {
    const rel = normalizePath(file.relative_path)
    if (!rel) continue
    const parts = rel.split('/').filter(Boolean)
    if (parts.length === 0) continue

    let parent = ''
    for (let i = 0; i < parts.length - 1; i++) {
      const current = parent ? `${parent}/${parts[i]}` : parts[i]
      if (!folderChildren.has(parent)) folderChildren.set(parent, new Set())
      folderChildren.get(parent)!.add(current)
      folderCounts.set(current, (folderCounts.get(current) ?? 0) + 1)
      parent = current
    }

    const folder = parts.length > 1 ? parts.slice(0, parts.length - 1).join('/') : ''
    if (!filesByFolder.has(folder)) filesByFolder.set(folder, [])
    filesByFolder.get(folder)!.push(rel)
  }

  const rows: FlatTreeNode[] = []
  const walk = (folder: string, depth: number) => {
    const childFolders = [...(folderChildren.get(folder) ?? new Set<string>())]
      .sort((a, b) => pathName(a).localeCompare(pathName(b)))

    for (const childFolder of childFolders) {
      rows.push({
        kind: 'folder',
        path: childFolder,
        name: pathName(childFolder),
        depth,
      })
      walk(childFolder, depth + 1)
    }

    const childFiles = [...(filesByFolder.get(folder) ?? [])]
      .sort((a, b) => pathName(a).localeCompare(pathName(b)))

    for (const relFilePath of childFiles) {
      rows.push({
        kind: 'file',
        path: relFilePath,
        name: pathName(relFilePath),
        depth,
      })
    }
  }

  walk('', 1)
  return { rows, folderCounts }
}

const builtTree = computed(() => buildTreeRows(tab.value?.treeFiles ?? []))
const treeRows = computed(() => builtTree.value.rows)
const folderFileCounts = computed(() => builtTree.value.folderCounts)

function selectFolder(folder: string | null) {
  activeFolder.value = folder
  activeFile.value = null
  fileSelectToken.value++
  store.setFilter({ subfolder: folder ?? undefined })
}

async function selectFile(filePath: string) {
  const token = ++fileSelectToken.value
  const normalizedFilePath = normalizePath(filePath)
  activeFile.value = normalizedFilePath
  const folder = parentFolder(normalizedFilePath)
  activeFolder.value = folder

  await store.setFilter({ subfolder: folder ?? undefined })

  if (token !== fileSelectToken.value) return
  const t = tab.value
  if (!t) return

  const photo = t.photos.find(p => normalizePath(p.relative_path) === normalizedFilePath)
  if (photo) {
    store.selectPhoto(photo.id, 'single')
  }
}

function formatBatchRenameMessage(summary: BatchRenameSummary) {
  return `共 ${summary.total} 张，成功 ${summary.renamed}，冲突跳过 ${summary.skipped_conflict}，缺失跳过 ${summary.skipped_missing}，失败 ${summary.failed}`
}

async function batchRenameAllPhotos() {
  const t = tab.value
  if (!t || batchRenaming.value) return

  const nextPrefix = window.prompt('请输入批量重命名前缀（默认 IMG_）', batchRenamePrefix.value) ?? ''
  const prefix = nextPrefix.trim()
  if (!prefix) {
    message.warning('重命名前缀不能为空')
    return
  }
  batchRenamePrefix.value = prefix

  const confirmed = window.confirm(
    '重命名会触发缩略图重新预热。\n当前缓存键包含文件路径，旧缓存不会直接命中，将按新路径逐步生成。\n是否继续？',
  )
  if (!confirmed) return

  batchRenaming.value = true
  try {
    const summary: BatchRenameSummary = await invoke('batch_rename_workspace_photos', {
      workspaceId: t.workspace.id,
      workspacePath: t.workspace.path,
      prefix,
      startIndex: 1,
      padding: 4,
    })

    clearGridThumbCaches()
    activeFolder.value = null
    activeFile.value = null
    fileSelectToken.value++

    await refreshTreeData()
    await store.loadPhotos()
    store.restartWarmupForActiveWorkspace()

    message.success(`${formatBatchRenameMessage(summary)}；已自动重新预热`) 
  } catch (error) {
    message.error(`批量重命名失败: ${String(error)}`)
  } finally {
    batchRenaming.value = false
  }
}

function onTreeContextMenu(e: MouseEvent, path: string | null, kind: ContextTargetType) {
  if (kind === 'file' && path) {
    activeFile.value = normalizePath(path)
  }
  if (kind === 'folder') {
    activeFolder.value = path
    activeFile.value = null
  }
  if (kind === 'root') {
    activeFolder.value = null
    activeFile.value = null
  }

  ctxX.value = e.clientX
  ctxY.value = e.clientY
  ctxTargetPath.value = path ? normalizePath(path) : null
  ctxTargetType.value = kind
  showContextMenu.value = true
}

const contextMenuOptions = computed(() => {
  if (ctxTargetType.value === 'file') {
    return [
      { label: '打开文件', key: 'open_file' },
      { label: '在文件管理器中显示', key: 'explorer' },
      { label: '打开上级文件夹', key: 'open_parent_folder' },
      { type: 'divider', key: 'd0' },
      { label: '复制绝对路径', key: 'copy_path' },
      { label: '复制相对路径', key: 'copy_relative_path' },
      { type: 'divider', key: 'd1' },
      { label: '重命名', key: 'rename' },
      { label: '删除', key: 'delete' },
    ]
  }

  if (ctxTargetType.value === 'folder') {
    return [
      { label: '打开文件夹', key: 'open_folder' },
      { label: '在文件管理器中显示', key: 'explorer' },
      { type: 'divider', key: 'd0' },
      { label: '复制绝对路径', key: 'copy_path' },
      { label: '复制相对路径', key: 'copy_relative_path' },
      { type: 'divider', key: 'd1' },
      { label: '重命名', key: 'rename' },
      { label: '新建子文件夹', key: 'new_folder' },
      { label: '删除文件夹', key: 'delete' },
      { type: 'divider', key: 'd2' },
      { label: '刷新文件树', key: 'refresh_tree' },
      { label: '重新扫描工作区', key: 'rescan_workspace' },
    ]
  }

  return [
    { label: '在文件管理器中显示', key: 'explorer' },
    { type: 'divider', key: 'd0' },
    { label: '复制绝对路径', key: 'copy_path' },
    { label: '新建文件夹', key: 'new_folder' },
    { label: '批量重命名全部图片', key: 'batch_rename_all' },
    { type: 'divider', key: 'd1' },
    { label: '刷新文件树', key: 'refresh_tree' },
    { label: '重新扫描工作区', key: 'rescan_workspace' },
  ]
})

function resolveContextAbsolutePath() {
  const t = tab.value
  if (!t) return null
  const rel = ctxTargetPath.value
  if (!rel) return t.workspace.path
  return `${t.workspace.path}/${rel}`
}

async function refreshTreeData() {
  const t = tab.value
  if (!t) return

  const [subfolders, files] = await Promise.all([
    invoke<string[]>('get_subfolders', {
      workspaceId: t.workspace.id,
      rootPath: t.workspace.path,
    }),
    invoke<WorkspaceFile[]>('get_workspace_files', {
      rootPath: t.workspace.path,
    }),
  ])

  t.subfolders = subfolders
  t.treeFiles = files
}

async function requestRescan() {
  const t = tab.value
  if (!t) return
  await invoke('rescan_workspace', {
    workspaceId: t.workspace.id,
    workspacePath: t.workspace.path,
  })
}

async function onContextMenuSelect(key: string) {
  showContextMenu.value = false
  const t = tab.value
  if (!t) return

  const absPath = resolveContextAbsolutePath()
  if (!absPath) return

  if (key === 'open_file') {
    await invoke('open_with_default_app', { path: absPath })
    return
  }

  if (key === 'open_folder') {
    selectFolder(ctxTargetPath.value)
    return
  }

  if (key === 'open_parent_folder') {
    const parent = parentFolder(ctxTargetPath.value ?? '')
    selectFolder(parent)
    return
  }

  if (key === 'explorer') {
    await invoke('open_in_explorer', { path: absPath })
    return
  }

  if (key === 'copy_path') {
    await navigator.clipboard.writeText(absPath)
    message.success('已复制绝对路径')
    return
  }

  if (key === 'copy_relative_path') {
    const rel = ctxTargetPath.value ?? '.'
    await navigator.clipboard.writeText(rel)
    message.success('已复制相对路径')
    return
  }

  if (key === 'batch_rename_all') {
    await batchRenameAllPhotos()
    return
  }

  if (key === 'rename') {
    if (ctxTargetType.value === 'root') return
    renameValue.value = pathName(ctxTargetPath.value ?? '')
    showRename.value = true
    return
  }

  if (key === 'new_folder') {
    newFolderName.value = ''
    showNewFolder.value = true
    return
  }

  if (key === 'delete') {
    if (ctxTargetType.value === 'root') return
    const targetLabel = pathName(ctxTargetPath.value ?? '')
    const targetKind = ctxTargetType.value === 'folder' ? '文件夹' : '文件'
    const confirmed = window.confirm(`确认永久删除${targetKind}“${targetLabel}”？`)
    if (!confirmed) return

    await invoke('delete_entry', {
      path: absPath,
      isDir: ctxTargetType.value === 'folder',
    })
    await refreshTreeData()
    await requestRescan()
    await store.loadPhotos()
    message.success(`已删除${targetKind}`)
    return
  }

  if (key === 'refresh_tree') {
    await refreshTreeData()
    message.success('文件树已刷新')
    return
  }

  if (key === 'rescan_workspace') {
    await requestRescan()
    message.success('已开始重新扫描工作区')
  }
}

async function doRename() {
  const t = tab.value
  if (!t || !ctxTargetPath.value || ctxTargetType.value === 'root') return

  const oldFullPath = `${t.workspace.path}/${ctxTargetPath.value}`
  const newAbsPath: string = await invoke('rename_entry', {
    path: oldFullPath,
    newName: renameValue.value,
  })

  showRename.value = false
  clearGridThumbCaches()
  await refreshTreeData()
  await requestRescan()
  await store.loadPhotos()

  const nextRelPath = toRelativePath(newAbsPath, t.workspace.path)
  if (ctxTargetType.value === 'file') {
    activeFile.value = nextRelPath
    activeFolder.value = parentFolder(nextRelPath)
  } else {
    activeFolder.value = nextRelPath
    activeFile.value = null
  }
  message.success(`${ctxTargetType.value === 'file' ? '文件' : '文件夹'}已重命名，将自动重新预热`)
}

async function doCreateFolder() {
  const t = tab.value
  if (!t) return

  const targetFolder = ctxTargetType.value === 'folder'
    ? ctxTargetPath.value
    : ctxTargetType.value === 'file'
      ? parentFolder(ctxTargetPath.value ?? '')
      : null

  const parentPath = targetFolder
    ? `${t.workspace.path}/${targetFolder}`
    : t.workspace.path

  await invoke('create_folder', { parentPath, name: newFolderName.value })
  showNewFolder.value = false
  await refreshTreeData()
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
  padding: 5px 8px;
  cursor: pointer;
  font-size: 13px;
  color: #aaa;
  border-radius: 4px;
  margin: 1px 4px;
  transition: background 0.12s;
}
.tree-item:hover { background: #2a2a2a; color: #ddd; }
.tree-item.active { background: #1e3a5f; color: #4F8EF7; }
.tree-item.is-file .tree-icon { opacity: 0.85; }
.tree-icon {
  font-size: 14px;
  line-height: 1;
  flex-shrink: 0;
  width: 16px;
  text-align: center;
}
.icon-root { opacity: 1; }
.icon-folder { opacity: 1; }
.icon-file { opacity: 1; }
.tree-item.active .icon-root,
.tree-item.active .icon-folder,
.tree-item.active .icon-file {
  filter: saturate(1.08);
}
.tree-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tree-count {
  font-size: 10px;
  color: #555;
  background: #2a2a2a;
  padding: 1px 5px;
  border-radius: 8px;
}
.tree-item.active .tree-count { background: #1a3050; color: #4F8EF7; }
</style>
