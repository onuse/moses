<template>
  <div class="file-browser">
    <!-- Drive header (unified with Format mode) -->
    <div class="drive-header">
      <div class="drive-info">
        <span class="drive-name">{{ drive.name }}</span>
        <span class="drive-separator">/</span>
        <span class="drive-id">{{ drive.id }}</span>
      </div>
      <div class="breadcrumb-path" v-if="currentPath !== '/'">
        <span class="path-separator">›</span>
        <button @click="navigateTo('/')" class="breadcrumb-item">
          Root
        </button>
        <template v-for="(segment, index) in pathSegments" :key="index">
          <span class="breadcrumb-separator">/</span>
          <button 
            @click="navigateTo(getPathToSegment(index))" 
            class="breadcrumb-item"
          >
            {{ segment }}
          </button>
        </template>
      </div>
    </div>

    <!-- Toolbar -->
    <div class="toolbar">
      <button @click="navigateUp" :disabled="currentPath === '/'">
        <i class="icon-up"></i> Up
      </button>
      <button @click="refresh">
        <i class="icon-refresh"></i> Refresh
      </button>
      <div class="view-toggles">
        <button 
          @click="viewMode = 'list'" 
          :class="{ active: viewMode === 'list' }"
        >
          <i class="icon-list"></i>
        </button>
        <button 
          @click="viewMode = 'grid'" 
          :class="{ active: viewMode === 'grid' }"
        >
          <i class="icon-grid"></i>
        </button>
      </div>
      <div class="search-box">
        <input 
          v-model="searchQuery" 
          placeholder="Search files..."
          @input="filterFiles"
        />
      </div>
    </div>

    <!-- File list/grid -->
    <div 
      class="file-container" 
      :class="`view-${viewMode}`"
      @contextmenu.prevent="showContextMenu"
    >
      <!-- Loading state -->
      <div v-if="loading" class="loading-state">
        <div class="spinner"></div>
        <p>Reading {{ drive.filesystem || 'unknown' }} filesystem...</p>
      </div>

      <!-- Error state -->
      <div v-else-if="error" class="error-state">
        <i class="icon-error"></i>
        <p>{{ error }}</p>
        <button @click="retry">Retry</button>
        <!-- Special handling for unknown filesystems -->
        <div v-if="error.includes('unknown filesystem') || error.includes('Unable to detect')" class="unknown-fs-options">
          <p class="help-text">This drive's filesystem couldn't be detected. It might be ext4, ext3, or another Linux filesystem.</p>
          <button @click="detectWithAdmin" class="admin-button">
            <i class="icon-shield"></i> Analyze Filesystem
          </button>
          <div class="try-options">
            <p>Or try reading as:</p>
            <button @click="tryFilesystem('ext4')">ext4</button>
            <button @click="tryFilesystem('ext3')">ext3</button>
            <button @click="tryFilesystem('exfat')">exFAT</button>
          </div>
        </div>
      </div>

      <!-- File list -->
      <div v-else class="file-list">
        <!-- Parent directory (if not root) -->
        <div 
          v-if="currentPath !== '/'"
          class="file-item parent-dir"
          @dblclick="navigateUp"
        >
          <i class="icon-folder-up"></i>
          <span class="file-name">..</span>
        </div>

        <!-- Files and folders -->
        <div
          v-for="item in filteredItems"
          :key="item.path"
          class="file-item"
          :class="{ 
            selected: isSelected(item),
            folder: item.type === 'directory'
          }"
          @click="selectItem(item, $event)"
          @dblclick="openItem(item)"
          draggable="true"
          @dragstart="startDrag(item, $event)"
        >
          <i :class="getFileIcon(item)"></i>
          <span class="file-name">{{ item.name }}</span>
          <span class="file-size">{{ formatSize(item.size) }}</span>
          <span class="file-date">{{ formatDate(item.modified) }}</span>
        </div>

        <!-- Empty state -->
        <div v-if="filteredItems.length === 0" class="empty-state">
          <p v-if="searchQuery">No files match "{{ searchQuery }}"</p>
          <p v-else>This folder is empty</p>
        </div>
      </div>
    </div>

    <!-- Status bar -->
    <div class="status-bar">
      <span class="item-count">
        {{ selectedItems.length }} of {{ filteredItems.length }} items selected
      </span>
      <span class="selection-size" v-if="selectedItems.length > 0">
        {{ formatSize(selectionSize) }}
      </span>
      <span class="drive-info">
        {{ formatFilesystemName(drive.filesystem) }} · {{ formatSize(drive.used || 0) }} used of {{ formatSize(drive.size) }}
      </span>
    </div>

    <!-- Context menu -->
    <div 
      v-if="contextMenuVisible"
      class="context-menu"
      :style="contextMenuStyle"
      @click.stop
    >
      <button @click="copySelected">
        <i class="icon-copy"></i> Copy to...
      </button>
      <button @click="exportSelected">
        <i class="icon-export"></i> Export
      </button>
      <hr>
      <button @click="showProperties">
        <i class="icon-info"></i> Properties
      </button>
    </div>
  </div>
