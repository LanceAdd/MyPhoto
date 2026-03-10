<template>
  <Teleport to="body">
    <div class="export-overlay" @click.self="onClose">
      <div class="export-dialog">
        <div class="dialog-header">
          <h2>导出照片</h2>
          <button class="close-btn" @click="onClose">×</button>
        </div>

        <div class="dialog-body">
          <div class="section">
            <div class="section-title">导出范围</div>
            <div class="radio-group">
              <label><input type="radio" v-model="source" value="selected" /> 已选 {{ photoIds.length }} 张</label>
              <label><input type="radio" v-model="source" value="all" /> 全部 {{ totalCount }} 张</label>
              <label><input type="radio" v-model="source" value="starred" /> 仅有星级</label>
            </div>
          </div>

          <div class="section">
            <div class="section-title">输出格式</div>
            <select v-model="format" class="sel">
              <option value="original">保持原格式</option>
              <option value="jpeg">JPEG</option>
              <option value="png">PNG</option>
              <option value="webp">WebP</option>
            </select>
          </div>

          <div class="section" v-if="format === 'jpeg' || format === 'webp'">
            <div class="section-title">质量 {{ quality }}%</div>
            <input type="range" min="1" max="100" v-model.number="quality" class="slider" />
          </div>

          <div class="section">
            <div class="section-title">最长边限制（px，0 表示不限）</div>
            <input type="number" v-model.number="maxDimension" class="inp" min="0" step="100" />
          </div>

          <div class="section">
            <div class="section-title">文件命名</div>
            <select v-model="namingRule" class="sel">
              <option value="original">保持原文件名</option>
              <option value="date_seq">日期_序号 (YYYYMMDD_001)</option>
            </select>
          </div>

          <div class="section">
            <div class="section-title">同名冲突处理</div>
            <div class="radio-group">
              <label><input type="radio" v-model="conflictAction" value="skip" /> 跳过</label>
              <label><input type="radio" v-model="conflictAction" value="overwrite" /> 覆盖</label>
              <label><input type="radio" v-model="conflictAction" value="rename" /> 自动重命名</label>
            </div>
          </div>

          <div class="section" v-if="conflictAction === 'rename'">
            <div class="section-title">自动重命名规则</div>
            <div class="rename-grid">
              <label class="rename-item">
                <span>前缀</span>
                <input v-model="renamePrefix" class="inp rename-input" placeholder="例如：EXP_" />
              </label>
              <label class="rename-item">
                <span>后半部分生成方式</span>
                <select v-model="renameSuffixMode" class="sel">
                  <option value="seq">序号（_001）</option>
                  <option value="date_seq">日期+序号（_YYYYMMDD_001）</option>
                  <option value="timestamp_seq">时间戳+序号（_YYYYMMDD_HHMMSS_001）</option>
                </select>
              </label>
            </div>
          </div>

          <div class="section">
            <div class="section-title">输出目录</div>
            <div class="dest-row">
              <span class="dest-path">{{ destFolder || '未选择' }}</span>
              <button class="pick-btn" @click="pickFolder">选择...</button>
            </div>
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

    <div v-if="showProgressPopup" class="progress-popup-overlay">
      <div class="progress-popup">
        <div class="progress-popup-header">
          <h3>导出进度</h3>
          <button class="close-btn" @click="closeProgressPopup" :disabled="exporting">×</button>
        </div>

        <div class="progress-popup-body">
          <div class="progress-numbers">{{ progressCurrent }} / {{ progressDisplayTotal }}</div>
          <div class="progress-bar progress-bar-large">
            <div class="progress-fill" :style="{ width: progressPercent + '%' }"></div>
          </div>
          <div class="progress-msg">{{ progressMsg || (exporting ? '准备导出...' : '导出完成') }}</div>

          <div v-if="exportSummary" class="summary-grid">
            <div class="summary-item">
              <span>总数</span>
              <strong>{{ exportSummary.total }}</strong>
            </div>
            <div class="summary-item">
              <span>导出成功</span>
              <strong>{{ exportSummary.exported }}</strong>
            </div>
            <div class="summary-item">
              <span>跳过</span>
              <strong>{{ exportSummary.skipped }}</strong>
            </div>
            <div class="summary-item">
              <span>失败</span>
              <strong>{{ exportSummary.failed }}</strong>
            </div>
          </div>
        </div>

        <div class="progress-popup-footer">
          <button class="cancel-btn" @click="closeProgressPopup" :disabled="exporting">关闭进度</button>
          <button class="export-btn" @click="closeAfterExport" :disabled="exporting">关闭窗口</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { useWorkspaceStore } from '../stores/workspace'

