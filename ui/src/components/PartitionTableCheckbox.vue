<template>
  <label class="checkbox-label" :class="{ disabled: isDisabled }">
    <input 
      type="checkbox" 
      :checked="modelValue"
      @change="handleChange"
      :disabled="isDisabled"
    >
    <span 
      class="checkbox-box" 
      :class="{ 
        checked: modelValue,
        disabled: isDisabled 
      }"
    ></span>
    <span class="checkbox-text">
      Create Partition Table
      <span class="checkbox-hint">{{ hint }}</span>
    </span>
  </label>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  modelValue: boolean
  filesystemType: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

// Filesystems that use system tools and handle partitioning automatically
const systemFormatters = ['fat32', 'exfat', 'ntfs']

const isDisabled = computed(() => {
  return systemFormatters.includes(props.filesystemType)
})

const hint = computed(() => {
  if (isDisabled.value) {
    return 'Automatically handled by system formatter'
  }
  return 'Creates MBR with single partition'
})

const handleChange = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (!isDisabled.value) {
    emit('update:modelValue', target.checked)
  }
}

// Auto-enable when a system formatter is selected
if (isDisabled.value && !props.modelValue) {
  emit('update:modelValue', true)
}
</script>

<style scoped>
.checkbox-label {
  display: flex;
  align-items: flex-start;
  cursor: pointer;
}

.checkbox-label.disabled {
  cursor: not-allowed;
  opacity: 0.6;
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

.checkbox-box.disabled {
  border-color: var(--text-disabled);
  background: var(--bg-tertiary);
}

.checkbox-label.disabled:hover .checkbox-box {
  background: var(--bg-tertiary);
  box-shadow: none;
}

.checkbox-label input:checked + .checkbox-box,
.checkbox-box.checked {
  background: var(--accent);
  border-color: var(--accent);
}

.checkbox-box.disabled.checked {
  background: var(--text-disabled);
  border-color: var(--text-disabled);
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

.checkbox-hint {
  font-size: 11px;
  color: var(--text-secondary);
  margin-top: 2px;
}
</style>