<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { ArrowRight, ArrowLeft } from '@lucide/vue'
import DocToc from '@/features/docs/DocToc.vue'
import DocsSidebar from '@/features/docs/DocsSidebar.vue'
import MarkdownContent from '@/features/docs/MarkdownContent.vue'
import { documents, groups, loadDocument } from '@/features/docs/catalog'
import { renderMarkdown } from '@/features/docs/markdown'
import type { Heading } from '@/features/docs/markdown'
import { useLocale } from '@/shared/locale'
const { locale, t } = useLocale()
const route = useRoute()
const entry = computed(() => documents.find(doc => doc.slug === route.params.slug))
const source = ref('')
const error = ref(false)
const loading = ref(false)
const headings = ref<Heading[]>([])
const anchors = ref<Heading[]>([])
const sections = computed(() => headings.value.filter(heading => heading.depth === 2 || heading.depth === 3))
const index = computed(() => documents.findIndex(doc => doc.slug === entry.value?.slug))
const adjacent = computed(() => [documents[index.value - 1], documents[index.value + 1]])
let requestId = 0
watch([() => route.params.slug, locale], async () => {
  const current = ++requestId
  source.value = ''; headings.value = []; anchors.value = []; error.value = false
  document.title = `${entry.value?.title[locale.value] ?? t.value.docs} — Cloud Provider Simulator`
  if (!entry.value) { error.value = Boolean(route.params.slug); loading.value = false; return }
  loading.value = true
  try {
    const [text, english] = await Promise.all([loadDocument(entry.value, locale.value), loadDocument(entry.value, 'en')])
    if (current !== requestId) return
    anchors.value = renderMarkdown(english, 'en').headings
    source.value = text
    await nextTick()
    if (route.hash) document.getElementById(decodeURIComponent(route.hash.slice(1)))?.scrollIntoView()
  } catch { if (current === requestId) error.value = true }
  finally { if (current === requestId) loading.value = false }
}, { immediate: true })
</script>

<template>
  <main id="main" class="mx-auto max-w-[1440px] px-5 py-10 md:px-10 md:py-14 lg:px-14">
    <div class="mb-8 flex flex-wrap items-center justify-between gap-4 border-b border-rule pb-5 text-xs">
      <RouterLink :to="`/${locale}/docs`" class="font-mono text-[10px] uppercase tracking-widest text-teal">{{ t.docs }}<span v-if="entry" class="ml-3 text-muted-ink">/ {{ entry.title[locale] }}</span></RouterLink>
      <RouterLink :to="`/${locale}#downloads`" class="flex items-center gap-2 text-muted-ink hover:text-teal">{{ t.downloads }}<ArrowRight class="h-3 w-3" /></RouterLink>
    </div>
    <div class="grid items-start gap-8 md:grid-cols-[220px_minmax(0,1fr)] lg:grid-cols-[240px_minmax(0,1fr)] lg:gap-12">
      <DocsSidebar />
      <div class="min-w-0">
        <div v-if="!route.params.slug">
          <p class="mb-5 font-mono text-[10px] uppercase tracking-widest text-teal">{{ locale === 'ru' ? 'Практическое руководство' : 'The field manual' }}</p>
          <h1 class="font-display text-[clamp(2.4rem,5vw,4.3rem)] font-medium uppercase leading-tight">{{ t.docs }}</h1>
          <p class="mt-5 max-w-[660px] text-sm leading-relaxed text-muted-ink">{{ locale === 'ru' ? 'От первого запуска до своей сети и хостинга. Руководства, команды и справочные материалы — всё здесь.' : 'From your first boot to your own network and hosting business. Guides, commands, and reference material, all here.' }}</p>
          <div class="mt-10 grid gap-x-10 md:grid-cols-2">
            <section v-for="group in groups" :key="group.id" class="mb-9 border-t border-rule pt-5"><h2 class="mb-4 font-display text-2xl font-medium uppercase">{{ group.title[locale] }}</h2><RouterLink v-for="doc in documents.filter(doc => doc.group === group.id)" :key="doc.slug" :to="`/${locale}/docs/${doc.slug}`" class="flex items-center justify-between gap-4 border-b border-rule/60 py-3 text-sm hover:text-teal">{{ doc.title[locale] }}<ArrowRight class="h-3.5 w-3.5 shrink-0" /></RouterLink></section>
          </div>
        </div>
        <div v-else-if="error" role="status" class="py-12"><h1 class="font-display text-3xl uppercase">{{ locale === 'ru' ? 'Руководство не найдено' : 'Guide not found' }}</h1><p class="mt-4 text-sm text-muted-ink">{{ locale === 'ru' ? 'Выберите другое руководство в списке.' : 'Choose another guide from the list.' }}</p><RouterLink :to="`/${locale}/docs`" class="mt-6 inline-block text-sm text-teal">{{ t.docs }} →</RouterLink></div>
        <div v-else-if="loading && !source" role="status" class="py-12 text-sm text-muted-ink">{{ locale === 'ru' ? 'Загружаем руководство…' : 'Loading guide…' }}</div>
        <div v-else class="grid items-start gap-8 xl:grid-cols-[minmax(0,1fr)_170px]">
          <article class="min-w-0"><MarkdownContent :source="source" :locale="locale" :anchors="anchors" @headings="headings = $event" />
            <nav class="mt-12 flex flex-wrap justify-between gap-6 border-t border-rule pt-6" :aria-label="locale === 'ru' ? 'Следующее руководство' : 'More guides'">
              <template v-for="(doc, position) in adjacent" :key="position"><RouterLink v-if="doc" :to="`/${locale}/docs/${doc.slug}`" class="flex max-w-[260px] items-center gap-3 text-xs text-teal"><ArrowLeft v-if="position === 0" class="h-4 w-4 shrink-0" />{{ doc.title[locale] }}<ArrowRight v-if="position === 1" class="h-4 w-4 shrink-0" /></RouterLink></template>
            </nav>
          </article>
          <DocToc v-if="sections.length" :headings="sections" />
        </div>
      </div>
    </div>
  </main>
</template>
