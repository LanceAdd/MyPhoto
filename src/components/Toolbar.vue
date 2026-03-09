<template>
  <div class="toolbar">
    <div class="toolbar-left">
      <div class="view-toggle">
        <button
          :class="{ active: tab.viewMode === 'grid' }"
          @click="store.setViewMode('grid')"
          title="网格模式 (Tab)"
        >▦ 网格</button>
        <button
          :class="{ active: tab.viewMode === 'cull' }"
          @click="store.setViewMode('cull')"
          title="选片模式 (Tab)"
        >◼ 选片</button>
      </div>

      <div class="divider" />

      <n-select
        v-model:value="sortBy"
        :options="sortOptions"
        size="small"
        style="width: 120px"
      />
      <button class="icon-btn" @click="toggleSortDir" :title="sortDesc ? '降序' : '升序'">
        {{ sortDesc ? '↓' : '↑' }}
      </button>

      <div class="divider" />

      <div class="filter-group filter-section">
        <span class="filter-label">星级筛选</span>
        <button
          class="star-filter-btn"
          :class="{ active: starFilter === 0 }"
          @click="setStarFilter(0)"
          title="全部"
        >全部</button>
        <button
          class="star-filter-btn"
          :class="{ active: starFilter === 'none' }"
          @click="setStarFilter('none')"
          title="无星级"
        >无星</button>
        <button
          v-for="n in [1, 2, 3, 4, 5]"
          :key="n"
          class="star-filter-btn"
          :class="{ active: starFilter === n }"
          @click="setStarFilter(n as 1 | 2 | 3 | 4 | 5)"
          :title="`${n} 星`"
        >{{ '★'.repeat(n) }}</button>
        <button
          class="icon-btn"
          @click="clearStarFilter"
          title="清除星级筛选"
          v-if="starFilter !== 0"
        >×</button>
      </div>

      <div class="divider divider-filter" />

      <div class="filter-group filter-section">
        <span class="filter-label">颜色筛选</span>
        <button
          class="none-filter-btn"
          :class="{ active: colorNone }"
          @click="toggleNoColorFilter"
          title="无颜色"
        >无色</button>
        <button
          v-for="color in colorOptions"
          :key="color.value"
          class="color-filter-btn"
          :class="{ active: activeColors.includes(color.value) }"
          :style="{ '--color': color.hex }"
          @click="toggleColorFilter(color.value)"
          :title="color.label"
        />
        <button
          class="icon-btn"
          @click="clearColorFilter"
          title="清除颜色筛选"
          v-if="colorNone || activeColors.length"
        >×</button>
      </div>
    </div>

    <div class="toolbar-right">
      <div class="view-toggle" v-if="tab.viewMode === 'grid'">
        <button
          :class="{ active: tab.gridLayout === 'fit' }"
          @click="setGridLayout('fit')"
          title="方格布局"
        >▦ 方格</button>
        <button
          :class="{ active: tab.gridLayout === 'flow' }"
          @click="setGridLayout('flow')"
          title="流式布局"
        >▤ 流式</button>
      </div>

      <div class="size-slider" v-if="tab.viewMode === 'grid'">
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
import { NSelect } from 'naive-ui'
import { useWorkspaceStore } from '../stores/workspace'

type StarFilterValue = 0 | 1 | 2 | 3 | 4 | 5 | 'none'

const THUMB_MIN = 80
const THUMB_MAX = 300
const THUMB_STEP = 10

const store = useWorkspaceStore()
const tab = computed(() => store.activeTab!)

const thumbnailSize = computed(() => tab.value?.thumbnailSize ?? THUMB_MIN)

const sortBy = computed({
  get: () => tab.value?.filter.sort_by ?? 'taken_at',
  set: (val: string) => store.setFilter({ sort_by: val }),
})

const sortDesc = computed(() => tab.value?.filter.sort_desc === true)
const starFilter = computed<StarFilterValue>(() => {
  if (tab.value?.filter.star_none === true) return 'none'
  const min = tab.value?.filter.star_min ?? 0
  return [0, 1, 2, 3, 4, 5].includes(min) ? (min as StarFilterValue) : 0
})
const activeColors = computed(() => tab.value?.filter.color_labels ?? [])
const colorNone = computed(() => tab.value?.filter.color_none === true)

const sortOptions = [
  { label: '拍摄时间', value: 'taken_at' },
  { label: '文件名', value: 'filename' },
  { label: '文件大小', value: 'file_size' },
  { label: '星级', value: 'star_rating' },
]

const colorOptions = [
  { value: 'red', label: '红色', hex: '#e74c3c' },
  { value: 'orange', label: '橙色', hex: '#e67e22' },
  { value: 'yellow', label: '黄色', hex: '#f1c40f' },
  { value: 'green', label: '绿色', hex: '#2ecc71' },
  { value: 'blue', label: '蓝色', hex: '#3498db' },
  { value: 'purple', label: '紫色', hex: '#9b59b6' },
]

function clampThumb(value: number) {
  return Math.max(THUMB_MIN, Math.min(THUMB_MAX, value))
}

function toggleSortDir() {
  store.setFilter({ sort_desc: !sortDesc.value })
}

function setStarFilter(val: StarFilterValue) {
  if (val === 'none') {
    store.setFilter({
      star_none: true,
      star_min: undefined,
    })
    return
  }
  store.setFilter({
    star_none: undefined,
    star_min: val > 0 ? val : undefined,
  })
}

function clearStarFilter() {
  setStarFilter(0)
}

function applyColorFilter(colors: string[], none: boolean) {
  store.setFilter({
    color_labels: colors.length ? [...colors] : undefined,
    color_none: none ? true : undefined,
  })
}

function toggleColorFilter(color: string) {
  const colors = [...activeColors.value]
  const idx = colors.indexOf(color)
  if (idx >= 0) colors.splice(idx, 1)
  else colors.push(color)
  applyColorFilter(colors, colorNone.value)
}

function toggleNoColorFilter() {
  applyColorFilter(activeColors.value, !colorNone.value)
}

function clearColorFilter() {
  applyColorFilter([], false)
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
.divider { width: 1px; height: 20px; background: #333; margin: 0 4px; }
.icon-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 3px 7px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.icon-btn:hover { border-color: #4F8EF7; color: #4F8EF7; }
.filter-group { display: flex; align-items: center; gap: 4px; }
.filter-section {
  padding: 2px 6px;
  border: 1px solid #2f2f2f;
  border-radius: 7px;
  background: #181818;
}
.filter-label {
  font-size: 11px;
  font-weight: 700;
  color: #b8c7dc;
  letter-spacing: 0.03em;
  background: #253a57;
  border: 1px solid #355a85;
  border-radius: 999px;
  padding: 1px 7px;
  margin-right: 2px;
  white-space: nowrap;
}
.divider-filter {
  margin: 0 10px;
  height: 24px;
  background: #3c3c3c;
}
.star-filter-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 2px 6px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  transition: all 0.15s;
}
.star-filter-btn.active { background: #f39c12; border-color: #f39c12; color: #1a1a1a; }
.star-filter-btn:hover:not(.active) { border-color: #f39c12; color: #f39c12; }
.none-filter-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 2px 6px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  transition: all 0.15s;
}
.none-filter-btn.active { background: #555; border-color: #888; color: #fff; }
.none-filter-btn:hover:not(.active) { border-color: #aaa; color: #ddd; }
.color-filter-btn {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--color);
  border: 2px solid transparent;
  cursor: pointer;
  transition: border-color 0.15s;
}
.color-filter-btn.active { border-color: #fff; }
.color-filter-btn:hover { border-color: rgba(255,255,255,0.6); }
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