</template>

<script>
import { ref, computed, watch, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export default {
  name: 'FileBrowser',
  props: {
    drive: {
      type: Object,
      required: true
    }
  },
  emits: ['copy-files', 'export-files', 'show-properties', 'update-filesystem'],
  setup(props, { emit }) {
    // State
    const currentPath = ref('/')
    const items = ref([])
    const selectedItems = ref([])
    const searchQuery = ref('')
    const viewMode = ref('list') // 'list' or 'grid'
    const loading = ref(false)
    const error = ref(null)
    const contextMenuVisible = ref(false)
    const contextMenuStyle = ref({})

    // Computed
    const pathSegments = computed(() => {
      return currentPath.value
        .split('/')
        .filter(segment => segment !== '')
    })

    const filteredItems = computed(() => {
      if (!searchQuery.value) return items.value
      
      const query = searchQuery.value.toLowerCase()
      return items.value.filter(item => 
        item.name.toLowerCase().includes(query)
      )
    })

    const selectionSize = computed(() => {
      return selectedItems.value.reduce((sum, item) => sum + item.size, 0)
    })

    // Methods
    async function loadDirectory(path) {
      loading.value = true
      error.value = null
      
      try {
        // Call Rust backend to read directory
        // Pass mount points to avoid re-enumerating devices
        console.log('Drive mount_points:', props.drive.mount_points)
        const mountPointStrings = props.drive.mount_points ? 
          props.drive.mount_points.map(p => {
            const str = typeof p === 'string' ? p : p.toString()
            console.log('Mount point converted:', p, '->', str)
            return str
          }) : undefined
        
        let result
        try {
          // First try normal read
          result = await invoke('read_directory', {
            deviceId: props.drive.id,
            path: path,
            filesystem: props.drive.filesystem || 'unknown',
            mountPoints: mountPointStrings
          })
        } catch (normalErr) {
          // If access denied, try elevated read
          if (normalErr.toString().toLowerCase().includes('access') || 
              normalErr.toString().toLowerCase().includes('denied') ||
              normalErr.toString().toLowerCase().includes('permission')) {
            console.log('Normal read failed with access error, trying elevated read:', normalErr)
            result = await invoke('read_directory_elevated', {
              deviceId: props.drive.id,
              path: path,
              filesystem: props.drive.filesystem || 'unknown',
              mountPoints: mountPointStrings
            })
          } else {
            // Other errors, re-throw
            throw normalErr
          }
        }
        
        items.value = result.entries.map(entry => ({
          name: entry.name,
          path: entry.path,
          type: entry.entry_type,
          size: entry.size || 0,
          modified: entry.modified,
          permissions: entry.permissions
        }))
        
        // Sort: folders first, then alphabetically
        items.value.sort((a, b) => {
          if (a.type === 'directory' && b.type !== 'directory') return -1
          if (a.type !== 'directory' && b.type === 'directory') return 1
          return a.name.localeCompare(b.name)
        })
        
      } catch (err) {
        error.value = `Failed to read directory: ${err}`
        console.error('Failed to read directory:', err)
      } finally {
        loading.value = false
      }
    }

    function navigateTo(path) {
      currentPath.value = path
      selectedItems.value = []
      loadDirectory(path)
    }

    function navigateUp() {
      const segments = currentPath.value.split('/').filter(s => s)
      segments.pop()
      navigateTo('/' + segments.join('/'))
    }

    function getPathToSegment(index) {
      const segments = pathSegments.value.slice(0, index + 1)
      return '/' + segments.join('/')
    }

    function openItem(item) {
      if (item.type === 'directory') {
        navigateTo(item.path)
      } else {
        // For files, emit event to show file preview or start download
        emit('preview-file', item)
      }
    }

    function selectItem(item, event) {
      if (event.ctrlKey || event.metaKey) {
        // Toggle selection
        const index = selectedItems.value.findIndex(i => i.path === item.path)
        if (index >= 0) {
          selectedItems.value.splice(index, 1)
        } else {
          selectedItems.value.push(item)
        }
      } else if (event.shiftKey && selectedItems.value.length > 0) {
        // Range selection
        const lastSelected = selectedItems.value[selectedItems.value.length - 1]
        const lastIndex = filteredItems.value.findIndex(i => i.path === lastSelected.path)
        const currentIndex = filteredItems.value.findIndex(i => i.path === item.path)
        
        const start = Math.min(lastIndex, currentIndex)
        const end = Math.max(lastIndex, currentIndex)
        
        selectedItems.value = filteredItems.value.slice(start, end + 1)
      } else {
        // Single selection
        selectedItems.value = [item]
      }
    }

    function isSelected(item) {
      return selectedItems.value.some(i => i.path === item.path)
    }

    function getFileIcon(item) {
      if (item.type === 'directory') {
        return 'icon-folder'
      }
      
      // Determine icon based on file extension
      const ext = item.name.split('.').pop().toLowerCase()
      const iconMap = {
        // Documents
        'txt': 'icon-file-text',
        'pdf': 'icon-file-pdf',
        'doc': 'icon-file-word',
        'docx': 'icon-file-word',
        // Images
        'jpg': 'icon-file-image',
        'jpeg': 'icon-file-image',
        'png': 'icon-file-image',
        'gif': 'icon-file-image',
        // Code
        'js': 'icon-file-code',
        'rs': 'icon-file-code',
        'py': 'icon-file-code',
        'vue': 'icon-file-code',
        // Archives
        'zip': 'icon-file-zip',
        'tar': 'icon-file-zip',
        'gz': 'icon-file-zip',
      }
      
      return iconMap[ext] || 'icon-file'
    }

    function formatSize(bytes) {
      if (!bytes) return '0 B'
      const units = ['B', 'KB', 'MB', 'GB', 'TB']
      const index = Math.floor(Math.log(bytes) / Math.log(1024))
      return `${(bytes / Math.pow(1024, index)).toFixed(1)} ${units[index]}`
    }
    
    function formatFilesystemName(fs) {
      if (!fs || fs === 'unknown') return 'Unknown filesystem'
      
      // Format common filesystem names nicely
      const fsMap = {
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
        'gpt': 'GPT partition table',
        'gpt-empty': 'Empty GPT disk',
        'mbr': 'MBR partition table',
        'mbr-empty': 'Empty MBR disk',
        'uninitialized': 'Uninitialized disk'
      }
      
      return fsMap[fs.toLowerCase()] || fs + ' filesystem'
    }

    function formatDate(timestamp) {
      if (!timestamp) return ''
      return new Date(timestamp).toLocaleDateString()
    }

    function startDrag(item, event) {
      event.dataTransfer.effectAllowed = 'copy'
      event.dataTransfer.setData('application/moses-files', JSON.stringify({
        drive: props.drive.id,
        filesystem: props.drive.filesystem,
        files: isSelected(item) ? selectedItems.value : [item]
      }))
    }

    function showContextMenu(event) {
      contextMenuVisible.value = true
      contextMenuStyle.value = {
        left: `${event.clientX}px`,
        top: `${event.clientY}px`
      }
    }

    async function copySelected() {
      emit('copy-files', {
        source: props.drive,
        files: selectedItems.value
      })
      contextMenuVisible.value = false
    }

    async function exportSelected() {
      emit('export-files', {
        source: props.drive,
        files: selectedItems.value
      })
      contextMenuVisible.value = false
    }

    function showProperties() {
      emit('show-properties', selectedItems.value[0])
      contextMenuVisible.value = false
    }

    function refresh() {
      loadDirectory(currentPath.value)
    }

    function retry() {
      loadDirectory(currentPath.value)
    }

    async function detectWithAdmin() {
      try {
        loading.value = true
        error.value = null
        
        // Show message about admin requirement
        const userConfirmed = confirm(
          'Analyzing the filesystem may require administrator privileges.\n\n' +
          'Windows will prompt you for permission (UAC).\n' +
          'The analysis will provide detailed information about partitions and filesystems.\n\n' +
          'Continue?'
        )
        
        if (!userConfirmed) {
          error.value = 'Filesystem analysis cancelled'
          loading.value = false
          return
        }
        
        // Request detection with elevation
        // The backend will handle the UAC prompt
        const result = await invoke('analyze_filesystem', {
          deviceId: props.drive.id
        })
        
        // Update the drive's filesystem from the analysis result
        if (result && result.filesystem) {
          // Emit event to parent to update the device
          emit('update-filesystem', {
            deviceId: props.drive.id,
            filesystem: result.filesystem
          })
          console.log(`Detected filesystem: ${result.filesystem}`)
          
          // Show the analysis result in a popup like the Analyze button does
          if (result.partition_table || result.partitions) {
            let message = `Drive Analysis Results:\n\n`
            message += `Filesystem: ${result.filesystem || 'Unknown'}\n`
            if (result.partition_table) {
              message += `Partition Table: ${result.partition_table}\n`
            }
            if (result.partitions && result.partitions.length > 0) {
              message += `\nPartitions:\n`
              result.partitions.forEach((p, i) => {
                message += `  ${i + 1}. ${p.filesystem || 'Unknown'} - ${formatSize(p.size)}\n`
              })
            }
            alert(message)
          }
          
          // Try loading directory again if we detected a readable filesystem
          if (result.filesystem && result.filesystem !== 'unknown' && 
              !result.filesystem.includes('gpt') && !result.filesystem.includes('mbr')) {
            await loadDirectory(currentPath.value)
          }
        } else {
          error.value = 'Unable to detect filesystem type'
        }
      } catch (err) {
        error.value = `Failed to analyze filesystem: ${err}`
        console.error('Filesystem analysis error:', err)
      } finally {
        loading.value = false
      }
    }
    
    async function tryFilesystem(fsType) {
      // Temporarily override the filesystem type and try to read
      const originalFs = props.drive.filesystem
      props.drive.filesystem = fsType
      
      try {
        await loadDirectory(currentPath.value)
        // If successful, keep the filesystem type
      } catch (err) {
        // If failed, restore original
        props.drive.filesystem = originalFs
        error.value = `Failed to read as ${fsType}: ${err}`
      }
    }

    // Hide context menu on click outside
    function handleClickOutside() {
      contextMenuVisible.value = false
    }

    // Lifecycle
    onMounted(() => {
      loadDirectory('/')
      document.addEventListener('click', handleClickOutside)
    })

    // Watch for drive changes after initial mount
    // Use a flag to skip the first load since onMounted already handles it
    let isFirstWatch = true
    watch(() => props.drive, () => {
      if (isFirstWatch) {
        isFirstWatch = false
        return
      }
      currentPath.value = '/'
      selectedItems.value = []
      loadDirectory('/')
    })

    return {
      currentPath,
      items,
      selectedItems,
      searchQuery,
      viewMode,
      loading,
      error,
      contextMenuVisible,
      contextMenuStyle,
      pathSegments,
      filteredItems,
      selectionSize,
      loadDirectory,
      navigateTo,
      navigateUp,
      getPathToSegment,
      openItem,
      selectItem,
      isSelected,
      getFileIcon,
      formatSize,
      formatFilesystemName,
      formatDate,
      startDrag,
      showContextMenu,
      copySelected,
      exportSelected,
      showProperties,
      refresh,
      retry,
      detectWithAdmin,
      tryFilesystem,
      filterFiles: () => {} // Filtering is handled by computed
    }
  }
}
</script>

<style scoped>
.file-browser {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-primary);
}

