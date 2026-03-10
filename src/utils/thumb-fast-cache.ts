const DEFAULT_FAST_CACHE_MAX_BYTES = 96 * 1024 * 1024

interface FastThumbEntry {
  src: string
  bytes: number
}

const fastCache = new Map<string, FastThumbEntry>()
let fastCacheBytes = 0
let fastCacheMaxBytes = DEFAULT_FAST_CACHE_MAX_BYTES

function touchFastKey(key: string) {
  const found = fastCache.get(key)
  if (!found) return
  fastCache.delete(key)
  fastCache.set(key, found)
}

function evictFastIfNeeded() {
  while (fastCacheBytes > fastCacheMaxBytes) {
    const oldest = fastCache.keys().next().value as string | undefined
    if (!oldest) break
    const entry = fastCache.get(oldest)
    fastCache.delete(oldest)
    if (entry) fastCacheBytes = Math.max(0, fastCacheBytes - entry.bytes)
  }
}

export function estimateThumbBytes(size: number) {
  const s = Math.max(1, Math.round(size))
  return s * s * 4
}

export function setFastThumbBudgetMB(mb: number) {
  if (!Number.isFinite(mb) || mb <= 0) return
  fastCacheMaxBytes = Math.max(8 * 1024 * 1024, Math.round(mb * 1024 * 1024))
  evictFastIfNeeded()
}

export function getFastThumb(key: string): string | null {
  const entry = fastCache.get(key)
  if (!entry) return null
  touchFastKey(key)
  return entry.src
}

export function putFastThumb(key: string, src: string, estimatedBytes: number): void {
  const bytes = Math.max(1, Math.round(estimatedBytes))
  const prev = fastCache.get(key)
  if (prev) {
    fastCacheBytes = Math.max(0, fastCacheBytes - prev.bytes)
  }
  fastCache.set(key, { src, bytes })
  fastCacheBytes += bytes
  touchFastKey(key)
  evictFastIfNeeded()
}

export function clearFastThumbCache() {
  fastCache.clear()
  fastCacheBytes = 0
}

export function getFastThumbCacheStats() {
  return {
    entries: fastCache.size,
    bytes: fastCacheBytes,
    maxBytes: fastCacheMaxBytes,
  }
}

