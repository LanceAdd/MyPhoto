<template>
  <div class="meta-panel">
    <div class="meta-header">元数据</div>

    <template v-if="photo">
      <!-- Thumbnail preview -->
      <div class="meta-thumb">
        <img v-if="thumbSrc" :src="thumbSrc" class="preview-thumb" />
      </div>

      <!-- Basic info -->
      <div class="meta-section">
        <div class="meta-title">基本信息</div>
        <MetaRow label="文件名" :value="photo.filename" />
        <MetaRow label="尺寸" :value="photo.width && photo.height ? `${photo.width} × ${photo.height}` : '-'" />
        <MetaRow label="大小" :value="formatSize(photo.file_size)" />
        <MetaRow label="路径" :value="photo.relative_path" small />
      </div>

      <!-- EXIF -->
      <div class="meta-section" v-if="hasExif">
        <div class="meta-title">拍摄参数</div>
        <MetaRow v-if="photo.camera_make" label="相机品牌" :value="photo.camera_make" />
        <MetaRow v-if="photo.camera_model" label="相机型号" :value="photo.camera_model" />
        <MetaRow v-if="photo.lens_model" label="镜头" :value="photo.lens_model" />
        <MetaRow v-if="photo.taken_at" label="拍摄时间" :value="formatDate(photo.taken_at)" />
        <MetaRow v-if="photo.shutter_speed" label="快门" :value="photo.shutter_speed + 's'" />
        <MetaRow v-if="photo.aperture" label="光圈" :value="`f/${photo.aperture}`" />
        <MetaRow v-if="photo.iso" label="ISO" :value="String(photo.iso)" />
        <MetaRow v-if="photo.focal_length" label="焦距" :value="`${photo.focal_length}mm`" />
      </div>

      <!-- User marks -->
      <div class="meta-section">
        <div class="meta-title">标记</div>

        <!-- Star rating -->
        <div class="meta-row">
          <span class="meta-label">星级</span>
          <div class="star-edit">
            <span
              v-for="n in 5"
              :key="n"
              class="star-edit-item"
              :class="{ filled: n <= photo.star_rating }"
              @click="setStar(n)"
            >★</span>
            <span class="star-clear" @click="setStar(0)" v-if="photo.star_rating > 0" title="清除">×</span>
          </div>
        </div>

        <!-- Color label -->
        <div class="meta-row">
          <span class="meta-label">颜色</span>
          <div class="color-edit">
            <span
              v-for="c in colorOptions"
              :key="c.value"
              class="color-dot"
              :style="{ background: c.hex }"
              :class="{ active: photo.color_label === c.value }"
              @click="setColor(c.value)"
              :title="c.label"
            />
            <span class="color-clear" @click="setColor('')" v-if="photo.color_label" title="清除">×</span>
          </div>
        </div>

        <!-- Notes -->
        <div class="meta-row notes-row">
          <span class="meta-label">备注</span>
          <textarea
            class="notes-input"
            :value="photo.notes"
            @change="updateNotes(($event.target as HTMLTextAreaElement).value)"
            placeholder="添加备注..."
            rows="3"
          />
        </div>
      </div>
    </template>

    <div v-else class="meta-empty">
      <span>选择一张照片查看详情</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useWorkspaceStore } from '../stores/workspace'
import MetaRow from './MetaRow.vue'
import {
  getExactCachedThumb,
  getNearestCachedThumb,
  normalizeThumbSize,
  putCachedThumb,
} from '../utils/thumb-cache'
import { ensureGridThumbSrc } from '../utils/thumb-loader'

const store = useWorkspaceStore()
const tab = computed(() => store.activeTab)
const photo = computed(() => {
  if (!tab.value?.activePhotoId) return null
  return tab.value.photos.find(p => p.id === tab.value!.activePhotoId) ?? null
})

const thumbSrc = ref<string | null>(null)
const loadSeq = ref(0)

const colorOptions = [
  { value: 'red', label: '红', hex: '#e74c3c' },
  { value: 'orange', label: '橙', hex: '#e67e22' },
  { value: 'yellow', label: '黄', hex: '#f1c40f' },
  { value: 'green', label: '绿', hex: '#2ecc71' },
  { value: 'blue', label: '蓝', hex: '#3498db' },
  { value: 'purple', label: '紫', hex: '#9b59b6' },
]

