export interface ViewerImageCacheStats {
  entries: number
  bytes: number
  maxBytes: number
}

export interface ViewerImageCache<T> {
  get(key: string): T | null
  set(key: string, value: T, bytes: number): void
  delete(key: string): void
  clear(): void
  stats(): ViewerImageCacheStats
}

interface CacheEntry<T> {
  value: T
  bytes: number
}

export function createViewerImageCache<T>(maxBytes: number): ViewerImageCache<T> {
  const budget = Math.max(1, Math.round(maxBytes))
  const entries = new Map<string, CacheEntry<T>>()
  let totalBytes = 0

  function touch(key: string) {
    const entry = entries.get(key)
    if (!entry) return
    entries.delete(key)
    entries.set(key, entry)
  }

  function evictIfNeeded() {
    while (totalBytes > budget) {
      const oldest = entries.keys().next().value as string | undefined
      if (!oldest) break
      const entry = entries.get(oldest)
      entries.delete(oldest)
      if (entry) {
        totalBytes = Math.max(0, totalBytes - entry.bytes)
      }
    }
  }

  return {
    get(key) {
      const entry = entries.get(key)
      if (!entry) return null
      touch(key)
      return entry.value
    },
    set(key, value, bytes) {
      const normalizedBytes = Math.max(1, Math.round(bytes))
      const prev = entries.get(key)
      if (prev) {
        totalBytes = Math.max(0, totalBytes - prev.bytes)
      }
      entries.set(key, { value, bytes: normalizedBytes })
      totalBytes += normalizedBytes
      touch(key)
      evictIfNeeded()
    },
    delete(key) {
      const entry = entries.get(key)
      entries.delete(key)
      if (entry) {
        totalBytes = Math.max(0, totalBytes - entry.bytes)
      }
    },
    clear() {
      entries.clear()
      totalBytes = 0
    },
    stats() {
      return {
        entries: entries.size,
        bytes: totalBytes,
        maxBytes: budget,
      }
    },
  }
}
