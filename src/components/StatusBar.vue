<template>
  <div class="status-bar">
    <span>Total {{ tab?.photos.length ?? 0 }}</span>
    <template v-if="hasFilter">
      <span class="sep">|</span>
      <span>Filtered {{ filteredCount }}</span>
    </template>
    <template v-if="selectedCount > 0">
      <span class="sep">|</span>
      <span class="selected-count">Selected {{ selectedCount }}</span>
    </template>
    <template v-if="missingCount > 0">
      <span class="sep">|</span>
      <span class="missing-warn">Missing {{ missingCount }}</span>
    </template>
    <div class="spacer" />

    <div v-if="activeTask" class="task-progress" :class="{ error: activeTask.kind === 'error' }">
      <span class="task-label">{{ activeTask.label }}</span>
      <span v-if="activeTask.content" class="task-content" :title="activeTask.content">{{ activeTask.content }}</span>
      <div v-if="activeTask.showBar" class="progress-track">
        <div class="progress-fill" :class="{ indeterminate: activeTask.indeterminate }" :style="fillStyle(activeTask.percent)" />
      </div>
      <span class="task-meta">{{ activeTask.meta }}</span>
      <button v-if="activeTask.kind === 'warmup'" class="task-link" @click="openWarmupPopup">详情</button>
    </div>
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
  return !!(f && (f.star_min || f.star_none || f.color_labels?.length || f.color_none || f.subfolder))
})
const filteredCount = computed(() => tab.value?.photos.length ?? 0)
const missingCount = computed(() => tab.value?.photos.filter(p => p.is_missing).length ?? 0)

function openWarmupPopup() {
  store.showWarmupPopup(store.activeTab?.workspace.id, false)
}

function scanPhaseLabel(phase?: string) {
  switch (phase) {
    case 'scan_files': return '扫描文件'
    case 'write_database': return '写入索引'
    case 'mark_missing': return '标记缺失'
    case 'done': return '扫描完成'
    default: return '后台任务'
  }
}

function fillStyle(percent: number | null) {
  if (percent == null) return { width: '40%' }
  return { width: `${Math.max(4, Math.min(100, percent))}%` }
}

function trimMiddle(text: string, max = 44) {
  if (text.length <= max) return text
  const head = Math.ceil((max - 1) / 2)
  const tail = Math.floor((max - 1) / 2)
  return `${text.slice(0, head)}…${text.slice(text.length - tail)}`
}

function normalizeTaskContent(raw?: string | null) {
  if (!raw) return null
  return trimMiddle(raw.replace(/\\/g, '/'))
}

const activeTask = computed(() => {
  const current = tab.value
  if (!current) return null

  if (current.scanError) {
    return {
      kind: 'error' as const,
      label: '扫描失败',
      content: normalizeTaskContent(current.scanCurrent) ?? null,
      showBar: false,
      indeterminate: false,
      percent: null as number | null,
      meta: current.scanError,
    }
  }

  if (current.scanning) {
    const total = current.scanTotal ?? 0
    const done = Math.max(0, current.scanDone)
    const hasTotal = total > 0
    const percent = hasTotal ? Math.round((Math.min(done, total) / total) * 100) : null
    return {
      kind: 'scan' as const,
      label: scanPhaseLabel(current.scanPhase),
      content: normalizeTaskContent(current.scanCurrent),
      showBar: true,
      indeterminate: !hasTotal,
      percent,
      meta: hasTotal ? `${Math.min(done, total)}/${total} (${percent ?? 0}%)` : `${done}`,
    }
  }

  if (current.warmupRunning) {
    const total = current.warmupTotal ?? 0
    const done = Math.max(0, current.warmupDone)
    const hasTotal = total > 0
    const percent = hasTotal ? Math.round((Math.min(done, total) / total) * 100) : null
    return {
      kind: 'warmup' as const,
      label: '预热缩略图',
      content: normalizeTaskContent(current.warmupCurrent),
      showBar: true,
      indeterminate: !hasTotal,
      percent,
      meta: hasTotal ? `${Math.min(done, total)}/${total} (${percent ?? 0}%)` : `${done}`,
    }
  }

  return null
})
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
.workspace-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 360px;
}
.task-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 280px;
  max-width: 620px;
}
.task-label { color: #8da2cc; white-space: nowrap; }
.task-content {
  color: #a5b3ca;
  min-width: 100px;
  max-width: 250px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.task-meta { color: #93a1b9; min-width: 46px; text-align: right; }
.task-link {
  border: 1px solid #3b4b64;
  background: #1d2b42;
  color: #9fc1ff;
  border-radius: 5px;
  padding: 2px 6px;
  font-size: 11px;
  cursor: pointer;
}
.task-link:hover { color: #fff; border-color: #5f8de0; }
.task-progress.error .task-label,
.task-progress.error .task-meta,
.task-progress.error .task-content { color: #e07a7a; }
.progress-track {
  width: 120px;
  height: 4px;
  border-radius: 999px;
  background: #2b3240;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, #4f8ef7, #7ab0ff);
  transition: width 160ms ease-out;
}
.progress-fill.indeterminate {
  width: 40%;
  animation: indeterminate-slide 1s linear infinite;
}
@keyframes indeterminate-slide {
  0% { transform: translateX(-120%); }
  100% { transform: translateX(260%); }
}
</style>
