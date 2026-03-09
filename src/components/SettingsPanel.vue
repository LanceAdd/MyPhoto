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
          <div class="actions">
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
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { getName, getTauriVersion, getVersion } from '@tauri-apps/api/app'
import { useKeybindingStore, ACTION_LABELS, ACTION_GROUPS } from '../stores/keybinding'
import { useWorkspaceStore } from '../stores/workspace'
import { readWarmupSettings, saveWarmupSettings } from '../utils/warmup-settings'
import { setGridRowAlignMode, useGridRowAlignMode, type GridRowAlignMode } from '../utils/grid-row-settings'

const emit = defineEmits<{ close: [] }>()

const kbStore = useKeybindingStore()
const workspaceStore = useWorkspaceStore()
const listeningAction = ref<string | null>(null)
const activeSection = ref<'performance' | 'keybindings' | 'about'>('performance')

const initialLimit = ref(200)
const continueInBackground = ref(true)
const gridRowAlignMode = ref<GridRowAlignMode>('center')
const performanceMessage = ref('')
const savingSettings = ref(false)
const rebuildingCache = ref(false)

const appName = ref('MyPhoto')
const appVersion = ref('-')
const tauriVersion = ref('-')
const runtimePlatform = ref(navigator.userAgent)

function loadWarmupPrefs() {
  const settings = readWarmupSettings()
  initialLimit.value = settings.initialLimit
  continueInBackground.value = settings.continueInBackground
  gridRowAlignMode.value = useGridRowAlignMode().value
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
    })
    setGridRowAlignMode(gridRowAlignMode.value)
    initialLimit.value = next.initialLimit
    continueInBackground.value = next.continueInBackground
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
    const removed: number = await invoke('rebuild_preview_cache')
    workspaceStore.restartWarmupForActiveWorkspace()
    performanceMessage.value = `缓存已重建，清理文件 ${removed} 个，已按当前策略重新开始预热。`
  } catch (e) {
    performanceMessage.value = `重建缓存失败：${String(e)}`
  } finally {
    rebuildingCache.value = false
  }
}

onMounted(async () => {
  loadWarmupPrefs()
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
