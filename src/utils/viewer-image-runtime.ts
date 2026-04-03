import { invoke } from '@tauri-apps/api/core'

import { toTauriImageSrc } from './image-src'
import { getNearestCachedThumb, normalizeThumbSize } from './thumb-cache'
import { createViewerImagePipeline } from './viewer-image-pipeline'

const PREVIEW_SIZE = 1600
const PREVIEW_PROFILE = 'preview'
const PREVIEW_QUALITY = 82
const VIEWER_CACHE_MAX_BYTES = 256 * 1024 * 1024
const VIEWER_IDLE_ORIGINAL_DELAY_MS = 200
const VIEWER_THUMB_REQUEST_SIZE = normalizeThumbSize(160)

function preloadImage(src: string) {
  return new Promise<boolean>((resolve) => {
    const img = new Image()
    img.onload = () => resolve(true)
    img.onerror = () => resolve(false)
    img.src = src
  })
}

async function loadReadyImageSrc(src: string) {
  const ready = await preloadImage(src)
  if (!ready) {
    throw new Error(`failed to preload image: ${src}`)
  }
  return src
}

export const sharedViewerImagePipeline = createViewerImagePipeline({
  getThumbSrc(photoPath) {
    return getNearestCachedThumb(photoPath, VIEWER_THUMB_REQUEST_SIZE)
  },
  async loadPreview(photoPath) {
    const cachedPath: string = await invoke('ensure_preview_cache', {
      photoPath,
      size: PREVIEW_SIZE,
      profile: PREVIEW_PROFILE,
      quality: PREVIEW_QUALITY,
    })
    return loadReadyImageSrc(toTauriImageSrc(cachedPath))
  },
  async loadOriginal(photoPath) {
    return loadReadyImageSrc(toTauriImageSrc(photoPath))
  },
  previewBytes: PREVIEW_SIZE * PREVIEW_SIZE * 4,
  originalBytes: PREVIEW_SIZE * PREVIEW_SIZE * 6,
  idleOriginalDelayMs: VIEWER_IDLE_ORIGINAL_DELAY_MS,
  maxCacheBytes: VIEWER_CACHE_MAX_BYTES,
  setTimeoutFn: setTimeout,
  clearTimeoutFn: clearTimeout,
})
