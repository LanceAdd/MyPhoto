<template>
  <div v-if="visible && tab" class="warmup-popup" :class="{ minimized }">
    <div class="popup-header">
      <div class="title-wrap">
        <strong>预热缩略图</strong>
        <span class="state">{{ stateLabel }}</span>
      </div>
      <div class="actions">
        <button v-if="minimized" class="icon-btn" @click="expand">展开</button>
        <button v-else class="icon-btn" @click="minimize">缩小</button>
        <button class="icon-btn" @click="closePopup">关闭</button>
      </div>
    </div>

    <div class="progress-row">
      <div class="row-head">
        <span class="row-title">缩略图总进度（网格+选片）</span>
        <span class="numbers">{{ done }} / {{ total }}</span>
      </div>
      <div class="bar">
        <div class="fill" :style="{ width: percent + '%' }" />
      </div>
    </div>

    <div v-if="!minimized" class="content">
      <div class="current" :title="currentFile">{{ currentFile || '缩略图预热空闲中' }}</div>
      <p>系统已根据设备性能自动计算首批预热数量：{{ initialLimit }} 张。</p>
      <p>当前使用 {{ workerConcurrency }} 条线程自动预热，预热已自动开始。</p>
      <p>当前进度条为统一进度：每张图会在一次处理中同时生成网格与选片缓存。</p>
      <p>预热会提前生成缩略图与预览缓存，首次加载稍慢，完成后浏览和切换会更快。</p>
      <p>你可以关闭弹窗，预热仍会在后台继续运行。</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import { readWarmupSettings } from '../utils/warmup-settings'

const store = useWorkspaceStore()
const initialLimit = ref(0)
const workerConcurrency = ref(0)

const tab = computed(() => {
  const target = store.warmupPopupWorkspaceId ?? store.activeTab?.workspace.id ?? null
  if (target == null) return null
  return store.tabs.find(t => t.workspace.id === target) ?? null
})

const visible = computed(() => {
  if (!store.warmupPopupVisible) return false
  return !!tab.value
})

watch(
  () => visible.value,
  nowVisible => {
    if (!nowVisible) return
    const settings = readWarmupSettings()
    initialLimit.value = settings.initialLimit
    workerConcurrency.value = settings.workerConcurrency
  },
  { immediate: true },
)

const minimized = computed(() => store.warmupPopupMinimized)
const running = computed(() => !!tab.value?.warmupRunning)
const done = computed(() => Math.max(0, tab.value?.warmupDone ?? 0))
const total = computed(() => {
  const raw = tab.value?.warmupTotal ?? 0
  if (raw > 0) return raw
  return Math.max(done.value, 1)
})
const percent = computed(() => {
  if (total.value <= 0) return 0
  return Math.min(100, Math.round((done.value / total.value) * 100))
})
const currentFile = computed(() => tab.value?.warmupCurrent ?? '')
const stateLabel = computed(() => {
  if (running.value) return '进行中'
  if (done.value < total.value) return '已暂停'
  return '已完成'
})

function expand() {
  store.setWarmupPopupMinimized(false)
}

function minimize() {
  store.setWarmupPopupMinimized(true)
}

function closePopup() {
  store.hideWarmupPopup()
}
</script>

<style scoped>
.warmup-popup {
  position: fixed;
  right: 16px;
  bottom: 44px;
  z-index: 1100;
  width: min(430px, calc(100vw - 32px));
  border: 1px solid #3a3a3a;
  border-radius: 10px;
  background: rgba(28, 28, 28, 0.96);
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
  color: #e6e6e6;
}

.warmup-popup.minimized {
  width: min(320px, calc(100vw - 32px));
}

.popup-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
}

.title-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}

.title-wrap strong {
  font-size: 13px;
}

.state {
  font-size: 11px;
  color: #9fb6dd;
}

.actions {
  display: flex;
  gap: 6px;
}

.icon-btn {
  border: 1px solid #3e3e3e;
  background: #2a2a2a;
  color: #d4d4d4;
  border-radius: 6px;
  padding: 4px 8px;
  font-size: 12px;
  cursor: pointer;
}

.icon-btn:hover {
  border-color: #4f8ef7;
  color: #fff;
}

.progress-row {
  padding: 0 12px 12px;
}

.row-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
}

.row-title {
  font-size: 12px;
  color: #d4dde9;
}

.numbers {
  font-size: 12px;
  color: #b7c3d9;
  margin-bottom: 6px;
}

.bar {
  width: 100%;
  height: 8px;
  border-radius: 999px;
  background: #2f3642;
  overflow: hidden;
}

.fill {
  height: 100%;
  background: linear-gradient(90deg, #4f8ef7, #79b1ff);
  transition: width 140ms ease-out;
}

.content {
  border-top: 1px solid #303030;
  padding: 10px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.current {
  font-size: 12px;
  color: #b9c7df;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.content p {
  margin: 0;
  font-size: 12px;
  line-height: 1.45;
  color: #a7a7a7;
}
</style>
