<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { Search } from '@lucide/vue'
import { Input } from '@/components/ui/input'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'
import { documents, groups } from './catalog'
import { useLocale } from '@/shared/locale'
const { locale } = useLocale()
const search = ref('')
const open = ref<string | undefined>(window.matchMedia('(min-width: 768px)').matches ? 'guides' : undefined)
const filtered = computed(() => {
  const query = search.value.trim().toLowerCase()
  return documents.filter(doc => `${doc.title.en} ${doc.title.ru} ${doc.file}`.toLowerCase().includes(query))
})
watch(search, () => { if (search.value) open.value = 'guides' })
</script>
<template>
  <aside class="min-w-0 md:sticky md:top-8 md:max-h-[calc(100vh-4rem)] md:overflow-y-auto md:pr-5" :aria-label="locale === 'ru' ? 'Руководства' : 'Guides'">
    <label for="doc-search" class="mb-3 block font-mono text-[10px] uppercase tracking-widest text-muted-ink">{{ locale === 'ru' ? 'Поиск документации' : 'Search documentation' }}</label>
    <div class="relative"><Search class="pointer-events-none absolute left-3 top-3 h-4 w-4 text-muted-ink" /><Input id="doc-search" v-model="search" type="search" class="pl-9" :placeholder="locale === 'ru' ? 'Сеть, серверы, хостинг…' : 'Network, servers, hosting…'" /></div>
    <Accordion v-model="open" type="single" collapsible class="mt-3">
      <AccordionItem value="guides" class="border-0"><AccordionTrigger class="py-4 text-xs hover:no-underline">{{ locale === 'ru' ? 'Все руководства' : 'Browse all guides' }} <span class="ml-auto pr-3 font-mono text-[10px] text-muted-ink">{{ filtered.length }}</span></AccordionTrigger><AccordionContent>
        <nav :aria-label="locale === 'ru' ? 'Разделы документации' : 'Documentation sections'" class="space-y-6 pb-2">
          <template v-for="group in groups" :key="group.id"><div v-if="filtered.some(doc => doc.group === group.id)"><p class="mb-2 font-mono text-[9px] uppercase tracking-widest text-muted-ink">{{ group.title[locale] }}</p><RouterLink v-for="doc in filtered.filter(doc => doc.group === group.id)" :key="doc.slug" :to="`/${locale}/docs/${doc.slug}`" class="block border-l-2 border-transparent py-2 pl-3 text-xs leading-relaxed text-muted-ink transition-colors hover:text-teal" active-class="!border-teal !text-teal">{{ doc.title[locale] }}</RouterLink></div></template>
          <p v-if="!filtered.length" class="text-xs text-muted-ink">{{ locale === 'ru' ? 'Ничего не найдено. Попробуйте другое название.' : 'No guides found. Try another title.' }}</p>
        </nav>
      </AccordionContent></AccordionItem>
    </Accordion>
  </aside>
</template>
