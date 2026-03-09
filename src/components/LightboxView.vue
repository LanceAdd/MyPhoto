<template>
  <Teleport to="body">
    <div class="lightbox-overlay" @click.self="$emit('close')" @keydown="handleKey" tabindex="0" ref="boxRef">
      <button class="lb-nav left" @click="navigate(-1)" :disabled="currentIndex <= 0">‹</button>
      <button class="lb-nav right" @click="navigate(1)" :disabled="currentIndex >= allPhotos.length - 1">›</button>

      <button class="lb-close" @click="$emit('close')">×</button>
      <div class="lb-tools">
        <button class="lb-tool-btn" @click="zoomBy(1 / 1.2)" title="缩小">-</button>
        <button class="lb-tool-btn" @click="zoomBy(1.2)" title="放大">+</button>
        <button class="lb-tool-btn" @click="rotateBy(-90)" title="向左旋转">⟲</button>
        <button class="lb-tool-btn" @click="rotateBy(90)" title="向右旋转">⟳</button>
        <button class="lb-tool-btn" @click="resetTransform" title="重置">1:1</button>
        <span class="lb-tool-meta">{{ Math.round(scale * 100) }}% / {{ normalizedRotation }}°</span>
      </div>

      <div
        class="lb-img-wrap"
        @wheel.prevent="onWheel"
        @mousedown="startDrag"
        @mousemove="onDrag"
        @mouseup="stopDrag"
        @mouseleave="stopDrag"
      >
        <img
          v-if="imgSrc && !currentPhoto?.is_missing"
          :src="imgSrc"
          class="lb-img"
          :style="imgTransformStyle"
          draggable="false"
        />
        <div v-else-if="currentPhoto?.is_missing" class="lb-missing">文件已丢失</div>
        <div v-else class="lb-loading">
          <div class="spin">⟳</div>
        </div>
      </div>

      <div class="lb-hud" v-if="currentPhoto">
        <div class="hud-info">
          <span class="hud-name">{{ currentPhoto.filename }}</span>
          <span v-if="currentPhoto.taken_at" class="hud-date">{{ formatDate(currentPhoto.taken_at) }}</span>
          <span v-if="currentPhoto.camera_model" class="hud-camera">{{ currentPhoto.camera_model }}</span>
          <span v-if="currentPhoto.shutter_speed" class="hud-exif">{{ currentPhoto.shutter_speed }}s</span>
          <span v-if="currentPhoto.aperture" class="hud-exif">f/{{ currentPhoto.aperture }}</span>
          <span v-if="currentPhoto.iso" class="hud-exif">ISO{{ currentPhoto.iso }}</span>
          <span v-if="currentPhoto.focal_length" class="hud-exif">{{ currentPhoto.focal_length }}mm</span>
        </div>

        <div class="hud-marks">
          <div class="hud-stars">
            <span
              v-for="n in 5" :key="n"
              class="hud-star"
              :class="{ filled: n <= currentPhoto.star_rating }"
              @click.stop="setStar(n)"
            >★</span>
          </div>
          <div class="hud-colors">
            <span
              v-for="c in colorOptions" :key="c.value"
              class="hud-color"
              :style="{ background: c.hex }"
              :class="{ active: currentPhoto.color_label === c.value }"
              @click.stop="setColor(c.value)"
              :title="c.label"
            />
            <span v-if="currentPhoto.color_label" class="hud-clear" @click.stop="setColor('')">×</span>
          </div>
        </div>
      </div>

      <div class="lb-counter">{{ currentIndex + 1 }} / {{ allPhotos.length }}</div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useWorkspaceStore, type Photo } from '../stores/workspace'
import { toTauriImageSrc } from '../utils/image-src'

const props = defineProps<{ photo: Photo }>()
const emit = defineEmits<{ close: [] }>()

const store = useWorkspaceStore()
const allPhotos = computed(() => store.activeTab?.photos ?? [])
const currentIndex = ref(Math.max(0, allPhotos.value.findIndex(p => p.id === props.photo.id)))
const currentPhoto = computed(() => allPhotos.value[currentIndex.value] ?? null)

const boxRef = ref<HTMLElement>()
const imgSrc = ref<string | null>(null)
const loadSeq = ref(0)
const scale = ref(1)
const rotation = ref(0)
const translateX = ref(0)
const translateY = ref(0)

