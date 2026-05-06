import { describe, expect, it, vi } from 'vitest'

import { bootstrapApp } from './app-bootstrap'

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((res) => {
    resolve = res
  })
  return { promise, resolve }
}

describe('bootstrapApp', () => {
  it('waits for keybindings before attaching workspace listeners', async () => {
    const order: string[] = []
    const keybindings = deferred()
    const loadKeybindings = vi.fn(async () => {
      order.push('keybindings:start')
      await keybindings.promise
      order.push('keybindings:end')
    })
    const setupListeners = vi.fn(async () => {
      order.push('listeners')
    })

    const boot = bootstrapApp(loadKeybindings, setupListeners)

    expect(loadKeybindings).toHaveBeenCalledTimes(1)
    expect(setupListeners).not.toHaveBeenCalled()

    keybindings.resolve()
    await boot

    expect(setupListeners).toHaveBeenCalledTimes(1)
    expect(order).toEqual(['keybindings:start', 'keybindings:end', 'listeners'])
  })
})
