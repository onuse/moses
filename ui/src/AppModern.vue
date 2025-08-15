<template>
  <div :class="['app-container', themeClass]">
    <!-- Title Bar -->
    <div class="title-bar">
      <div class="title-bar-content">
        <!-- Icon removed temporarily -->
        <span class="app-title">Moses Drive Formatter</span>
      </div>
      <div class="title-bar-controls">
        <button class="title-btn minimize">─</button>
        <button class="title-btn maximize">□</button>
        <button class="title-btn close">✕</button>
      </div>
    </div>

    <!-- Toolbar -->
    <div class="toolbar">
      <button class="tool-btn" @click="refreshDevices" :disabled="isRefreshing">
        <span class="tool-icon">↻</span>
        Refresh
      </button>
      
      <div class="toolbar-spacer"></div>
      
      <button class="tool-btn" @click="toggleTheme" title="Toggle theme">
        <span class="tool-icon">{{ isDarkMode ? '☀' : '🌙' }}</span>
        {{ isDarkMode ? 'Light' : 'Dark' }}
      </button>
      
      <button class="tool-btn" @click="simulateFormat" :disabled="!canSimulate">
        <span class="tool-icon">⚡</span>
        Simulate
      </button>
      
      <button class="tool-btn accent" @click="executeFormat" :disabled="!canFormat">
        <span class="tool-icon">▶</span>
        Format Drive
      </button>
    </div>

    <!-- Main Content Area -->
    <div class="main-content">
      <!-- Left Sidebar -->
      <div class="sidebar">
        <div class="sidebar-header">
          DRIVES
          <span class="drive-count">{{ devices.length }}</span>
        </div>
        
        <div class="drive-list">
          <div v-if="loading" class="loading-state">
            <div class="spinner-small"></div>
            Scanning drives...
          </div>
          
          <div v-else-if="devices.length === 0" class="empty-state">
            No drives detected
          </div>
          
          <div v-else>
            <!-- System Drives -->
            <div v-if="systemDrives.length > 0" class="drive-section">
              <div class="section-label">
                <span class="section-icon">▼</span>
                System Drives
              </div>
              <div class="section-items">
                <div 
                  v-for="device in systemDrives" 
                  :key="device.id"
                  :class="['drive-item', { 
                    selected: selectedDevice?.id === device.id,
                    disabled: device.is_system 
                  }]"
                  @click="selectDevice(device)"
                >
                  <div class="drive-icon-wrapper">
                    <span class="drive-icon">{{ getDeviceIcon(device.device_type) }}</span>
                  </div>
                  <div class="drive-info">
                    <div class="drive-name">{{ device.name }}</div>
                    <div class="drive-meta">
                      {{ formatSize(device.size) }} • {{ device.device_type }}
                    </div>
                  </div>
                  <div v-if="device.is_system" class="drive-badge system">System</div>
                </div>
              </div>
            </div>

            <!-- Removable Drives -->
            <div v-if="removableDrives.length > 0" class="drive-section">
              <div class="section-label">
                <span class="section-icon">▼</span>
                Removable Drives
              </div>
              <div class="section-items">
                <div 
                  v-for="device in removableDrives" 
                  :key="device.id"
                  :class="['drive-item', { 
                    selected: selectedDevice?.id === device.id 
                  }]"
                  @click="selectDevice(device)"
                >
                  <div class="drive-icon-wrapper">
                    <span class="drive-icon">{{ getDeviceIcon(device.device_type) }}</span>
                  </div>
                  <div class="drive-info">
                    <div class="drive-name">{{ device.name }}</div>
                    <div class="drive-meta">
                      {{ formatSize(device.size) }} • {{ device.device_type }}
                    </div>
                  </div>
                  <div class="drive-badge removable">Ready</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Main Panel -->
      <div class="main-panel">
        <!-- No Selection State -->
        <div v-if="!selectedDevice" class="empty-panel">
          <div class="empty-icon">💾</div>
          <h2>Select a drive to format</h2>
          <p>Choose a removable drive from the sidebar</p>
        </div>

        <!-- System Drive Warning -->
        <div v-else-if="selectedDevice.is_system" class="warning-panel">
          <div class="warning-icon">🛡️</div>
          <h2>System Drive Protected</h2>
          <p>This drive contains your operating system and cannot be formatted.</p>
          <p class="warning-note">Moses protects system drives to prevent accidental data loss.</p>
        </div>

        <!-- Format Options -->
        <div v-else class="format-content">
          <!-- Header -->
          <div class="content-header">
            <h2>{{ selectedDevice.name }}</h2>
            <div class="device-path">{{ selectedDevice.id }}</div>
          </div>

          <!-- Options Grid -->
          <div class="options-container">
            <div class="option-card">
              <h3>Format Options</h3>
              
              <div class="form-group">
                <label>File System</label>
                <select v-model="formatOptions.filesystem_type" class="form-control">
                  <option value="">Select a file system...</option>
                  <optgroup label="Recommended">
                    <option value="exfat">exFAT - Universal, no size limits</option>
                  </optgroup>
                  <optgroup label="Other Options">
                    <option value="ntfs">NTFS - Windows native</option>
                    <option value="fat32">FAT32 - Legacy, 4GB file limit</option>
                    <option value="ext4">EXT4 - Linux native</option>
                  </optgroup>
                </select>
              </div>

              <div class="form-group">
                <label>Volume Label</label>
                <input 
                  v-model="formatOptions.label" 
                  type="text" 
                  class="form-control"
                  :placeholder="labelPlaceholder"
                  :maxlength="maxLabelLength"
                >
                <span class="form-hint">{{ labelHint }}</span>
              </div>

              <div class="form-group">
                <label class="checkbox-label">
                  <input type="checkbox" v-model="formatOptions.quick_format">
                  <span class="checkbox-box"></span>
                  <span class="checkbox-text">
                    Quick Format
                    <span class="checkbox-hint">Faster but less thorough</span>
                  </span>
                </label>
              </div>

              <div class="form-group">
                <label class="checkbox-label">
                  <input type="checkbox" v-model="formatOptions.create_partition" checked disabled>
                  <span class="checkbox-box checked"></span>
                  <span class="checkbox-text">
                    Create Partition Table
                    <span class="checkbox-hint">Required for formatting</span>
                  </span>
                </label>
              </div>
            </div>

            <!-- Simulation Results -->
            <div v-if="simulationReport" class="option-card">
              <h3>Simulation Results</h3>
              
              <div class="result-item">
                <span class="result-label">Estimated Time</span>
                <span class="result-value">{{ formatDuration(simulationReport.estimated_time) }}</span>
              </div>
              
              <div class="result-item">
                <span class="result-label">Available Space</span>
                <span class="result-value">{{ formatSize(simulationReport.space_after_format) }}</span>
              </div>
              
              <div v-if="simulationReport.warnings.length > 0" class="warnings-box">
                <div class="warning-title">Important Information</div>
                <div v-for="(warning, i) in simulationReport.warnings" :key="i" class="warning-item">
                  {{ warning }}
                </div>
              </div>
            </div>

            <!-- Progress -->
            <div v-if="isFormatting" class="option-card">
              <h3>Format Progress</h3>
              
              <div class="progress-wrapper">
                <div class="progress-bar">
                  <div class="progress-fill" :style="{ width: formatProgress + '%' }"></div>
                </div>
                <div class="progress-info">
                  <span class="progress-percent">{{ formatProgress }}%</span>
                  <span class="progress-status">{{ progressStatus }}</span>
                </div>
              </div>
              
              <div class="progress-details">
                <div>Elapsed: {{ formatTime }}</div>
                <div>{{ currentOperation }}</div>
              </div>
            </div>
          </div>

          <!-- Action Buttons -->
          <div class="action-bar">
            <button 
              class="btn btn-secondary" 
              @click="simulateFormat"
              :disabled="!canSimulate"
              :title="!canSimulate ? 'Select a drive and filesystem type first' : 'Run safety simulation'"
            >
              <span v-if="isSimulating" class="btn-spinner"></span>
              {{ isSimulating ? 'Simulating...' : 'Run Simulation (Required)' }}
            </button>
            
            <button 
              class="btn btn-primary" 
              @click="executeFormat"
              :disabled="!canFormat"
              :title="!canFormat ? 'Run simulation first' : 'Format the drive'"
            >
              <span v-if="isFormatting" class="btn-spinner"></span>
              {{ isFormatting ? 'Formatting...' : simulationReport ? 'Format Drive' : 'Format Drive (Run Simulation First)' }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Log Console -->
    <LogConsole ref="logConsole" />
    
    <!-- Status Bar -->
    <div class="status-bar">
      <div class="status-item">
        <span v-if="selectedDevice">{{ selectedDevice.name }}</span>
        <span v-else>No drive selected</span>
      </div>
      <div class="status-item">
        {{ devices.length }} drive{{ devices.length !== 1 ? 's' : '' }} detected
      </div>
      <div class="status-item">
        <span v-if="isFormatting" class="status-busy">Formatting...</span>
        <span v-else class="status-ready">Ready</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import LogConsole from './components/LogConsole.vue'

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
  cluster_size: number | null
  quick_format: boolean
  enable_compression: boolean
  additional_options: Record<string, string>
}

