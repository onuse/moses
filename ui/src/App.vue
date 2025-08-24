<template>
  <div class="container">
    <!-- Toast Notifications -->
    <div class="toast-container">
      <transition-group name="toast">
        <div 
          v-for="toast in toasts" 
          :key="toast.id"
          :class="['toast', toast.type]"
        >
          <div class="toast-icon">{{ getToastIcon(toast.type) }}</div>
          <div class="toast-message">{{ toast.message }}</div>
          <button @click="removeToast(toast.id)" class="toast-close">×</button>
        </div>
      </transition-group>
    </div>

    <header>
      <h1>Moses Drive Formatter</h1>
      <p class="subtitle">Cross-platform drive formatting made easy</p>
    </header>

    <main>
      <!-- Device Selection Section -->
      <section class="device-selection">
        <div class="section-header">
          <h2>Select Drive</h2>
          <button 
            @click="refreshDevices" 
            :disabled="isRefreshing"
            class="btn-icon"
            title="Refresh devices"
          >
            <span :class="['refresh-icon', { spinning: isRefreshing }]">⟳</span>
          </button>
        </div>
        
        <div v-if="loading" class="loading">
          <div class="spinner"></div>
          <p>Scanning for drives...</p>
        </div>
        
        <div v-else-if="devices.length === 0" class="empty-state">
          <div class="empty-icon">💾</div>
          <p>No drives detected</p>
          <small>Connect a drive and click refresh</small>
        </div>
        
        <div v-else class="device-list">
          <div 
            v-for="device in devices" 
            :key="device.id"
            :class="['device-card', { 
              selected: selectedDevice?.id === device.id,
              'system-drive': device.is_system 
            }]"
            @click="selectDevice(device)"
          >
            <div class="device-icon">
              {{ getDeviceIcon(device.device_type) }}
            </div>
            <div class="device-info">
              <h3>{{ device.name }}</h3>
              <p class="device-type">{{ device.device_type }}</p>
              <p class="device-size">{{ formatSize(device.size) }}</p>
              <div v-if="device.is_system" class="warning-badge">
                <span class="badge-icon">⚠️</span> System Drive
              </div>
              <div v-if="device.is_removable" class="info-badge">
                <span class="badge-icon">🔌</span> Removable
              </div>
            </div>
            <div v-if="selectedDevice?.id === device.id" class="selection-indicator">
              ✓
            </div>
          </div>
        </div>
      </section>

      <!-- Format Options Section -->
      <section v-if="selectedDevice && !selectedDevice.is_system" class="format-options">
        <h2>Format Options</h2>
        
        <div class="form-group">
          <label for="filesystem">Filesystem Type</label>
          <div class="select-wrapper">
            <select 
              id="filesystem" 
              v-model="formatOptions.filesystem_type"
              :disabled="isFormatting"
            >
              <option value="">Select filesystem...</option>
              <option value="ext4">EXT4 - Linux Native</option>
              <option value="ntfs">NTFS - Windows Native</option>
              <option value="fat32">FAT32 - Universal (4GB limit)</option>
              <option value="exfat">exFAT - Universal (No limits)</option>
            </select>
            <div class="select-arrow">▼</div>
          </div>
        </div>

        <div class="form-group">
          <label for="label">Volume Label (Optional)</label>
          <input 
            id="label" 
            type="text" 
            v-model="formatOptions.label"
            :disabled="isFormatting"
            placeholder="Enter volume label"
            maxlength="32"
            class="input-field"
          />
          <small class="input-hint">{{ labelHint }}</small>
        </div>

        <div class="form-group checkbox-group">
          <label class="checkbox-label">
            <input 
              type="checkbox" 
              v-model="formatOptions.quick_format"
              :disabled="isFormatting"
            />
            <span class="checkbox-text">Quick Format</span>
            <small class="checkbox-hint">Faster but less thorough</small>
          </label>
        </div>

        <div class="action-buttons">
          <button 
            @click="simulateFormat" 
            :disabled="!canFormat || isSimulating || isFormatting"
            class="btn-primary"
          >
            <span v-if="isSimulating" class="btn-spinner"></span>
            {{ isSimulating ? 'Simulating...' : 'Dry Run (Simulate)' }}
          </button>
          <button 
            @click="executeFormat" 
            :disabled="!canFormat || !simulationComplete || isFormatting"
            class="btn-danger"
          >
            <span v-if="isFormatting" class="btn-spinner"></span>
            {{ isFormatting ? 'Formatting...' : 'Format Drive' }}
          </button>
        </div>
      </section>

      <!-- System Drive Warning -->
      <section v-else-if="selectedDevice?.is_system" class="warning-section">
        <div class="warning-icon">🛡️</div>
        <h2>System Drive Protected</h2>
        <p>This drive contains your operating system and cannot be formatted.</p>
        <small>Moses protects you from accidentally damaging your system.</small>
      </section>

      <!-- Simulation Report Section -->
      <section v-if="simulationReport && !isFormatting" class="simulation-report">
        <h2>Simulation Report</h2>
        <div class="report-content">
          <div class="report-item">
            <span class="report-label">⏱️ Estimated Time:</span>
            <span class="report-value">{{ formatDuration(simulationReport.estimated_time) }}</span>
          </div>
          <div class="report-item">
            <span class="report-label">💾 Available Space:</span>
            <span class="report-value">{{ formatSize(simulationReport.space_after_format) }}</span>
          </div>
          
          <div v-if="simulationReport.warnings.length > 0" class="warnings">
            <h3>⚠️ Important Information</h3>
            <ul class="warning-list">
              <li v-for="(warning, index) in simulationReport.warnings" :key="index">
                {{ warning }}
              </li>
            </ul>
          </div>
          
          <div v-if="simulationReport.required_tools.length > 0" class="required-tools">
            <h3>🔧 Required Tools</h3>
            <ul class="tool-list">
              <li v-for="(tool, index) in simulationReport.required_tools" :key="index">
                <span class="tool-icon">📦</span> {{ tool }}
              </li>
            </ul>
          </div>
        </div>
      </section>

      <!-- Progress Section -->
      <section v-if="isFormatting" class="progress-section">
        <h2>Formatting in Progress</h2>
        <div class="progress-container">
          <div class="progress-bar">
            <div 
              class="progress-fill" 
              :style="{ width: formatProgress + '%' }"
            ></div>
          </div>
          <div class="progress-text">{{ formatProgress }}%</div>
        </div>
        <p class="progress-status">{{ progressStatus }}</p>
        <div class="progress-animation">
          <span class="dot"></span>
          <span class="dot"></span>
          <span class="dot"></span>
        </div>
        <p class="progress-warning">
          ⚠️ Do not disconnect the drive or close this application
        </p>
      </section>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
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
}

