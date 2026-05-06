export type ViewerDisplayState = 'transition' | 'missing' | 'loading' | 'none'

export interface ViewerDisplayStateInput {
  hasDisplaySrc: boolean
  isMissing: boolean
  showTransitionOverlay: boolean
}

export function resolveViewerDisplayState(input: ViewerDisplayStateInput): ViewerDisplayState {
  if (input.showTransitionOverlay && input.hasDisplaySrc) {
    return 'transition'
  }
  if (input.isMissing) {
    return 'missing'
  }
  if (!input.hasDisplaySrc) {
    return 'loading'
  }
  return 'none'
}
