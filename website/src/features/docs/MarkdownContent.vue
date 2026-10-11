<script setup lang="ts">
import { computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { renderMarkdown } from './markdown'
import type { Heading } from './markdown'
import type { Locale } from './catalog'
const props = defineProps<{ source: string; locale: Locale; compact?: boolean; anchors?: Heading[] }>()
const emit = defineEmits<{ headings: [value: Heading[]] }>()
const router = useRouter()
const rendered = computed(() => renderMarkdown(props.source, props.locale, props.anchors))
watch(rendered, value => emit('headings', value.headings), { immediate: true })
function navigate(event: MouseEvent) {
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
  const anchor = (event.target as HTMLElement).closest<HTMLAnchorElement>('a')
  const href = anchor?.getAttribute('href')
  if (href?.match(/^\/(en|ru)(?:\/|#|$)/)) { event.preventDefault(); void router.push(href) }
}
</script>
<template><div class="doc-prose" :class="{ 'doc-prose-compact': compact }" @click="navigate" v-html="rendered.html" /></template>
