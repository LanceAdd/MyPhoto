<template>
  <Teleport to="body">
    <div class="export-overlay" @click.self="onClose">
      <div class="export-dialog">
        <div class="dialog-header">
          <h2>导出照片</h2>
          <button class="close-btn" @click="onClose">×</button>
        </div>

        <div class="dialog-body">
          <!-- Source -->
          <div class="section">
            <div class="section-title">导出范围</div>
            <div class="radio-group">
              <label><input type="radio" v-model="source" value="selected" /> 已选 {{ photoIds.length }} 张</label>
              <label><input type="radio" v-model="source" value="all" /> 全部 {{ totalCount }} 张</label>
              <label><input type="radio" v-model="source" value="starred" /> 有星级的照片</label>
            </div>
          </div>

          <!-- Format -->
          <div class="section">
            <div class="section-title">输出格式</div>
            <select v-model="format" class="sel">
              <option value="original">保持原格式</option>
              <option value="jpeg">JPEG</option>
              <option value="png">PNG</option>
              <option value="webp">WebP</option>
            </select>
          </div>

          <!-- Quality -->
          <div class="section" v-if="format === 'jpeg' || format === 'webp'">
            <div class="section-title">质量 {{ quality }}%</div>
            <input type="range" min="1" max="100" v-model.number="quality" class="slider" />
          </div>

          <!-- Max dimension -->
          <div class="section">
            <div class="section-title">最长边限制（px，0 = 不限）</div>
            <input type="number" v-model.number="maxDimension" class="inp" min="0" step="100" />
          </div>

          <!-- Naming -->
          <div class="section">
            <div class="section-title">文件命名</div>
            <select v-model="namingRule" class="sel">
              <option value="original">保持原文件名</option>
              <option value="date_seq">日期_序号 (YYYYMMDD_001)</option>
            </select>
          </div>

          <!-- Conflict -->
          <div class="section">
            <div class="section-title">同名冲突处理</div>
            <div class="radio-group">
              <label><input type="radio" v-model="conflictAction" value="skip" /> 跳过</label>
              <label><input type="radio" v-model="conflictAction" value="overwrite" /> 覆盖</label>
              <label><input type="radio" v-model="conflictAction" value="rename" /> 自动重命名</label>
            </div>
          </div>

          <!-- Destination -->
          <div class="section">
            <div class="section-title">输出目录</div>
            <div class="dest-row">
              <span class="dest-path">{{ destFolder || '未选择' }}</span>
              <button class="pick-btn" @click="pickFolder">选择...</button>
            </div>
          </div>

          <!-- Progress -->
          <div class="section" v-if="exporting">
            <div class="section-title">导出中... {{ progressCurrent }}/{{ progressTotal }}</div>
            <div class="progress-bar">
              <div class="progress-fill" :style="{ width: progressPercent + '%' }"></div>
            </div>
            <div class="progress-msg">{{ progressMsg }}</div>
          </div>
        </div>

        <div class="dialog-footer">
          <span class="error-msg" v-if="errorMsg">{{ errorMsg }}</span>
          <button class="cancel-btn" @click="onClose" :disabled="exporting">取消</button>
          <button class="export-btn" @click="startExport" :disabled="!canExport">
            {{ exporting ? '导出中...' : '开始导出' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { useWorkspaceStore } from '../stores/workspace'

const props = defineProps<{
  photoIds: number[]
  totalCount: number
}>()
const emit = defineEmits(['close'])

const wsStore = useWorkspaceStore()
const source = ref<'selected'|'all'|'starred'>('selected')
const format = ref('original')
const quality = ref(90)
const maxDimension = ref(0)
const namingRule = ref('original')
const conflictAction = ref<'skip'|'overwrite'|'rename'>('rename')
const destFolder = ref('')
const exporting = ref(false)
const progressCurrent = ref(0)
const progressTotal = ref(0)
const progressMsg = ref('')
const errorMsg = ref('')

const progressPercent = computed(() =>
  progressTotal.value > 0 ? Math.round(progressCurrent.value / progressTotal.value * 100) : 0
)

const canExport = computed(() => destFolder.value.length > 0 && !exporting.value)

async function pickFolder() {
  const result = await open({ directory: true, multiple: false })
  if (result && typeof result === 'string') destFolder.value = result
}

async function startExport() {
  if (!canExport.value) return
  exporting.value = true
  errorMsg.value = ''
  progressCurrent.value = 0
  progressTotal.value = 0

  const tab = wsStore.activeTab
  if (!tab) { exporting.value = false; return }

  // Determine IDs to export
  let ids: number[] = []
  if (source.value === 'selected') {
    ids = props.photoIds
  } else if (source.value === 'all') {
    ids = tab.photos.map(p => p.id)
  } else {
    ids = tab.photos.filter(p => (p.star_rating ?? 0) > 0).map(p => p.id)
  }

  const unlisten = await listen<{ current: number; total: number; current_file: string; done: boolean; error?: string }>(
    'export-progress', (ev) => {
      progressCurrent.value = ev.payload.current
      progressTotal.value = ev.payload.total
      progressMsg.value = ev.payload.current_file
      if (ev.payload.done) {
        exporting.value = false
        unlisten()
        if (ev.payload.error) {
          errorMsg.value = ev.payload.error
        } else {
          emit('close')
        }
      }
    }
  )

  try {
    await invoke('export_photos', {
      workspaceId: tab.workspace.id,
      photoIds: ids,
      options: {
        destination: destFolder.value,
        format: format.value,
        quality: quality.value,
        max_dimension: maxDimension.value > 0 ? maxDimension.value : null,
        naming_rule: namingRule.value,
        conflict_action: conflictAction.value,
      }
    })
  } catch (e: any) {
    errorMsg.value = String(e)
    exporting.value = false
    unlisten()
  }
}

function onClose() {
  if (!exporting.value) emit('close')
}
</script>

<style scoped>
.export-overlay {
  position: fixed; inset: 0; z-index: 800;
  background: rgba(0,0,0,0.65);
  display: flex; align-items: center; justify-content: center;
}
.export-dialog {
  background: #242424; border: 1px solid #333; border-radius: 8px;
  width: 480px; max-height: 85vh; display: flex; flex-direction: column;
  box-shadow: 0 20px 60px rgba(0,0,0,0.6);
}
.dialog-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 20px; border-bottom: 1px solid #333;
}
.dialog-header h2 { font-size: 16px; color: #eee; }
.close-btn {
  background: none; border: none; color: #666; cursor: pointer;
  font-size: 20px; width: 28px; height: 28px;
  display: flex; align-items: center; justify-content: center; border-radius: 4px;
}
.close-btn:hover { background: #333; color: #ddd; }

.dialog-body { flex: 1; overflow-y: auto; padding: 4px 20px; }
.section { padding: 12px 0; border-bottom: 1px solid #2a2a2a; }
.section:last-child { border-bottom: none; }
.section-title { font-size: 12px; color: #888; margin-bottom: 8px; }

.radio-group { display: flex; gap: 16px; }
.radio-group label { display: flex; align-items: center; gap: 5px; font-size: 13px; color: #ccc; cursor: pointer; }

.sel {
  background: #1a1a1a; border: 1px solid #333; color: #ccc;
  padding: 5px 10px; border-radius: 4px; font-size: 13px; width: 100%;
}
.sel:focus { outline: none; border-color: #4F8EF7; }

.inp {
  background: #1a1a1a; border: 1px solid #333; color: #ccc;
  padding: 5px 10px; border-radius: 4px; font-size: 13px; width: 140px;
}
.inp:focus { outline: none; border-color: #4F8EF7; }

.slider { width: 100%; accent-color: #4F8EF7; margin-top: 4px; }

.dest-row { display: flex; align-items: center; gap: 10px; }
.dest-path {
  flex: 1; font-size: 12px; color: #888; word-break: break-all;
  background: #1a1a1a; padding: 5px 8px; border-radius: 4px;
}
.pick-btn {
  background: #333; border: 1px solid #444; color: #ccc;
  padding: 5px 12px; border-radius: 4px; cursor: pointer; font-size: 13px;
  white-space: nowrap;
}
.pick-btn:hover { background: #3d3d3d; }

.progress-bar {
  height: 4px; background: #333; border-radius: 2px; overflow: hidden; margin: 6px 0;
}
.progress-fill {
  height: 100%; background: #4F8EF7; transition: width 0.2s;
}
.progress-msg { font-size: 11px; color: #666; }

.dialog-footer {
  display: flex; align-items: center; justify-content: flex-end; gap: 10px;
  padding: 14px 20px; border-top: 1px solid #2a2a2a;
}
.error-msg { flex: 1; font-size: 12px; color: #f77; }
.cancel-btn {
  background: #333; border: 1px solid #444; color: #ccc;
  padding: 7px 18px; border-radius: 4px; cursor: pointer; font-size: 13px;
}
.cancel-btn:hover:not(:disabled) { background: #3d3d3d; }
.export-btn {
  background: #4F8EF7; border: none; color: #fff;
  padding: 7px 20px; border-radius: 4px; cursor: pointer; font-size: 13px;
}
.export-btn:hover:not(:disabled) { background: #3d7de9; }
.export-btn:disabled, .cancel-btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
