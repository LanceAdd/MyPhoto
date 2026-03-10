export interface WarmupSettings {
  initialLimit: number
  continueInBackground: boolean
  backgroundBatch: number
  backgroundDelayMs: number
  workerConcurrency: number
  popupAutoShow: boolean
}

export const WARMUP_INITIAL_LIMIT_KEY = 'warmup.initial_limit'
export const WARMUP_CONTINUE_KEY = 'warmup.continue'
export const WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY = 'warmup.initial_limit.customized'
export const WARMUP_WORKER_CONCURRENCY_KEY = 'warmup.worker_concurrency'
export const WARMUP_WORKER_CONCURRENCY_CUSTOMIZED_KEY = 'warmup.worker_concurrency.customized'
export const WARMUP_POPUP_AUTO_SHOW_KEY = 'warmup.popup.auto_show'
export const WARMUP_ADAPTIVE_INITIALIZED_KEY = 'warmup.adaptive_initialized'

export const DEFAULT_WARMUP_SETTINGS: WarmupSettings = {
  initialLimit: 40,
  continueInBackground: true,
  backgroundBatch: 16,
  backgroundDelayMs: 1500,
  workerConcurrency: 3,
  popupAutoShow: true,
}

function clampInt(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) return min
  return Math.max(min, Math.min(max, Math.round(value)))
}

function detectAdaptivePreset() {
  const nav = navigator as Navigator & { deviceMemory?: number }
  const cores = clampInt(Number(nav.hardwareConcurrency ?? 4), 1, 64)
  const memory = Number(nav.deviceMemory ?? 0)

  const lowMemory = Number.isFinite(memory) && memory > 0 && memory <= 4
  const highMemory = Number.isFinite(memory) && memory >= 16

  if (cores <= 4 || lowMemory) {
    return { initialLimit: 20, workerConcurrency: 2 }
  }
  if (cores >= 12 || highMemory) {
    return { initialLimit: 80, workerConcurrency: 4 }
  }
  return { initialLimit: 40, workerConcurrency: 3 }
}

export function ensureWarmupSettingsInitialized() {
  const initialized = localStorage.getItem(WARMUP_ADAPTIVE_INITIALIZED_KEY) === 'true'
  if (initialized) return readWarmupSettings()

  const limitCustomized = localStorage.getItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY) === 'true'
  const concurrencyCustomized = localStorage.getItem(WARMUP_WORKER_CONCURRENCY_CUSTOMIZED_KEY) === 'true'
  const preset = detectAdaptivePreset()

  if (!limitCustomized) {
    localStorage.setItem(WARMUP_INITIAL_LIMIT_KEY, String(preset.initialLimit))
    localStorage.setItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY, 'false')
  }
  if (!concurrencyCustomized) {
    localStorage.setItem(WARMUP_WORKER_CONCURRENCY_KEY, String(preset.workerConcurrency))
    localStorage.setItem(WARMUP_WORKER_CONCURRENCY_CUSTOMIZED_KEY, 'false')
  }
  if (localStorage.getItem(WARMUP_POPUP_AUTO_SHOW_KEY) == null) {
    localStorage.setItem(WARMUP_POPUP_AUTO_SHOW_KEY, 'true')
  }
  if (localStorage.getItem(WARMUP_CONTINUE_KEY) == null) {
    localStorage.setItem(WARMUP_CONTINUE_KEY, String(DEFAULT_WARMUP_SETTINGS.continueInBackground))
  }
  localStorage.setItem(WARMUP_ADAPTIVE_INITIALIZED_KEY, 'true')
  return readWarmupSettings()
}

export function readWarmupSettings(): WarmupSettings {
  const limitRaw = Number(localStorage.getItem(WARMUP_INITIAL_LIMIT_KEY))
  const continueRaw = localStorage.getItem(WARMUP_CONTINUE_KEY)
  const customizedRaw = localStorage.getItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY)
  const workerRaw = Number(localStorage.getItem(WARMUP_WORKER_CONCURRENCY_KEY))
  const workerCustomizedRaw = localStorage.getItem(WARMUP_WORKER_CONCURRENCY_CUSTOMIZED_KEY)
  const popupAutoShowRaw = localStorage.getItem(WARMUP_POPUP_AUTO_SHOW_KEY)
  const hasCustomizedFlag = customizedRaw === 'true' || customizedRaw === 'false'
  const customized = customizedRaw === 'true'
  const hasWorkerCustomizedFlag = workerCustomizedRaw === 'true' || workerCustomizedRaw === 'false'
  const workerCustomized = workerCustomizedRaw === 'true'

  // Migration logic:
  // - No customized flag + legacy non-zero value -> treat as user-customized.
  // - No customized flag + zero/invalid value -> treat as default (40).
  if (!hasCustomizedFlag && Number.isFinite(limitRaw) && limitRaw > 0) {
    localStorage.setItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY, 'true')
  }

  const effectiveCustomized = hasCustomizedFlag
    ? customized
    : (Number.isFinite(limitRaw) && limitRaw > 0)
  const effectiveWorkerCustomized = hasWorkerCustomizedFlag
    ? workerCustomized
    : Number.isFinite(workerRaw)

  return {
    ...DEFAULT_WARMUP_SETTINGS,
    initialLimit: (effectiveCustomized && Number.isFinite(limitRaw))
      ? clampInt(limitRaw, 0, 10000)
      : DEFAULT_WARMUP_SETTINGS.initialLimit,
    continueInBackground: continueRaw == null
      ? DEFAULT_WARMUP_SETTINGS.continueInBackground
      : continueRaw === 'true',
    workerConcurrency: effectiveWorkerCustomized && Number.isFinite(workerRaw)
      ? clampInt(workerRaw, 1, 8)
      : DEFAULT_WARMUP_SETTINGS.workerConcurrency,
    popupAutoShow: popupAutoShowRaw == null
      ? DEFAULT_WARMUP_SETTINGS.popupAutoShow
      : popupAutoShowRaw === 'true',
  }
}

export function saveWarmupSettings(
  update: Partial<Pick<WarmupSettings, 'initialLimit' | 'continueInBackground' | 'workerConcurrency' | 'popupAutoShow'>>
) {
  const merged = {
    ...readWarmupSettings(),
    ...update,
  }
  localStorage.setItem(WARMUP_INITIAL_LIMIT_KEY, String(clampInt(merged.initialLimit, 0, 10000)))
  localStorage.setItem(WARMUP_WORKER_CONCURRENCY_KEY, String(clampInt(merged.workerConcurrency, 1, 8)))
  localStorage.setItem(WARMUP_CONTINUE_KEY, String(!!merged.continueInBackground))
  localStorage.setItem(WARMUP_POPUP_AUTO_SHOW_KEY, String(!!merged.popupAutoShow))
  if (update.initialLimit !== undefined) {
    localStorage.setItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY, 'true')
  }
  if (update.workerConcurrency !== undefined) {
    localStorage.setItem(WARMUP_WORKER_CONCURRENCY_CUSTOMIZED_KEY, 'true')
  }
  localStorage.setItem(WARMUP_ADAPTIVE_INITIALIZED_KEY, 'true')
  return readWarmupSettings()
}
