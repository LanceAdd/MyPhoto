<template>
  <div class="rail-thumb">
    <img v-if="thumbSrc" :src="thumbSrc" class="thumb-img" />
    <div v-else-if="photo.is_missing" class="missing">🚫</div>
    <div v-else class="placeholder" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import type { Photo } from '../stores/workspace'
import {
  getExactCachedThumb,
  getNearestCachedThumb,
  normalizeThumbSize,
  putCachedThumb,
} from '../utils/thumb-cache'
import { ensureGridThumbSrc } from '../utils/thumb-loader'

const props = defineProps<{ photo: Photo; workspacePath: string }>()
const thumbSrc = ref<string | null>(null)

async function load() {
  if (props.photo.is_missing) return
  const fullPath = `${props.workspacePath}/${props.photo.relative_path}`
  const requestSize = normalizeThumbSize(160)
  const nearest = getNearestCachedThumb(fullPath, requestSize)
  if (nearest && thumbSrc.value !== nearest) {
    thumbSrc.value = nearest
  }
  const exact = getExactCachedThumb(fullPath, requestSize)
  if (exact) {
    thumbSrc.value = exact
    return
  }
  try {
    const { size: normalizedSize, src } = await ensureGridThumbSrc(fullPath, requestSize)
    thumbSrc.value = src
    putCachedThumb(fullPath, normalizedSize, src)
  } catch { thumbSrc.value = null }
}
onMounted(() => {
  void load()
})
watch(() => props.photo.relative_path, load)
watch(() => props.workspacePath, load)
</script>

<style scoped>
.rail-thumb { width: 100%; height: 100%; background: #222; }
.thumb-img { width: 100%; height: 100%; object-fit: cover; }
.missing { display: flex; align-items: center; justify-content: center; height: 100%; font-size: 20px; }
.placeholder {
  width: 100%; height: 100%;
  background: linear-gradient(90deg, #1a1a1a 25%, #222 50%, #1a1a1a 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}
@keyframes shimmer { 0% { background-position: -200% 0; } 100% { background-position: 200% 0; } }
</style>