/* Unified Drive Header (matches Format mode) */
.drive-header {
  padding: 8px 16px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  gap: 12px;
}

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

.drive-header .drive-separator {
  color: var(--text-secondary);
  opacity: 0.5;
  font-size: 12px;
}

.drive-header .drive-id {
  font-size: 11px;
  color: var(--text-secondary);
  opacity: 0.8;
}

.breadcrumb-path {
  display: flex;
  align-items: center;
  gap: 4px;
}

.path-separator {
  color: var(--text-secondary);
  margin: 0 4px;
  opacity: 0.5;
}

.breadcrumb-item {
  background: none;
  border: none;
  color: var(--text-primary);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 3px;
  transition: background 0.2s;
  font-size: 12px;
}

.breadcrumb-item:hover {
  background: var(--hover-bg);
}

.breadcrumb-separator {
  color: var(--text-secondary);
  opacity: 0.5;
  font-size: 12px;
}

/* Toolbar */
.toolbar {
  padding: 8px 16px;
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  gap: 8px;
}

.toolbar button {
  padding: 6px 12px;
  background: var(--button-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  color: var(--text-primary);
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 4px;
  transition: all 0.2s;
}

.toolbar button:hover:not(:disabled) {
  background: var(--button-hover-bg);
}

.toolbar button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.view-toggles {
  display: flex;
  margin-left: auto;
  gap: 2px;
}

.view-toggles button {
  padding: 6px 8px;
}

.view-toggles button.active {
  background: var(--accent-color);
  color: white;
}

.search-box {
  margin-left: 8px;
}

.search-box input {
  padding: 6px 12px;
  background: var(--input-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  color: var(--text-primary);
  width: 200px;
}

/* File container */
.file-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

/* List view */
.file-container.view-list .file-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.file-container.view-list .file-item {
  display: grid;
  grid-template-columns: 24px 1fr 100px 120px;
  align-items: center;
  padding: 8px 12px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.2s;
}

.file-container.view-list .file-item:hover {
  background: var(--hover-bg);
}

.file-container.view-list .file-item.selected {
  background: var(--selection-bg);
  color: var(--selection-color);
}

/* Grid view */
.file-container.view-grid .file-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 16px;
}

.file-container.view-grid .file-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 16px 8px;
  border-radius: 8px;
  cursor: pointer;
  text-align: center;
  transition: background 0.2s;
}