const PREVIEW_SIZE = 1600
const PREVIEW_PROFILE = 'preview'
const PREVIEW_QUALITY = 82
const LEGACY_LIGHTBOX_SIZE = 2400
const useLightboxV2 = readLightboxV2Flag()

const normalizedRotation = computed(() => {
  const v = rotation.value % 360
  return v >= 0 ? v : v + 360
})

const imgTransformStyle = computed(() => ({
  transform: `translate(${translateX.value}px, ${translateY.value}px) scale(${scale.value}) rotate(${rotation.value}deg)`,
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

function onWheel(e: WheelEvent) {
  const delta = e.deltaY > 0 ? (1 / 1.15) : 1.15
  zoomBy(delta)
}

function resetTransform() {
  scale.value = 1
  rotation.value = 0
  translateX.value = 0
  translateY.value = 0
}

const colorOptions = [
  { value: 'red', label: '红', hex: '#e74c3c' },
  { value: 'orange', label: '橙', hex: '#e67e22' },
  { value: 'yellow', label: '黄', hex: '#f1c40f' },
  { value: 'green', label: '绿', hex: '#2ecc71' },
  { value: 'blue', label: '蓝', hex: '#3498db' },
  { value: 'purple', label: '紫', hex: '#9b59b6' },
]

function formatDate(d: string | null) {
  return d?.replace('T', ' ').slice(0, 16) ?? '-'
}

function readLightboxV2Flag() {
  const raw = localStorage.getItem('feature.lightbox_streaming_v2')
  if (raw == null) return true
  return raw !== 'false'
}

function fullPathOf(photo: Photo): string | null {
  const root = store.activeTab?.workspace.path
  if (!root) return null
  return `${root}/${photo.relative_path}`
}

function preloadImage(src: string) {
  return new Promise<boolean>((resolve) => {
    const img = new Image()
    img.onload = () => resolve(true)
    img.onerror = () => resolve(false)
    img.src = src
  })
}

async function loadLegacy(fullPath: string, seq: number) {
  try {
    const b64: string = await invoke('get_thumbnail', { photoPath: fullPath, size: LEGACY_LIGHTBOX_SIZE })
    if (seq === loadSeq.value) {
      imgSrc.value = `data:image/jpeg;base64,${b64}`
    }
  } catch {
    if (seq === loadSeq.value) {
      imgSrc.value = null
    }
  }
}

function prefetchNeighbors(center: number) {
  const offsets = [-2, -1, 1, 2]
  for (const offset of offsets) {
    const idx = center + offset
    if (idx < 0 || idx >= allPhotos.value.length) continue
    const photo = allPhotos.value[idx]
    if (!photo || photo.is_missing) continue
    const fullPath = fullPathOf(photo)
    if (!fullPath) continue

    void invoke('ensure_preview_cache', {
      photoPath: fullPath,
      size: PREVIEW_SIZE,
      profile: PREVIEW_PROFILE,
      quality: PREVIEW_QUALITY,
    }).catch(() => {})

    if (Math.abs(offset) === 1) {
      void preloadImage(toTauriImageSrc(fullPath))
    }
  }
}

async function loadImage() {
  const seq = ++loadSeq.value
  const photo = currentPhoto.value
  if (!photo || photo.is_missing) {
    imgSrc.value = null
    return
  }
  resetTransform()
  imgSrc.value = null

  const fullPath = fullPathOf(photo)
  if (!fullPath) return

  if (!useLightboxV2) {
    await loadLegacy(fullPath, seq)
    return
  }

  let previewShown = false
  try {
    const previewPath: string = await invoke('ensure_preview_cache', {
      photoPath: fullPath,
      size: PREVIEW_SIZE,
      profile: PREVIEW_PROFILE,
      quality: PREVIEW_QUALITY,
    })
    if (seq !== loadSeq.value) return
    const previewSrc = toTauriImageSrc(previewPath)
    const previewReady = await preloadImage(previewSrc)
    if (seq !== loadSeq.value) return
    if (previewReady) {
      imgSrc.value = previewSrc
      previewShown = true
    }
  } catch {
    // keep fallback path below
  }

  const originalSrc = toTauriImageSrc(fullPath)
  const originalReady = await preloadImage(originalSrc)
  if (seq !== loadSeq.value) return
  if (originalReady) {
    imgSrc.value = originalSrc
    return
  }

  if (!previewShown) {
    await loadLegacy(fullPath, seq)
  }
}

watch(currentIndex, (idx) => {
  void loadImage()
  prefetchNeighbors(idx)
}, { immediate: true })

function navigate(delta: number) {
  const next = currentIndex.value + delta
  if (next >= 0 && next < allPhotos.value.length) {
    currentIndex.value = next
  }
}

function handleKey(e: KeyboardEvent) {
  if (e.key === 'ArrowLeft') { e.preventDefault(); navigate(-1) }
  else if (e.key === 'ArrowRight') { e.preventDefault(); navigate(1) }
  else if (e.key === 'Escape') emit('close')
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
}

async function setStar(n: number) {
  const p = currentPhoto.value
  if (!p) return
  const newRating = p.star_rating === n ? 0 : n
  await store.updateMeta(p.id, newRating, p.color_label, p.notes)
}

async function setColor(c: string) {
  const p = currentPhoto.value
  if (!p) return
  await store.updateMeta(p.id, p.star_rating, c, p.notes)
}

onMounted(() => {
  nextTick(() => boxRef.value?.focus())
})

onUnmounted(() => {
  loadSeq.value += 1
})
</script>

<style scoped>
.lightbox-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(0, 0, 0, 0.95);
  display: flex;
  align-items: center;
  justify-content: center;
  outline: none;
}
.lb-nav {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  background: rgba(255, 255, 255, 0.1);
  border: none;
  color: #fff;
  font-size: 48px;
  width: 60px;
  height: 80px;
  cursor: pointer;
  z-index: 10;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s;
}
.lb-nav:hover { background: rgba(255, 255, 255, 0.2); }
.lb-nav:disabled { opacity: 0.2; cursor: default; }
.lb-nav.left { left: 12px; }
.lb-nav.right { right: 12px; }

.lb-close {
  position: absolute;
  top: 12px;
  right: 16px;
  background: rgba(255, 255, 255, 0.1);
  border: none;
  color: #fff;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  font-size: 20px;
  cursor: pointer;
  z-index: 11;
  display: flex;
  align-items: center;
  justify-content: center;
}
.lb-close:hover { background: rgba(255, 255, 255, 0.25); }

.lb-tools {
  position: absolute;
  top: 12px;
  right: 60px;
  z-index: 11;
  display: flex;
  align-items: center;
  gap: 6px;
}
.lb-tool-btn {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.22);
  color: #eee;
  height: 30px;
  min-width: 30px;
  padding: 0 8px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}
.lb-tool-btn:hover {
  border-color: #4F8EF7;
  color: #4F8EF7;
}
.lb-tool-meta {
  font-size: 12px;
  color: #8ea3c3;
  min-width: 84px;
  text-align: right;
}

.lb-img-wrap {
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  position: relative;
}
.lb-img {
  max-width: 100vw;
  max-height: 90vh;
  object-fit: contain;
  transition: transform 0.05s;
  user-select: none;
}
.lb-missing, .lb-loading {
  color: #555;
  font-size: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.spin { animation: spin 1s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

.lb-hud {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.85));
  padding: 20px 60px 12px;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
}
.hud-info { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.hud-name { font-size: 14px; color: #ddd; font-weight: 500; }
.hud-date, .hud-camera { font-size: 12px; color: #aaa; }
.hud-exif { font-size: 12px; color: #888; }
.hud-marks { display: flex; align-items: center; gap: 12px; }
.hud-stars { display: flex; gap: 4px; }
.hud-star { font-size: 20px; cursor: pointer; color: #555; transition: color 0.1s; }
.hud-star:hover, .hud-star.filled { color: #f39c12; }
.hud-colors { display: flex; gap: 5px; align-items: center; }
.hud-color { width: 18px; height: 18px; border-radius: 50%; cursor: pointer; border: 2px solid transparent; }
.hud-color.active { border-color: #fff; }
.hud-color:hover { border-color: rgba(255, 255, 255, 0.6); }
.hud-clear { color: #888; cursor: pointer; font-size: 16px; }
.hud-clear:hover { color: #ddd; }

.lb-counter {
  position: absolute;
  top: 14px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 13px;
  color: #888;
  background: rgba(0, 0, 0, 0.5);
  padding: 4px 12px;
  border-radius: 12px;
}
</style>
