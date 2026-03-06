<template>
  <div class="settings-panel">
    <div class="settings-header">
      <h2>设置 — 快捷键</h2>
    </div>
    <div class="settings-body">
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
                title="恢复默认"
                @click.stop="restoreDefault(actionId)"
              >↩</button>
            </template>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useKeybindingStore, ACTION_LABELS, ACTION_GROUPS } from '../stores/keybinding'
import { invoke } from '@tauri-apps/api/core'

const kbStore = useKeybindingStore()
const listeningAction = ref<string | null>(null)

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
    id: b.id,
    keyCombo: b.key_combo,
    enabled: false
  })
  await kbStore.load()
}

async function restoreDefault(actionId: string) {
  const b = getBinding(actionId)
  if (!b) return
  // reset by re-enabling with original key (server keeps default)
  await invoke('update_keybinding', {
    id: b.id,
    keyCombo: b.key_combo,
    enabled: true
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

  const ignore = ['Control','Alt','Shift','Meta']
  if (!ignore.includes(e.key)) {
    parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key)
  }

  const combo = parts.join('+')
  const b = getBinding(listeningAction.value)
  if (!b) { listeningAction.value = null; return }

  await invoke('update_keybinding', {
    id: b.id,
    keyCombo: combo,
    enabled: true
  })
  await kbStore.load()
  listeningAction.value = null
}

onMounted(() => window.addEventListener('keydown', onKeyDown, true))
onUnmounted(() => window.removeEventListener('keydown', onKeyDown, true))
</script>

<style scoped>
.settings-panel {
  height: 100%; display: flex; flex-direction: column;
  background: #1a1a1a;
}
.settings-header {
  padding: 16px 20px; border-bottom: 1px solid #2a2a2a;
}
.settings-header h2 { font-size: 16px; color: #eee; }
.settings-body { flex: 1; overflow-y: auto; padding: 8px 20px; }

.notice {
  font-size: 12px; color: #666;
  padding: 10px 0 6px;
}

.kb-group { margin-bottom: 16px; }
.group-title {
  font-size: 10px; color: #555; text-transform: uppercase;
  letter-spacing: 0.08em; padding: 8px 0 4px;
}
.kb-row {
  display: flex; align-items: center; justify-content: space-between;
  padding: 6px 8px; margin: 0 -8px;
  border-radius: 4px; cursor: pointer;
  font-size: 13px;
  border-bottom: 1px solid #222;
}
.kb-row:hover { background: #252525; }
.kb-row.listening { background: #1e2d4a; }

.kb-label { color: #bbb; }
.kb-right { display: flex; align-items: center; gap: 8px; }
.listening-hint { color: #4F8EF7; font-size: 12px; }
.kb-disabled { color: #555; font-size: 11px; }

kbd {
  background: #333; border: 1px solid #555;
  padding: 2px 8px; border-radius: 4px; font-size: 11px;
  color: #ccc; font-family: monospace;
}
.restore-btn {
  background: none; border: none; color: #555; cursor: pointer;
  font-size: 14px; padding: 1px 4px; border-radius: 3px;
}
.restore-btn:hover { color: #aaa; background: #333; }
</style>
