<template>
  <div class="app-container">
    <!-- Title Bar -->
    <div class="title-bar">
      <div class="title-bar-content">
        <img src="/icon.png" class="app-icon" alt="">
        <span class="app-title">Moses Drive Formatter</span>
      </div>
      <div class="title-bar-controls">
        <button class="title-btn minimize">─</button>
        <button class="title-btn maximize">□</button>
        <button class="title-btn close">✕</button>
      </div>
    </div>

    <!-- Menu Bar -->
    <div class="menu-bar">
      <button class="menu-item">File</button>
      <button class="menu-item">Edit</button>
      <button class="menu-item">View</button>
      <button class="menu-item">Tools</button>
      <button class="menu-item">Help</button>
    </div>

    <!-- Toolbar -->
    <div class="toolbar">
      <button class="tool-btn" @click="refreshDevices" :disabled="isRefreshing">
        <span class="tool-icon">🔄</span>
        <span class="tool-label">Refresh</span>
      </button>
      <div class="toolbar-separator"></div>
      <button class="tool-btn" @click="simulateFormat" :disabled="!canSimulate">
        <span class="tool-icon">📋</span>
        <span class="tool-label">Simulate</span>
      </button>
      <button class="tool-btn danger" @click="executeFormat" :disabled="!canFormat">
        <span class="tool-icon">💾</span>
        <span class="tool-label">Format</span>
      </button>
      <div class="toolbar-separator"></div>
      <button class="tool-btn" disabled>
        <span class="tool-icon">⚙️</span>
        <span class="tool-label">Options</span>
      </button>
    </div>

    <!-- Main Content Area -->
    <div class="main-content">
      <!-- Left Pane - Drive List -->
      <div class="left-pane">
        <div class="pane-header">
          <span>Drives</span>
          <span class="drive-count">({{ devices.length }})</span>
        </div>
        
        <div class="drive-tree">
          <div v-if="loading" class="loading-state">
            <div class="spinner-small"></div>
            Scanning drives...
          </div>
          
          <div v-else-if="devices.length === 0" class="empty-state">
            No drives detected
          </div>
          
          <div v-else class="drive-list">
            <!-- System Drives Group -->
            <div v-if="systemDrives.length > 0" class="drive-group">
              <div class="group-header">
                <span class="expand-icon">▼</span>
                <span class="group-icon">💻</span>
                System Drives
              </div>
              <div class="group-items">
                <div 
                  v-for="device in systemDrives" 
                  :key="device.id"
                  :class="['drive-item', { 
                    selected: selectedDevice?.id === device.id,
                    disabled: device.is_system 
                  }]"
                  @click="selectDevice(device)"
                >
                  <span class="drive-icon">{{ getDeviceIcon(device.device_type) }}</span>
                  <div class="drive-details">
                    <div class="drive-name">{{ device.name }}</div>
                    <div class="drive-info">
                      <span class="drive-size">{{ formatSize(device.size) }}</span>
                      <span class="drive-type">{{ device.device_type }}</span>
                    </div>
                    <div class="drive-bar">
                      <div class="drive-bar-fill system"></div>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            <!-- Removable Drives Group -->
            <div v-if="removableDrives.length > 0" class="drive-group">
              <div class="group-header">
                <span class="expand-icon">▼</span>
                <span class="group-icon">🔌</span>
                Removable Drives
              </div>
              <div class="group-items">
                <div 
                  v-for="device in removableDrives" 
                  :key="device.id"
                  :class="['drive-item', { 
                    selected: selectedDevice?.id === device.id 
                  }]"
                  @click="selectDevice(device)"
                >
                  <span class="drive-icon">{{ getDeviceIcon(device.device_type) }}</span>
                  <div class="drive-details">
                    <div class="drive-name">{{ device.name }}</div>
                    <div class="drive-info">
                      <span class="drive-size">{{ formatSize(device.size) }}</span>
                      <span class="drive-type">{{ device.device_type }}</span>
                    </div>
                    <div class="drive-bar">
                      <div class="drive-bar-fill removable"></div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Splitter -->
      <div class="splitter"></div>

      <!-- Right Pane - Details/Actions -->
      <div class="right-pane">
        <!-- No Selection State -->
        <div v-if="!selectedDevice" class="no-selection">
          <div class="no-selection-icon">💾</div>
          <div class="no-selection-text">Select a drive to format</div>
          <div class="no-selection-hint">Choose a removable drive from the list</div>
        </div>

        <!-- System Drive Warning -->
        <div v-else-if="selectedDevice.is_system" class="system-warning">
          <div class="warning-icon">🛡️</div>
          <div class="warning-title">System Drive Protected</div>
          <div class="warning-text">
            This drive contains your operating system and cannot be formatted.
            Moses protects system drives to prevent accidental data loss.
          </div>
        </div>

        <!-- Format Options -->
        <div v-else class="format-panel">
          <!-- Drive Info Panel -->
          <div class="info-panel">
            <div class="panel-title">Drive Information</div>
            <div class="info-grid">
              <div class="info-label">Name:</div>
              <div class="info-value">{{ selectedDevice.name }}</div>
              
              <div class="info-label">Type:</div>
              <div class="info-value">{{ selectedDevice.device_type }}</div>
              
              <div class="info-label">Size:</div>
              <div class="info-value">{{ formatSize(selectedDevice.size) }}</div>
              
              <div class="info-label">Path:</div>
              <div class="info-value">{{ selectedDevice.id }}</div>
              
              <div class="info-label">Status:</div>
              <div class="info-value">
                <span v-if="selectedDevice.is_removable" class="status-badge removable">Removable</span>
                <span v-else class="status-badge fixed">Fixed</span>
              </div>
            </div>
          </div>

          <!-- Format Options Panel -->
          <div class="options-panel">
            <div class="panel-title">Format Options</div>
            
            <div class="option-group">
              <label class="option-label">File System:</label>
              <select v-model="formatOptions.filesystem_type" class="option-select">
                <option value="">-- Select File System --</option>
                <optgroup label="Universal">
                  <option value="exfat">exFAT (Recommended)</option>
                  <option value="fat32">FAT32 (Legacy)</option>
                </optgroup>
                <optgroup label="Platform Specific">
                  <option value="ntfs">NTFS (Windows)</option>
                  <option value="ext4">EXT4 (Linux)</option>
                </optgroup>
              </select>
            </div>

            <div class="option-group">
              <label class="option-label">Volume Label:</label>
              <input 
                v-model="formatOptions.label" 
                type="text" 
                class="option-input"
                :placeholder="labelPlaceholder"
                :maxlength="maxLabelLength"
              >
              <div class="option-hint">{{ labelHint }}</div>
            </div>

            <div class="option-group">
              <label class="option-checkbox">
                <input type="checkbox" v-model="formatOptions.quick_format">
                <span>Quick Format</span>
              </label>
              <div class="option-hint">Performs a quick format (faster but less thorough)</div>
            </div>

            <div class="option-group">
              <label class="option-checkbox">
                <input type="checkbox" v-model="formatOptions.create_partition" checked disabled>
                <span>Create New Partition Table</span>
              </label>
              <div class="option-hint">Creates a new partition table (GPT/MBR)</div>
            </div>
          </div>

          <!-- Progress Panel (when formatting) -->
          <div v-if="isFormatting" class="progress-panel">
            <div class="panel-title">Format Progress</div>
            <div class="progress-container">
              <div class="progress-bar-native">
                <div class="progress-fill-native" :style="{ width: formatProgress + '%' }">
                  <div class="progress-shine"></div>
                </div>
              </div>
              <div class="progress-info">
                <span class="progress-percent">{{ formatProgress }}%</span>
                <span class="progress-status">{{ progressStatus }}</span>
              </div>
            </div>
            <div class="progress-details">
              <div>Time elapsed: {{ formatTime }}</div>
              <div>Current operation: {{ currentOperation }}</div>
            </div>
          </div>

          <!-- Simulation Results -->
          <div v-else-if="simulationReport" class="simulation-panel">
            <div class="panel-title">Simulation Results</div>
            <div class="simulation-content">
              <div class="sim-item">
                <span class="sim-icon">⏱️</span>
                <span class="sim-label">Estimated Time:</span>
                <span class="sim-value">{{ formatDuration(simulationReport.estimated_time) }}</span>
              </div>
              <div class="sim-item">
                <span class="sim-icon">💾</span>
                <span class="sim-label">Space After Format:</span>
                <span class="sim-value">{{ formatSize(simulationReport.space_after_format) }}</span>
              </div>
              <div v-if="simulationReport.warnings.length > 0" class="sim-warnings">
                <div class="sim-warning-title">⚠️ Warnings:</div>
                <ul class="sim-warning-list">
                  <li v-for="(warning, i) in simulationReport.warnings" :key="i">
                    {{ warning }}
                  </li>
                </ul>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Status Bar -->
    <div class="status-bar">
      <div class="status-section">
        <span v-if="selectedDevice">
          Selected: {{ selectedDevice.name }}
        </span>
        <span v-else>
          No drive selected
        </span>
      </div>
      <div class="status-separator"></div>
      <div class="status-section">
        {{ devices.length }} drive(s) detected
      </div>
      <div class="status-separator"></div>
      <div class="status-section">
        <span v-if="isFormatting" class="status-busy">
          Formatting in progress...
        </span>
        <span v-else class="status-ready">
          Ready
        </span>
      </div>
      <div class="status-grip"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

