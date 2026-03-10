<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-panel">
      <div class="settings-header">
        <h2>设置</h2>
        <button class="close-btn" @click="emit('close')">×</button>
      </div>

      <div class="settings-nav">
        <button :class="{ active: activeSection === 'performance' }" @click="activeSection = 'performance'">性能与缓存</button>
        <button :class="{ active: activeSection === 'keybindings' }" @click="activeSection = 'keybindings'">快捷键</button>
        <button :class="{ active: activeSection === 'about' }" @click="activeSection = 'about'">软件信息</button>
      </div>

      <div class="settings-body">
        <div v-if="activeSection === 'performance'" class="section">
          <div class="section-title">预热策略</div>
          <div class="notice">首次打开工作区时先预热固定数量；可选开启后台小批持续预热，减少后续大图和网格首次加载等待。</div>

          <div class="form-row">
            <label>启动预热数量</label>
            <input v-model.number="initialLimit" type="number" min="0" max="10000" step="10" />
          </div>

          <div class="form-row checkbox-row">
            <label>
              <input v-model="continueInBackground" type="checkbox" />
              无感继续热处理（小批后台持续）
            </label>
          </div>

          <div class="form-row">
            <label>预热线程数</label>
            <input v-model.number="workerConcurrency" type="number" min="1" max="8" step="1" />
          </div>

          <div class="form-row checkbox-row">
            <label>
              <input v-model="popupAutoShow" type="checkbox" />
              每次打开工作区时自动显示预热进度卡片
            </label>
          </div>

          <div class="form-row">
            <label>网格行布局</label>
            <select v-model="gridRowAlignMode">
              <option value="center">整行居中（默认）</option>
              <option value="stretch">自动拉伸填满整行</option>
            </select>
          </div>

          <div class="actions">
            <button class="primary" :disabled="savingSettings" @click="saveWarmupPrefs">
              {{ savingSettings ? '保存中...' : '保存并应用' }}
            </button>
          </div>

          <div class="section-title cache-title">缓存维护</div>
          <div class="notice">重建缓存会清空现有缩略图缓存，并按当前预热策略重新开始预热。</div>
          <div class="cache-info">
            <div class="cache-info-row">
              <span>缓存目录</span>
              <code
                class="cache-path"
                :class="{ clickable: !!cachePath }"
                :title="cachePath || '无可用路径'"
                @click="copyCachePath"
              >{{ cachePath || '-' }}</code>
            </div>
            <div class="cache-info-row">
              <span>占用大小</span>
              <strong>{{ cacheInfoLoading ? '计算中...' : formatBytes(cacheSizeBytes) }}</strong>
            </div>
            <div class="cache-profiles">
              <div class="cache-profiles-title">分组占用</div>
              <div v-if="cacheProfileEntries.length === 0" class="cache-profile-row">
                <span>-</span>
                <strong>-</strong>
              </div>
              <div v-for="[name, size] in cacheProfileEntries" :key="name" class="cache-profile-row">
                <span>{{ formatProfileName(name) }}</span>
                <strong>{{ formatBytes(size) }}</strong>
              </div>
            </div>
          </div>
          <div class="actions">
            <button :disabled="cacheInfoLoading" @click="loadCacheInfo">
              {{ cacheInfoLoading ? '刷新中...' : '刷新缓存信息' }}
            </button>
            <button :disabled="openingCacheFolder || !cachePath" @click="openCacheFolder">
              {{ openingCacheFolder ? '打开中...' : '在文件管理器中打开缓存目录' }}
            </button>
            <button :disabled="rebuildingCache" @click="rebuildCache">
              {{ rebuildingCache ? '重建中...' : '重建缩略图缓存' }}
            </button>
          </div>
          <div v-if="performanceMessage" class="msg">{{ performanceMessage }}</div>
        </div>

        <div v-if="activeSection === 'keybindings'" class="section">
          <div class="notice">点击操作行，然后按下新的按键组合以重新绑定；双击禁用某个快捷键。</div>
          <div v-for="(actions, group) in ACTION_GROUPS" :key="group" class="kb-group">
            <div class="group-title">{{ group }}</div>
            <div
              v-for="actionId in actions"
              :key="actionId"
              class="kb-row"
              :class="{ listening: listeningAction === actionId }"
              @click="startListen(actionId)"
              @dblclick="disableBinding(actionId)"
            >
              <span class="kb-label">{{ ACTION_LABELS[actionId] ?? actionId }}</span>
              <div class="kb-right">
                <span v-if="listeningAction === actionId" class="listening-hint">请按下新按键... (ESC 取消)</span>
                <template v-else>
                  <kbd v-if="getBinding(actionId)?.enabled">{{ getBinding(actionId)?.key_combo }}</kbd>
                  <span v-else class="kb-disabled">已禁用</span>
                  <button
                    class="restore-btn"
                    title="重新启用"
                    @click.stop="restoreDefault(actionId)"
                  >↩</button>
                </template>
              </div>
            </div>
          </div>
        </div>

        <div v-if="activeSection === 'about'" class="section">
          <div class="section-title">应用信息</div>
          <div class="info-list">
            <div class="info-row"><span>名称</span><strong>{{ appName }}</strong></div>
            <div class="info-row"><span>版本</span><strong>{{ appVersion }}</strong></div>
            <div class="info-row"><span>Tauri</span><strong>{{ tauriVersion }}</strong></div>
            <div class="info-row"><span>前端栈</span><strong>Vue 3 + TypeScript + Vite</strong></div>
            <div class="info-row"><span>平台</span><strong>{{ runtimePlatform }}</strong></div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getName, getTauriVersion, getVersion } from '@tauri-apps/api/app'
