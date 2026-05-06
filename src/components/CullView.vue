<template>
  <div class="cull-view" @keydown="handleKey" tabindex="0" ref="cullRef">
    <div class="cull-toolbar">
      <button class="back-btn" @click="store.setViewMode('grid')" title="返回网格模式 (Tab)">
        返回网格
      </button>
      <button class="back-btn" @click="restartFromBeginning" :disabled="photos.length === 0" title="从第一张重新选片">
        重新开始
      </button>
      <span class="cull-info">{{ currentIndex + 1 }} / {{ photos.length }}</span>
      <div style="flex: 1" />
      <div class="preview-tools" title="预览缩放和旋转">
        <button class="tool-btn" @click="zoomBy(1 / 1.2)" title="缩小">-</button>
        <button class="tool-btn" @click="zoomBy(1.2)" title="放大">+</button>
        <button class="tool-btn" @click="rotateBy(-90)" title="向左旋转">⟲</button>
        <button class="tool-btn" @click="rotateBy(90)" title="向右旋转">⟳</button>
        <button class="tool-btn" @click="resetTransform" title="重置">1:1</button>
        <span class="tool-meta">{{ Math.round(scale * 100) }}% / {{ normalizedRotation }}°</span>
      </div>
    </div>

    <div class="cull-main">
      <div class="cull-preview" v-if="currentPhoto">
        <button class="nav-arrow left" @click="navigate(-1)" :disabled="currentIndex <= 0">&lt;</button>
        <button class="nav-arrow right" @click="navigate(1)" :disabled="currentIndex >= photos.length - 1">&gt;</button>

        <div
          class="preview-img-wrap"
          @wheel.prevent="onPreviewWheel"
          @mousedown="startDrag"
          @mousemove="onDrag"
          @mouseup="stopDrag"
          @mouseleave="stopDrag"
        >
          <img
            v-if="currentPhoto && !currentPhoto.is_missing && previewSrc"
            :src="previewSrc"
            class="preview-img"
            :style="previewTransformStyle"
            draggable="false"
          />
          <div v-if="viewerDisplayState === 'transition'" class="preview-transition">
            <div class="loading-spin">加载中</div>
          </div>
          <div v-else-if="viewerDisplayState === 'missing'" class="preview-missing">文件已丢失</div>
          <div v-else-if="viewerDisplayState === 'loading'" class="preview-loading">
            <div class="loading-spin">加载中</div>
          </div>
        </div>

        <div class="preview-info" v-if="currentPhoto">
          <span class="info-name">{{ currentPhoto.filename }}</span>
          <span class="info-sep">|</span>
          <span class="info-date">{{ formatDate(currentPhoto.taken_at) }}</span>
          <span class="info-sep">|</span>
          <div class="inline-stars">
            <span
              v-for="n in 5"
              :key="n"
              class="inline-star"
              :class="{ filled: n <= currentPhoto.star_rating }"
              @click.stop="setStar(n)"
            >★</span>
          </div>
          <span class="info-sep">|</span>
          <div class="inline-colors">
            <span
              v-for="c in colorOptions"
              :key="c.value"
              class="inline-color"
              :style="{ background: c.hex }"
              :class="{ active: currentPhoto.color_label === c.value }"
              @click.stop="setColor(c.value)"
              :title="c.label"
            />
            <span
              v-if="currentPhoto.color_label"
              class="inline-clear"
              @click.stop="setColor('')"
              title="清除标签"
            >x</span>
          </div>
        </div>
      </div>

      <div class="cull-empty" v-else>
        <p>没有照片</p>
      </div>
    </div>

    <div class="cull-rail" ref="railRef" @wheel.prevent="onRailWheel">
      <div
        v-for="(photo, i) in photos"
        :key="photo.id"
        class="rail-item"
        :class="{ active: i === currentIndex }"
        :ref="el => { if (i === currentIndex && el) scrollRailToItem(el as HTMLElement) }"
        @click="goTo(i)"
      >
        <RailThumb :photo="photo" :workspace-path="tab?.workspace.path ?? ''" />
        <div v-if="photo.star_rating > 0" class="rail-star">{{ '★'.repeat(photo.star_rating) }}</div>
        <div
          v-if="photo.color_label"
          class="rail-color"
          :style="{ background: colorHex(photo.color_label) }"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import RailThumb from './RailThumb.vue'