interface SimulationReport {
  estimated_time: number
  warnings: string[]
  required_tools: string[]
  space_after_format: number
}

// State
const isDarkMode = ref(true)
const themeClass = computed(() => isDarkMode.value ? 'theme-dark' : 'theme-light')
const logConsole = ref<InstanceType<typeof LogConsole> | null>(null)
const devices = ref<Device[]>([])
const selectedDevice = ref<Device | null>(null)
const loading = ref(false)
const isRefreshing = ref(false)
const isFormatting = ref(false)
const isSimulating = ref(false)
const simulationReport = ref<SimulationReport | null>(null)
const formatProgress = ref(0)
const progressStatus = ref('')
const formatTime = ref('00:00')
const currentOperation = ref('')

const formatOptions = ref<FormatOptions>({
  filesystem_type: '',
  label: '',
  cluster_size: null,
  quick_format: true,
  enable_compression: false,
  additional_options: {}
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
  formatOptions.value.filesystem_type &&
  !isFormatting.value
)

const canFormat = computed(() => 
  canSimulate.value && 
  simulationReport.value && 
  !isFormatting.value &&
  !isSimulating.value
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
    case 'fat32': return 'Maximum 11 characters, uppercase only'
    case 'exfat': return 'Maximum 15 characters'
    case 'ntfs': return 'Maximum 32 characters'
    case 'ext4': return 'Maximum 16 characters'
    default: return 'Select a filesystem to see requirements'
  }
})