interface ExportProgressPayload {
  done: number
  total: number
  current_file: string
  finished: boolean
}

interface ExportSummary {
  total: number
  exported: number
  skipped: number
  failed: number
}

const props = defineProps<{
  photoIds: number[]
  totalCount: number
}>()
const emit = defineEmits(['close'])

const wsStore = useWorkspaceStore()
const source = ref<'selected' | 'all' | 'starred'>('selected')
const format = ref('original')
const quality = ref(90)
const maxDimension = ref(0)
const namingRule = ref('original')
const conflictAction = ref<'skip' | 'overwrite' | 'rename'>('rename')
const renamePrefix = ref('')
const renameSuffixMode = ref<'seq' | 'date_seq' | 'timestamp_seq'>('seq')
const destFolder = ref('')
const exporting = ref(false)
const showProgressPopup = ref(false)
const progressCurrent = ref(0)
const progressTotal = ref(0)
const progressExpected = ref(0)
const progressMsg = ref('')
const errorMsg = ref('')
const exportSummary = ref<ExportSummary | null>(null)

let progressUnlisten: null | (() => void) = null

const progressDisplayTotal = computed(() =>
  progressTotal.value > 0 ? progressTotal.value : progressExpected.value
)

const progressPercent = computed(() => {
  const total = progressDisplayTotal.value
  if (total <= 0) return 0
  return Math.min(100, Math.round((progressCurrent.value / total) * 100))
})

const canExport = computed(() => destFolder.value.length > 0 && !exporting.value)

function cleanupProgressListener() {
  if (progressUnlisten) {
    progressUnlisten()
    progressUnlisten = null
  }
}

async function pickFolder() {
  const result = await open({ directory: true, multiple: false })
  if (result && typeof result === 'string') destFolder.value = result
}

async function startExport() {
  if (!canExport.value) return

  exporting.value = true
  showProgressPopup.value = true
  errorMsg.value = ''
  exportSummary.value = null
  progressCurrent.value = 0
  progressTotal.value = 0
  progressMsg.value = ''

  const tab = wsStore.activeTab
  if (!tab) {
    exporting.value = false
    return
  }

  let ids: number[] = []
  if (source.value === 'selected') {
    ids = props.photoIds
  } else if (source.value === 'all') {
    ids = tab.photos.map(p => p.id)
  } else {
    ids = tab.photos.filter(p => (p.star_rating ?? 0) > 0).map(p => p.id)
  }

  progressExpected.value = ids.length

  cleanupProgressListener()
  progressUnlisten = await listen<ExportProgressPayload>('export-progress', ev => {
    progressCurrent.value = ev.payload.done
    progressTotal.value = ev.payload.total
    progressMsg.value = ev.payload.current_file

    if (ev.payload.finished) {
      exporting.value = false
      cleanupProgressListener()
      if (progressTotal.value <= 0) {
        progressTotal.value = progressExpected.value
      }
    }
  })

  try {
    const exportedCount = await invoke<number>('export_photos', {
      workspacePath: tab.workspace.path,
      options: {
        photo_ids: ids,
        dest_folder: destFolder.value,
        format: format.value,
        quality: quality.value,
        max_dimension: maxDimension.value > 0 ? maxDimension.value : null,
        naming_rule: namingRule.value,
        conflict: conflictAction.value,
        rename_prefix: conflictAction.value === 'rename' && renamePrefix.value.trim().length > 0
          ? renamePrefix.value
          : null,
        rename_suffix_mode: conflictAction.value === 'rename' ? renameSuffixMode.value : null,
      }
    })

    const total = ids.length
    const exported = Math.max(0, Math.min(total, Number(exportedCount ?? 0)))
    const skipped = Math.max(0, total - exported)
    exportSummary.value = { total, exported, skipped, failed: 0 }
    if (progressTotal.value <= 0) progressTotal.value = total
    if (progressCurrent.value < exported) progressCurrent.value = exported
    exporting.value = false
    cleanupProgressListener()
  } catch (e: any) {
    errorMsg.value = String(e)
    const total = progressExpected.value
    const exported = Math.max(0, Math.min(total, progressCurrent.value))
    const failed = Math.max(0, total - exported)
    exportSummary.value = { total, exported, skipped: 0, failed }
    if (progressTotal.value <= 0) progressTotal.value = total
    exporting.value = false
    cleanupProgressListener()
  }
}

function closeProgressPopup() {
  if (exporting.value) return
  showProgressPopup.value = false
}

function closeAfterExport() {
  if (exporting.value) return
  showProgressPopup.value = false
  emit('close')
}

function onClose() {
  if (exporting.value) return
  cleanupProgressListener()
  emit('close')
}