import { sharedViewerImagePipeline } from '../utils/viewer-image-runtime'
import type { ViewerImageSnapshot } from '../utils/viewer-image-pipeline'
import { resolveViewerDisplayState } from '../utils/viewer-display-state'

const store = useWorkspaceStore()
const tab = computed(() => store.activeTab)
const photos = computed(() => tab.value?.photos ?? [])
const currentIndex = computed({
  get: () => tab.value?.cullIndex ?? 0,
  set: (v) => store.setCullIndex(v),
})
const currentPhoto = computed(() => photos.value[currentIndex.value] ?? null)

const cullRef = ref<HTMLElement>()
const railRef = ref<HTMLElement>()
const previewSrc = ref<string | null>(null)
const previewLoading = ref(false)
const displayedPhotoPath = ref<string | null>(null)
const scale = ref(1)
const rotation = ref(0)
const translateX = ref(0)
const translateY = ref(0)
let stopViewerSubscription: (() => void) | null = null

const normalizedRotation = computed(() => {
  const v = rotation.value % 360
  return v >= 0 ? v : v + 360
})

const previewTransformStyle = computed(() => ({
  transform: `translate(${translateX.value}px, ${translateY.value}px) scale(${scale.value}) rotate(${rotation.value}deg)`,
  transformOrigin: 'center center',
  cursor: scale.value > 1 ? 'grab' : 'default',
}))

let dragging = false
let dragStartX = 0
let dragStartY = 0
let dragOriginX = 0
let dragOriginY = 0

function startDrag(e: MouseEvent) {
  if (scale.value <= 1) return
  dragging = true
  dragStartX = e.clientX
  dragStartY = e.clientY
  dragOriginX = translateX.value
  dragOriginY = translateY.value
}

function onDrag(e: MouseEvent) {
  if (!dragging) return
  translateX.value = dragOriginX + (e.clientX - dragStartX)
  translateY.value = dragOriginY + (e.clientY - dragStartY)
}

function stopDrag() {
  dragging = false
}

const colorOptions = [
  { value: 'red', label: '红', hex: '#e74c3c' },
  { value: 'orange', label: '橙', hex: '#e67e22' },
  { value: 'yellow', label: '黄', hex: '#f1c40f' },
  { value: 'green', label: '绿', hex: '#2ecc71' },
  { value: 'blue', label: '蓝', hex: '#3498db' },
  { value: 'purple', label: '紫', hex: '#9b59b6' },
]

function colorHex(c: string) {
  return colorOptions.find(o => o.value === c)?.hex ?? '#888'
}

function formatDate(d: string | null) {
  if (!d) return '未知时间'
  return d.replace('T', ' ').slice(0, 16)
}

function fullPathOf(index: number) {
  const photo = photos.value[index]
  if (!photo) return null
  const root = tab.value?.workspace.path
  if (!root) return null
  return `${root}/${photo.relative_path}`
}

const currentFullPath = computed(() => fullPathOf(currentIndex.value))
const orderedPhotoPaths = computed(() =>
  photos.value
    .map((_, index) => fullPathOf(index))
    .filter((path): path is string => !!path)
)
const showTransitionOverlay = computed(() =>
  !!previewSrc.value
  && !!currentFullPath.value
  && displayedPhotoPath.value !== currentFullPath.value
  && !currentPhoto.value?.is_missing
)
const viewerDisplayState = computed(() => resolveViewerDisplayState({
  hasDisplaySrc: !!previewSrc.value,
  isMissing: !!currentPhoto.value?.is_missing,
  showTransitionOverlay: showTransitionOverlay.value,
}))

function disconnectViewer() {
  stopViewerSubscription?.()
  stopViewerSubscription = null
}

function applyViewerSnapshot(path: string, snapshot: ViewerImageSnapshot) {
  if (currentFullPath.value !== path) return
  previewLoading.value = snapshot.isLoading
  if (snapshot.displaySrc) {
    previewSrc.value = snapshot.displaySrc
    displayedPhotoPath.value = path
    return
  }
  if (displayedPhotoPath.value === path) {
    previewSrc.value = null
    displayedPhotoPath.value = null
  }
}

