import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface Keybinding {
  id: number
  action_id: string
  key_combo: string
  enabled: boolean
}

export const ACTION_LABELS: Record<string, string> = {
  open_workspace: '打开工作区',
  close_workspace: '关闭工作区',
  open_settings: '打开设置',
  show_help: '显示快捷键帮助',
  nav_left: '向左导航',
  nav_right: '向右导航',
  nav_up: '向上导航',
  nav_down: '向下导航',
  enter_lightbox: '进入大图模式',
  add_to_selection: '附加选中',
  toggle_cull_mode: '切换选片/网格模式',
  star_1: '设置1星',
  star_2: '设置2星',
  star_3: '设置3星',
  star_4: '设置4星',
  star_5: '设置5星',
  color_red: '红色标签',
  color_orange: '橙色标签',
  color_yellow: '黄色标签',
  color_green: '绿色标签',
  clear_meta: '清除所有标记',
  delete_photos: '删除照片',
  zoom_in: '放大',
  zoom_out: '缩小',
  zoom_reset: '重置缩放',
  exit_lightbox: '退出大图模式',
}

export const ACTION_GROUPS: Record<string, string[]> = {
  '全局': ['open_workspace', 'close_workspace', 'open_settings', 'show_help'],
  '网格模式': ['nav_left', 'nav_right', 'nav_up', 'nav_down', 'enter_lightbox', 'add_to_selection', 'toggle_cull_mode'],
  '选片模式': ['nav_left', 'nav_right', 'toggle_cull_mode'],
  '大图模式': ['nav_left', 'nav_right', 'zoom_in', 'zoom_out', 'zoom_reset', 'exit_lightbox'],
  '标记': ['star_1', 'star_2', 'star_3', 'star_4', 'star_5', 'color_red', 'color_orange', 'color_yellow', 'color_green', 'clear_meta', 'delete_photos'],
}

export const useKeybindingStore = defineStore('keybinding', () => {
  const bindings = ref<Keybinding[]>([])
  const loaded = ref(false)

  async function load() {
    bindings.value = await invoke('get_keybindings')
    loaded.value = true
  }

  async function updateBinding(actionId: string, keyCombo: string, enabled: boolean) {
    await invoke('update_keybinding', { actionId, keyCombo, enabled })
    const b = bindings.value.find(b => b.action_id === actionId)
    if (b) {
      b.key_combo = keyCombo
      b.enabled = enabled
    }
  }

  function getBinding(actionId: string): Keybinding | undefined {
    return bindings.value.find(b => b.action_id === actionId)
  }

  function matchesAction(event: KeyboardEvent, actionId: string): boolean {
    const b = getBinding(actionId)
    if (!b || !b.enabled) return false
    return matchesCombo(event, b.key_combo)
  }

  function matchesCombo(event: KeyboardEvent, combo: string): boolean {
    const parts = combo.split('+')
    const key = parts[parts.length - 1]
    const needCtrl = parts.includes('Ctrl')
    const needShift = parts.includes('Shift')
    const needAlt = parts.includes('Alt')

    if (needCtrl !== event.ctrlKey) return false
    if (needShift !== event.shiftKey) return false
    if (needAlt !== event.altKey) return false

    const keyMap: Record<string, string> = {
      'ArrowLeft': 'ArrowLeft',
      'ArrowRight': 'ArrowRight',
      'ArrowUp': 'ArrowUp',
      'ArrowDown': 'ArrowDown',
      'Enter': 'Enter',
      'Space': ' ',
      'Tab': 'Tab',
      'Delete': 'Delete',
      'Escape': 'Escape',
    }
    const expected = keyMap[key] ?? key
    return event.key === expected
  }

  return { bindings, loaded, load, updateBinding, getBinding, matchesAction, matchesCombo }
})