interface SimulationReport {
  estimated_time: number
  warnings: string[]
  required_tools: string[]
  space_after_format: number
}

interface Toast {
  id: number
  type: 'success' | 'error' | 'warning' | 'info'
  message: string
}

// State
const devices = ref<Device[]>([])
const selectedDevice = ref<Device | null>(null)
const loading = ref(false)
const isRefreshing = ref(false)
const isSimulating = ref(false)
const isFormatting = ref(false)
const simulationReport = ref<SimulationReport | null>(null)
const simulationComplete = ref(false)
const formatProgress = ref(0)
const progressStatus = ref('')
const toasts = ref<Toast[]>([])
let toastId = 0

const formatOptions = ref<FormatOptions>({
  filesystem_type: '',
  label: '',
  quick_format: true,
})

// Computed
const canFormat = computed(() => {
  return selectedDevice.value && 
         formatOptions.value.filesystem_type && 
         !selectedDevice.value.is_system &&
         !isFormatting.value
})

const labelHint = computed(() => {
  switch (formatOptions.value.filesystem_type) {
    case 'fat32': return 'Max 11 characters, uppercase'
    case 'exfat': return 'Max 15 characters'
    case 'ntfs': return 'Max 32 characters'
    case 'ext4': return 'Max 16 characters'
    default: return 'Enter a name for your drive'
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

const getToastIcon = (type: string) => {
  const icons: Record<string, string> = {
    'success': '✅',
    'error': '❌',
    'warning': '⚠️',
    'info': 'ℹ️'
  }
  return icons[type] || 'ℹ️'
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
  const remainingSeconds = seconds % 60
  if (remainingSeconds === 0) {
    return `${minutes} minute${minutes !== 1 ? 's' : ''}`
  }
  return `${minutes} minute${minutes !== 1 ? 's' : ''} ${remainingSeconds} seconds`
}

const showToast = (type: Toast['type'], message: string) => {
  const toast: Toast = {
    id: ++toastId,
    type,
    message
  }
  toasts.value.push(toast)
  
  // Auto remove after 5 seconds
  setTimeout(() => {
    removeToast(toast.id)
  }, 5000)
}

const removeToast = (id: number) => {
  const index = toasts.value.findIndex(t => t.id === id)
  if (index > -1) {
    toasts.value.splice(index, 1)
  }
}

const selectDevice = (device: Device) => {
  if (isFormatting.value) {
    showToast('warning', 'Cannot change device while formatting')
    return
  }
  
  selectedDevice.value = device
  simulationReport.value = null
  simulationComplete.value = false
  
  if (device.is_system) {
    showToast('info', 'System drive selected - formatting disabled for safety')
  }
}

const refreshDevices = async () => {
  if (isFormatting.value) {
    showToast('warning', 'Cannot refresh while formatting')
    return
  }
  
  isRefreshing.value = true
  loading.value = true
  
  try {
    devices.value = await invoke('enumerate_devices')
    showToast('success', `Found ${devices.value.length} drive${devices.value.length !== 1 ? 's' : ''}`)
  } catch (error) {
    console.error('Failed to enumerate devices:', error)
    showToast('error', `Failed to scan drives: ${error}`)
  } finally {
    loading.value = false
    // Keep spinner going a bit longer for visual feedback
    setTimeout(() => {
      isRefreshing.value = false
    }, 500)
  }
}

const simulateFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  isSimulating.value = true
  progressStatus.value = 'Running safety checks...'
  
  try {
    // Check for missing tools first
    const missingTools = await invoke('check_formatter_requirements', {
      filesystemType: formatOptions.value.filesystem_type
    })
    
    if (missingTools && (missingTools as string[]).length > 0) {
      showToast('warning', `Missing tools: ${(missingTools as string[]).join(', ')}`)
    }
    
    simulationReport.value = await invoke('simulate_format', {
      device: selectedDevice.value,
      options: formatOptions.value
    })
    
    simulationComplete.value = true
    showToast('success', 'Simulation complete - ready to format')
  } catch (error) {
    console.error('Simulation failed:', error)
    showToast('error', `Simulation failed: ${error}`)
  } finally {
    isSimulating.value = false
  }
}

const executeFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  const deviceName = selectedDevice.value.name
  const filesystem = formatOptions.value.filesystem_type.toUpperCase()
  
  const confirmMessage = `⚠️ WARNING ⚠️\n\nYou are about to format:\n${deviceName}\n\nFilesystem: ${filesystem}\n\nThis will PERMANENTLY ERASE all data on this drive.\nThis action cannot be undone.\n\nAre you absolutely sure?`
  
  if (!confirm(confirmMessage)) {
    showToast('info', 'Format cancelled')
    return
  }
  
  isFormatting.value = true
  formatProgress.value = 0
  progressStatus.value = 'Preparing to format...'
  
  // Simulate progress updates
  const progressInterval = setInterval(() => {
    if (formatProgress.value < 90) {
      formatProgress.value += Math.random() * 15
      if (formatProgress.value > 90) formatProgress.value = 90
      
      // Update status based on progress
      if (formatProgress.value < 20) {
        progressStatus.value = 'Initializing formatter...'
      } else if (formatProgress.value < 40) {
        progressStatus.value = 'Preparing device...'
      } else if (formatProgress.value < 60) {
        progressStatus.value = 'Writing filesystem structures...'
      } else if (formatProgress.value < 80) {
        progressStatus.value = 'Creating partition table...'
      } else {
        progressStatus.value = 'Finalizing...'
      }
    }
  }, 500)
  
  try {
    const result = await invoke('execute_format', {
      device: selectedDevice.value,
      options: formatOptions.value
    })
    
    // Complete the progress
    clearInterval(progressInterval)
    formatProgress.value = 100
    progressStatus.value = 'Format complete!'
    
    showToast('success', `Successfully formatted ${deviceName} as ${filesystem}`)
    
    // Reset after a short delay
    setTimeout(async () => {
      isFormatting.value = false
      formatProgress.value = 0
      progressStatus.value = ''
      simulationReport.value = null
      simulationComplete.value = false
      selectedDevice.value = null
      
      // Refresh devices to show updated state
      await refreshDevices()
    }, 2000)
    
  } catch (error) {
    clearInterval(progressInterval)
    console.error('Format failed:', error)
    showToast('error', `Format failed: ${error}`)
    
    isFormatting.value = false
    formatProgress.value = 0
    progressStatus.value = ''
  }
}

// Watch for filesystem type changes to clear simulation
watch(() => formatOptions.value.filesystem_type, () => {
  simulationReport.value = null
  simulationComplete.value = false
})

// Initial load
onMounted(() => {
  refreshDevices()
  showToast('info', 'Welcome to Moses Drive Formatter')
})
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  min-height: 100vh;
  color: #333;
}