.file-container.view-grid .file-item:hover {
  background: var(--hover-bg);
}

.file-container.view-grid .file-item.selected {
  background: var(--selection-bg);
  color: var(--selection-color);
}

.file-container.view-grid .file-item i {
  font-size: 48px;
  margin-bottom: 8px;
}

.file-container.view-grid .file-size,
.file-container.view-grid .file-date {
  display: none;
}

/* File item common */
.file-item.folder {
  font-weight: 500;
}

.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-size,
.file-date {
  color: var(--text-secondary);
  font-size: 0.9em;
}

/* States */
.loading-state,
.error-state,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-secondary);
}

/* Unknown filesystem options */
.unknown-fs-options {
  margin-top: 24px;
  padding: 20px;
  background: var(--bg-secondary);
  border-radius: 8px;
  max-width: 500px;
}

.unknown-fs-options .help-text {
  margin-bottom: 16px;
  font-size: 0.9em;
  color: var(--text-secondary);
}

.unknown-fs-options .admin-button {
  padding: 10px 20px;
  background: var(--accent-color);
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0 auto 16px;
  font-weight: 500;
}

.unknown-fs-options .admin-button:hover {
  background: var(--accent-hover);
}

.unknown-fs-options .try-options {
  text-align: center;
  border-top: 1px solid var(--border-color);
  padding-top: 16px;
  margin-top: 16px;
}

