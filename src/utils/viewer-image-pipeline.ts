import { createViewerImageCache, type ViewerImageCache } from './viewer-image-cache'

export type ViewerDisplayStage = 'thumb' | 'preview' | 'original'

export interface ViewerImageSnapshot {
  displaySrc: string | null
  displayStage: ViewerDisplayStage | null
  isLoading: boolean
}

export interface ViewerImagePipelineDeps {
  getThumbSrc(path: string): string | null
  loadPreview(path: string): Promise<string>
  loadOriginal(path: string): Promise<string>
  previewBytes: number
  originalBytes: number
  idleOriginalDelayMs: number
  maxCacheBytes: number
  setTimeoutFn: typeof setTimeout
  clearTimeoutFn: typeof clearTimeout
}

export interface ViewerFocusInput {
  activePath: string
  orderedPaths: string[]
}

type PipelineTaskKind = 'preview' | 'original'

interface PipelineTask {
  key: string
  path: string
  kind: PipelineTaskKind
  priority: number
}

interface InternalState extends ViewerImageSnapshot {
  thumbSrc: string | null
  previewSrc: string | null
  originalSrc: string | null
  previewStatus: 'idle' | 'queued' | 'loading' | 'ready' | 'error'
  originalStatus: 'idle' | 'queued' | 'loading' | 'ready' | 'error'
  subscribers: Set<(snapshot: ViewerImageSnapshot) => void>
}

const ACTIVE_PREVIEW_PRIORITY = 0
const ACTIVE_ORIGINAL_PRIORITY = 1
const NEAR_PREVIEW_PRIORITY = 2
const MID_PREVIEW_PRIORITY = 3
const FAR_PREVIEW_PRIORITY = 4
const PREFETCH_WINDOW = 8
const MAX_CONCURRENCY = 2

function emptySnapshot(): ViewerImageSnapshot {
  return {
    displaySrc: null,
    displayStage: null,
    isLoading: false,
  }
}

function createState(): InternalState {
  return {
    ...emptySnapshot(),
    thumbSrc: null,
    previewSrc: null,
    originalSrc: null,
    previewStatus: 'idle',
    originalStatus: 'idle',
    subscribers: new Set(),
  }
}

