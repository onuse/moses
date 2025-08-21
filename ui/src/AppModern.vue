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
      
      <button 
        class="tool-btn" 
        @click="showCleanDiskDialog" 
        :disabled="!selectedDevice || selectedDevice.is_system"
        title="Remove all partitions and signatures from disk"
      >
        <span class="tool-icon">🧹</span>
        Clean Disk
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
      
      <button 
        class="tool-btn" 
        @click="analyzeFilesystem" 
        :disabled="!selectedDevice"
        title="Analyze filesystem boot sector and signatures"
      >
        <span class="tool-icon">🔍</span>
        Analyze
      </button>
      
      <button 
        v-if="viewMode === 'browse'" 
        class="tool-btn" 
        @click="viewMode = 'format'" 
        :disabled="!selectedDevice || selectedDevice.is_system"
      >
        <span class="tool-icon">🔧</span>
        Format Mode
      </button>
      
      <button 
        v-else-if="viewMode === 'format'" 
        class="tool-btn" 
        @click="viewMode = 'browse'"
      >
        <span class="tool-icon">📁</span>
        Browse Mode
      </button>
      
      <button 
        v-if="viewMode === 'format'" 
        class="tool-btn accent" 
        @click="executeFormat" 
        :disabled="!canFormat"
      >
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
                      <span v-if="device.filesystem" class="filesystem-info">
                        • {{ formatFilesystemName(device.filesystem) }}
                      </span>
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
                      <span v-if="device.filesystem" class="filesystem-info">
                        • {{ formatFilesystemName(device.filesystem) }}
                      </span>
                    </div>
                  </div>
                  <div class="drive-badge removable">Ready</div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Vertical Resizer -->
      <div class="resizer vertical-resizer" @mousedown="startResize('vertical', $event)"></div>

      <!-- Main Panel -->
      <div class="main-panel">
        <!-- No Selection State -->
        <div v-if="!selectedDevice" class="empty-panel">
          <div class="empty-icon">💾</div>
          <h2>Select a drive to explore</h2>
          <p>Choose a drive from the sidebar to browse files or format</p>
        </div>

        <!-- File Browser View (Default) -->
        <FileBrowser 
          v-else-if="viewMode === 'browse' && selectedDevice && !selectedDevice.is_system" 
          :drive="selectedDeviceWithFs"
          @copy-files="handleCopyFiles"
          @export-files="handleExportFiles"
          @show-properties="handleShowProperties"
          @update-filesystem="handleUpdateFilesystem"
        />

        <!-- System Drive Warning (Browse Mode) -->
        <div v-else-if="viewMode === 'browse' && selectedDevice?.is_system" class="warning-panel">
          <div class="warning-icon">🛡️</div>
          <h2>System Drive Protected</h2>
          <p>Browsing system drives coming soon.</p>
          <p class="warning-note">You can still format removable drives.</p>
        </div>

        <!-- Format Options (Format Mode) -->
        <div v-else-if="viewMode === 'format' && selectedDevice && !selectedDevice.is_system" class="format-content">
          <!-- Header -->
          <!-- Drive header (unified with Browse mode) -->
          <div class="drive-header">
            <div class="drive-info">
              <span class="drive-name">{{ selectedDevice.name }}</span>
              <span class="drive-separator">/</span>
              <span class="drive-id">{{ selectedDevice.id }}</span>
            </div>
          </div>

          <!-- Options Grid -->
          <div class="options-container">
            <div class="option-card">
              <h3>Format Options</h3>
              
              <!-- Top Row: Basic Options -->
              <div class="options-row-basic">
                <div class="form-group flex-grow">
                  <label>File System</label>
                  <select v-model="formatOptions.filesystem_type" class="form-control">
                    <option value="">Select a file system...</option>
                    <optgroup label="Recommended">
                      <option value="exfat">exFAT - Universal, no size limits</option>
                    </optgroup>
                    <optgroup label="Windows">
                      <option value="ntfs">NTFS - Windows native</option>
                      <option value="fat32">FAT32 - Legacy, 4GB file limit</option>
                      <option value="fat16">FAT16 - Legacy, max 4GB volume</option>
                    </optgroup>
                    <optgroup label="Linux (ext family)">
                      <option value="ext4">ext4 - Modern Linux</option>
                      <option value="ext3">ext3 - Linux with journal</option>
                      <option value="ext2">ext2 - Simple Linux (2TB limit)</option>
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
                    style="width: 200px;"
                  >
                  <span class="form-hint">{{ labelHint }}</span>
                </div>
              </div>

              <!-- Bottom Row: Advanced Options (Horizontal) -->
              <div class="options-row-advanced">
                <!-- Format Speed -->
                <div class="option-section compact">
                  <div class="section-title">Format Speed</div>
                  <div class="radio-group horizontal">
                    <label class="radio-label compact">
                      <input type="radio" v-model="formatOptions.quick_format" :value="true">
                      <span class="radio-circle"></span>
                      <span class="radio-text">
                        Quick
                        <span class="option-hint">Seconds</span>
                      </span>
                    </label>
                    <label class="radio-label compact">
                      <input type="radio" v-model="formatOptions.quick_format" :value="false">
                      <span class="radio-circle"></span>
                      <span class="radio-text">
                        Full
                        <span class="option-hint">Minutes</span>
                      </span>
                    </label>
                  </div>
                </div>

                <!-- Disk Preparation -->
                <div class="option-section compact">
                  <div class="section-title">Disk Preparation</div>
                  <div class="checkbox-group">
                    <label class="checkbox-label compact">
                      <input 
                        type="checkbox" 
                        v-model="formatOptions.create_partition_table"
                      >
                      <span class="checkbox-box" :class="{ checked: formatOptions.create_partition_table }"></span>
                      <span class="checkbox-text">
                        New Partition Table
                        <span class="checkbox-hint">Fresh MBR/GPT</span>
                      </span>
                    </label>

                    <label class="checkbox-label compact" 
                           title="Use this for problematic disks that won't format normally">
                      <input 
                        type="checkbox" 
                        v-model="formatOptions.clean_before_format"
                      >
                      <span class="checkbox-box" :class="{ checked: formatOptions.clean_before_format }"></span>
                      <span class="checkbox-text">
                        Deep Clean
                        <span class="checkbox-hint">Fix stubborn disks</span>
                      </span>
                    </label>
                  </div>
                </div>

                <!-- Verification -->
                <div class="option-section compact">
                  <div class="section-title">Verification</div>
                  <div class="checkbox-group">
                    <label class="checkbox-label compact">
                      <input 
                        type="checkbox" 
                        v-model="formatOptions.verify_after_format"
                        :disabled="formatOptions.quick_format"
                      >
                      <span class="checkbox-box" :class="{ 
                        checked: formatOptions.verify_after_format,
                        disabled: formatOptions.quick_format 
                      }"></span>
                      <span class="checkbox-text" :class="{ disabled: formatOptions.quick_format }">
                        Verify After
                        <span class="checkbox-hint">{{ formatOptions.quick_format ? 'N/A' : 'Check integrity' }}</span>
                      </span>
                    </label>
                  </div>
                </div>
              </div>
            </div>

            <!-- Simulation Results removed - shown only in popup -->

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

    <!-- Horizontal Resizer -->
    <div class="resizer horizontal-resizer" @mousedown="startResize('horizontal', $event)"></div>

    <!-- Log Console -->
    <LogConsole ref="logConsole" />
    
    <!-- Analysis Modal -->
    <div v-if="showAnalysisModal" class="modal-overlay" @click="closeAnalysisModal">
      <div class="modal-content analysis-modal" @click.stop>
        <div class="modal-header">
          <h3>Filesystem Analysis</h3>
          <button class="modal-close" @click="closeAnalysisModal">✕</button>
        </div>
        <div class="modal-body">
          <div v-if="analysisLoading" class="analysis-loading">
            <div class="spinner"></div>
            <p>Analyzing filesystem...</p>
          </div>
          <pre v-else class="analysis-result">{{ analysisResult }}</pre>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="copyAnalysisToClipboard">
            Copy to Clipboard
          </button>
          <button class="btn btn-primary" @click="closeAnalysisModal">
            Close
          </button>
        </div>
      </div>
    </div>
    
    <!-- Simulation Results Modal -->
    <div v-if="showSimulationModal" class="modal-overlay" @click="showSimulationModal = false">
      <div class="modal-content" @click.stop>
        <div class="modal-header">
          <h3>Simulation Complete</h3>
          <button class="modal-close" @click="showSimulationModal = false">✕</button>
        </div>
        <div class="modal-body">
          <div v-if="simulationReport" class="simulation-details">
            <div class="result-item">
              <span class="result-label">Estimated Time:</span>
              <span class="result-value">{{ formatDuration(simulationReport.estimated_time) }}</span>
            </div>
            
            <div class="result-item">
              <span class="result-label">Available Space:</span>
              <span class="result-value">{{ formatSize(simulationReport.space_after_format) }}</span>
            </div>
            
            <div v-if="simulationReport.warnings.length > 0" class="warnings-box">
              <div class="warning-title">Important Information:</div>
              <div v-for="(warning, i) in simulationReport.warnings" :key="i" class="warning-item">
                • {{ warning }}
              </div>
            </div>
            
            <div v-if="simulationReport.required_tools?.length > 0" class="info-box">
              <div class="info-title">Required Tools:</div>
              <div v-for="(tool, i) in simulationReport.required_tools" :key="i">
                • {{ tool }}
              </div>
            </div>
          </div>
          
          <div class="success-message">
            ✅ Simulation successful! You can now format the drive.
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showSimulationModal = false">
            Close
          </button>
          <button class="btn btn-primary" @click="showSimulationModal = false; executeFormat()" :disabled="!canFormat">
            Format Drive Now
          </button>
        </div>
      </div>
    </div>
    
    <!-- Clean Disk Dialog -->
    <div v-if="showCleanDialog" class="modal-overlay" @click="closeCleanDialog">
      <div class="modal-content clean-modal" @click.stop>
        <div class="modal-header">
          <h3>Clean Disk</h3>
          <button class="modal-close" @click="closeCleanDialog">✕</button>
        </div>
        <div class="modal-body">
          <div class="warning-box">
            <span class="warning-icon">⚠️</span>
            <div>
              <strong>Warning!</strong> This will remove ALL data and partition structures from:
              <div class="device-name">{{ selectedDevice?.name }} ({{ formatBytes(selectedDevice?.size || 0) }})</div>
            </div>
          </div>
          
          <div class="clean-options">
            <label class="radio-option">
              <input type="radio" v-model="cleanMethod" value="quick" />
              <span>
                <strong>Quick Clean</strong> - Remove partition tables only (fastest)
              </span>
            </label>
            <label class="radio-option">
              <input type="radio" v-model="cleanMethod" value="zero" />
              <span>
                <strong>Zero Fill</strong> - Overwrite entire disk with zeros
              </span>
            </label>
            <label class="radio-option">
              <input type="radio" v-model="cleanMethod" value="dod" />
              <span>
                <strong>DoD 5220.22-M</strong> - 3-pass secure wipe
              </span>
            </label>
            <label class="radio-option">
              <input type="radio" v-model="cleanMethod" value="random" />
              <span>
                <strong>Random Data</strong> - Overwrite with random data
              </span>
            </label>
          </div>
          
          <div v-if="cleanProgress.active" class="progress-section">
            <div class="progress-bar">
              <div class="progress-fill" :style="{ width: cleanProgress.percent + '%' }"></div>
            </div>
            <p>{{ cleanProgress.message }}</p>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="closeCleanDialog" :disabled="cleanProgress.active">
            Cancel
          </button>
          <button 
            class="btn btn-danger" 
            @click="executeClean" 
            :disabled="cleanProgress.active"
          >
            {{ cleanProgress.active ? 'Cleaning...' : 'Clean Disk' }}
          </button>
        </div>
      </div>
    </div>
    
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
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import LogConsole from './components/LogConsole.vue'
import FileBrowser from './components/FileBrowser.vue'

