import { describe, expect, it } from 'vitest'

import { resolveViewerDisplayState } from './viewer-display-state'

describe('resolveViewerDisplayState', () => {
  it('shows no overlay when an image is already displayed and no transition is pending', () => {
    expect(resolveViewerDisplayState({
      hasDisplaySrc: true,
      isMissing: false,
      showTransitionOverlay: false,
    })).toBe('none')
  })

  it('shows the transition badge while the previous image is kept on screen', () => {
    expect(resolveViewerDisplayState({
      hasDisplaySrc: true,
      isMissing: false,
      showTransitionOverlay: true,
    })).toBe('transition')
  })

  it('shows missing state before loading state', () => {
    expect(resolveViewerDisplayState({
      hasDisplaySrc: false,
      isMissing: true,
      showTransitionOverlay: false,
    })).toBe('missing')
  })

  it('shows loading only when no image is currently displayed', () => {
    expect(resolveViewerDisplayState({
      hasDisplaySrc: false,
      isMissing: false,
      showTransitionOverlay: false,
    })).toBe('loading')
  })
})