/* Toast Notifications */
.toast-container {
  position: fixed;
  top: 20px;
  right: 20px;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.toast {
  display: flex;
  align-items: center;
  background: white;
  border-radius: 8px;
  padding: 12px 16px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  min-width: 300px;
  animation: slideIn 0.3s ease;
}

.toast.success { border-left: 4px solid #48bb78; }
.toast.error { border-left: 4px solid #f56565; }
.toast.warning { border-left: 4px solid #ed8936; }
.toast.info { border-left: 4px solid #4299e1; }

.toast-icon {
  font-size: 1.2rem;
  margin-right: 12px;
}

.toast-message {
  flex: 1;
  font-size: 0.95rem;
}

.toast-close {
  background: none;
  border: none;
  font-size: 1.5rem;
  cursor: pointer;
  opacity: 0.5;
  transition: opacity 0.2s;
}

.toast-close:hover {
  opacity: 1;
}

/* Animations */
@keyframes slideIn {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

.toast-enter-active {
  transition: all 0.3s ease;
}

.toast-leave-active {
  transition: all 0.3s ease;
}

.toast-enter-from {
  transform: translateX(100%);
  opacity: 0;
}

.toast-leave-to {
  transform: translateX(100%);
  opacity: 0;
}

/* Main Layout */
.container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 2rem;
}

header {
  text-align: center;
  color: white;
  margin-bottom: 3rem;
}

header h1 {
  font-size: 2.5rem;
  margin-bottom: 0.5rem;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

.subtitle {
  font-size: 1.1rem;
  opacity: 0.95;
}

main {
  display: grid;
  gap: 2rem;
}

section {
  background: white;
  border-radius: 12px;
  padding: 2rem;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.1);
  animation: fadeIn 0.3s ease;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* Section Headers */
.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1.5rem;
}

h2 {
  color: #667eea;
  margin: 0;
}

/* Loading States */
.loading {
  text-align: center;
  padding: 3rem;
}

.spinner {
  width: 50px;
  height: 50px;
  border: 4px solid #f3f3f3;
  border-top: 4px solid #667eea;
  border-radius: 50%;
  animation: spin 1s linear infinite;
  margin: 0 auto 1rem;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.btn-spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid #ffffff;
  border-radius: 50%;
  border-top-color: transparent;
  animation: spin 0.6s linear infinite;
  margin-right: 8px;
}

/* Empty State */
.empty-state {
  text-align: center;
  padding: 3rem;
  color: #666;
}

.empty-icon {
  font-size: 3rem;
  margin-bottom: 1rem;
  opacity: 0.5;
}

.empty-state small {
  display: block;
  margin-top: 0.5rem;
  opacity: 0.7;
}

/* Device List */
.device-list {
  display: grid;
  gap: 1rem;
  margin-bottom: 1rem;
}

.device-card {
  display: flex;
  align-items: center;
  padding: 1rem;
  border: 2px solid #e0e0e0;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s ease;
  position: relative;
}

.device-card:hover {
  border-color: #667eea;
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.15);
}

.device-card.selected {
  border-color: #667eea;
  background: linear-gradient(to right, #f7f9ff, #fff);
}

.device-card.system-drive {
  opacity: 0.7;
  cursor: not-allowed;
}

.device-icon {
  font-size: 2rem;
  margin-right: 1rem;
}

.device-info {
  flex: 1;
}

.device-info h3 {
  margin-bottom: 0.25rem;
  color: #2d3748;
}

.device-type, .device-size {
  color: #666;
  font-size: 0.9rem;
  margin: 0.2rem 0;
}

.warning-badge, .info-badge {
  display: inline-flex;
  align-items: center;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  font-size: 0.8rem;
  margin-top: 0.5rem;
}

.warning-badge {
  background: #fff5f5;
  color: #c53030;
  border: 1px solid #feb2b2;
}

.info-badge {
  background: #ebf8ff;
  color: #2b6cb0;
  border: 1px solid #90cdf4;
}

.badge-icon {
  margin-right: 0.25rem;
  font-size: 0.9rem;
}

.selection-indicator {
  position: absolute;
  right: 1rem;
  top: 50%;
  transform: translateY(-50%);
  width: 30px;
  height: 30px;
  background: #667eea;
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: bold;
}

/* Forms */
.form-group {
  margin-bottom: 1.5rem;
}

.form-group label {
  display: block;
  margin-bottom: 0.5rem;
  font-weight: 500;
  color: #4a5568;
}

.select-wrapper {
  position: relative;
}

select, .input-field {
  width: 100%;
  padding: 0.75rem;
  border: 2px solid #e2e8f0;
  border-radius: 6px;
  font-size: 1rem;
  transition: border-color 0.2s;
  background: white;
}

select {
  appearance: none;
  padding-right: 2.5rem;
}

.select-arrow {
  position: absolute;
  right: 1rem;
  top: 50%;
  transform: translateY(-50%);
  pointer-events: none;
  color: #718096;
}

select:focus, .input-field:focus {
  outline: none;
  border-color: #667eea;
}

select:disabled, .input-field:disabled {
  background: #f7fafc;
  cursor: not-allowed;
  opacity: 0.7;
}

.input-hint {
  display: block;
  margin-top: 0.25rem;
  font-size: 0.85rem;
  color: #718096;
}

.checkbox-group {
  display: flex;
  align-items: flex-start;
}

.checkbox-label {
  display: flex;
  align-items: flex-start;
  cursor: pointer;
}

.checkbox-label input[type="checkbox"] {
  margin-right: 0.75rem;
  margin-top: 0.25rem;
  cursor: pointer;
}

.checkbox-text {
  font-weight: 500;
  color: #4a5568;
}

.checkbox-hint {
  display: block;
  font-size: 0.85rem;
  color: #718096;
  margin-top: 0.25rem;
}

/* Buttons */
.action-buttons {
  display: flex;
  gap: 1rem;
  margin-top: 2rem;
}

.btn-primary, .btn-danger, .btn-secondary, .btn-icon {
  padding: 0.75rem 1.5rem;
  border: none;
  border-radius: 6px;
  font-size: 1rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.3s ease;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.btn-primary {
  background: #667eea;
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: #5a67d8;
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(102, 126, 234, 0.3);
}

.btn-danger {
  background: #f56565;
  color: white;
}

.btn-danger:hover:not(:disabled) {
  background: #e53e3e;
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(245, 101, 101, 0.3);
}

.btn-secondary {
  background: #e2e8f0;
  color: #4a5568;
}

.btn-secondary:hover:not(:disabled) {
  background: #cbd5e0;
}

.btn-icon {
  padding: 0.5rem;
  background: transparent;
  border: 2px solid #e2e8f0;
  width: 40px;
  height: 40px;
}

.btn-icon:hover:not(:disabled) {
  border-color: #667eea;
  background: #f7f9ff;
}

button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none !important;
}

.refresh-icon {
  display: inline-block;
  font-size: 1.2rem;
  transition: transform 0.3s;
}

.refresh-icon.spinning {
  animation: spin 1s linear infinite;
}

/* Warning Section */
.warning-section {
  text-align: center;
  padding: 3rem;
}

.warning-section .warning-icon {
  font-size: 4rem;
  margin-bottom: 1rem;
}

.warning-section h2 {
  color: #c53030;
  margin-bottom: 1rem;
}

.warning-section p {
  color: #666;
  margin-bottom: 0.5rem;
}

.warning-section small {
  color: #999;
  font-style: italic;
}

/* Simulation Report */
.report-content {
  margin-top: 1.5rem;
}

.report-item {
  display: flex;
  justify-content: space-between;
  padding: 0.75rem;
  background: #f7fafc;
  border-radius: 6px;
  margin-bottom: 0.75rem;
}

.report-label {
  font-weight: 500;
  color: #4a5568;
}

.report-value {
  color: #2d3748;
}

.warnings, .required-tools {
  margin-top: 1.5rem;
}

.warnings h3, .required-tools h3 {
  color: #e53e3e;
  font-size: 1rem;
  margin-bottom: 0.75rem;
}

.required-tools h3 {
  color: #4299e1;
}

.warning-list, .tool-list {
  list-style: none;
  padding: 0;
}

.warning-list li, .tool-list li {
  padding: 0.5rem;
  margin-bottom: 0.5rem;
  background: #fffaf0;
  border-left: 3px solid #ed8936;
  border-radius: 4px;
  font-size: 0.9rem;
}

.tool-list li {
  background: #ebf8ff;
  border-left-color: #4299e1;
  display: flex;
  align-items: center;
}

.tool-icon {
  margin-right: 0.5rem;
}

/* Progress Section */
.progress-section {
  text-align: center;
}

.progress-container {
  margin: 2rem 0;
}

.progress-bar {
  width: 100%;
  height: 30px;
  background: #e2e8f0;
  border-radius: 15px;
  overflow: hidden;
  position: relative;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #667eea, #764ba2);
  transition: width 0.3s ease;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding-right: 1rem;
  position: relative;
  overflow: hidden;
}

.progress-fill::after {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.3),
    transparent
  );
  animation: shimmer 2s infinite;
}

@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}

.progress-text {
  margin-top: 0.5rem;
  font-size: 1.5rem;
  font-weight: bold;
  color: #667eea;
}

.progress-status {
  color: #4a5568;
  margin: 1rem 0;
  font-size: 1.1rem;
}

.progress-animation {
  display: flex;
  justify-content: center;
  gap: 0.5rem;
  margin: 2rem 0;
}

.progress-animation .dot {
  width: 10px;
  height: 10px;
  background: #667eea;
  border-radius: 50%;
  animation: bounce 1.4s infinite ease-in-out both;
}

.progress-animation .dot:nth-child(1) {
  animation-delay: -0.32s;
}

.progress-animation .dot:nth-child(2) {
  animation-delay: -0.16s;
}

@keyframes bounce {
  0%, 80%, 100% {
    transform: scale(0);
  }
  40% {
    transform: scale(1);
  }
}

.progress-warning {
  color: #e53e3e;
  font-weight: 500;
  padding: 1rem;
  background: #fff5f5;
  border-radius: 6px;
  border: 1px solid #feb2b2;
}

/* Responsive Design */
@media (max-width: 768px) {
  .container {
    padding: 1rem;
  }
  
  header h1 {
    font-size: 2rem;
  }
  
  .action-buttons {
    flex-direction: column;
  }
  
  .toast-container {
    left: 20px;
    right: 20px;
  }
  
  .toast {
    min-width: auto;
  }
}
</style>