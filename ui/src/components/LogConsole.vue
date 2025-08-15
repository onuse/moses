<template>
  <div class="log-console" :class="{ collapsed: !isExpanded }">
    <!-- Header -->
    <div class="console-header">
      <div class="header-left">
        <button @click="toggleExpanded" class="toggle-btn">
          <span>{{ isExpanded ? '▼' : '▶' }}</span>
        </button>
        <span class="console-title">Console Output ({{ filteredLogs.length }} logs)</span>
      </div>

      <div class="header-right">
        <!-- Filter buttons -->
        <div class="filter-buttons">
          <button 
            v-for="level in ['ALL', 'DEBUG', 'INFO', 'WARN', 'ERROR']"
            :key="level"
            @click="filter = level"
            :class="{ active: filter === level }"
            class="filter-btn"
          >
            {{ level }}
          </button>
        </div>

        <!-- Auto-scroll toggle -->
        <label class="auto-scroll">
          <input 
            type="checkbox" 
            v-model="autoScroll"
            @change="handleAutoScrollChange"
          />
          Auto-scroll
        </label>

        <!-- Action buttons -->
        <button @click="copyLogs" class="action-btn" title="Copy logs">
          📋
        </button>
        <button @click="downloadLogs" class="action-btn" title="Download logs">
          💾
        </button>
        <button @click="clearLogs" class="action-btn clear" title="Clear logs">
          🗑️
        </button>
      </div>
    </div>

    <!-- Console Output -->
    <div 
      v-if="isExpanded"
      ref="consoleOutput"
      class="console-output"
      @scroll="handleScroll"
      @keydown.ctrl.a.prevent="selectAll"
      @keydown.ctrl.c="copySelection"
      tabindex="0"
    >
      <div v-if="filteredLogs.length === 0" class="no-logs">
        No logs to display
      </div>
      <div 
        v-else
        v-for="(log, index) in filteredLogs" 
        :key="index"
        class="log-entry"
        @contextmenu.prevent="copyLogEntry(log)"
        :title="'Right-click to copy this log entry'"
      >
        <span class="timestamp">[{{ formatTimestamp(log.timestamp) }}]</span>
        <span :class="['level', log.level.toLowerCase()]">[{{ log.level }}]</span>
        <span v-if="log.source" class="source">[{{ log.source }}]</span>
        <span class="message">{{ log.message }}</span>
      </div>
    </div>
  </div>
</template>

