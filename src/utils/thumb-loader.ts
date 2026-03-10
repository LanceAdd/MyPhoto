import { invoke } from '@tauri-apps/api/core'
import { toTauriImageSrc } from './image-src'
import { clearThumbCache, normalizeThumbSize } from './thumb-cache'
import { clearFastThumbCache, estimateThumbBytes, getFastThumb, putFastThumb } from './thumb-fast-cache'
import {
  cancelPendingThumbTasks,
  enqueueThumbTask,
  getThumbQueueStats,
  hasRecentInteractiveThumbDemand,
  type ThumbTaskPriority,
} from './thumb-priority-queue'

const GRID_THUMB_PROFILE = 'grid'
const GRID_THUMB_QUALITY = 72
const GRID_FAST_SIZE = 256
const SCHEDULER_FLAG_KEY = 'thumb.scheduler.v2'

let totalRequests = 0
let fastCacheHits = 0
let fastCacheMisses = 0

export interface EnsureGridThumbOptions {
  priority?: ThumbTaskPriority
  phase?: 'auto' | 'fast' | 'final'
}

function isSchedulerV2Enabled() {
  try {
    const raw = localStorage.getItem(SCHEDULER_FLAG_KEY)
    return raw == null ? true : raw !== 'false'
  } catch {
    return true
  }
}

function buildThumbKey(photoPath: string, size: number) {
  return `${photoPath}|${size}`
}

function upgradePriority(priority: ThumbTaskPriority): ThumbTaskPriority {
  switch (priority) {
    case 'p0': return 'p2'
    case 'p1': return 'p3'
    default: return 'p4'
  }
}

async function ensureGridThumbBySize(photoPath: string, size: number, priority: ThumbTaskPriority): Promise<string> {
  const key = buildThumbKey(photoPath, size)
  if (size === GRID_FAST_SIZE) {
    const fast = getFastThumb(key)
    if (fast) {
      fastCacheHits += 1
      return fast
    }
    fastCacheMisses += 1
  }
  if (!isSchedulerV2Enabled()) {
    const cachedPath = await invoke<string>('ensure_preview_cache', {
      photoPath,
      size,
      profile: GRID_THUMB_PROFILE,
      quality: GRID_THUMB_QUALITY,
    })
    const src = toTauriImageSrc(cachedPath)
    if (size === GRID_FAST_SIZE) {
      putFastThumb(key, src, estimateThumbBytes(size))
    }
    return src
  }
  const cachedPath = await enqueueThumbTask<string>(key, priority, () =>
    invoke<string>('ensure_preview_cache', {
      photoPath,
      size,
      profile: GRID_THUMB_PROFILE,
      quality: GRID_THUMB_QUALITY,
    }),
  )
  const src = toTauriImageSrc(cachedPath)
  if (size === GRID_FAST_SIZE) {
    putFastThumb(key, src, estimateThumbBytes(size))
  }
  return src
}

export async function ensureGridThumbSrc(
  photoPath: string,
  requestedSize: number,
  options: EnsureGridThumbOptions = {},
): Promise<{ size: number; src: string }> {
  totalRequests += 1
  const size = normalizeThumbSize(requestedSize)
  const priority = options.priority ?? 'p2'
  const phase = options.phase ?? 'auto'

  if (phase === 'fast') {
    const src = await ensureGridThumbBySize(photoPath, GRID_FAST_SIZE, priority)
    return { size: GRID_FAST_SIZE, src }
  }

  if (phase === 'final') {
    const target = size <= GRID_FAST_SIZE ? GRID_FAST_SIZE : size
    const src = await ensureGridThumbBySize(photoPath, target, priority)
    return { size: target, src }
  }

  if (!isSchedulerV2Enabled()) {
    const target = size <= GRID_FAST_SIZE ? GRID_FAST_SIZE : size
    const src = await ensureGridThumbBySize(photoPath, target, priority)
    return { size: target, src }
  }

  if (size <= GRID_FAST_SIZE) {
    const src = await ensureGridThumbBySize(photoPath, GRID_FAST_SIZE, priority)
    return { size: GRID_FAST_SIZE, src }
  }

  const fastSrc = await ensureGridThumbBySize(photoPath, GRID_FAST_SIZE, priority)
  const finalSrc = await ensureGridThumbBySize(photoPath, size, upgradePriority(priority))
  if (finalSrc) {
    return { size, src: finalSrc }
  }
  return { size: GRID_FAST_SIZE, src: fastSrc }
}

export function hasActiveGridDemand() {
  return isSchedulerV2Enabled() && hasRecentInteractiveThumbDemand()
}

export function trimLowPriorityThumbTasks() {
  if (!isSchedulerV2Enabled()) return 0
  return cancelPendingThumbTasks((priority) => priority === 'p3' || priority === 'p4')
}

export function clearGridThumbCaches() {
  clearThumbCache()
  clearFastThumbCache()
  cancelPendingThumbTasks(() => true)
}

export function getGridThumbQueueStats() {
  const queue = getThumbQueueStats()
  const fastTotal = fastCacheHits + fastCacheMisses
  const fastHitRate = fastTotal > 0 ? fastCacheHits / fastTotal : 0
  return {
    ...queue,
    totalRequests,
    fastCacheHits,
    fastCacheMisses,
    fastHitRate,
    schedulerV2Enabled: isSchedulerV2Enabled(),
  }
}

export function setGridThumbSchedulerEnabled(enabled: boolean) {
  try {
    localStorage.setItem(SCHEDULER_FLAG_KEY, enabled ? 'true' : 'false')
  } catch {
    // ignore storage failures
  }
}

export function readGridThumbSchedulerEnabled() {
  return isSchedulerV2Enabled()
}
