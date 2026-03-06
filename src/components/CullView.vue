<template>
  <div class="cull-view" @keydown="handleKey" tabindex="0" ref="cullRef">
    <!-- Top toolbar -->
    <div class="cull-toolbar">
      <button class="back-btn" @click="store.setViewMode('grid')" title="返回网格模式 (Tab)">
        ← 返回网格
      </button>
      <span class="cull-info">{{ currentIndex + 1 }} / {{ photos.length }} 张</span>
      <div style="flex:1" />
    </div>

    <!-- Main area: big preview -->
    <div class="cull-main">
      <div class="cull-preview" v-if="currentPhoto">
        <!-- Navigation arrows -->
        <button class="nav-arrow left" @click="navigate(-1)" :disabled="currentIndex <= 0">‹</button>
        <button class="nav-arrow right" @click="navigate(1)" :disabled="currentIndex >= photos.length - 1">›</button>

        <!-- Big image -->
        <div class="preview-img-wrap">
          <img
            v-if="currentPhoto && !currentPhoto.is_missing && previewSrc"
            :src="previewSrc"
            class="preview-img"
          />
          <div v-else-if="currentPhoto?.is_missing" class="preview-missing">🚫 文件已丢失</div>
          <div v-else class="preview-loading">
            <div class="loading-spin">⟳</div>
          </div>
        </div>

        <!-- Info bar -->
        <div class="preview-info" v-if="currentPhoto">
          <span class="info-name">{{ currentPhoto.filename }}</span>
          <span class="info-sep">|</span>
          <span class="info-date">{{ formatDate(currentPhoto.taken_at) }}</span>
          <span class="info-sep">|</span>
          <!-- Inline star setter -->
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
          <!-- Inline color setter -->
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
            >×</span>
          </div>
        </div>
      </div>

      <div class="cull-empty" v-else>
        <p>没有照片</p>
      </div>
    </div>

    <!-- Rail (horizontal thumbnail strip) -->
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
        <!-- Star indicator -->
        <div v-if="photo.star_rating > 0" class="rail-star">{{ '★'.repeat(photo.star_rating) }}</div>
        <!-- Color indicator -->
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
import { ref, computed, watch, onMounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useWorkspaceStore } from '../stores/workspace'
import RailThumb from './RailThumb.vue'

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

async function loadPreview() {
  const photo = currentPhoto.value
  if (!photo || photo.is_missing) { previewSrc.value = null; return }
  const fullPath = `${tab.value?.workspace.path}/${photo.relative_path}`
  try {
    const b64: string = await invoke('get_thumbnail', { photoPath: fullPath, size: 1600 })
    previewSrc.value = `data:image/jpeg;base64,${b64}`
  } catch { previewSrc.value = null }
}

watch(currentIndex, loadPreview, { immediate: true })
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
  else if (['1','2','3','4','5'].includes(e.key)) setStar(parseInt(e.key))
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

onMounted(() => { cullRef.value?.focus(); loadPreview() })
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
  background: none; border: 1px solid #333; color: #888;
  padding: 3px 10px; border-radius: 4px; cursor: pointer; font-size: 12px;
}
.back-btn:hover { border-color: #4F8EF7; color: #4F8EF7; }
.cull-info { font-size: 13px; color: #666; }

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
  background: rgba(0,0,0,0.5);
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
.nav-arrow:hover { background: rgba(0,0,0,0.8); }
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
}
.preview-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}
.preview-missing {
  font-size: 48px;
  color: #555;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}
.preview-loading {
  font-size: 48px;
  color: #333;
  animation: spin 1s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.preview-info {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px;
  background: rgba(0,0,0,0.6);
  border-radius: 6px;
  margin-bottom: 6px;
  font-size: 13px;
  color: #aaa;
}
.info-name { max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #ddd; }
.info-sep { color: #444; }
.inline-stars { display: flex; gap: 3px; }
.inline-star { font-size: 16px; cursor: pointer; color: #555; transition: color 0.1s; }
.inline-star:hover, .inline-star.filled { color: #f39c12; }
.inline-colors { display: flex; gap: 4px; align-items: center; }
.inline-color { width: 14px; height: 14px; border-radius: 50%; cursor: pointer; border: 2px solid transparent; transition: border-color 0.1s; }
.inline-color.active { border-color: #fff; }
.inline-color:hover { border-color: rgba(255,255,255,0.6); }
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
  position: absolute; bottom: 2px; right: 2px;
  font-size: 8px; color: #f39c12; letter-spacing: -1px;
  text-shadow: 0 1px 2px #000;
}
.rail-color {
  position: absolute; top: 3px; left: 3px;
  width: 8px; height: 8px; border-radius: 50%;
  border: 1px solid rgba(0,0,0,0.4);
}
.cull-empty {
  display: flex; align-items: center; justify-content: center;
  height: 100%; color: #555;
}
</style>
