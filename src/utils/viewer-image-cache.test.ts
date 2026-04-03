import { describe, expect, it } from 'vitest'

import { createViewerImageCache } from './viewer-image-cache'

describe('createViewerImageCache', () => {
  it('evicts the oldest entry when the budget is exceeded', () => {
    const cache = createViewerImageCache<string>(10)

    cache.set('a', 'A', 4)
    cache.set('b', 'B', 4)
    cache.set('c', 'C', 4)

    expect(cache.get('a')).toBeNull()
    expect(cache.get('b')).toBe('B')
    expect(cache.get('c')).toBe('C')
    expect(cache.stats()).toMatchObject({
      entries: 2,
      bytes: 8,
      maxBytes: 10,
    })
  })

  it('keeps a recently-read entry during later eviction', () => {
    const cache = createViewerImageCache<string>(10)

    cache.set('a', 'A', 4)
    cache.set('b', 'B', 4)
    expect(cache.get('a')).toBe('A')

    cache.set('c', 'C', 4)

    expect(cache.get('a')).toBe('A')
    expect(cache.get('b')).toBeNull()
    expect(cache.get('c')).toBe('C')
  })
})
