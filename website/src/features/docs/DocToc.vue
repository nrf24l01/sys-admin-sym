<script setup lang="ts">
import { ref } from 'vue'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'
import { useLocale } from '@/shared/locale'
import type { Heading } from './markdown'
defineProps<{ headings: Heading[] }>()
const { locale } = useLocale()
const open = ref<string | undefined>(window.matchMedia('(min-width: 1280px)').matches ? 'sections' : undefined)
</script>
<template>
  <aside class="order-first border-b border-rule pb-2 xl:sticky xl:top-8 xl:order-none xl:max-h-[calc(100vh-4rem)] xl:overflow-y-auto xl:border-0">
    <Accordion v-model="open" type="single" collapsible><AccordionItem value="sections" class="border-0"><AccordionTrigger class="py-2 font-mono text-[9px] uppercase tracking-widest text-muted-ink hover:no-underline">{{ locale === 'ru' ? 'На этой странице' : 'On this page' }}</AccordionTrigger><AccordionContent><nav :aria-label="locale === 'ru' ? 'На этой странице' : 'On this page'"><a v-for="heading in headings" :key="heading.id" :href="`#${heading.id}`" class="block py-1.5 text-[11px] leading-relaxed text-muted-ink hover:text-teal" :class="heading.depth === 3 ? 'pl-3' : ''">{{ heading.text }}</a></nav></AccordionContent></AccordionItem></Accordion>
  </aside>
</template>
