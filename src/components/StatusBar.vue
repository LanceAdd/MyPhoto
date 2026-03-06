<template>
  <div class="status-bar">
    <span>共 {{ tab?.photos.length ?? 0 }} 张</span>
    <template v-if="hasFilter">
      <span class="sep">|</span>
      <span>已筛选 {{ filteredCount }} 张</span>
    </template>
    <template v-if="selectedCount > 0">
      <span class="sep">|</span>
      <span class="selected-count">已选 {{ selectedCount }} 张</span>
    </template>
    <template v-if="missingCount > 0">
      <span class="sep">|</span>
      <span class="missing-warn">⚠ {{ missingCount }} 张文件已丢失</span>
    </template>
    <div class="spacer" />
    <span v-if="tab?.scanning" class="scanning">⟳ 扫描中...</span>
    <span v-else class="workspace-path">{{ tab?.workspace.path }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'

const store = useWorkspaceStore()
const tab = computed(() => store.activeTab)
const selectedCount = computed(() => tab.value?.selectedIds.size ?? 0)
const hasFilter = computed(() => {
  const f = tab.value?.filter
  return f && (f.star_min || f.color_labels?.length || f.subfolder)
})
const filteredCount = computed(() => tab.value?.photos.length ?? 0)
const missingCount = computed(() => tab.value?.photos.filter(p => p.is_missing).length ?? 0)
</script>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  height: 24px;
  background: #111;
  border-top: 1px solid #2a2a2a;
  padding: 0 12px;
  font-size: 11px;
  color: #666;
  gap: 0;
  flex-shrink: 0;
}
.sep { margin: 0 8px; color: #333; }
.selected-count { color: #4F8EF7; }
.missing-warn { color: #e67e22; }
.spacer { flex: 1; }
.scanning { color: #4F8EF7; animation: flash 1s infinite; }
.workspace-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 300px; }
@keyframes flash { 0%,100%{opacity:1} 50%{opacity:0.5} }
</style>