interface Device {
  id: string
  name: string
  size: number
  device_type: string
  mount_points: string[]
  is_removable: boolean
  is_system: boolean
}

interface FormatOptions {
  filesystem_type: string
  label: string
  quick_format: boolean
  create_partition: boolean
}

interface SimulationReport {
  estimated_time: number
  warnings: string[]
  required_tools: string[]
  space_after_format: number
}

// State
const devices = ref<Device[]>([])
const selectedDevice = ref<Device | null>(null)
const loading = ref(false)
const isRefreshing = ref(false)
const isFormatting = ref(false)
const simulationReport = ref<SimulationReport | null>(null)
const formatProgress = ref(0)
const progressStatus = ref('')
const formatTime = ref('00:00')
const currentOperation = ref('')

const formatOptions = ref<FormatOptions>({
  filesystem_type: '',
  label: '',
  quick_format: true,
  create_partition: true
})

// Computed
const systemDrives = computed(() => 
  devices.value.filter(d => d.is_system)
)

const removableDrives = computed(() => 
  devices.value.filter(d => !d.is_system)
)

const canSimulate = computed(() => 
  selectedDevice.value && 
  !selectedDevice.value.is_system && 
  formatOptions.value.filesystem_type
)

const canFormat = computed(() => 
  canSimulate.value && 
  simulationReport.value && 
  !isFormatting.value
)

