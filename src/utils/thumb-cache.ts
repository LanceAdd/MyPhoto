const MAX_PHOTOS = 800
const MAX_SIZES_PER_PHOTO = 2
const SIZE_PRESETS = [256, 512]

type SizeMap = Map<number, string>

const cacheByPhoto = new Map<string, SizeMap>()

function normalizePathPrefix(path: string) {
  return path.replace(/\\/g, '/').replace(/\/+$/, '')
}

function touchPhoto(photoKey: string) {
  const found = cacheByPhoto.get(photoKey)
  if (!found) return
  cacheByPhoto.delete(photoKey)
  cacheByPhoto.set(photoKey, found)
}

function evictIfNeeded() {
  while (cacheByPhoto.size > MAX_PHOTOS) {
    const oldest = cacheByPhoto.keys().next().value as string | undefined
    if (!oldest) break
    cacheByPhoto.delete(oldest)
  }
}

function capPerPhoto(photoKey: string) {
  const sizes = cacheByPhoto.get(photoKey)
  if (!sizes) return
  while (sizes.size > MAX_SIZES_PER_PHOTO) {
    const oldestSize = sizes.keys().next().value as number | undefined
    if (oldestSize == null) break
    sizes.delete(oldestSize)
  }
}

export function normalizeThumbSize(size: number): number {
  const normalized = Math.max(64, Math.round(size))
  let best = SIZE_PRESETS[0]
  let bestDist = Math.abs(best - normalized)
  for (let i = 1; i < SIZE_PRESETS.length; i++) {
    const candidate = SIZE_PRESETS[i]
    const dist = Math.abs(candidate - normalized)
    if (dist < bestDist) {
      best = candidate
      bestDist = dist
    }
  }
  return best
}

export function putCachedThumb(photoKey: string, size: number, src: string): void {
  const normalizedSize = normalizeThumbSize(size)
  let sizes = cacheByPhoto.get(photoKey)
  if (!sizes) {
    sizes = new Map<number, string>()
    cacheByPhoto.set(photoKey, sizes)
  }
  sizes.set(normalizedSize, src)
  capPerPhoto(photoKey)
  touchPhoto(photoKey)
  evictIfNeeded()
}

export function getExactCachedThumb(photoKey: string, size: number): string | null {
  const normalizedSize = normalizeThumbSize(size)
  const sizes = cacheByPhoto.get(photoKey)
  if (!sizes) return null
  const src = sizes.get(normalizedSize) ?? null
  if (src) {
    touchPhoto(photoKey)
  }
  return src
}

export function getNearestCachedThumb(photoKey: string, size: number): string | null {
  const normalizedSize = normalizeThumbSize(size)
  const sizes = cacheByPhoto.get(photoKey)
  if (!sizes || sizes.size === 0) return null

  let bestSrc: string | null = null
  let bestDist = Number.POSITIVE_INFINITY
  for (const [s, src] of sizes.entries()) {
    const dist = Math.abs(s - normalizedSize)
    if (dist < bestDist) {
      bestDist = dist
      bestSrc = src
      if (dist === 0) break
    }
  }
  if (bestSrc) {
    touchPhoto(photoKey)
  }
  return bestSrc
}

export function clearThumbCache() {
  cacheByPhoto.clear()
}

export function invalidateThumbCacheByPaths(paths: string[]) {
  if (!paths.length) return
  const normalized = paths
    .map(normalizePathPrefix)
    .filter(Boolean)
  if (!normalized.length) return

  for (const photoKey of [...cacheByPhoto.keys()]) {
    const key = normalizePathPrefix(photoKey)
    const matched = normalized.some(prefix =>
      key === prefix || key.startsWith(`${prefix}/`)
    )
    if (matched) {
      cacheByPhoto.delete(photoKey)
    }
  }
}
