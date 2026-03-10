<template>
  <div class="toolbar">
    <div class="toolbar-left">
      <div class="view-toggle">
        <button
          :class="{ active: tab.viewMode === 'grid' }"
          @click="store.setViewMode('grid')"
          title="网格模式 (Tab)"
        >■ 网格</button>
        <button
          :class="{ active: tab.viewMode === 'cull' }"
          @click="store.setViewMode('cull')"
          title="选片模式 (Tab)"
        >▤ 选片</button>
      </div>
    </div>

    <div class="toolbar-right" v-if="tab.viewMode === 'grid'">
      <div class="view-toggle">
        <button
          :class="{ active: tab.gridLayout === 'fit' }"
          @click="setGridLayout('fit')"
          title="方格布局"
        >■ 方格</button>
        <button
          :class="{ active: tab.gridLayout === 'flow' }"
          @click="setGridLayout('flow')"
          title="流式布局"
        >▦ 流式</button>
      </div>

      <div class="size-slider">
        <button
          class="size-step-btn"
          :disabled="thumbnailSize <= THUMB_MIN"
          @click="changeThumbnailSize(-THUMB_STEP)"
          title="缩小网格"
        >-</button>
        <input
          type="range"
          :min="THUMB_MIN"
          :max="THUMB_MAX"
          :step="THUMB_STEP"
          :value="thumbnailSize"
          @input="onThumbnailSizeChange($event)"
          style="width: 90px"
        />
        <button
          class="size-step-btn"
          :disabled="thumbnailSize >= THUMB_MAX"
          @click="changeThumbnailSize(THUMB_STEP)"
          title="放大网格"
        >+</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'

const THUMB_MIN = 80
const THUMB_MAX = 300
const THUMB_STEP = 10

const store = useWorkspaceStore()
const tab = computed(() => store.activeTab!)

const thumbnailSize = computed(() => tab.value?.thumbnailSize ?? THUMB_MIN)

function clampThumb(value: number) {
  return Math.max(THUMB_MIN, Math.min(THUMB_MAX, value))
}

function setGridLayout(layout: 'fit' | 'flow') {
  const t = tab.value
  if (!t) return
  t.gridLayout = layout
}

function onThumbnailSizeChange(e: Event) {
  const t = tab.value
  if (!t) return
  const raw = parseInt((e.target as HTMLInputElement).value, 10)
  t.thumbnailSize = clampThumb(raw)
}

function changeThumbnailSize(delta: number) {
  const t = tab.value
  if (!t) return
  t.thumbnailSize = clampThumb(t.thumbnailSize + delta)
}
</script>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  background: #1e1e1e;
  border-bottom: 1px solid #2a2a2a;
  padding: 0 12px;
  gap: 8px;
  flex-shrink: 0;
}
.toolbar-left, .toolbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
}
.view-toggle {
  display: flex;
  background: #111;
  border-radius: 5px;
  overflow: hidden;
  border: 1px solid #333;
}
.view-toggle button {
  background: none;
  border: none;
  color: #888;
  padding: 4px 10px;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.view-toggle button.active { background: #4F8EF7; color: #fff; }
.view-toggle button:hover:not(.active) { background: #2a2a2a; color: #ccc; }
.size-slider {
  display: flex;
  align-items: center;
  gap: 5px;
  color: #666;
}
.size-step-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  width: 22px;
  height: 22px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  line-height: 1;
}
.size-step-btn:hover:not(:disabled) {
  border-color: #4F8EF7;
  color: #4F8EF7;
}
.size-step-btn:disabled {
  opacity: 0.35;
  cursor: default;
}
input[type=range] { accent-color: #4F8EF7; }
</style>