const maxLabelLength = computed(() => {
  switch (formatOptions.value.filesystem_type) {
    case 'fat32': return 11
    case 'exfat': return 15
    case 'ntfs': return 32
    case 'ext4': return 16
    default: return 32
  }
})

const labelPlaceholder = computed(() => {
  switch (formatOptions.value.filesystem_type) {
    case 'fat32': return 'MAX 11 CHARS'
    case 'exfat': return 'Volume Label'
    case 'ntfs': return 'Volume Label'
    case 'ext4': return 'Volume Label'
    default: return 'Select filesystem first'
  }
})

const labelHint = computed(() => {
  switch (formatOptions.value.filesystem_type) {
    case 'fat32': return 'FAT32: Maximum 11 characters, uppercase only'
    case 'exfat': return 'exFAT: Maximum 15 characters'
    case 'ntfs': return 'NTFS: Maximum 32 characters'
    case 'ext4': return 'EXT4: Maximum 16 characters, native support'
    default: return 'Select a filesystem to see label requirements'
  }
})

// Methods
const getDeviceIcon = (type: string) => {
  const icons: Record<string, string> = {
    'USB': '🔌',
    'HardDisk': '💾',
    'SSD': '💿',
    'SDCard': '🗂️',
    'Virtual': '📦',
    'Unknown': '❓'
  }
  return icons[type] || '💾'
}

const formatSize = (bytes: number): string => {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let size = bytes
  let unitIndex = 0
  
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024
    unitIndex++
  }
  
  return `${size.toFixed(2)} ${units[unitIndex]}`
}

const formatDuration = (seconds: number): string => {
  if (seconds < 60) return `${seconds} seconds`
  const minutes = Math.floor(seconds / 60)
  const secs = seconds % 60
  return secs > 0 ? `${minutes}m ${secs}s` : `${minutes} minute${minutes !== 1 ? 's' : ''}`
}

const selectDevice = (device: Device) => {
  if (isFormatting.value) return
  selectedDevice.value = device
  simulationReport.value = null
}

const refreshDevices = async () => {
  if (isFormatting.value) return
  
  isRefreshing.value = true
  loading.value = true
  
  try {
    devices.value = await invoke('enumerate_devices')
  } catch (error) {
    console.error('Failed to enumerate devices:', error)
    alert(`Failed to scan drives: ${error}`)
  } finally {
    loading.value = false
    setTimeout(() => {
      isRefreshing.value = false
    }, 300)
  }
}

const simulateFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  try {
    simulationReport.value = await invoke('simulate_format', {
      device: selectedDevice.value,
      options: formatOptions.value
    })
  } catch (error) {
    console.error('Simulation failed:', error)
    alert(`Simulation failed: ${error}`)
  }
}

const executeFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  const confirmMsg = `WARNING: This will permanently erase all data on ${selectedDevice.value.name}.\n\nAre you sure you want to continue?`
  
  if (!confirm(confirmMsg)) return
  
  isFormatting.value = true
  formatProgress.value = 0
  progressStatus.value = 'Initializing...'
  currentOperation.value = 'Preparing device'
  
  // Simulate progress
  const startTime = Date.now()
  const progressInterval = setInterval(() => {
    const elapsed = Math.floor((Date.now() - startTime) / 1000)
    const mins = Math.floor(elapsed / 60)
    const secs = elapsed % 60
    formatTime.value = `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`
    
    if (formatProgress.value < 90) {
      formatProgress.value += Math.random() * 10
      if (formatProgress.value > 90) formatProgress.value = 90
      
      if (formatProgress.value < 20) {
        currentOperation.value = 'Unmounting device'
        progressStatus.value = 'Preparing...'
      } else if (formatProgress.value < 40) {
        currentOperation.value = 'Creating partition table'
        progressStatus.value = 'Partitioning...'
      } else if (formatProgress.value < 60) {
        currentOperation.value = 'Writing filesystem structures'
        progressStatus.value = 'Formatting...'
      } else if (formatProgress.value < 80) {
        currentOperation.value = 'Verifying filesystem'
        progressStatus.value = 'Verifying...'
      } else {
        currentOperation.value = 'Finalizing'
        progressStatus.value = 'Almost done...'
      }
    }
  }, 500)
  
  try {
    await invoke('execute_format', {
      device: selectedDevice.value,
      options: formatOptions.value
    })
    
    clearInterval(progressInterval)
    formatProgress.value = 100
    progressStatus.value = 'Complete!'
    currentOperation.value = 'Format successful'
    
    setTimeout(() => {
      alert(`Successfully formatted ${selectedDevice.value?.name}`)
      isFormatting.value = false
      formatProgress.value = 0
      progressStatus.value = ''
      currentOperation.value = ''
      simulationReport.value = null
      refreshDevices()
    }, 1500)
    
  } catch (error) {
    clearInterval(progressInterval)
    console.error('Format failed:', error)
    alert(`Format failed: ${error}`)
    
    isFormatting.value = false
    formatProgress.value = 0
    progressStatus.value = ''
    currentOperation.value = ''
  }
}

onMounted(() => {
  refreshDevices()
})
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  font-size: 12px;
  color: #000;
  background: #f0f0f0;
  overflow: hidden;
  user-select: none;
}

.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: #ece9d8;
}