interface Device {
  id: string
  name: string
  size: number
  device_type: string
  mount_points: string[]
  is_removable: boolean
  is_system: boolean
  filesystem?: string
}

interface FormatOptions {
  filesystem_type: string
  label: string
  cluster_size: number | null
  quick_format: boolean
  enable_compression: boolean
  verify_after_format: boolean
  create_partition_table: boolean
  clean_before_format: boolean
  additional_options: Record<string, string>
}

interface SimulationReport {
  estimated_time: number | { secs: number, nanos?: number } // Can be either seconds or Rust Duration
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
const isElevated = ref(false)
const simulationReport = ref<SimulationReport | null>(null)
const formatProgress = ref(0)
const progressStatus = ref('')
const formatTime = ref('00:00')
const currentOperation = ref('')
const viewMode = ref<'browse' | 'format'>('browse') // Default to browse mode

// Resizable panes state
const isResizing = ref(false)
const resizeType = ref<'vertical' | 'horizontal' | null>(null)
const leftPane = ref<HTMLElement | null>(null)
const mainContent = ref<HTMLElement | null>(null)

// Analysis modal state
const showAnalysisModal = ref(false)
const analysisLoading = ref(false)
const analysisResult = ref('')

// Simulation modal state
const showSimulationModal = ref(false)

// Clean disk state
const showCleanDialog = ref(false)
const cleanMethod = ref('quick')
const cleanProgress = ref({
  active: false,
  percent: 0,
  message: ''
})

const formatOptions = ref<FormatOptions>({
  filesystem_type: '',
  label: '',
  cluster_size: null,
  quick_format: true,
  enable_compression: false,
  verify_after_format: false,
  create_partition_table: true,
  clean_before_format: false,  // Default to false to preserve current behavior
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

// Device with filesystem info for FileBrowser
const selectedDeviceWithFs = computed(() => {
  if (!selectedDevice.value) return null
  
  // Use the filesystem already detected by the backend
  return {
    ...selectedDevice.value,
    filesystem: selectedDevice.value.filesystem || 'unknown',
    readable: isFilesystemReadable(selectedDevice.value)
  }
})

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

// Resizing Methods
const startResize = (type: 'vertical' | 'horizontal', event: MouseEvent) => {
  isResizing.value = true
  resizeType.value = type
  
  const startX = event.clientX
  const startY = event.clientY
  
  if (type === 'vertical' && leftPane.value) {
    const startWidth = leftPane.value.offsetWidth
    
    const doDrag = (e: MouseEvent) => {
      if (!leftPane.value) return
      const newWidth = startWidth + e.clientX - startX
      // Constrain width between 200px and 500px
      leftPane.value.style.width = `${Math.min(500, Math.max(200, newWidth))}px`
    }
    
    const stopDrag = () => {
      document.removeEventListener('mousemove', doDrag)
      document.removeEventListener('mouseup', stopDrag)
      isResizing.value = false
      resizeType.value = null
      document.body.classList.remove('resizing')
    }
    
    document.addEventListener('mousemove', doDrag)
    document.addEventListener('mouseup', stopDrag)
    document.body.classList.add('resizing')
  } else if (type === 'horizontal') {
    const logConsoleElement = logConsole.value?.$el as HTMLElement
    if (!logConsoleElement) return
    
    const startHeight = logConsoleElement.offsetHeight
    
    const doDrag = (e: MouseEvent) => {
      const newHeight = startHeight - (e.clientY - startY)
      // Constrain height between 100px and 500px
      logConsoleElement.style.height = `${Math.min(500, Math.max(100, newHeight))}px`
    }
    
    const stopDrag = () => {
      document.removeEventListener('mousemove', doDrag)
      document.removeEventListener('mouseup', stopDrag)
      isResizing.value = false
      resizeType.value = null
      document.body.classList.remove('resizing')
    }
    
    document.addEventListener('mousemove', doDrag)
    document.addEventListener('mouseup', stopDrag)
    document.body.classList.add('resizing')
  }
}

// Handle option relationships
// Note: Removed handleCleanChange as the options are now independent
// - Create Partition Table: Always replaces existing partitions
// - Deep Clean: Extra step for problematic disks

// Watch for quick format changes to handle verify option
watch(() => formatOptions.value.quick_format, (isQuick) => {
  if (isQuick && formatOptions.value.verify_after_format) {
    // Disable verify for quick format as it's not useful
    formatOptions.value.verify_after_format = false
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

const formatFilesystemName = (fs: string): string => {
  if (!fs || fs === 'unknown') return 'Unknown'
  
  // Format common filesystem names nicely
  const fsMap: Record<string, string> = {
    'ntfs': 'NTFS',
    'fat32': 'FAT32',
    'fat16': 'FAT16',
    'exfat': 'exFAT',
    'ext2': 'ext2',
    'ext3': 'ext3',
    'ext4': 'ext4',
    'apfs': 'APFS',
    'hfs+': 'HFS+',
    'btrfs': 'Btrfs',
    'xfs': 'XFS',
    'zfs': 'ZFS',
    'gpt': 'GPT',
    'gpt-empty': 'GPT (Empty)',
    'mbr': 'MBR',
    'mbr-empty': 'MBR (Empty)',
    'uninitialized': 'Uninitialized'
  }
  
  return fsMap[fs.toLowerCase()] || fs
}

// Cache for analysis results to avoid re-analyzing
const analysisCache = ref<Map<string, string>>(new Map())

const formatDuration = (duration: number | { secs: number, nanos: number }): string => {
  // Handle both formats: simple number (seconds) or Duration object from Rust
  let seconds: number
  
  if (typeof duration === 'number') {
    seconds = duration
  } else if (duration && typeof duration === 'object' && 'secs' in duration) {
    // Rust Duration object with secs and nanos fields
    seconds = duration.secs + (duration.nanos || 0) / 1_000_000_000
  } else {
    console.warn('Invalid duration format:', duration)
    return 'Unknown'
  }
  
  // Handle invalid numbers
  if (!isFinite(seconds) || isNaN(seconds)) {
    return 'Unknown'
  }
  
  seconds = Math.round(seconds) // Round to nearest second
  
  if (seconds < 60) return `${seconds} seconds`
  const minutes = Math.floor(seconds / 60)
  const secs = seconds % 60
  return secs > 0 ? `${minutes}m ${secs}s` : `${minutes} minute${minutes !== 1 ? 's' : ''}`
}

const selectDevice = (device: Device) => {
  if (isFormatting.value) return
  selectedDevice.value = device
  simulationReport.value = null
  // Default to browse mode when selecting a new device
  viewMode.value = 'browse'
}

// Detect filesystem type from device (placeholder - would call backend)
const detectFilesystem = (device: Device): string => {
  // This would actually call a backend method to detect the filesystem
  // For now, return a placeholder
  if (device.mount_points.length > 0) {
    const mountPoint = device.mount_points[0].toLowerCase()
    if (mountPoint.includes('c:') || mountPoint.includes('windows')) {
      return 'ntfs'
    } else if (mountPoint.includes('boot')) {
      return 'fat32'
    }
  }
  return 'unknown'
}

// Check if we can read this filesystem
const isFilesystemReadable = (device: Device): boolean => {
  const fs = device.filesystem || 'unknown'
  // We can read ext2/3/4, FAT32, and exFAT so far
  return ['ext2', 'ext3', 'ext4', 'fat32', 'exfat'].includes(fs.toLowerCase())
}

// Handle file operations from FileBrowser
const handleCopyFiles = (event: any) => {
  console.log('Copy files:', event)
  // TODO: Implement cross-filesystem copy
}

const handleUpdateFilesystem = (event: { deviceId: string, filesystem: string }) => {
  // Find and update the device in the devices array
  const deviceIndex = devices.value.findIndex(d => d.id === event.deviceId)
  if (deviceIndex !== -1) {
    devices.value[deviceIndex] = {
      ...devices.value[deviceIndex],
      filesystem: event.filesystem
    }
    console.log(`Updated device ${event.deviceId} filesystem to: ${event.filesystem}`)
  }
}

const handleExportFiles = (event: any) => {
  console.log('Export files:', event)
  // TODO: Implement file export
}

const handleShowProperties = (file: any) => {
  console.log('Show properties:', file)
  // TODO: Show file properties dialog
}

const toggleTheme = () => {
  isDarkMode.value = !isDarkMode.value
  // Save preference to localStorage
  localStorage.setItem('theme', isDarkMode.value ? 'dark' : 'light')
}

// Utility functions
const formatBytes = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
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
      label: formatOptions.value.label?.trim() || null,
      additional_options: {
        ...formatOptions.value.additional_options,
        create_partition_table: formatOptions.value.create_partition_table ? 'true' : 'false'
      }
    }
    console.log('Starting simulation for:', selectedDevice.value.name, options)
    simulationReport.value = await invoke('simulate_format', {
      device: selectedDevice.value,
      options: options
    })
    console.log('Simulation successful:', simulationReport.value)
    console.log('Estimated time received:', simulationReport.value.estimated_time)
    showSimulationModal.value = true
  } catch (error) {
    console.error('Simulation failed:', error)
    alert(`Simulation failed: ${error}`)
    simulationReport.value = null
  } finally {
    isSimulating.value = false
  }
}

const checkElevation = async () => {
  try {
    isElevated.value = await invoke('check_elevation_status')
    return isElevated.value
  } catch (error) {
    console.error('Failed to check elevation status:', error)
    return false
  }
}

const analyzeFilesystem = async () => {
  if (!selectedDevice.value) {
    alert('Please select a drive to analyze')
    return
  }
  
  showAnalysisModal.value = true
  analysisLoading.value = true
  analysisResult.value = ''
  
  logConsole.value?.info(`Analyzing filesystem on ${selectedDevice.value.name}...`, 'Analyzer')
  
  try {
    const result = await invoke('analyze_filesystem', {
      deviceId: selectedDevice.value.id
    })
    
    analysisResult.value = result as string
    
    // Also get the filesystem type to update the device
    try {
      const fsType = await invoke('get_filesystem_type', {
        deviceId: selectedDevice.value.id
      }) as string
      
      if (fsType && fsType !== 'unknown') {
        selectedDevice.value.filesystem = fsType
        logConsole.value?.info(`Detected filesystem type: ${formatFilesystemName(fsType)}`, 'Analyzer')
        
        // Update in the devices list too
        const deviceIndex = devices.value.findIndex(d => d.id === selectedDevice.value!.id)
        if (deviceIndex >= 0) {
          devices.value[deviceIndex].filesystem = fsType
        }
      }
    } catch (e) {
      // If quick detection fails, try to extract from analysis
      let detectedType: string | null = null
      
      if (result.includes('GPT Header Found') && result.includes('No active partitions found')) {
        detectedType = 'gpt-empty'
        logConsole.value?.info('Detected: GPT disk with no partitions', 'Analyzer')
      } else if (result.includes('GPT Header Found')) {
        detectedType = 'gpt'
        logConsole.value?.info('Detected: GPT disk', 'Analyzer')
      } else if (result.includes('MBR with partition table')) {
        detectedType = 'mbr'
        logConsole.value?.info('Detected: MBR disk', 'Analyzer')
      }
      
      // Update both selected device and devices list
      if (detectedType) {
        selectedDevice.value.filesystem = detectedType
        const deviceIndex = devices.value.findIndex(d => d.id === selectedDevice.value!.id)
        if (deviceIndex >= 0) {
          devices.value[deviceIndex].filesystem = detectedType
        }
      }
    }
    
    logConsole.value?.info('Filesystem analysis completed', 'Analyzer')
  } catch (error: any) {
    const errorStr = error.toString()
    
    // Check if elevation is required
    if (errorStr.includes('ELEVATION_REQUIRED')) {
      logConsole.value?.info('Elevation required, requesting administrator privileges...', 'Analyzer')
      
      try {
        // Use socket-based analyze command for single UAC prompt
        const result = await invoke('analyze_filesystem_socket', {
          deviceId: selectedDevice.value.id
        })
        
        analysisResult.value = result as string
        logConsole.value?.info('Filesystem analysis completed with elevation', 'Analyzer')
      } catch (elevatedError: any) {
        console.error('Failed to analyze with elevation:', elevatedError)
        analysisResult.value = `Failed to analyze filesystem even with elevation:\n${elevatedError}`
        logConsole.value?.error(`Analysis failed: ${elevatedError}`, 'Analyzer')
      }
    } else if (errorStr.includes('os error 5') || errorStr.includes('Access is denied') || 
               errorStr.includes('Åtkomst nekad')) {
      // Other access denied errors
      analysisResult.value = `Administrator privileges required\n\n` +
        `To analyze this filesystem, please:\n` +
        `1. Close Moses\n` +
        `2. Right-click on Moses\n` +
        `3. Select "Run as administrator"\n` +
        `4. Try the analysis again\n\n` +
        `This is required to read raw device sectors.`
      
      logConsole.value?.error('Analysis requires administrator privileges', 'Analyzer')
    } else {
      console.error('Failed to analyze filesystem:', error)
      analysisResult.value = `Failed to analyze filesystem:\n${error}`
      logConsole.value?.error(`Analysis failed: ${error}`, 'Analyzer')
    }
  } finally {
    analysisLoading.value = false
  }
}

const closeAnalysisModal = () => {
  showAnalysisModal.value = false
}

const copyAnalysisToClipboard = async () => {
  try {
    await navigator.clipboard.writeText(analysisResult.value)
    logConsole.value?.info('Analysis copied to clipboard', 'Analyzer')
  } catch (error) {
    console.error('Failed to copy to clipboard:', error)
    logConsole.value?.error('Failed to copy to clipboard', 'Analyzer')
  }
}

// Clean disk methods
const showCleanDiskDialog = () => {
  if (!selectedDevice.value) {
    alert('Please select a drive to clean')
    return
  }
  
  if (selectedDevice.value.is_system) {
    alert('Cannot clean system drive!')
    return
  }
  
  showCleanDialog.value = true
  cleanMethod.value = 'quick'
  cleanProgress.value = {
    active: false,
    percent: 0,
    message: ''
  }
}

const closeCleanDialog = () => {
  if (!cleanProgress.value.active) {
    showCleanDialog.value = false
  }
}

const executeClean = async () => {
  if (!selectedDevice.value) return
  
  const confirmMsg = `Are you sure you want to clean ${selectedDevice.value.name}?\n\n` +
    `This will permanently remove ALL data and partition structures!\n\n` +
    `Method: ${cleanMethod.value.toUpperCase()}`
  
  if (!confirm(confirmMsg)) return
  
  cleanProgress.value.active = true
  cleanProgress.value.percent = 0
  cleanProgress.value.message = 'Starting disk clean...'
  
  logConsole.value?.info(`Starting ${cleanMethod.value} clean on ${selectedDevice.value.name}`, 'Cleaner')
  
  try {
    // Start clean operation
    cleanProgress.value.percent = 10
    cleanProgress.value.message = 'Preparing disk for cleaning...'
    
    // Use socket-based clean command
    const result = await invoke('clean_disk_socket', {
      request: {
        device_id: selectedDevice.value.id,
        wipe_method: cleanMethod.value
      }
    })
    
    cleanProgress.value.percent = 100
    cleanProgress.value.message = 'Clean completed successfully!'
    logConsole.value?.success(`Disk cleaned successfully: ${result}`, 'Cleaner')
    
    // Refresh devices after clean
    setTimeout(() => {
      refreshDevices()
      closeCleanDialog()
    }, 2000)
    
  } catch (error) {
    console.error('Clean failed:', error)
    logConsole.value?.error(`Clean failed: ${error}`, 'Cleaner')
    cleanProgress.value.message = `Clean failed: ${error}`
    
    setTimeout(() => {
      cleanProgress.value.active = false
    }, 3000)
  }
}

const executeFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  // Log elevation status if not elevated (but don't show popup - UAC will handle that)
  if (navigator.userAgent.includes('Windows')) {
    const elevated = await checkElevation()
    if (!elevated) {
      logConsole.value?.info('Administrator privileges will be requested for the format operation', 'System')
    }
  }
  
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
  progressStatus.value = 'Requesting administrator privileges...'
  currentOperation.value = 'Waiting for UAC approval'
  
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
    // Clean the disk first if requested
    if (formatOptions.value.clean_before_format) {
      logConsole.value?.info('Cleaning disk before format...', 'Formatter')
      progressStatus.value = 'Cleaning disk...'
      currentOperation.value = 'Removing partition structures'
      
      try {
        // Use socket-based clean command if available
        const cleanResult = await invoke('clean_disk_socket', {
          request: {
            device_id: selectedDevice.value.id,
            wipe_method: 'quick'  // Quick clean is sufficient before format
          }
        })
        
        // Log success if we got here
        if (logConsole.value) {
          logConsole.value.info('Disk cleaned successfully', 'Formatter')
        }
        
        // Brief pause to let the system recognize the clean disk
        await new Promise(resolve => setTimeout(resolve, 1000))
      } catch (cleanError: any) {
        // Log the error safely
        if (logConsole.value) {
          logConsole.value.error(`Clean failed: ${cleanError}`, 'Formatter')
        }
        console.error('Clean error:', cleanError)
        // Continue with format anyway - the format operation may still succeed
      }
    }
    
    // Add create_partition_table to additional_options
    const options = {
      ...formatOptions.value,
      additional_options: {
        ...formatOptions.value.additional_options,
        create_partition_table: formatOptions.value.create_partition_table ? 'true' : 'false'
      }
    }
    
    progressStatus.value = 'Formatting...'
    currentOperation.value = 'Creating filesystem'
    
    // Use socket-based format for single UAC prompt
    await invoke('format_disk_socket', {
      device: selectedDevice.value,
      options
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
  
  // Check elevation status on Windows
  if (navigator.userAgent.includes('Windows')) {
    const elevated = await checkElevation()
    if (elevated) {
      logConsole.value?.info('Running with administrator privileges', 'System')
    } else {
      logConsole.value?.info('Running without administrator privileges', 'System')
      logConsole.value?.info('Administrator privileges will be requested when formatting', 'System')
    }
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

/* Root level defaults (fallback values) */
:root {
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
  width: 260px;
  min-width: 200px;
  max-width: 400px;
  overflow: auto;
  flex-shrink: 0;
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

/* Sidebar drive info - must be specific to avoid header override */
.drive-item .drive-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.drive-item .drive-name {
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

.filesystem-info {
  color: var(--text-tertiary);
  font-weight: 500;
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
  min-width: 600px;
}

/* Empty Panel */
.empty-panel, .warning-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 24px;
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

/* Unified Drive Header (matches FileBrowser.vue exactly) */
.drive-header {
  padding: 8px 16px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  gap: 12px;
}

/* Header drive info - specific to header */
.drive-header .drive-info {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.drive-header .drive-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.drive-separator {
  color: var(--text-secondary);
  opacity: 0.5;
  font-size: 12px;
}

.drive-id {
  font-size: 11px;
  color: var(--text-secondary);
  opacity: 0.8;
}

/* Options Container */
.options-container {
  flex: 1;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
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
  padding: 4px 6px;
  background: var(--bg-input);
  border: 1px solid var(--border-color);
  border-radius: 3px;
  color: var(--text-primary);
  font-size: 12px;
  transition: all 0.15s;
}

.form-control:focus {
  outline: none;
  border-color: var(--accent);
  background: var(--bg-tertiary);
}

.form-hint {
  display: block;
  margin-top: 2px;
  font-size: 10px;
  color: var(--text-secondary);
}

/* New layout for left/right columns */
/* New horizontal layout for format options */
.options-row-basic {
  display: flex;
  gap: 30px;
  margin-bottom: 20px;
  align-items: flex-start;
}

.options-row-basic .flex-grow {
  flex: 1;
  max-width: 400px;
}

.options-row-advanced {
  display: flex;
  gap: 15px;
  flex-wrap: nowrap;
  align-items: flex-start;
  justify-content: space-between;
}

/* Compact option sections for horizontal layout */
.option-section.compact {
  flex: 1;
  min-width: 160px;
  margin-bottom: 0;
  padding: 10px 14px;
}

.radio-group.horizontal {
  display: flex;
  flex-direction: row;
  gap: 12px;
}

.radio-label.compact,
.checkbox-label.compact {
  padding: 2px 0;
}

.radio-label.compact .radio-text,
.checkbox-label.compact .checkbox-text {
  font-size: 13px;
}

.radio-label.compact .radio-circle,
.checkbox-label.compact .checkbox-box {
  width: 16px;
  height: 16px;
  margin-right: 8px;
}

.option-section.compact .section-title {
  font-size: 10px;
  margin-bottom: 8px;
}

.checkbox-hint, .option-hint {
  font-size: 10px;
  line-height: 1.2;
}

.options-left {
  flex: 1;
  min-width: 200px;
}

.options-right {
  flex: 0 0 auto;
  min-width: 220px;
  padding-top: 10px;
  padding-left: 20px;
}

.checkbox-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
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
  width: 18px;
  height: 18px;
  background: var(--bg-primary);
  border: 2px solid var(--accent);
  border-radius: 3px;
  margin-right: 8px;
  flex-shrink: 0;
  position: relative;
  transition: all 0.2s;
}

.checkbox-label:hover .checkbox-box {
  background: var(--bg-hover);
  box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.2);
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
  top: -1px;
  left: 3px;
  color: white;
  font-size: 14px;
  font-weight: bold;
}

.checkbox-text {
  display: flex;
  flex-direction: column;
}

.checkbox-hint, .option-hint {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 2px;
}

/* New styles for reorganized options */
.option-section {
  margin-bottom: 24px;
  padding: 12px;
  background: var(--bg-tertiary, rgba(0, 0, 0, 0.02));
  border-radius: 6px;
  border: 1px solid var(--border-light, rgba(0, 0, 0, 0.05));
}

.theme-dark .option-section {
  background: rgba(255, 255, 255, 0.02);
  border-color: rgba(255, 255, 255, 0.05);
}

.section-title {
  font-size: 11px;
  font-weight: 700;
  color: var(--accent, #6366f1);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  margin-bottom: 10px;
  opacity: 0.8;
}

/* Radio button styles */
.radio-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.radio-label {
  display: flex;
  align-items: flex-start;
  cursor: pointer;
  padding: 4px 0;
}

.radio-label input[type="radio"] {
  display: none;
}

.radio-circle {
  width: 18px;
  height: 18px;
  border: 2px solid var(--border-color);
  border-radius: 50%;
  margin-right: 10px;
  flex-shrink: 0;
  position: relative;
  transition: all 0.2s;
  background: var(--bg-primary);
}

.radio-label:hover .radio-circle {
  background: var(--bg-hover);
  box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.2);
}

.radio-label input:checked + .radio-circle {
  border-color: var(--accent);
}

.radio-label input:checked + .radio-circle::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent);
}

.radio-text {
  flex: 1;
  display: flex;
  flex-direction: column;
}

/* Disabled state styles */
.checkbox-box.disabled {
  opacity: 0.5;
  cursor: not-allowed;
  background: var(--bg-disabled, #f0f0f0);
}

.checkbox-text.disabled {
  opacity: 0.5;
  cursor: not-allowed;
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
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid var(--border-color);
  font-size: 10px;
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

/* Modal Styles */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(2px);
}

.modal-content {
  background: var(--bg-secondary, #2d2d30);
  border-radius: 8px;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  max-width: 80%;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  position: relative;
  border: 1px solid var(--border-color, #444);
  color: var(--text-primary, #ffffff);
}

.analysis-modal {
  width: 900px;
}

.clean-modal {
  width: 600px;
}

.modal-body {
  padding: 20px;
  flex: 1;
  overflow-y: auto;
  color: var(--text-primary, #000000);
  background: transparent;
}

.warning-box {
  display: flex;
  gap: 12px;
  padding: 16px;
  background: rgba(255, 193, 7, 0.1);
  border: 1px solid rgba(255, 193, 7, 0.3);
  border-radius: 6px;
  margin-bottom: 20px;
}

.theme-light .warning-box {
  background: #fff3cd;
  border-color: #ffc107;
  color: #856404;
}

.theme-dark .warning-box {
  background: rgba(255, 193, 7, 0.15);
  border-color: rgba(255, 193, 7, 0.4);
  color: #ffc107;
}

.warning-icon {
  font-size: 24px;
  flex-shrink: 0;
}

.device-name {
  font-weight: 600;
  color: var(--accent-color);
  margin-top: 8px;
}

.clean-options {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 20px;
}

.radio-option {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

.radio-option:hover {
  background: var(--hover-bg);
  border-color: var(--accent-color);
}

.radio-option input[type="radio"] {
  margin-top: 2px;
}

.radio-option span {
  flex: 1;
}

.radio-option strong {
  display: block;
  margin-bottom: 4px;
  color: var(--text-primary);
}

.progress-section {
  margin-top: 20px;
}

.progress-bar {
  height: 8px;
  background: var(--border-color);
  border-radius: 4px;
  overflow: hidden;
  margin-bottom: 8px;
}

.progress-fill {
  height: 100%;
  background: var(--accent-color);
  transition: width 0.3s ease;
}

.btn-danger {
  background: #dc3545;
  color: white;
}

.btn-danger:hover:not(:disabled) {
  background: #c82333;
}

.modal-header {
  padding: 16px 20px;
  border-bottom: 1px solid var(--border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.modal-header h3 {
  color: var(--text-primary);
  font-size: 16px;
  font-weight: 500;
}

.modal-close {
  background: none;
  border: none;
  color: var(--text-secondary);
  font-size: 20px;
  cursor: pointer;
  padding: 0;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.modal-close:hover {
  background: var(--bg-hover);
}

.modal-body {
  flex: 1;
  overflow: auto;
  padding: 20px;
}

.analysis-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 40px;
}

.analysis-loading .spinner {
  width: 40px;
  height: 40px;
  border: 3px solid var(--border-color);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-bottom: 16px;
}

.analysis-result {
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-primary);
  background: var(--bg-primary);
  padding: 16px;
  border-radius: 4px;
  white-space: pre-wrap;
  word-wrap: break-word;
}

.modal-footer {
  padding: 16px 20px;
  border-top: 1px solid var(--border-color);
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.btn {
  padding: 8px 16px;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  border: none;
}

.btn-primary {
  background: var(--accent);
  color: white;
}

.btn-primary:hover {
  background: var(--accent-hover);
}

.btn-secondary {
  background: var(--bg-tertiary);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.btn-secondary:hover {
  background: var(--bg-hover);
}
</style>