function connectViewer(path: string | null) {
  disconnectViewer()
  if (!path || currentPhoto.value?.is_missing) {
    previewSrc.value = null
    previewLoading.value = false
    displayedPhotoPath.value = path
    return
  }

  stopViewerSubscription = sharedViewerImagePipeline.subscribe(path, (snapshot) => {
    applyViewerSnapshot(path, snapshot)
  })
  sharedViewerImagePipeline.focus({
    activePath: path,
    orderedPaths: orderedPhotoPaths.value,
  })
}

function zoomBy(multiplier: number) {
  scale.value = Math.max(0.2, Math.min(10, scale.value * multiplier))
  if (scale.value <= 1) {
    translateX.value = 0
    translateY.value = 0
  }
}

function rotateBy(delta: number) {
  rotation.value += delta
}

function resetTransform() {
  scale.value = 1
  rotation.value = 0
  translateX.value = 0
  translateY.value = 0
}

function onPreviewWheel(e: WheelEvent) {
  const delta = e.deltaY > 0 ? (1 / 1.12) : 1.12
  zoomBy(delta)
}
watch([currentFullPath, orderedPhotoPaths], ([path]) => {
  resetTransform()
  connectViewer(path)
}, { immediate: true })
watch(scale, (nextScale) => {
  const path = currentFullPath.value
  if (!path || nextScale <= 1) return
  sharedViewerImagePipeline.setZoom(path, nextScale)
})
watch(() => photos.value.length, () => {
  if (currentIndex.value >= photos.value.length) {
    store.setCullIndex(Math.max(0, photos.value.length - 1))
  }
})

function navigate(delta: number) {
  const newIdx = currentIndex.value + delta
  if (newIdx >= 0 && newIdx < photos.value.length) {
    currentIndex.value = newIdx
  }
}

function goTo(i: number) {
  currentIndex.value = i
}

function restartFromBeginning() {
  if (photos.value.length === 0) return
  store.setCullIndex(0)
}

function setStar(n: number) {
  const photo = currentPhoto.value
  if (!photo) return
  const newRating = photo.star_rating === n ? 0 : n
  store.updateMeta(photo.id, newRating, photo.color_label, photo.notes)
}

function setColor(c: string) {
  const photo = currentPhoto.value
  if (!photo) return
  store.updateMeta(photo.id, photo.star_rating, c, photo.notes)
}

function handleKey(e: KeyboardEvent) {
  const tag = (e.target as HTMLElement).tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA') return

  if (e.key === 'ArrowLeft') { e.preventDefault(); navigate(-1) }
  else if (e.key === 'ArrowRight') { e.preventDefault(); navigate(1) }
  else if (e.key === 'Tab') { e.preventDefault(); store.setViewMode('grid') }
  else if (e.key === '+' || e.key === '=') { e.preventDefault(); zoomBy(1.2) }
  else if (e.key === '-') { e.preventDefault(); zoomBy(1 / 1.2) }
  else if (e.key.toLowerCase() === 'r') {
    e.preventDefault()
    rotateBy(e.shiftKey ? -90 : 90)
  }
  else if (e.key === '0' && e.ctrlKey) { e.preventDefault(); resetTransform() }
  else if (['1', '2', '3', '4', '5'].includes(e.key)) setStar(parseInt(e.key, 10))
  else if (e.key === '6') setColor('red')
  else if (e.key === '7') setColor('orange')
  else if (e.key === '8') setColor('yellow')
  else if (e.key === '9') setColor('green')
  else if (e.key === '0') { setStar(0); setColor('') }
}

function onRailWheel(e: WheelEvent) {
  if (railRef.value) railRef.value.scrollLeft += e.deltaY
}

function scrollRailToItem(el: HTMLElement) {
  nextTick(() => {
    el.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' })
  })
}

onMounted(() => {
  cullRef.value?.focus()
})

onUnmounted(() => {
  disconnectViewer()
})
</script>

