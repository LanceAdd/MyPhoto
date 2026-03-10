export type ThumbTaskPriority = 'p0' | 'p1' | 'p2' | 'p3' | 'p4'

type TaskRunner = () => Promise<unknown>

interface QueueTask {
  key: string
  priority: ThumbTaskPriority
  enqueuedAt: number
  order: number
  run: TaskRunner
  resolve: (value: unknown) => void
  reject: (error?: unknown) => void
}

const PRIORITY_ORDER: Record<ThumbTaskPriority, number> = {
  p0: 0,
  p1: 1,
  p2: 2,
  p3: 3,
  p4: 4,
}

const DEFAULT_CONCURRENCY = 3
const INTERACTIVE_WINDOW_MS = 1500

const pending: QueueTask[] = []
const inFlightByKey = new Map<string, Promise<unknown>>()
const pendingByKey = new Map<string, QueueTask>()
let running = 0
let sequence = 0
let concurrency = DEFAULT_CONCURRENCY
let lastInteractiveDemandAt = 0
let activeInteractiveTasks = 0

let waitSampleCount = 0
let waitSampleTotal = 0

function markInteractive(priority: ThumbTaskPriority) {
  if (priority === 'p0' || priority === 'p1') {
    lastInteractiveDemandAt = Date.now()
  }
}

function compareTask(a: QueueTask, b: QueueTask) {
  const pa = PRIORITY_ORDER[a.priority]
  const pb = PRIORITY_ORDER[b.priority]
  if (pa !== pb) return pa - pb
  return a.order - b.order
}

function resortQueue() {
  pending.sort(compareTask)
}

function takeNextTask(): QueueTask | undefined {
  if (pending.length === 0) return undefined
  resortQueue()
  const task = pending.shift()
  if (task) pendingByKey.delete(task.key)
  return task
}

function pumpQueue() {
  while (running < concurrency) {
    const task = takeNextTask()
    if (!task) break

    running += 1
    if (task.priority === 'p0' || task.priority === 'p1') {
      activeInteractiveTasks += 1
      markInteractive(task.priority)
    }

    const waitMs = Date.now() - task.enqueuedAt
    waitSampleCount += 1
    waitSampleTotal += waitMs

    task
      .run()
      .then((result) => task.resolve(result))
      .catch((error) => task.reject(error))
      .finally(() => {
        running = Math.max(0, running - 1)
        if (task.priority === 'p0' || task.priority === 'p1') {
          activeInteractiveTasks = Math.max(0, activeInteractiveTasks - 1)
        }
        inFlightByKey.delete(task.key)
        pumpQueue()
      })
  }
}

export function setThumbTaskConcurrency(next: number) {
  if (!Number.isFinite(next)) return
  concurrency = Math.max(1, Math.min(8, Math.round(next)))
  pumpQueue()
}

export function enqueueThumbTask<T>(
  key: string,
  priority: ThumbTaskPriority,
  run: () => Promise<T>,
): Promise<T> {
  markInteractive(priority)
  const existing = inFlightByKey.get(key)
  if (existing) {
    const pendingTask = pendingByKey.get(key)
    if (pendingTask && PRIORITY_ORDER[priority] < PRIORITY_ORDER[pendingTask.priority]) {
      pendingTask.priority = priority
      resortQueue()
    }
    return existing as Promise<T>
  }

  const enqueuedAt = Date.now()
  const order = sequence++
  let resolveTask!: (value: T) => void
  let rejectTask!: (error?: unknown) => void

  const promise = new Promise<T>((resolve, reject) => {
    resolveTask = resolve
    rejectTask = reject
  })

  pending.push({
    key,
    priority,
    enqueuedAt,
    order,
    run: run as TaskRunner,
    resolve: resolveTask as (value: unknown) => void,
    reject: rejectTask,
  })
  inFlightByKey.set(key, promise as Promise<unknown>)
  pendingByKey.set(key, pending[pending.length - 1])

  pumpQueue()
  return promise
}

export function cancelPendingThumbTasks(predicate: (priority: ThumbTaskPriority, key: string) => boolean) {
  if (pending.length === 0) return 0
  let removed = 0
  for (let i = pending.length - 1; i >= 0; i--) {
    const task = pending[i]
    if (!predicate(task.priority, task.key)) continue
    pending.splice(i, 1)
    pendingByKey.delete(task.key)
    inFlightByKey.delete(task.key)
    task.reject(new Error('thumb task cancelled'))
    removed += 1
  }
  return removed
}

export function hasRecentInteractiveThumbDemand(windowMs = INTERACTIVE_WINDOW_MS) {
  if (activeInteractiveTasks > 0) return true
  return Date.now() - lastInteractiveDemandAt <= windowMs
}

export function getThumbQueueStats() {
  const avgWaitMs = waitSampleCount > 0 ? waitSampleTotal / waitSampleCount : 0
  return {
    pending: pending.length,
    running,
    inFlight: inFlightByKey.size,
    averageWaitMs: avgWaitMs,
    interactiveActive: activeInteractiveTasks,
    interactiveLastAt: lastInteractiveDemandAt,
  }
}
