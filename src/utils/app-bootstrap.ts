export async function bootstrapApp(
  loadKeybindings: () => Promise<unknown>,
  setupWorkspaceListeners: () => Promise<unknown>,
) {
  await loadKeybindings()
  await setupWorkspaceListeners()
}