import { useKeybindingStore, ACTION_LABELS, ACTION_GROUPS } from '../stores/keybinding'
import { useWorkspaceStore } from '../stores/workspace'
import { readWarmupSettings, saveWarmupSettings } from '../utils/warmup-settings'
import { setGridRowAlignMode, useGridRowAlignMode, type GridRowAlignMode } from '../utils/grid-row-settings'
import { clearGridThumbCaches } from '../utils/thumb-loader'

interface PreviewCacheInfo {
  path: string
  size_bytes: number
  profile_sizes?: Record<string, number>
}

const emit = defineEmits<{ close: [] }>()

const kbStore = useKeybindingStore()
const workspaceStore = useWorkspaceStore()
const listeningAction = ref<string | null>(null)
const activeSection = ref<'performance' | 'keybindings' | 'about'>('performance')

const initialLimit = ref(40)
const continueInBackground = ref(true)
const workerConcurrency = ref(3)
const popupAutoShow = ref(true)
const gridRowAlignMode = ref<GridRowAlignMode>('center')
const performanceMessage = ref('')
const savingSettings = ref(false)
const rebuildingCache = ref(false)
const cacheInfoLoading = ref(false)
const openingCacheFolder = ref(false)
const cachePath = ref('')
const cacheSizeBytes = ref<number | null>(null)
const cacheProfileSizes = ref<Record<string, number>>({})
const cacheProfileEntries = computed(() => Object.entries(cacheProfileSizes.value).sort((a, b) => b[1] - a[1]))

const appName = ref('MyPhoto')
const appVersion = ref('-')
const tauriVersion = ref('-')
const runtimePlatform = ref(navigator.userAgent)

function loadWarmupPrefs() {
  const settings = readWarmupSettings()
  initialLimit.value = settings.initialLimit
  continueInBackground.value = settings.continueInBackground
  workerConcurrency.value = settings.workerConcurrency
  popupAutoShow.value = settings.popupAutoShow
  gridRowAlignMode.value = useGridRowAlignMode().value
}

function formatBytes(bytes: number | null) {
  if (bytes == null || bytes < 0) return '-'
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let idx = 0
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024
    idx += 1
  }
  return `${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : 2)} ${units[idx]}`
}

function formatProfileName(raw: string) {
  if (!raw) return '未分类'
  if (raw === 'legacy') return 'Legacy/未分组'
  return raw
}

async function loadCacheInfo() {
  cacheInfoLoading.value = true
  try {
    const info: PreviewCacheInfo = await invoke('get_preview_cache_info')
    cachePath.value = info.path ?? ''
    cacheSizeBytes.value = Number.isFinite(info.size_bytes) ? info.size_bytes : 0
    cacheProfileSizes.value = info.profile_sizes ?? {}
  } catch (e) {
    cachePath.value = ''
    cacheSizeBytes.value = null
    cacheProfileSizes.value = {}
    performanceMessage.value = `读取缓存信息失败：${String(e)}`
  } finally {
    cacheInfoLoading.value = false
  }
}