// Methods
const getDeviceIcon = (type: string) => {
  const icons: Record<string, string> = {
    'USB': '◉',
    'HardDisk': '◪',
    'SSD': '▣',
    'SDCard': '▤',
    'Virtual': '◈',
    'Unknown': '●'
  }
  return icons[type] || '●'
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

const toggleTheme = () => {
  isDarkMode.value = !isDarkMode.value
  // Save preference to localStorage
  localStorage.setItem('theme', isDarkMode.value ? 'dark' : 'light')
}

const refreshDevices = async () => {
  if (isFormatting.value) return
  
  isRefreshing.value = true
  loading.value = true
  logConsole.value?.info('Scanning for devices...', 'DeviceManager')
  
  try {
    devices.value = await invoke('enumerate_devices')
    logConsole.value?.info(`Found ${devices.value.length} devices`, 'DeviceManager')
    devices.value.forEach(device => {
      logConsole.value?.debug(`Device: ${device.name} (${device.id}) - ${formatSize(device.size)}`, 'DeviceManager')
    })
  } catch (error) {
    console.error('Failed to enumerate devices:', error)
    logConsole.value?.error(`Failed to scan drives: ${error}`, 'DeviceManager')
    alert(`Failed to scan drives: ${error}`)
  } finally {
    loading.value = false
    setTimeout(() => {
      isRefreshing.value = false
    }, 300)
  }
}

const simulateFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) {
    alert('Please select a drive and filesystem type first')
    return
  }
  
  isSimulating.value = true
  simulationReport.value = null // Reset previous simulation
  
  try {
    // Prepare options with proper null handling for label
    const options = {
      ...formatOptions.value,
      label: formatOptions.value.label?.trim() || null
    }
    console.log('Starting simulation for:', selectedDevice.value.name, options)
    simulationReport.value = await invoke('simulate_format', {
      device: selectedDevice.value,
      options: options
    })
    console.log('Simulation successful:', simulationReport.value)
    alert('Simulation complete! You can now format the drive.')
  } catch (error) {
    console.error('Simulation failed:', error)
    alert(`Simulation failed: ${error}`)
    simulationReport.value = null
  } finally {
    isSimulating.value = false
  }
}

const executeFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  const confirmMsg = `WARNING: This will permanently erase all data on ${selectedDevice.value.name}.\n\nAre you sure you want to continue?`
  
  if (!confirm(confirmMsg)) return
  
  // Second confirmation
  const typed = prompt(`Type "FORMAT" to confirm formatting of ${selectedDevice.value.name}:`)
  if (typed !== 'FORMAT') {
    alert('Format cancelled')
    return
  }
  
  isFormatting.value = true
  formatProgress.value = 0
  progressStatus.value = 'Initializing...'
  currentOperation.value = 'Preparing device'
  
  // Log format operation start
  logConsole.value?.info('='.repeat(60), 'Formatter')
  logConsole.value?.info(`Starting format operation`, 'Formatter')
  logConsole.value?.info(`Device: ${selectedDevice.value.name} (${selectedDevice.value.id})`, 'Formatter')
  logConsole.value?.info(`Filesystem: ${formatOptions.value.filesystem_type}`, 'Formatter')
  logConsole.value?.info(`Label: ${formatOptions.value.label || '(none)'}`, 'Formatter')
  logConsole.value?.debug(`Cluster size: ${formatOptions.value.cluster_size || 'default'}`, 'Formatter')
  logConsole.value?.info('='.repeat(60), 'Formatter')
  
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

// Backend log listener
let unlistenBackendLogs: (() => void) | null = null

onMounted(async () => {
  // Load theme preference
  const savedTheme = localStorage.getItem('theme')
  if (savedTheme) {
    isDarkMode.value = savedTheme === 'dark'
  }
  
  // Set up backend log listener
  try {
    unlistenBackendLogs = await listen('backend-log', (event) => {
      const log = event.payload as any
      logConsole.value?.addLog(log.level, log.message, log.source)
    })
    logConsole.value?.info('Log console connected to backend', 'System')
  } catch (error) {
    console.error('Failed to set up log listener:', error)
  }
  
  refreshDevices()
})

onUnmounted(() => {
  // Clean up listener
  if (unlistenBackendLogs) {
    unlistenBackendLogs()
  }
})
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  font-size: 13px;
  overflow: hidden;
  user-select: none;
}

/* Default to dark theme */
.theme-dark {
  --bg-primary: #1e1e1e;
  --bg-secondary: #252526;
  --bg-tertiary: #2d2d30;
  --bg-hover: #2a2d2e;
  --bg-active: #094771;
  --bg-input: #3c3c3c;
  --border-color: #3e3e42;
  --text-primary: #cccccc;
  --text-secondary: #969696;
  --text-disabled: #5a5a5a;
  --accent: #007acc;
  --accent-hover: #1e8ad6;
  --danger: #d83b3b;
  --danger-bg: #5a1d1d;
  --success: #4db84d;
  --success-bg: #1d3d1d;
  --warning: #d8a936;
  --warning-bg: #3c3c3c;
}

/* Light theme */
.theme-light {
  --bg-primary: #ffffff;
  --bg-secondary: #f3f3f3;
  --bg-tertiary: #e8e8e8;
  --bg-hover: #e1e1e1;
  --bg-active: #cce8ff;
  --bg-input: #ffffff;
  --border-color: #d1d1d1;
  --text-primary: #1e1e1e;
  --text-secondary: #606060;
  --text-disabled: #a0a0a0;
  --accent: #0066b8;
  --accent-hover: #0052a3;
  --danger: #c42b1c;
  --danger-bg: #fde7e9;
  --success: #0f7938;
  --success-bg: #dff6dd;
  --warning: #9a5d00;
  --warning-bg: #fff4ce;
}

body {
  color: var(--text-primary);
  background: var(--bg-primary);
}

.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg-primary);
  transition: background-color 0.3s ease;
}

/* Title Bar */
.title-bar {
  height: 32px;
  background: var(--bg-tertiary);
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 12px;
  -webkit-app-region: drag;
  transition: background-color 0.3s ease;
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
  font-size: 12px;
  color: var(--text-primary);
}

