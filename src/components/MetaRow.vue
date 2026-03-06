<template>
  <div class="meta-row">
    <span class="meta-label">{{ label }}</span>
    <span class="meta-value" :class="{ small }">{{ value || '-' }}</span>
  </div>
</template>
<script setup lang="ts">
defineProps<{ label: string; value: string | number | null | undefined; small?: boolean }>()
</script>
<style scoped>
.meta-row { display: flex; align-items: flex-start; padding: 3px 12px; gap: 6px; }
.meta-label { color: #555; width: 56px; flex-shrink: 0; padding-top: 1px; font-size: 12px; }
.meta-value { color: #bbb; word-break: break-all; font-size: 12px; }
.meta-value.small { font-size: 10px; color: #666; }
</style>
