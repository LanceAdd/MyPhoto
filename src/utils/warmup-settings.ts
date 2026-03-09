export interface WarmupSettings {
  initialLimit: number
  continueInBackground: boolean
  backgroundBatch: number
  backgroundDelayMs: number
}

export const WARMUP_INITIAL_LIMIT_KEY = 'warmup.initial_limit'
export const WARMUP_CONTINUE_KEY = 'warmup.continue'
export const WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY = 'warmup.initial_limit.customized'

export const DEFAULT_WARMUP_SETTINGS: WarmupSettings = {
  initialLimit: 200,
  continueInBackground: true,
  backgroundBatch: 32,
  backgroundDelayMs: 900,
}

function clampInt(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) return min
  return Math.max(min, Math.min(max, Math.round(value)))
}

export function readWarmupSettings(): WarmupSettings {
  const limitRaw = Number(localStorage.getItem(WARMUP_INITIAL_LIMIT_KEY))
  const continueRaw = localStorage.getItem(WARMUP_CONTINUE_KEY)
  const customizedRaw = localStorage.getItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY)
  const hasCustomizedFlag = customizedRaw === 'true' || customizedRaw === 'false'
  const customized = customizedRaw === 'true'

  // Migration logic:
  // - No customized flag + legacy non-zero value -> treat as user-customized.
  // - No customized flag + zero/invalid value -> treat as default (200).
  if (!hasCustomizedFlag && Number.isFinite(limitRaw) && limitRaw > 0) {
    localStorage.setItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY, 'true')
  }

  const effectiveCustomized = hasCustomizedFlag
    ? customized
    : (Number.isFinite(limitRaw) && limitRaw > 0)

  return {
    ...DEFAULT_WARMUP_SETTINGS,
    initialLimit: (effectiveCustomized && Number.isFinite(limitRaw))
      ? clampInt(limitRaw, 0, 10000)
      : DEFAULT_WARMUP_SETTINGS.initialLimit,
    continueInBackground: continueRaw == null
      ? DEFAULT_WARMUP_SETTINGS.continueInBackground
      : continueRaw === 'true',
  }
}

export function saveWarmupSettings(update: Partial<Pick<WarmupSettings, 'initialLimit' | 'continueInBackground'>>) {
  const merged = {
    ...readWarmupSettings(),
    ...update,
  }
  localStorage.setItem(WARMUP_INITIAL_LIMIT_KEY, String(clampInt(merged.initialLimit, 0, 10000)))
  localStorage.setItem(WARMUP_INITIAL_LIMIT_CUSTOMIZED_KEY, 'true')
  localStorage.setItem(WARMUP_CONTINUE_KEY, String(!!merged.continueInBackground))
  return readWarmupSettings()
}