async function openCacheFolder() {
  if (!cachePath.value) return
  openingCacheFolder.value = true
  try {
    await invoke('open_with_default_app', { path: cachePath.value })
  } catch (e) {
    performanceMessage.value = `打开缓存目录失败：${String(e)}`
  } finally {
    openingCacheFolder.value = false
  }
}

async function copyCachePath() {
  if (!cachePath.value) return
  try {
    await navigator.clipboard.writeText(cachePath.value)
    performanceMessage.value = '缓存目录路径已复制。'
  } catch (e) {
    performanceMessage.value = `复制缓存目录失败：${String(e)}`
  }
}

function getBinding(actionId: string) {
  return kbStore.getBinding(actionId)
}

function startListen(actionId: string) {
  listeningAction.value = actionId
}

async function disableBinding(actionId: string) {
  listeningAction.value = null
  const b = getBinding(actionId)
  if (!b) return
  await invoke('update_keybinding', {
    actionId,
    keyCombo: b.key_combo,
    enabled: false,
  })
  await kbStore.load()
}

async function restoreDefault(actionId: string) {
  const b = getBinding(actionId)
  if (!b) return
  await invoke('update_keybinding', {
    actionId,
    keyCombo: b.key_combo,
    enabled: true,
  })
  await kbStore.load()
}

async function onKeyDown(e: KeyboardEvent) {
  if (!listeningAction.value) return
  e.preventDefault()
  e.stopPropagation()

  if (e.key === 'Escape') {
    listeningAction.value = null
    return
  }

  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.altKey) parts.push('Alt')
  if (e.shiftKey) parts.push('Shift')
  if (e.metaKey) parts.push('Meta')

  const ignore = ['Control', 'Alt', 'Shift', 'Meta']
  if (!ignore.includes(e.key)) {
    parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key)
  }

  const combo = parts.join('+')
  const actionId = listeningAction.value
  await invoke('update_keybinding', {
    actionId,
    keyCombo: combo,
    enabled: true,
  })
  await kbStore.load()
  listeningAction.value = null
}

async function saveWarmupPrefs() {
  savingSettings.value = true
  try {
    const next = saveWarmupSettings({
      initialLimit: initialLimit.value,
      continueInBackground: continueInBackground.value,
      workerConcurrency: workerConcurrency.value,
      popupAutoShow: popupAutoShow.value,
    })
    setGridRowAlignMode(gridRowAlignMode.value)
    initialLimit.value = next.initialLimit
    continueInBackground.value = next.continueInBackground
    workerConcurrency.value = next.workerConcurrency
    popupAutoShow.value = next.popupAutoShow
    workspaceStore.restartWarmupForActiveWorkspace()
    performanceMessage.value = '已保存并应用到当前工作区。'
  } finally {
    savingSettings.value = false
  }
}

async function rebuildCache() {
  rebuildingCache.value = true
  performanceMessage.value = ''
  try {
    await workspaceStore.cancelAllWarmupPipelines()
    clearGridThumbCaches()
    const removed: number = await invoke('rebuild_preview_cache')
    workspaceStore.restartWarmupForActiveWorkspace()
    performanceMessage.value = `缓存已重建，清理文件 ${removed} 个，已按当前策略重新开始预热。`
    await loadCacheInfo()
  } catch (e) {
    performanceMessage.value = `重建缓存失败：${String(e)}`
  } finally {
    rebuildingCache.value = false
  }
}

onMounted(async () => {
  loadWarmupPrefs()
  await loadCacheInfo()
  window.addEventListener('keydown', onKeyDown, true)
  appName.value = await getName().catch(() => 'MyPhoto')
  appVersion.value = await getVersion().catch(() => '-')
  tauriVersion.value = await getTauriVersion().catch(() => '-')
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown, true)
})
</script>

<style scoped>
.settings-overlay {
  position: fixed;
  inset: 0;
  z-index: 1200;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  justify-content: center;
  align-items: center;
}

.settings-panel {
  width: min(980px, calc(100vw - 48px));
  max-height: calc(100vh - 56px);
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: 10px;
  display: flex;
  flex-direction: column;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid #2a2a2a;
}

.settings-header h2 {
  color: #eee;
  font-size: 16px;
}

