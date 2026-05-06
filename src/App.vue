<template>
  <n-config-provider :theme="darkTheme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-dialog-provider>
        <AppLayout v-if="ready" />
        <div v-else class="app-booting">正在初始化…</div>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NConfigProvider, NMessageProvider, NDialogProvider, darkTheme } from 'naive-ui'
import AppLayout from './components/AppLayout.vue'
import { useWorkspaceStore } from './stores/workspace'
import { useKeybindingStore } from './stores/keybinding'
import { bootstrapApp } from './utils/app-bootstrap'

const workspaceStore = useWorkspaceStore()
const keybindingStore = useKeybindingStore()
const ready = ref(false)

const themeOverrides = {
  common: {
    primaryColor: '#4F8EF7',
    primaryColorHover: '#6BA3F9',
    primaryColorPressed: '#3B7CE5',
    bodyColor: '#1a1a1a',
    cardColor: '#242424',
  }
}

void bootstrapApp(
  () => keybindingStore.load(),
  () => workspaceStore.setupListeners(),
).finally(() => {
  ready.value = true
})
</script>

<style>
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body, #app {
  width: 100%;
  height: 100%;
  background: #1a1a1a;
  color: #e0e0e0;
  font-family: 'Inter', 'Segoe UI', system-ui, sans-serif;
  overflow: hidden;
  user-select: none;
}

.app-booting {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #7f90ad;
  font-size: 14px;
}
</style>
