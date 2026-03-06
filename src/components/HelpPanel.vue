<template>
  <Teleport to="body">
    <div class="help-overlay" @click.self="$emit('close')" @keydown.escape="$emit('close')" tabindex="0" ref="helpRef">
      <div class="help-panel">
        <div class="help-header">
          <h2>快捷键</h2>
          <input v-model="search" class="search-input" placeholder="搜索快捷键..." />
          <button class="close-btn" @click="$emit('close')">×</button>
        </div>
        <div class="help-content">
          <div v-for="(actions, group) in filteredGroups" :key="group" class="kb-group">
            <div class="group-title">{{ group }}</div>
            <div v-for="actionId in actions" :key="actionId" class="kb-row">
              <span class="kb-label">{{ ACTION_LABELS[actionId] }}</span>
              <span class="kb-combo" v-if="getBinding(actionId)?.enabled">
                <kbd>{{ getBinding(actionId)?.key_combo }}</kbd>
              </span>
              <span class="kb-disabled" v-else>已禁用</span>
            </div>
          </div>
        </div>
        <div class="help-footer">
          按 <kbd>?</kbd> 显示此面板 · 在设置中自定义快捷键
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick } from 'vue'
import { useKeybindingStore, ACTION_LABELS, ACTION_GROUPS } from '../stores/keybinding'

const emit = defineEmits(['close'])
const kbStore = useKeybindingStore()
const helpRef = ref<HTMLElement>()
const search = ref('')

// Deduplicate actions shown (some appear in multiple groups)
const allUniqueGroups = computed(() => {
  const groups: Record<string, string[]> = {}
  for (const [group, actions] of Object.entries(ACTION_GROUPS)) {
    const unique = [...new Set(actions)].filter(a => {
      const label = ACTION_LABELS[a] ?? a
      return !search.value || label.toLowerCase().includes(search.value.toLowerCase())
        || (kbStore.getBinding(a)?.key_combo ?? '').toLowerCase().includes(search.value.toLowerCase())
    })
    if (unique.length > 0) groups[group] = unique
  }
  return groups
})

const filteredGroups = computed(() => allUniqueGroups.value)

function getBinding(actionId: string) {
  return kbStore.getBinding(actionId)
}

onMounted(() => { nextTick(() => helpRef.value?.focus()) })
</script>

<style scoped>
.help-overlay {
  position: fixed; inset: 0; z-index: 900;
  background: rgba(0,0,0,0.7);
  display: flex; align-items: center; justify-content: center;
  outline: none;
}
.help-panel {
  background: #242424;
  border: 1px solid #333;
  border-radius: 8px;
  width: 560px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 60px rgba(0,0,0,0.6);
}
.help-header {
  display: flex; align-items: center; gap: 12px;
  padding: 16px 20px; border-bottom: 1px solid #333;
}
.help-header h2 { font-size: 16px; color: #eee; flex-shrink: 0; }
.search-input {
  flex: 1; background: #1a1a1a; border: 1px solid #333;
  color: #ccc; padding: 5px 10px; border-radius: 4px; font-size: 13px;
}
.search-input:focus { outline: none; border-color: #4F8EF7; }
.close-btn {
  background: none; border: none; color: #666; cursor: pointer;
  font-size: 20px; width: 28px; height: 28px;
  display: flex; align-items: center; justify-content: center; border-radius: 4px;
}
.close-btn:hover { background: #333; color: #ddd; }

.help-content { overflow-y: auto; flex: 1; padding: 8px 20px; }
.kb-group { margin-bottom: 16px; }
.group-title {
  font-size: 10px; color: #555; text-transform: uppercase;
  letter-spacing: 0.08em; padding: 8px 0 4px;
}
.kb-row {
  display: flex; align-items: center; justify-content: space-between;
  padding: 5px 0; border-bottom: 1px solid #2a2a2a;
  font-size: 13px;
}
.kb-label { color: #bbb; }
.kb-disabled { color: #555; font-size: 11px; }
kbd {
  background: #333; border: 1px solid #555;
  padding: 2px 8px; border-radius: 4px; font-size: 11px;
  color: #ccc; font-family: monospace;
}

.help-footer {
  padding: 12px 20px; border-top: 1px solid #2a2a2a;
  font-size: 12px; color: #555; text-align: center;
}
</style>