.close-btn {
  border: none;
  background: transparent;
  color: #aaa;
  font-size: 20px;
  cursor: pointer;
  line-height: 1;
}
.close-btn:hover { color: #fff; }

.settings-nav {
  display: flex;
  gap: 8px;
  padding: 10px 16px;
  border-bottom: 1px solid #2a2a2a;
}
.settings-nav button {
  border: 1px solid #3a3a3a;
  background: #232323;
  color: #aaa;
  border-radius: 6px;
  padding: 6px 10px;
  cursor: pointer;
}
.settings-nav button.active {
  color: #fff;
  border-color: #4f8ef7;
  background: #1f2f4b;
}

.settings-body {
  overflow: auto;
  padding: 14px 20px 20px;
}
.section { display: flex; flex-direction: column; gap: 10px; }
.section-title {
  font-size: 12px;
  letter-spacing: 0.04em;
  color: #7f8ea9;
}
.cache-title { margin-top: 8px; }
.cache-info {
  border: 1px solid #2a2a2a;
  border-radius: 8px;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  background: #1c1c1c;
}
.cache-info-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 12px;
  color: #b2bccd;
}
.cache-info-row span {
  color: #8894aa;
  flex-shrink: 0;
}
.cache-info-row code {
  color: #d4dbe8;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 12px;
  background: #242424;
  border: 1px solid #303030;
  border-radius: 6px;
  padding: 3px 6px;
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cache-path.clickable {
  cursor: pointer;
}
.cache-path.clickable:hover {
  border-color: #4f8ef7;
  background: #1f2f4b;
}
.cache-info-row strong { color: #d4dbe8; font-weight: 600; }
.cache-profiles {
  border-top: 1px dashed #2b2b2b;
  padding-top: 6px;
  margin-top: 2px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.cache-profiles-title {
  color: #7f8ea9;
  font-size: 11px;
}
.cache-profile-row {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  font-size: 12px;
  color: #b2bccd;
}
.cache-profile-row span {
  color: #8894aa;
}
.cache-profile-row strong {
  color: #d4dbe8;
  font-weight: 600;
}
.notice {
  color: #8c8c8c;
  font-size: 12px;
  line-height: 1.5;
}
.form-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.form-row label { width: 130px; color: #ccc; font-size: 13px; }
.form-row input[type='number'] {
  width: 120px;
  background: #232323;
  color: #ddd;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  padding: 6px 8px;
}
.form-row select {
  min-width: 220px;
  background: #232323;
  color: #ddd;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  padding: 6px 8px;
}
.checkbox-row label { width: auto; }
.actions {
  display: flex;
  gap: 10px;
}
.actions button {
  border: 1px solid #3a3a3a;
  background: #242424;
  color: #ddd;
  border-radius: 6px;
  padding: 7px 12px;
  cursor: pointer;
}
.actions button.primary {
  border-color: #4f8ef7;
  background: #1e3a5f;
}
.actions button:disabled {
  opacity: 0.6;
  cursor: default;
}
.msg {
  color: #95a6c5;
  font-size: 12px;
}

.kb-group { margin-bottom: 16px; }
.group-title {
  font-size: 10px;
  color: #555;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  padding: 8px 0 4px;
}
.kb-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  margin: 0 -8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  border-bottom: 1px solid #222;
}
.kb-row:hover { background: #252525; }
.kb-row.listening { background: #1e2d4a; }
.kb-label { color: #bbb; }
.kb-right { display: flex; align-items: center; gap: 8px; }
.listening-hint { color: #4f8ef7; font-size: 12px; }
.kb-disabled { color: #555; font-size: 11px; }
kbd {
  background: #333;
  border: 1px solid #555;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 11px;
  color: #ccc;
  font-family: monospace;
}
.restore-btn {
  background: none;
  border: none;
  color: #555;
  cursor: pointer;
  font-size: 14px;
  padding: 1px 4px;
  border-radius: 3px;
}
.restore-btn:hover { color: #aaa; background: #333; }

.info-list {
  border: 1px solid #2a2a2a;
  border-radius: 8px;
  overflow: hidden;
}
.info-row {
  display: flex;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid #252525;
}
.info-row:last-child { border-bottom: none; }
.info-row span { color: #888; }
.info-row strong { color: #ddd; font-weight: 600; }
</style>