.unknown-fs-options .try-options p {
  margin-bottom: 12px;
  font-size: 0.9em;
}

.unknown-fs-options .try-options button {
  margin: 0 4px;
  padding: 6px 16px;
  background: var(--button-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  cursor: pointer;
}

.unknown-fs-options .try-options button:hover {
  background: var(--button-hover-bg);
}

.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-color);
  border-top-color: var(--accent-color);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Status bar */
.status-bar {
  padding: 8px 16px;
  background: var(--bg-secondary);
  border-top: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  gap: 16px;
  font-size: 0.9em;
  color: var(--text-secondary);
}

.status-bar .drive-info {
  margin-left: auto;
}

/* Context menu */
.context-menu {
  position: fixed;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  padding: 4px;
  z-index: 1000;
}

.context-menu button {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 12px;
  background: none;
  border: none;
  border-radius: 4px;
  color: var(--text-primary);
  cursor: pointer;
  text-align: left;
  transition: background 0.2s;
}

.context-menu button:hover {
  background: var(--hover-bg);
}

.context-menu hr {
  margin: 4px 8px;
  border: none;
  border-top: 1px solid var(--border-color);
}

/* Icons (placeholder - would use icon font or SVGs) */
[class^="icon-"] {
  display: inline-block;
  width: 16px;
  height: 16px;
  background: var(--text-secondary);
  border-radius: 2px;
}

/* Dark mode support */
@media (prefers-color-scheme: dark) {
  .file-browser {
    --bg-primary: #1e1e1e;
    --bg-secondary: #252525;
    --text-primary: #e0e0e0;
    --text-secondary: #999;
    --border-color: #333;
    --hover-bg: #2a2a2a;
    --selection-bg: #0e639c;
    --selection-color: white;
    --button-bg: #2a2a2a;
    --button-hover-bg: #333;
    --input-bg: #1e1e1e;
    --accent-color: #0e639c;
  }
}

/* Light mode support */
@media (prefers-color-scheme: light) {
  .file-browser {
    --bg-primary: white;
    --bg-secondary: #f5f5f5;
    --text-primary: #333;
    --text-secondary: #666;
    --border-color: #ddd;
    --hover-bg: #f0f0f0;
    --selection-bg: #0e639c;
    --selection-color: white;
    --button-bg: white;
    --button-hover-bg: #f0f0f0;
    --input-bg: white;
    --accent-color: #0e639c;
  }
}
</style>