.title-bar-controls {
  display: flex;
  -webkit-app-region: no-drag;
}

.title-btn {
  width: 46px;
  height: 32px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  font-size: 10px;
}

.title-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.title-btn.close:hover {
  background: #e81123;
  color: #fff;
}

/* Toolbar */
.toolbar {
  height: 35px;
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 8px;
  transition: background-color 0.3s ease;
}

.tool-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 12px;
  background: transparent;
  border: none;
  border-radius: 3px;
  color: var(--text-primary);
  cursor: pointer;
  font-size: 12px;
  transition: all 0.15s;
}

.tool-btn:hover:not(:disabled) {
  background: var(--bg-hover);
}

.tool-btn:active:not(:disabled) {
  background: var(--accent);
  color: white;
}

.tool-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.tool-btn.accent {
  background: var(--accent);
  color: white;
}

.tool-btn.accent:hover:not(:disabled) {
  background: var(--accent-hover);
}

.tool-icon {
  font-size: 14px;
}

.toolbar-spacer {
  flex: 1;
}

/* Main Content */
.main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
}

/* Sidebar */
.sidebar {
  width: 280px;
  background: var(--bg-secondary);
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  transition: background-color 0.3s ease;
}

.sidebar-header {
  padding: 12px 16px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: 0.5px;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.drive-count {
  background: var(--border-color);
  padding: 2px 6px;
  border-radius: 10px;
  font-size: 10px;
  color: var(--text-primary);
}

.drive-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px 8px;
}

.loading-state, .empty-state {
  padding: 24px;
  text-align: center;
  color: var(--text-secondary);
  font-size: 12px;
}

.spinner-small {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border-color);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin: 0 auto 8px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Drive Sections */
.drive-section {
  margin-bottom: 16px;
}

.section-label {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  color: var(--text-secondary);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}

.section-icon {
  font-size: 8px;
  transition: transform 0.2s;
}

.section-items {
  margin-top: 4px;
}

/* Drive Items */
.drive-item {
  display: flex;
  align-items: center;
  padding: 8px;
  margin-bottom: 2px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.15s;
}

.drive-item:hover {
  background: var(--bg-hover);
}

.drive-item.selected {
  background: var(--bg-active);
}

.drive-item.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.drive-icon-wrapper {
  width: 32px;
  height: 32px;
  background: var(--border-color);
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 12px;
  flex-shrink: 0;
}

.drive-item.selected .drive-icon-wrapper {
  background: var(--accent);
  color: white;
}

.drive-icon {
  font-size: 16px;
  color: var(--text-primary);
}

.drive-info {
  flex: 1;
  min-width: 0;
}

.drive-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.drive-meta {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 2px;
}

.drive-badge {
  padding: 2px 8px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.drive-badge.system {
  background: var(--danger-bg);
  color: var(--danger);
}

.drive-badge.removable {
  background: var(--success-bg);
  color: var(--success);
}

/* Main Panel */
.main-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--bg-primary);
  overflow-y: auto;
}

/* Empty Panel */
.empty-panel, .warning-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px;
  text-align: center;
}

.empty-icon, .warning-icon {
  font-size: 64px;
  margin-bottom: 24px;
  opacity: 0.2;
}

.warning-icon {
  opacity: 1;
}

.empty-panel h2, .warning-panel h2 {
  font-size: 24px;
  font-weight: 300;
  margin-bottom: 8px;
  color: var(--text-primary);
}

.empty-panel p, .warning-panel p {
  color: var(--text-secondary);
  font-size: 14px;
}

.warning-panel h2 {
  color: var(--danger);
}

.warning-note {
  margin-top: 16px;
  padding: 12px;
  background: var(--bg-tertiary);
  border-radius: 4px;
  font-size: 12px;
}

