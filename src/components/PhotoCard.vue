<template>
  <div
    class="photo-card"
    :class="{ selected, active, missing: photo.is_missing }"
    :style="cardStyle"
    @click.stop="$emit('click', $event)"
    @dblclick.stop="$emit('dblclick')"
    @contextmenu.stop="$emit('contextmenu', $event)"
  >
    <!-- Thumbnail -->
    <div class="thumb-wrap" :style="thumbWrapStyle">
      <img
        v-if="!photo.is_missing && thumbSrc"
        :src="thumbSrc"
        class="thumb"
        :style="imgStyle"
        loading="lazy"
      />
      <div v-else-if="photo.is_missing" class="missing-overlay">
        <span>🚫</span>
      </div>
      <div v-else class="loading-placeholder" />
    </div>

    <!-- Color label dot -->
    <div
      v-if="photo.color_label"
      class="color-dot"
      :style="{ background: colorMap[photo.color_label] ?? '#888' }"
    />

    <!-- Star rating -->
    <div v-if="photo.star_rating > 0" class="star-badge">
      {{ '★'.repeat(photo.star_rating) }}
    </div>

    <!-- Hover overlay with quick-set stars -->
    <div class="hover-overlay" v-if="!photo.is_missing">
      <div class="quick-stars">
        <span
          v-for="n in 5"
          :key="n"
          class="quick-star"
          :class="{ filled: n <= photo.star_rating }"
          @click.stop="setStar(n)"
          @mousedown.stop
        >★</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useWorkspaceStore, type Photo } from '../stores/workspace'

const props = defineProps<{
  photo: Photo
  size: number
  layout: 'fit' | 'flow'
  selected: boolean
  active: boolean
  workspacePath: string
}>()
const emit = defineEmits(['click', 'dblclick', 'contextmenu'])

const store = useWorkspaceStore()
const thumbSrc = ref<string | null>(null)

const colorMap: Record<string, string> = {
  red: '#e74c3c',
  orange: '#e67e22',
  yellow: '#f1c40f',
  green: '#2ecc71',
  blue: '#3498db',
  purple: '#9b59b6',
}

const cardStyle = computed(() => ({
  width: props.size + 'px',
  height: props.size + 'px',
  flexShrink: 0,
}))

const thumbWrapStyle = computed(() => ({
  width: '100%',
  height: '100%',
}))

const imgStyle = computed(() => {
  if (props.layout === 'fit') {
    return { objectFit: 'contain' as const, width: '100%', height: '100%' }
  }
  return { objectFit: 'cover' as const, width: '100%', height: '100%' }
})

async function loadThumb() {
  if (props.photo.is_missing) return
  try {
    const fullPath = `${props.workspacePath}/${props.photo.relative_path}`
    const b64: string = await invoke('get_thumbnail', { photoPath: fullPath, size: props.size * 2 })
    thumbSrc.value = `data:image/jpeg;base64,${b64}`
  } catch (e) {
    thumbSrc.value = null
  }
}

function setStar(n: number) {
  const current = props.photo.star_rating
  const newRating = current === n ? 0 : n
  store.updateMeta(props.photo.id, newRating, props.photo.color_label, props.photo.notes)
}

// Load thumbnail shortly after mount
onMounted(() => {
  setTimeout(loadThumb, 50)
})

watch(() => props.photo.relative_path, loadThumb)
watch(() => props.workspacePath, loadThumb)
</script>

<style scoped>
.photo-card {
  position: relative;
  background: #2a2a2a;
  border-radius: 4px;
  overflow: hidden;
  cursor: pointer;
  border: 2px solid transparent;
  transition: border-color 0.1s, transform 0.1s;
  flex-shrink: 0;
}
.photo-card:hover { border-color: #4a4a4a; }
.photo-card.selected { border-color: #4F8EF7; }
.photo-card.active { border-color: #fff; }
.photo-card.missing { opacity: 0.5; }

.thumb-wrap {
  position: relative;
  background: #222;
  display: flex;
  align-items: center;
  justify-content: center;
}
.thumb { transition: opacity 0.2s; }
.missing-overlay {
  display: flex; align-items: center; justify-content: center;
  width: 100%; height: 100%; font-size: 32px;
  background: #1a1a1a;
}
.loading-placeholder {
  width: 100%; height: 100%;
  background: linear-gradient(90deg, #222 25%, #2a2a2a 50%, #222 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}
@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

.color-dot {
  position: absolute;
  top: 4px;
  left: 4px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  border: 1px solid rgba(0,0,0,0.3);
}
.star-badge {
  position: absolute;
  bottom: 3px;
  right: 4px;
  font-size: 10px;
  color: #f39c12;
  text-shadow: 0 1px 2px rgba(0,0,0,0.8);
  letter-spacing: -1px;
}

.hover-overlay {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 4px 4px 3px;
  background: linear-gradient(transparent, rgba(0,0,0,0.7));
  display: flex;
  justify-content: center;
  opacity: 0;
  transition: opacity 0.15s;
}
.photo-card:hover .hover-overlay { opacity: 1; }
.quick-stars { display: flex; gap: 2px; }
.quick-star {
  font-size: 14px;
  color: #555;
  cursor: pointer;
  transition: color 0.1s;
}
.quick-star:hover, .quick-star.filled { color: #f39c12; }
</style>
