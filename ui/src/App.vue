<template>
  <div class="container">
    <header>
      <h1>Moses Drive Formatter</h1>
      <p class="subtitle">Cross-platform drive formatting made easy</p>
    </header>

    <main>
      <section class="device-selection">
        <h2>Select Drive</h2>
        <div v-if="loading" class="loading">Loading devices...</div>
        <div v-else-if="devices.length === 0" class="empty-state">
          No drives detected. Please connect a drive and refresh.
        </div>
        <div v-else class="device-list">
          <div 
            v-for="device in devices" 
            :key="device.id"
            :class="['device-card', { selected: selectedDevice?.id === device.id }]"
            @click="selectDevice(device)"
          >
            <div class="device-icon">
              {{ getDeviceIcon(device.device_type) }}
            </div>
            <div class="device-info">
              <h3>{{ device.name }}</h3>
              <p class="device-type">{{ device.device_type }}</p>
              <p class="device-size">{{ formatSize(device.size) }}</p>
              <div v-if="device.is_system" class="warning-badge">System Drive</div>
            </div>
          </div>
        </div>
        <button @click="refreshDevices" class="btn-secondary">Refresh Devices</button>
      </section>

      <section v-if="selectedDevice" class="format-options">
        <h2>Format Options</h2>
        
        <div class="form-group">
          <label for="filesystem">Filesystem Type</label>
          <select id="filesystem" v-model="formatOptions.filesystem_type">
            <option value="">Select filesystem...</option>
            <option value="ext4">EXT4</option>
            <option value="ntfs">NTFS</option>
            <option value="fat32">FAT32</option>
            <option value="exfat">exFAT</option>
          </select>
        </div>

        <div class="form-group">
          <label for="label">Volume Label (Optional)</label>
          <input 
            id="label" 
            type="text" 
            v-model="formatOptions.label"
            placeholder="Enter volume label"
            maxlength="32"
          />
        </div>

        <div class="form-group">
          <label>
            <input type="checkbox" v-model="formatOptions.quick_format" />
            Quick Format
          </label>
        </div>

        <div class="action-buttons">
          <button 
            @click="simulateFormat" 
            :disabled="!canFormat"
            class="btn-primary"
          >
            Dry Run (Simulate)
          </button>
          <button 
            @click="executeFormat" 
            :disabled="!canFormat || !simulationComplete"
            class="btn-danger"
          >
            Format Drive
          </button>
        </div>
      </section>

      <section v-if="simulationReport" class="simulation-report">
        <h2>Simulation Report</h2>
        <div class="report-content">
          <p><strong>Estimated Time:</strong> {{ formatDuration(simulationReport.estimated_time) }}</p>
          <p><strong>Space After Format:</strong> {{ formatSize(simulationReport.space_after_format) }}</p>
          
          <div v-if="simulationReport.warnings.length > 0" class="warnings">
            <h3>Warnings</h3>
            <ul>
              <li v-for="(warning, index) in simulationReport.warnings" :key="index">
                {{ warning }}
              </li>
            </ul>
          </div>
          
          <div v-if="simulationReport.required_tools.length > 0" class="required-tools">
            <h3>Required Tools</h3>
            <ul>
              <li v-for="(tool, index) in simulationReport.required_tools" :key="index">
                {{ tool }}
              </li>
            </ul>
          </div>
        </div>
      </section>
    </main>
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
}

interface SimulationReport {
  estimated_time: number
  warnings: string[]
  required_tools: string[]
  space_after_format: number
}

const devices = ref<Device[]>([])
const selectedDevice = ref<Device | null>(null)
const loading = ref(false)
const simulationReport = ref<SimulationReport | null>(null)
const simulationComplete = ref(false)

const formatOptions = ref<FormatOptions>({
  filesystem_type: '',
  label: '',
  quick_format: true,
})

const canFormat = computed(() => {
  return selectedDevice.value && 
         formatOptions.value.filesystem_type && 
         !selectedDevice.value.is_system
})

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
  return `${minutes} minute${minutes !== 1 ? 's' : ''}`
}

const selectDevice = (device: Device) => {
  selectedDevice.value = device
  simulationReport.value = null
  simulationComplete.value = false
}

const refreshDevices = async () => {
  loading.value = true
  try {
    devices.value = await invoke('enumerate_devices')
  } catch (error) {
    console.error('Failed to enumerate devices:', error)
  } finally {
    loading.value = false
  }
}

const simulateFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  try {
    simulationReport.value = await invoke('simulate_format', {
      device: selectedDevice.value,
      options: formatOptions.value
    })
    simulationComplete.value = true
  } catch (error) {
    console.error('Simulation failed:', error)
    alert(`Simulation failed: ${error}`)
  }
}

const executeFormat = async () => {
  if (!selectedDevice.value || !formatOptions.value.filesystem_type) return
  
  const confirmMessage = `WARNING: This will erase all data on ${selectedDevice.value.name}. This action cannot be undone. Are you sure you want to continue?`
  
  if (!confirm(confirmMessage)) return
  
  try {
    await invoke('execute_format', {
      device: selectedDevice.value,
      options: formatOptions.value
    })
    alert('Format completed successfully!')
    await refreshDevices()
    selectedDevice.value = null
    simulationReport.value = null
    simulationComplete.value = false
  } catch (error) {
    console.error('Format failed:', error)
    alert(`Format failed: ${error}`)
  }
}

onMounted(() => {
  refreshDevices()
})
</script>