<style scoped>
.cull-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #111;
  outline: none;
}
.cull-toolbar {
  display: flex;
  align-items: center;
  height: 36px;
  background: #1a1a1a;
  border-bottom: 1px solid #2a2a2a;
  padding: 0 12px;
  gap: 12px;
  flex-shrink: 0;
}
.back-btn {
  background: none;
  border: 1px solid #333;
  color: #888;
  padding: 3px 10px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
}
.back-btn:hover { border-color: #4F8EF7; color: #4F8EF7; }
.cull-info { font-size: 13px; color: #666; }
.preview-tools {
  display: flex;
  align-items: center;
  gap: 6px;
}
.tool-btn {
  background: none;
  border: 1px solid #333;
  color: #9a9a9a;
  padding: 2px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  min-width: 28px;
  height: 24px;
}
.tool-btn:hover { border-color: #4F8EF7; color: #4F8EF7; }
.tool-meta {
  font-size: 11px;
  color: #7789a3;
  min-width: 72px;
  text-align: right;
}

.cull-main {
  flex: 1;
  min-height: 0;
  position: relative;
}
.cull-preview {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  position: relative;
}
.nav-arrow {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  background: rgba(0, 0, 0, 0.5);
  border: 1px solid #444;
  color: #fff;
  width: 40px;
  height: 60px;
  font-size: 28px;
  cursor: pointer;
  z-index: 5;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s;
}
.nav-arrow:hover { background: rgba(0, 0, 0, 0.8); }
.nav-arrow:disabled { opacity: 0.2; cursor: default; }
.nav-arrow.left { left: 12px; }
.nav-arrow.right { right: 12px; }

.preview-img-wrap {
  flex: 1;
  width: 100%;
  min-height: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  position: relative;
}
.preview-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  transition: transform 0.08s;
  user-select: none;
}
.preview-transition {
  position: absolute;
  right: 16px;
  top: 16px;
  padding: 6px 10px;
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.5);
  color: #9aa8bc;
  font-size: 12px;
}
.preview-missing {
  font-size: 36px;
  color: #555;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}
.preview-loading {
  font-size: 48px;
  color: #333;
}
.loading-spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

.preview-info {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  background: rgba(0, 0, 0, 0.6);
  border-radius: 6px;
  margin-bottom: 6px;
  font-size: 13px;
  color: #aaa;
}
.info-name { max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #ddd; }
.info-sep { color: #444; }
.inline-stars { display: flex; gap: 3px; }
.inline-star {
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
  color: #555;
  transition: color 0.1s;
}
.inline-star:hover, .inline-star.filled { color: #f39c12; }
.inline-colors { display: flex; gap: 4px; align-items: center; }
.inline-color { width: 14px; height: 14px; border-radius: 50%; cursor: pointer; border: 2px solid transparent; transition: border-color 0.1s; }
.inline-color.active { border-color: #fff; }
.inline-color:hover { border-color: rgba(255, 255, 255, 0.6); }
.inline-clear { font-size: 14px; color: #666; cursor: pointer; }
.inline-clear:hover { color: #ddd; }

.cull-rail {
  height: 96px;
  background: #0e0e0e;
  border-top: 1px solid #2a2a2a;
  display: flex;
  align-items: center;
  overflow-x: auto;
  overflow-y: hidden;
  gap: 4px;
  padding: 4px 8px;
  flex-shrink: 0;
  scroll-behavior: smooth;
}
.cull-rail::-webkit-scrollbar { height: 4px; }
.cull-rail::-webkit-scrollbar-thumb { background: #333; border-radius: 2px; }

.rail-item {
  position: relative;
  flex-shrink: 0;
  width: 80px;
  height: 80px;
  border-radius: 3px;
  overflow: hidden;
  cursor: pointer;
  border: 2px solid transparent;
  transition: border-color 0.1s;
  background: #1a1a1a;
}
.rail-item:hover { border-color: #555; }
.rail-item.active { border-color: #fff; }
.rail-star {
  position: absolute;
  bottom: 2px;
  right: 2px;
  font-size: 11px;
  line-height: 1;
  color: #f39c12;
  letter-spacing: -0.5px;
  text-shadow: 0 1px 2px #000;
}
.rail-color {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.4);
}
.cull-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #555;
}
</style>
