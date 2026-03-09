import { convertFileSrc } from '@tauri-apps/api/core'

export function toTauriImageSrc(absPath: string): string {
  return convertFileSrc(absPath)
}