<script>
export default {
  name: 'LogConsole',
  props: {
    maxHeight: {
      type: String,
      default: '300px'
    }
  },
  data() {
    return {
      logs: [],
      isExpanded: true,
      filter: 'ALL',
      autoScroll: true
    };
  },
  computed: {
    filteredLogs() {
      if (this.filter === 'ALL') {
        return this.logs;
      }
      return this.logs.filter(log => log.level === this.filter);
    }
  },
  methods: {
    addLog(level, message, source = null) {
      const entry = {
        timestamp: new Date(),
        level,
        message,
        source
      };
      
      this.logs.push(entry);
      
      // Keep only last 5000 logs
      if (this.logs.length > 5000) {
        this.logs = this.logs.slice(-5000);
      }
      
      // Auto-scroll if enabled
      if (this.autoScroll) {
        this.$nextTick(() => {
          this.scrollToBottom();
        });
      }
    },
    
    debug(message, source) {
      this.addLog('DEBUG', message, source);
    },
    
    info(message, source) {
      this.addLog('INFO', message, source);
    },
    
    warn(message, source) {
      this.addLog('WARN', message, source);
    },
    
    error(message, source) {
      this.addLog('ERROR', message, source);
    },
    
    toggleExpanded() {
      this.isExpanded = !this.isExpanded;
    },
    
    formatTimestamp(date) {
      const pad = (n) => n.toString().padStart(2, '0');
      const hours = pad(date.getHours());
      const minutes = pad(date.getMinutes());
      const seconds = pad(date.getSeconds());
      const ms = date.getMilliseconds().toString().padStart(3, '0');
      return `${hours}:${minutes}:${seconds}.${ms}`;
    },
    
    handleScroll() {
      if (!this.$refs.consoleOutput) return;
      
      const { scrollTop, scrollHeight, clientHeight } = this.$refs.consoleOutput;
      // Disable auto-scroll if user scrolls up
      if (scrollTop < scrollHeight - clientHeight - 10) {
        this.autoScroll = false;
      }
    },
    
    handleAutoScrollChange() {
      if (this.autoScroll) {
        this.scrollToBottom();
      }
    },
    
    scrollToBottom() {
      if (this.$refs.consoleOutput) {
        this.$refs.consoleOutput.scrollTop = this.$refs.consoleOutput.scrollHeight;
      }
    },
    
    copyLogs() {
      const text = this.filteredLogs
        .map(log => `[${this.formatTimestamp(log.timestamp)}] [${log.level}] ${log.source ? `[${log.source}] ` : ''}${log.message}`)
        .join('\n');
      
      navigator.clipboard.writeText(text).then(() => {
        this.info('Logs copied to clipboard', 'Console');
      });
    },
    
    copyLogEntry(log) {
      const text = `[${this.formatTimestamp(log.timestamp)}] [${log.level}] ${log.source ? `[${log.source}] ` : ''}${log.message}`;
      navigator.clipboard.writeText(text).then(() => {
        // Show brief feedback
        this.info('Log entry copied', 'Console');
      });
    },
    
    downloadLogs() {
      const text = this.logs
        .map(log => `[${this.formatTimestamp(log.timestamp)}] [${log.level}] ${log.source ? `[${log.source}] ` : ''}${log.message}`)
        .join('\n');
      
      const blob = new Blob([text], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `moses-logs-${new Date().toISOString().replace(/[:.]/g, '-')}.txt`;
      a.click();
      URL.revokeObjectURL(url);
      
      this.info('Logs downloaded', 'Console');
    },
    
    clearLogs() {
      this.logs = [];
      this.info('Console cleared', 'Console');
    },
    
    selectAll() {
      // Select all text in the console
      if (this.$refs.consoleOutput) {
        const selection = window.getSelection();
        const range = document.createRange();
        range.selectNodeContents(this.$refs.consoleOutput);
        selection.removeAllRanges();
        selection.addRange(range);
      }
    },
    
    copySelection() {
      // Let the browser handle the copy if there's selected text
      const selection = window.getSelection();
      if (selection.toString()) {
        // Browser will handle Ctrl+C naturally
        return;
      }
      // If nothing selected, copy all logs
      this.copyLogs();
    }
  }
};
</script>

<style scoped>
.log-console {
  background: #1a1a1a;
  border: 1px solid #333;
  border-radius: 4px;
  overflow: hidden;
  font-family: 'Consolas', 'Monaco', monospace;
  margin-top: 10px;
  min-height: 100px;
  max-height: 500px;
  resize: vertical;
}

.console-header {
  background: #2a2a2a;
  padding: 8px 12px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #333;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.toggle-btn {
  background: none;
  border: none;
  color: #888;
  cursor: pointer;
  padding: 0;
  font-size: 12px;
}

.toggle-btn:hover {
  color: #fff;
}

.console-title {
  color: #ccc;
  font-size: 13px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.filter-buttons {
  display: flex;
  gap: 4px;
}

.filter-btn {
  background: transparent;
  border: 1px solid #444;
  color: #888;
  padding: 2px 8px;
  font-size: 11px;
  cursor: pointer;
  border-radius: 3px;
  transition: all 0.2s;
}

.filter-btn:hover {
  color: #fff;
  border-color: #666;
}

.filter-btn.active {
  background: #0066cc;
  color: #fff;
  border-color: #0066cc;
}

.auto-scroll {
  display: flex;
  align-items: center;
  gap: 4px;
  color: #888;
  font-size: 12px;
}

.auto-scroll input {
  cursor: pointer;
}

.action-btn {
  background: none;
  border: none;
  color: #888;
  cursor: pointer;
  padding: 4px;
  font-size: 14px;
  transition: all 0.2s;
}

.action-btn:hover {
  color: #fff;
}

.action-btn.clear:hover {
  color: #ff4444;
}

.console-output {
  background: #0a0a0a;
  color: #d4d4d4;
  padding: 8px;
  max-height: 300px;
  overflow-y: auto;
  font-size: 12px;
  line-height: 1.4;
  user-select: text;  /* Allow text selection */
  cursor: text;       /* Show text cursor */
}

.no-logs {
  color: #666;
  font-style: italic;
  padding: 20px;
  text-align: center;
}

.log-entry {
  padding: 2px 4px;
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;  /* Ensure entries are selectable */
  position: relative;
}

.log-entry:hover {
  background: #1a1a1a;
}

/* Highlight selected text for better visibility */
.console-output ::selection {
  background: #3a6ea5;
  color: #ffffff;
}

.console-output ::-moz-selection {
  background: #3a6ea5;
  color: #ffffff;
}

.timestamp {
  color: #666;
}

.level {
  font-weight: bold;
  margin-left: 4px;
}

.level.debug {
  color: #888;
}

.level.info {
  color: #4a9eff;
}

.level.warn {
  color: #ffb84d;
}

.level.error {
  color: #ff5555;
}

.source {
  color: #b19cd9;
  margin-left: 4px;
}

.message {
  margin-left: 4px;
}

/* Dark scrollbar */
.console-output::-webkit-scrollbar {
  width: 8px;
}

.console-output::-webkit-scrollbar-track {
  background: #1a1a1a;
}

.console-output::-webkit-scrollbar-thumb {
  background: #444;
  border-radius: 4px;
}

.console-output::-webkit-scrollbar-thumb:hover {
  background: #555;
}

/* Collapsed state */
.log-console.collapsed .console-output {
  display: none;
}
</style>