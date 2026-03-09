import { invoke } from '@tauri-apps/api/core'
import { toTauriImageSrc } from './image-src'
import { normalizeThumbSize } from './thumb-cache'

const GRID_THUMB_PROFILE = 'grid'
const GRID_THUMB_QUALITY = 72

const inFlightByKey = new Map<string, Promise<string>>()

export async function ensureGridThumbSrc(photoPath: string, requestedSize: number): Promise<{ size: number; src: string }> {
  const size = normalizeThumbSize(requestedSize)
  const key = `${photoPath}|${size}`
  let task = inFlightByKey.get(key)
  if (!task) {
    task = invoke<string>('ensure_preview_cache', {
      photoPath,
      size,
      profile: GRID_THUMB_PROFILE,
      quality: GRID_THUMB_QUALITY,
    })
      .then((cachedPath) => toTauriImageSrc(cachedPath))
      .finally(() => {
        inFlightByKey.delete(key)
      })
    inFlightByKey.set(key, task)
  }
  const src = await task
  return { size, src }
}