/* Title Bar */
.title-bar {
  height: 30px;
  background: linear-gradient(to bottom, #0054e3, #0046c7);
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 6px;
  color: white;
}

.title-bar-content {
  display: flex;
  align-items: center;
  gap: 8px;
}

.app-icon {
  width: 16px;
  height: 16px;
}

.app-title {
  font-weight: 600;
  font-size: 13px;
}

.title-bar-controls {
  display: flex;
  gap: 2px;
}

.title-btn {
  width: 26px;
  height: 22px;
  background: linear-gradient(to bottom, #fff, #e0e0e0);
  border: 1px solid #8492a6;
  border-radius: 3px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  cursor: pointer;
}

.title-btn:hover {
  background: linear-gradient(to bottom, #e5f1fb, #c7e0f4);
}

.title-btn.close:hover {
  background: linear-gradient(to bottom, #ffb4b4, #ff6b6b);
}

/* Menu Bar */
.menu-bar {
  height: 22px;
  background: #f0f0f0;
  display: flex;
  align-items: center;
  border-bottom: 1px solid #d4d0c8;
  padding: 0 4px;
}

.menu-item {
  padding: 2px 12px;
  background: none;
  border: none;
  cursor: pointer;
  font-size: 11px;
}

.menu-item:hover {
  background: #316ac5;
  color: white;
}

/* Toolbar */
.toolbar {
  height: 38px;
  background: linear-gradient(to bottom, #fefefe, #e3e3e3);
  border-bottom: 1px solid #aca899;
  display: flex;
  align-items: center;
  padding: 0 4px;
  gap: 4px;
}

.tool-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 3px 6px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 3px;
  cursor: pointer;
  min-width: 48px;
}

.tool-btn:hover:not(:disabled) {
  background: linear-gradient(to bottom, #fff, #e5f1fb);
  border-color: #7da7d9;
}

.tool-btn:active:not(:disabled) {
  background: linear-gradient(to bottom, #e5f1fb, #c7e0f4);
}

.tool-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.tool-btn.danger {
  color: #a00;
}

.tool-icon {
  font-size: 16px;
}

.tool-label {
  font-size: 11px;
  margin-top: 2px;
}

.toolbar-separator {
  width: 1px;
  height: 24px;
  background: #aca899;
  margin: 0 2px;
}

/* Main Content */
.main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  background: white;
  border: 1px solid #848484;
  margin: 4px;
}

/* Left Pane */
.left-pane {
  width: 280px;
  display: flex;
  flex-direction: column;
  background: white;
  border-right: 1px solid #aca899;
}

.pane-header {
  height: 24px;
  background: linear-gradient(to bottom, #fefefe, #e3e3e3);
  border-bottom: 1px solid #aca899;
  display: flex;
  align-items: center;
  padding: 0 8px;
  font-weight: 600;
}

.drive-count {
  margin-left: 4px;
  color: #666;
  font-weight: normal;
}

.drive-tree {
  flex: 1;
  overflow-y: auto;
  padding: 4px;
}

.loading-state, .empty-state {
  padding: 20px;
  text-align: center;
  color: #666;
}

.spinner-small {
  width: 16px;
  height: 16px;
  border: 2px solid #e0e0e0;
  border-top-color: #316ac5;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin: 0 auto 8px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Drive Groups */
.drive-group {
  margin-bottom: 8px;
}

.group-header {
  display: flex;
  align-items: center;
  padding: 4px;
  cursor: pointer;
  font-weight: 600;
}

.group-header:hover {
  background: #f0f8ff;
}

.expand-icon {
  width: 16px;
  font-size: 10px;
}

.group-icon {
  margin: 0 4px;
}

.group-items {
  padding-left: 20px;
}

/* Drive Items */
.drive-item {
  display: flex;
  align-items: flex-start;
  padding: 6px;
  cursor: pointer;
  border: 1px solid transparent;
  margin: 2px 0;
}

.drive-item:hover {
  background: #e8f4fd;
  border-color: #7da7d9;
}

.drive-item.selected {
  background: #316ac5;
  color: white;
}

.drive-item.disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.drive-icon {
  font-size: 20px;
  margin-right: 8px;
  margin-top: 2px;
}

.drive-details {
  flex: 1;
  min-width: 0;
}

.drive-name {
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.drive-info {
  display: flex;
  gap: 8px;
  font-size: 11px;
  margin-top: 2px;
  opacity: 0.8;
}

.drive-item.selected .drive-info {
  opacity: 0.9;
}

.drive-bar {
  height: 8px;
  background: #e0e0e0;
  border: 1px solid #999;
  margin-top: 4px;
  position: relative;
}

.drive-bar-fill {
  height: 100%;
  position: absolute;
  left: 0;
  top: 0;
}

.drive-bar-fill.system {
  width: 75%;
  background: linear-gradient(to bottom, #ff6b6b, #dc3545);
}

.drive-bar-fill.removable {
  width: 10%;
  background: linear-gradient(to bottom, #28a745, #218838);
}

/* Splitter */
.splitter {
  width: 3px;
  background: #aca899;
  cursor: ew-resize;
}

.splitter:hover {
  background: #7da7d9;
}

/* Right Pane */
.right-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  background: #f0f0f0;
}

/* No Selection */
.no-selection, .system-warning {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  text-align: center;
}

.no-selection-icon, .warning-icon {
  font-size: 64px;
  margin-bottom: 16px;
  opacity: 0.3;
}

.warning-icon {
  opacity: 1;
}

.no-selection-text, .warning-title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
}

.warning-title {
  color: #a00;
}

.no-selection-hint, .warning-text {
  color: #666;
  max-width: 300px;
}

/* Panels */
.format-panel {
  padding: 12px;
}

.info-panel, .options-panel, .progress-panel, .simulation-panel {
  background: white;
  border: 1px solid #aca899;
  margin-bottom: 12px;
}

.panel-title {
  background: linear-gradient(to bottom, #fefefe, #e3e3e3);
  border-bottom: 1px solid #aca899;
  padding: 4px 8px;
  font-weight: 600;
  font-size: 11px;
}

/* Info Panel */
.info-grid {
  padding: 8px;
  display: grid;
  grid-template-columns: 80px 1fr;
  gap: 4px 8px;
}

.info-label {
  font-weight: 600;
  text-align: right;
}

.info-value {
  word-break: break-all;
}

.status-badge {
  padding: 1px 6px;
  border-radius: 2px;
  font-size: 11px;
}

.status-badge.removable {
  background: #d4edda;
  color: #155724;
  border: 1px solid #c3e6cb;
}

.status-badge.fixed {
  background: #f8d7da;
  color: #721c24;
  border: 1px solid #f5c6cb;
}

/* Options Panel */
.options-panel {
  padding-bottom: 8px;
}

.option-group {
  padding: 8px;
  border-bottom: 1px solid #e0e0e0;
}

.option-group:last-child {
  border-bottom: none;
}

.option-label {
  display: block;
  font-weight: 600;
  margin-bottom: 4px;
}

.option-select, .option-input {
  width: 100%;
  padding: 3px 4px;
  border: 1px solid #7f9db9;
  background: white;
  font-size: 11px;
}

.option-select:focus, .option-input:focus {
  outline: none;
  border-color: #316ac5;
}

.option-checkbox {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.option-checkbox input {
  margin-right: 6px;
}

.option-hint {
  font-size: 10px;
  color: #666;
  margin-top: 2px;
  margin-left: 18px;
}

/* Progress Panel */
.progress-container {
  padding: 12px;
}

.progress-bar-native {
  height: 20px;
  background: #e0e0e0;
  border: 1px solid #7f9db9;
  position: relative;
  overflow: hidden;
}

.progress-fill-native {
  height: 100%;
  background: linear-gradient(to right, 
    #316ac5 0%, #316ac5 25%, 
    #5b9bd5 25%, #5b9bd5 50%,
    #316ac5 50%, #316ac5 75%,
    #5b9bd5 75%, #5b9bd5 100%);
  background-size: 30px 100%;
  animation: progress-move 1s linear infinite;
  position: relative;
}

@keyframes progress-move {
  0% { background-position: 0 0; }
  100% { background-position: 30px 0; }
}

.progress-shine {
  position: absolute;
  top: 0;
  left: -100%;
  width: 100%;
  height: 100%;
  background: linear-gradient(90deg, 
    transparent, 
    rgba(255,255,255,0.3), 
    transparent);
  animation: shine 2s infinite;
}

@keyframes shine {
  100% { left: 100%; }
}

.progress-info {
  display: flex;
  justify-content: space-between;
  margin-top: 8px;
  font-size: 11px;
}

.progress-percent {
  font-weight: 600;
}

.progress-details {
  padding: 8px 12px;
  background: #f8f8f8;
  border-top: 1px solid #e0e0e0;
  font-size: 11px;
  color: #666;
}

/* Simulation Panel */
.simulation-content {
  padding: 12px;
}

.sim-item {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
}

.sim-icon {
  font-size: 16px;
  margin-right: 8px;
}

.sim-label {
  font-weight: 600;
  margin-right: 8px;
}

.sim-warnings {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #e0e0e0;
}

.sim-warning-title {
  font-weight: 600;
  margin-bottom: 6px;
  color: #a00;
}

.sim-warning-list {
  margin-left: 20px;
  font-size: 11px;
  color: #666;
}

/* Status Bar */
.status-bar {
  height: 24px;
  background: #ece9d8;
  border-top: 1px solid #fff;
  display: flex;
  align-items: center;
  padding: 0 4px;
  font-size: 11px;
}

.status-section {
  padding: 0 8px;
  display: flex;
  align-items: center;
  height: 100%;
}

.status-separator {
  width: 1px;
  height: 16px;
  background: #aca899;
  box-shadow: 1px 0 0 #fff;
}

.status-busy {
  color: #a00;
  font-weight: 600;
}

.status-ready {
  color: #080;
}

.status-grip {
  margin-left: auto;
  width: 16px;
  height: 16px;
  background: repeating-linear-gradient(
    135deg,
    #aca899,
    #aca899 1px,
    transparent 1px,
    transparent 3px
  );
}
</style>