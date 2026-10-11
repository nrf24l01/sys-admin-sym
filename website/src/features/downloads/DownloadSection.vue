<script setup lang="ts">
import { ArrowUpRight, Monitor, Terminal } from '@lucide/vue'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'
import { useLocale } from '@/shared/locale'
import { RouterLink } from 'vue-router'
import { docPath } from '@/features/docs/catalog'
import DownloadRow from './DownloadRow.vue'
import type { DownloadsState } from './useDownloads'

const props = defineProps<{ state: DownloadsState }>()
const { platform, detected, game, client } = props.state
const { t, locale } = useLocale()
function selectPlatform(value: unknown) {
  if (value === 'windows' || value === 'linux') platform.value = value
}
</script>

<template>
  <section id="downloads" aria-labelledby="downloads-heading" class="mx-auto max-w-[1440px] px-5 py-16 md:px-10 md:py-22 lg:px-14">
    <Tabs :model-value="platform" class="gap-0" @update:model-value="selectPlatform">
    <div class="grid gap-6 md:grid-cols-2 md:items-end" data-reveal>
      <div><p class="mb-5 font-mono text-[10px] uppercase tracking-widest text-teal">{{ t.downloadLabel }}</p><h2 id="downloads-heading" class="whitespace-pre-line font-display text-[clamp(2.2rem,4.2vw,3.8rem)] font-medium uppercase leading-[1.15] tracking-tight">{{ t.downloadTitle }}</h2></div>
      <div class="md:justify-self-end"><p class="mb-5 text-sm text-muted-ink">{{ t.downloadText }}</p><TabsList class="h-11 w-full justify-start rounded-none border border-rule bg-transparent p-1 md:w-auto"><TabsTrigger value="windows" class="h-full gap-2 rounded-none px-5 text-xs data-[state=active]:bg-ink data-[state=active]:text-paper data-[state=active]:shadow-none"><Monitor class="h-3.5 w-3.5" />Windows</TabsTrigger><TabsTrigger value="linux" class="h-full gap-2 rounded-none px-5 text-xs data-[state=active]:bg-ink data-[state=active]:text-paper data-[state=active]:shadow-none"><Terminal class="h-3.5 w-3.5" />Linux</TabsTrigger></TabsList></div>
    </div>
    <p v-if="detected === null" class="mt-6 text-xs leading-relaxed text-muted-ink">{{ t.unsupported }}</p>
    <TabsContent :value="platform" class="mt-8 border-t border-rule">
      <div class="hidden grid-cols-[1.4fr_0.75fr_auto] border-b border-rule py-3 font-mono text-[9px] uppercase tracking-widest text-muted-ink lg:grid"><span>{{ t.package }}</span><span>{{ t.release }} / {{ t.file }}</span><span class="w-[150px]">{{ t.download }}</span></div>
      <ul :aria-label="locale === 'ru' ? 'Пакеты для загрузки' : 'Download packages'"><DownloadRow kind="game" :platform="platform" :target="game" /><DownloadRow kind="client" :platform="platform" :target="client" /></ul>
    </TabsContent>
    </Tabs>
    <div class="mt-12 grid gap-8 border-t border-rule pt-8 md:grid-cols-[0.65fr_1.35fr] md:gap-14">
      <div><h3 class="font-display text-2xl font-medium uppercase">{{ t.installation }}</h3><p class="mt-5 text-xs text-muted-ink">{{ t.docsPrompt }}</p><div class="mt-3 flex flex-col items-start gap-3 text-xs"><RouterLink v-for="guide in [{ name: t.networkGuide, file: 'PROVIDER_NETWORK_GUIDE.md' }, { name: t.linuxGuide, file: 'LINUX_GUIDE.md' }, { name: t.terminalGuide, file: 'GAME_SSH.md' }]" :key="guide.file" :to="docPath(guide.file, locale)" class="inline-flex items-center gap-2 hover:text-teal">{{ guide.name }}<ArrowUpRight class="h-3 w-3" /></RouterLink></div></div>
      <Accordion type="single" collapsible class="w-full">
        <AccordionItem value="windows"><AccordionTrigger class="text-sm hover:no-underline">{{ t.windowsInstall }}</AccordionTrigger><AccordionContent class="text-xs leading-[1.85] text-muted-ink">{{ t.windowsHelp }}</AccordionContent></AccordionItem>
        <AccordionItem value="linux"><AccordionTrigger class="text-sm hover:no-underline">{{ t.linuxInstall }}</AccordionTrigger><AccordionContent class="text-xs leading-[1.85] text-muted-ink"><p>{{ t.linuxHelp }}</p><pre class="mt-3 overflow-x-auto bg-ink p-4 font-mono text-[11px] text-paper"><code>chmod +x cloud-provider-sim
./cloud-provider-sim</code></pre></AccordionContent></AccordionItem>
        <AccordionItem value="console"><AccordionTrigger class="text-sm hover:no-underline">{{ t.consoleInstall }}</AccordionTrigger><AccordionContent class="text-xs leading-[1.85] text-muted-ink">{{ t.consoleHelp }}</AccordionContent></AccordionItem>
      </Accordion>
    </div>
  </section>
</template>