onBeforeUnmount(() => {
  cleanupProgressListener()
})
</script>

<style scoped>
.export-overlay {
  position: fixed;
  inset: 0;
  z-index: 800;
  background: rgba(0, 0, 0, 0.65);
  display: flex;
  align-items: center;
  justify-content: center;
}

.export-dialog {
  background: #242424;
  border: 1px solid #333;
  border-radius: 8px;
  width: 520px;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid #333;
}

.dialog-header h2 {
  font-size: 16px;
  color: #eee;
}

.close-btn {
  background: none;
  border: none;
  color: #666;
  cursor: pointer;
  font-size: 20px;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
}

.close-btn:hover:not(:disabled) {
  background: #333;
  color: #ddd;
}

.close-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.dialog-body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 20px;
}

.section {
  padding: 12px 0;
  border-bottom: 1px solid #2a2a2a;
}

.section:last-child {
  border-bottom: none;
}

.section-title {
  font-size: 12px;
  color: #888;
  margin-bottom: 8px;
}

.radio-group {
  display: flex;
  gap: 16px;
}

.radio-group label {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 13px;
  color: #ccc;
  cursor: pointer;
}

.sel {
  background: #1a1a1a;
  border: 1px solid #333;
  color: #ccc;
  padding: 5px 10px;
  border-radius: 4px;
  font-size: 13px;
  width: 100%;
}

.sel:focus {
  outline: none;
  border-color: #4f8ef7;
}

.inp {
  background: #1a1a1a;
  border: 1px solid #333;
  color: #ccc;
  padding: 5px 10px;
  border-radius: 4px;
  font-size: 13px;
  width: 140px;
}

.inp:focus {
  outline: none;
  border-color: #4f8ef7;
}

.rename-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
}

.rename-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
  color: #a6a6a6;
}

.rename-input {
  width: 100%;
}

.slider {
  width: 100%;
  accent-color: #4f8ef7;
  margin-top: 4px;
}

.dest-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.dest-path {
  flex: 1;
  font-size: 12px;
  color: #888;
  word-break: break-all;
  background: #1a1a1a;
  padding: 5px 8px;
  border-radius: 4px;
}

.pick-btn {
  background: #333;
  border: 1px solid #444;
  color: #ccc;
  padding: 5px 12px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  white-space: nowrap;
}

.pick-btn:hover {
  background: #3d3d3d;
}

.progress-bar {
  height: 6px;
  background: #333;
  border-radius: 3px;
  overflow: hidden;
  margin: 8px 0;
}

.progress-bar-large {
  height: 10px;
  border-radius: 5px;
}

.progress-fill {
  height: 100%;
  background: #4f8ef7;
  transition: width 0.2s;
}

.progress-msg {
  font-size: 12px;
  color: #999;
  min-height: 18px;
}

.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 20px;
  border-top: 1px solid #2a2a2a;
}

.error-msg {
  flex: 1;
  font-size: 12px;
  color: #f77;
}

.cancel-btn {
  background: #333;
  border: 1px solid #444;
  color: #ccc;
  padding: 7px 18px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}

.cancel-btn:hover:not(:disabled) {
  background: #3d3d3d;
}

.export-btn {
  background: #4f8ef7;
  border: none;
  color: #fff;
  padding: 7px 20px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}

.export-btn:hover:not(:disabled) {
  background: #3d7de9;
}

.export-btn:disabled,
.cancel-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.progress-popup-overlay {
  position: fixed;
  inset: 0;
  z-index: 920;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
}

.progress-popup {
  width: min(540px, calc(100vw - 40px));
  background: #1f1f1f;
  border: 1px solid #3a3a3a;
  border-radius: 10px;
  box-shadow: 0 24px 80px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
}

.progress-popup-header,
.progress-popup-footer {
  padding: 14px 16px;
  border-bottom: 1px solid #323232;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.progress-popup-footer {
  border-bottom: none;
  border-top: 1px solid #323232;
  justify-content: flex-end;
  gap: 10px;
}

.progress-popup-header h3 {
  margin: 0;
  font-size: 16px;
  color: #ececec;
}

.progress-popup-body {
  padding: 16px;
}

.progress-numbers {
  font-size: 18px;
  font-weight: 600;
  color: #e9e9e9;
  margin-bottom: 8px;
}

.summary-grid {
  margin-top: 12px;
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.summary-item {
  background: #262626;
  border: 1px solid #343434;
  border-radius: 8px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.summary-item span {
  font-size: 11px;
  color: #9a9a9a;
}

.summary-item strong {
  font-size: 16px;
  color: #f2f2f2;
}
</style>