/* Format Content */
.format-content {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.content-header {
  padding: 24px;
  border-bottom: 1px solid var(--bg-tertiary);
}

.content-header h2 {
  font-size: 20px;
  font-weight: 400;
  color: var(--text-primary);
  margin-bottom: 4px;
}

.device-path {
  font-size: 12px;
  color: var(--text-secondary);
}

/* Options Container */
.options-container {
  flex: 1;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.option-card {
  background: var(--bg-secondary);
  border-radius: 6px;
  padding: 20px;
  transition: background-color 0.3s ease;
}

.option-card h3 {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: 16px;
}

/* Form Controls */
.form-group {
  margin-bottom: 16px;
}

.form-group:last-child {
  margin-bottom: 0;
}

.form-group label {
  display: block;
  font-size: 12px;
  font-weight: 500;
  color: var(--text-secondary);
  margin-bottom: 6px;
}

.form-control {
  width: 100%;
  padding: 6px 8px;
  background: var(--bg-input);
  border: 1px solid var(--border-color);
  border-radius: 3px;
  color: var(--text-primary);
  font-size: 13px;
  transition: all 0.15s;
}

.form-control:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--bg-tertiary);
}

.form-hint {
  display: block;
  margin-top: 4px;
  font-size: 11px;
  color: var(--text-secondary);
}

/* Checkbox */
.checkbox-label {
  display: flex;
  align-items: flex-start;
  cursor: pointer;
}

.checkbox-label input[type="checkbox"] {
  display: none;
}

.checkbox-box {
  width: 16px;
  height: 16px;
  background: var(--bg-input);
  border: 1px solid var(--border-color);
  border-radius: 3px;
  margin-right: 8px;
  flex-shrink: 0;
  position: relative;
}

.checkbox-label:hover .checkbox-box {
  border-color: var(--accent);
}

.checkbox-label input:checked + .checkbox-box,
.checkbox-box.checked {
  background: var(--accent);
  border-color: var(--accent);
}

.checkbox-label input:checked + .checkbox-box::after,
.checkbox-box.checked::after {
  content: '✓';
  position: absolute;
  top: -2px;
  left: 2px;
  color: white;
  font-size: 12px;
}

.checkbox-text {
  display: flex;
  flex-direction: column;
}

.checkbox-hint {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 2px;
}

/* Results */
.result-item {
  display: flex;
  justify-content: space-between;
  padding: 8px 0;
  border-bottom: 1px solid var(--border-color);
}

.result-item:last-child {
  border-bottom: none;
}

.result-label {
  color: var(--text-secondary);
  font-size: 12px;
}

.result-value {
  color: var(--text-primary);
  font-size: 12px;
  font-weight: 500;
}

/* Warnings */
.warnings-box {
  margin-top: 16px;
  padding: 12px;
  background: var(--warning-bg);
  border-radius: 4px;
  border-left: 3px solid var(--warning);
}

.warning-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--warning);
  margin-bottom: 8px;
}

.warning-item {
  font-size: 11px;
  color: var(--text-secondary);
  margin-bottom: 4px;
}

/* Progress */
.progress-wrapper {
  margin: 16px 0;
}

.progress-bar {
  height: 4px;
  background: var(--bg-input);
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--accent);
  transition: width 0.3s;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  margin-top: 8px;
  font-size: 12px;
}

.progress-percent {
  font-weight: 600;
  color: var(--accent);
}

.progress-status {
  color: var(--text-secondary);
}

.progress-details {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border-color);
  font-size: 11px;
  color: var(--text-secondary);
}

/* Action Bar */
.action-bar {
  padding: 16px 24px;
  background: var(--bg-secondary);
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

/* Buttons */
.btn {
  padding: 6px 16px;
  border: none;
  border-radius: 3px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.btn-secondary {
  background: var(--bg-input);
  color: var(--text-primary);
}

.btn-secondary:hover:not(:disabled) {
  background: var(--bg-hover);
}

.btn-primary {
  background: var(--accent);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: var(--accent-hover);
}

.btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-spinner {
  width: 12px;
  height: 12px;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

/* Status Bar */
.status-bar {
  height: 22px;
  background: var(--accent);
  color: white;
  display: flex;
  align-items: center;
  padding: 0 12px;
  font-size: 12px;
  gap: 24px;
}

.status-item {
  display: flex;
  align-items: center;
}

.status-busy {
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

.status-ready {
  color: white;
}

/* Scrollbar Styling */
::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

::-webkit-scrollbar-track {
  background: var(--bg-primary);
}

::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 5px;
}

::-webkit-scrollbar-thumb:hover {
  background: var(--text-secondary);
}
</style>