const hasExif = computed(() =>
  photo.value && (
    photo.value.camera_model || photo.value.taken_at ||
    photo.value.shutter_speed || photo.value.aperture || photo.value.iso
  )
)

function formatSize(bytes: number | null) {
  if (!bytes) return '-'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function formatDate(d: string | null) {
  if (!d) return '-'
  return d.replace('T', ' ').slice(0, 19)
}

async function loadThumb() {
  const seq = ++loadSeq.value
  const p = photo.value
  if (!p || p.is_missing) {
    if (seq === loadSeq.value) thumbSrc.value = null
    return
  }
  const fullPath = `${tab.value?.workspace.path}/${p.relative_path}`
  const requestSize = normalizeThumbSize(480)
  const nearest = getNearestCachedThumb(fullPath, requestSize)
  if (nearest && seq === loadSeq.value && thumbSrc.value !== nearest) {
    thumbSrc.value = nearest
  }
  const exact = getExactCachedThumb(fullPath, requestSize)
  if (exact) {
    if (seq === loadSeq.value) thumbSrc.value = exact
    return
  }
  try {
    const { size: normalizedSize, src } = await ensureGridThumbSrc(fullPath, requestSize)
    if (seq !== loadSeq.value) return
    thumbSrc.value = src
    putCachedThumb(fullPath, normalizedSize, src)
  } catch {
    if (seq === loadSeq.value && !thumbSrc.value) {
      thumbSrc.value = null
    }
  }
}

watch(() => photo.value?.id, loadThumb, { immediate: true })

async function setStar(n: number) {
  const p = photo.value
  if (!p) return
  const newRating = p.star_rating === n ? 0 : n
  await store.updateMeta(p.id, newRating, p.color_label, p.notes)
}

async function setColor(c: string) {
  const p = photo.value
  if (!p) return
  await store.updateMeta(p.id, p.star_rating, c, p.notes)
}

async function updateNotes(notes: string) {
  const p = photo.value
  if (!p) return
  await store.updateMeta(p.id, p.star_rating, p.color_label, notes)
}
</script>

<style scoped>
.meta-panel {
  height: 100%;
  overflow-y: auto;
  background: #1c1c1c;
  font-size: 12px;
}
.meta-header {
  padding: 8px 12px;
  font-size: 11px;
  color: #555;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  border-bottom: 1px solid #2a2a2a;
}
.meta-thumb {
  padding: 8px;
  background: #111;
  border-bottom: 1px solid #2a2a2a;
}
.preview-thumb {
  width: 100%; max-height: 180px;
  object-fit: contain;
  display: block;
}
.meta-section {
  padding: 8px 0;
  border-bottom: 1px solid #242424;
}
.meta-title {
  padding: 2px 12px 6px;
  font-size: 10px;
  color: #555;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.meta-row {
  display: flex;
  align-items: flex-start;
  padding: 3px 12px;
  gap: 6px;
}
.meta-label { color: #555; width: 56px; flex-shrink: 0; padding-top: 1px; }
.meta-value { color: #bbb; word-break: break-all; }
.meta-value.small { font-size: 10px; color: #666; }

.star-edit { display: flex; gap: 3px; align-items: center; }
.star-edit-item { font-size: 16px; cursor: pointer; color: #444; transition: color 0.1s; }
.star-edit-item:hover, .star-edit-item.filled { color: #f39c12; }
.star-clear { color: #555; cursor: pointer; font-size: 14px; margin-left: 2px; }
.star-clear:hover { color: #ddd; }

.color-edit { display: flex; gap: 5px; align-items: center; flex-wrap: wrap; }
.color-dot {
  width: 16px; height: 16px; border-radius: 50%;
  cursor: pointer; border: 2px solid transparent; transition: border-color 0.1s;
}
.color-dot.active { border-color: #fff; }
.color-dot:hover { border-color: rgba(255,255,255,0.6); }
.color-clear { color: #555; cursor: pointer; font-size: 14px; }
.color-clear:hover { color: #ddd; }

.notes-row { flex-direction: column; gap: 4px; }
.notes-input {
  width: 100%;
  background: #222;
  border: 1px solid #333;
  color: #bbb;
  padding: 6px 8px;
  border-radius: 4px;
  font-size: 12px;
  resize: vertical;
  font-family: inherit;
}
.notes-input:focus { outline: none; border-color: #4F8EF7; }

.meta-empty {
  padding: 32px 12px;
  text-align: center;
  color: #444;
  font-size: 12px;
}
</style>
