import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  createViewerImagePipeline,
  type ViewerImagePipelineDeps,
} from './viewer-image-pipeline'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

async function flushPromises() {
  await Promise.resolve()
  await Promise.resolve()
}

describe('createViewerImagePipeline', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('promotes thumb to preview without clearing the display', async () => {
    const preview = deferred<string>()
    const deps: ViewerImagePipelineDeps = {
      getThumbSrc: (path) => path === 'a' ? 'thumb:a' : null,
      loadPreview: vi.fn(() => preview.promise),
      loadOriginal: vi.fn(async (path) => `original:${path}`),
      previewBytes: 16,
      originalBytes: 32,
      idleOriginalDelayMs: 200,
      maxCacheBytes: 256,
      setTimeoutFn: setTimeout,
      clearTimeoutFn: clearTimeout,
    }
    const pipeline = createViewerImagePipeline(deps)

    pipeline.focus({
      activePath: 'a',
      orderedPaths: ['a', 'b', 'c'],
    })

    expect(pipeline.getSnapshot('a')).toMatchObject({
      displaySrc: 'thumb:a',
      displayStage: 'thumb',
    })

    preview.resolve('preview:a')
    await flushPromises()

    expect(pipeline.getSnapshot('a')).toMatchObject({
      displaySrc: 'preview:a',
      displayStage: 'preview',
    })
  })

  it('loads previews with active photo first and only within the configured neighbor window', async () => {
    const calls: string[] = []
    const deps: ViewerImagePipelineDeps = {
      getThumbSrc: () => null,
      loadPreview: vi.fn(async (path) => {
        calls.push(path)
        return `preview:${path}`
      }),
      loadOriginal: vi.fn(async (path) => `original:${path}`),
      previewBytes: 16,
      originalBytes: 32,
      idleOriginalDelayMs: 200,
      maxCacheBytes: 256,
      setTimeoutFn: setTimeout,
      clearTimeoutFn: clearTimeout,
    }
    const pipeline = createViewerImagePipeline(deps)

    pipeline.focus({
      activePath: 'p5',
      orderedPaths: ['p0', 'p1', 'p2', 'p3', 'p4', 'p5', 'p6', 'p7', 'p8', 'p9', 'p10', 'p11', 'p12', 'p13', 'p14'],
    })
    await flushPromises()

    expect(calls[0]).toBe('p5')
    expect(calls).toContain('p4')
    expect(calls).toContain('p6')
    expect(calls).not.toContain('p14')
  })

  it('defers original loading until idle or zoom', async () => {
    const deps: ViewerImagePipelineDeps = {
      getThumbSrc: () => 'thumb:a',
      loadPreview: vi.fn(async (path) => `preview:${path}`),
      loadOriginal: vi.fn(async (path) => `original:${path}`),
      previewBytes: 16,
      originalBytes: 32,
      idleOriginalDelayMs: 200,
      maxCacheBytes: 256,
      setTimeoutFn: setTimeout,
      clearTimeoutFn: clearTimeout,
    }
    const pipeline = createViewerImagePipeline(deps)

    pipeline.focus({
      activePath: 'a',
      orderedPaths: ['a', 'b', 'c'],
    })
    await flushPromises()

    expect(deps.loadOriginal).not.toHaveBeenCalled()

    vi.advanceTimersByTime(199)
    await flushPromises()
    expect(deps.loadOriginal).not.toHaveBeenCalled()

    vi.advanceTimersByTime(1)
    await flushPromises()
    expect(deps.loadOriginal).toHaveBeenCalledWith('a')

    pipeline.focus({
      activePath: 'b',
      orderedPaths: ['a', 'b', 'c'],
    })
    pipeline.setZoom('b', 1.25)
    await flushPromises()

    expect(deps.loadOriginal).toHaveBeenCalledWith('b')
  })
})
