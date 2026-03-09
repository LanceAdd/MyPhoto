import { ref } from 'vue'

export type GridRowAlignMode = 'center' | 'stretch'

export const GRID_ROW_ALIGN_MODE_KEY = 'grid.row_align_mode'
export const DEFAULT_GRID_ROW_ALIGN_MODE: GridRowAlignMode = 'center'

function normalizeMode(mode: string | null): GridRowAlignMode {
  if (mode === 'stretch') return 'stretch'
  return 'center'
}

const gridRowAlignModeRef = ref<GridRowAlignMode>(
  normalizeMode(localStorage.getItem(GRID_ROW_ALIGN_MODE_KEY)),
)

export function useGridRowAlignMode() {
  return gridRowAlignModeRef
}

export function setGridRowAlignMode(mode: GridRowAlignMode) {
  const normalized = normalizeMode(mode)
  gridRowAlignModeRef.value = normalized
  localStorage.setItem(GRID_ROW_ALIGN_MODE_KEY, normalized)
}
