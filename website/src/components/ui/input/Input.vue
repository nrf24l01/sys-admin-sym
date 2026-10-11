<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { useVModel } from '@vueuse/core'
import { cn } from '@/shared/lib/utils'
const props = defineProps<{ defaultValue?: string | number; modelValue?: string | number; class?: HTMLAttributes['class'] }>()
const emits = defineEmits<{ 'update:modelValue': [payload: string | number] }>()
const modelValue = useVModel(props, 'modelValue', emits, { passive: true, defaultValue: props.defaultValue })
</script>
<template>
  <input v-model="modelValue" data-slot="input" :class="cn('flex h-10 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-xs transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50', props.class)" />
</template>