export function createViewerImagePipeline(deps: ViewerImagePipelineDeps) {
  const states = new Map<string, InternalState>()
  const cache: ViewerImageCache<string> = createViewerImageCache(deps.maxCacheBytes)
  const pending = new Map<string, PipelineTask>()
  const running = new Map<string, Promise<void>>()
  let activePath: string | null = null
  let activeIdleTimer: ReturnType<typeof setTimeout> | null = null

  function getState(path: string) {
    let state = states.get(path)
    if (!state) {
      state = createState()
      states.set(path, state)
    }
    return state
  }

  function emit(path: string) {
    const state = getState(path)
    const snapshot = {
      displaySrc: state.displaySrc,
      displayStage: state.displayStage,
      isLoading: state.isLoading,
    }
    state.subscribers.forEach(listener => listener(snapshot))
  }

  function setLoading(path: string) {
    const state = getState(path)
    const nextLoading = state.previewStatus === 'queued'
      || state.previewStatus === 'loading'
      || (path === activePath && (state.originalStatus === 'queued' || state.originalStatus === 'loading'))
    if (state.isLoading === nextLoading) return
    state.isLoading = nextLoading
    emit(path)
  }

  function updateDisplay(path: string, stage: ViewerDisplayStage, src: string) {
    const state = getState(path)
    const currentRank = state.displayStage === 'thumb'
      ? 1
      : state.displayStage === 'preview'
        ? 2
        : state.displayStage === 'original'
          ? 3
          : 0
    const nextRank = stage === 'thumb'
      ? 1
      : stage === 'preview'
        ? 2
        : 3

    if (nextRank < currentRank) return
    if (state.displaySrc === src && state.displayStage === stage) return
    state.displaySrc = src
    state.displayStage = stage
    emit(path)
  }

  function hydrateKnownSources(path: string) {
    const state = getState(path)
    if (!state.thumbSrc) {
      state.thumbSrc = deps.getThumbSrc(path)
    }
    if (!state.previewSrc) {
      state.previewSrc = cache.get(`${path}|preview`)
      if (state.previewSrc) {
        state.previewStatus = 'ready'
      }
    }
    if (!state.originalSrc) {
      state.originalSrc = cache.get(`${path}|original`)
      if (state.originalSrc) {
        state.originalStatus = 'ready'
      }
    }

    if (state.originalSrc) {
      updateDisplay(path, 'original', state.originalSrc)
      return
    }
    if (state.previewSrc) {
      updateDisplay(path, 'preview', state.previewSrc)
      return
    }
    if (state.thumbSrc) {
      updateDisplay(path, 'thumb', state.thumbSrc)
    }
  }

  function enqueue(path: string, kind: PipelineTaskKind, priority: number) {
    const state = getState(path)
    if (kind === 'preview' && (state.previewStatus === 'queued' || state.previewStatus === 'loading' || state.previewStatus === 'ready')) {
      return
    }
    if (kind === 'original' && (state.originalStatus === 'queued' || state.originalStatus === 'loading' || state.originalStatus === 'ready')) {
      return
    }

    const key = `${kind}|${path}`
    if (pending.has(key) || running.has(key)) return

    pending.set(key, { key, path, kind, priority })
    if (kind === 'preview') {
      state.previewStatus = 'queued'
    } else {
      state.originalStatus = 'queued'
    }
    setLoading(path)
    void runQueue()
  }

  async function runTask(task: PipelineTask) {
    const state = getState(task.path)
    try {
      if (task.kind === 'preview') {
        state.previewStatus = 'loading'
        setLoading(task.path)
        const src = await deps.loadPreview(task.path)
        state.previewSrc = src
        state.previewStatus = 'ready'
        cache.set(`${task.path}|preview`, src, deps.previewBytes)
        if (state.displayStage !== 'original') {
          updateDisplay(task.path, 'preview', src)
        }
      } else {
        state.originalStatus = 'loading'
        setLoading(task.path)
        const src = await deps.loadOriginal(task.path)
        state.originalSrc = src
        state.originalStatus = 'ready'
        cache.set(`${task.path}|original`, src, deps.originalBytes)
        if (task.path === activePath) {
          updateDisplay(task.path, 'original', src)
        }
      }
    } catch {
      if (task.kind === 'preview') {
        state.previewStatus = 'error'
      } else {
        state.originalStatus = 'error'
      }
    } finally {
      setLoading(task.path)
    }
  }

  async function runQueue() {
    while (running.size < MAX_CONCURRENCY && pending.size > 0) {
      const task = [...pending.values()].sort((a, b) => a.priority - b.priority)[0]
      if (!task) break
      pending.delete(task.key)
      const promise = runTask(task).finally(() => {
        running.delete(task.key)
        void runQueue()
      })
      running.set(task.key, promise)
    }
  }

  function clearIdleTimer() {
    if (!activeIdleTimer) return
    deps.clearTimeoutFn(activeIdleTimer)
    activeIdleTimer = null
  }

  function scheduleIdleOriginal(path: string) {
    clearIdleTimer()
    activeIdleTimer = deps.setTimeoutFn(() => {
      if (activePath !== path) return
      enqueue(path, 'original', ACTIVE_ORIGINAL_PRIORITY)
    }, deps.idleOriginalDelayMs)
  }

  function trimPendingForFocus(focusPath: string, allowedPreviewPaths: Set<string>) {
    for (const [key, task] of pending.entries()) {
      if (task.kind === 'preview' && allowedPreviewPaths.has(task.path)) continue
      if (task.kind === 'original' && task.path === focusPath) continue

      pending.delete(key)
      const state = getState(task.path)
      if (task.kind === 'preview' && state.previewStatus === 'queued') {
        state.previewStatus = state.previewSrc ? 'ready' : 'idle'
      }
      if (task.kind === 'original' && state.originalStatus === 'queued') {
        state.originalStatus = state.originalSrc ? 'ready' : 'idle'
      }
      setLoading(task.path)
    }
  }

  return {
    focus(input: ViewerFocusInput) {
      activePath = input.activePath
      hydrateKnownSources(input.activePath)

      const activeIndex = input.orderedPaths.indexOf(input.activePath)
      const previewCandidates: Array<{ path: string; priority: number }> = [
        { path: input.activePath, priority: ACTIVE_PREVIEW_PRIORITY },
      ]

      const allowedPreviewPaths = new Set<string>([input.activePath])
      for (let distance = 1; distance <= PREFETCH_WINDOW; distance++) {
        const leftIndex = activeIndex - distance
        const rightIndex = activeIndex + distance
        const priority = distance <= 2
          ? NEAR_PREVIEW_PRIORITY
          : distance <= 4
            ? MID_PREVIEW_PRIORITY
            : FAR_PREVIEW_PRIORITY

        if (leftIndex >= 0) {
          const leftPath = input.orderedPaths[leftIndex]
          allowedPreviewPaths.add(leftPath)
          previewCandidates.push({ path: leftPath, priority })
        }
        if (rightIndex >= 0 && rightIndex < input.orderedPaths.length) {
          const rightPath = input.orderedPaths[rightIndex]
          allowedPreviewPaths.add(rightPath)
          previewCandidates.push({ path: rightPath, priority })
        }
      }

      trimPendingForFocus(input.activePath, allowedPreviewPaths)

      for (const candidate of previewCandidates) {
        hydrateKnownSources(candidate.path)
        enqueue(candidate.path, 'preview', candidate.priority)
      }
      scheduleIdleOriginal(input.activePath)
    },
    setZoom(path: string, scale: number) {
      if (path !== activePath) return
      if (scale <= 1) return
      enqueue(path, 'original', ACTIVE_ORIGINAL_PRIORITY)
    },
    primeThumb(path: string, src: string) {
      const state = getState(path)
      state.thumbSrc = src
      if (!state.displaySrc) {
        updateDisplay(path, 'thumb', src)
      }
    },
    getSnapshot(path: string): ViewerImageSnapshot {
      hydrateKnownSources(path)
      const state = getState(path)
      return {
        displaySrc: state.displaySrc,
        displayStage: state.displayStage,
        isLoading: state.isLoading,
      }
    },
    subscribe(path: string, listener: (snapshot: ViewerImageSnapshot) => void) {
      const state = getState(path)
      state.subscribers.add(listener)
      listener(this.getSnapshot(path))
      return () => {
        state.subscribers.delete(listener)
      }
    },
    reset() {
      clearIdleTimer()
      pending.clear()
      running.clear()
      states.clear()
      cache.clear()
      activePath = null
    },
  }
}
