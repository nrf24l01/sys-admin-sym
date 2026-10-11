<script setup lang="ts">
import { ArrowDown, Box, Terminal } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/shared/locale'
import type { DownloadTarget } from './useDownloads'
import type { Package, Platform } from './releases'
defineProps<{ kind: Package; platform: Platform; target: DownloadTarget }>()
const { t } = useLocale()
</script>

<template>
  <li :id="kind === 'client' ? 'client-download' : undefined" class="border-b border-rule py-7">
    <div class="grid gap-5 sm:grid-cols-[1fr_auto] lg:grid-cols-[1.4fr_0.75fr_auto] lg:items-center">
      <div class="flex items-start gap-4">
        <Box v-if="kind === 'game'" class="mt-1 h-5 w-5 shrink-0 text-teal" :stroke-width="1.4" /><Terminal v-else class="mt-1 h-5 w-5 shrink-0 text-teal" :stroke-width="1.4" />
        <div class="min-w-0"><h3 class="text-base font-medium" :class="kind === 'client' ? 'font-mono' : ''">{{ kind === 'game' ? t.gameName : t.clientName }}</h3><p class="mt-1 text-xs leading-relaxed text-muted-ink">{{ kind === 'game' ? t.gameDescription : t.clientDescription }}</p></div>
      </div>
      <div class="pl-9 font-mono text-[10px] leading-[1.8] text-muted-ink sm:col-start-1 lg:col-auto lg:pl-0"><span>{{ target.source === 'configured' ? t.latestBuild : t.development }}</span><br /><span>{{ platform === 'windows' ? 'Windows' : 'Linux' }} / x86-64</span><span v-if="target.url" class="ml-2">{{ t.zip }}</span><br /><a v-if="target.checksumUrl" :href="target.checksumUrl" class="inline-block text-teal hover:underline">{{ t.checksum }} ↓</a></div>
      <Button v-if="target.url" as-child :variant="kind === 'game' ? 'default' : 'outline'" class="h-11 w-full justify-between gap-5 px-5 sm:col-start-2 sm:row-span-2 sm:row-start-1 sm:w-auto lg:col-auto lg:row-auto" :class="kind === 'client' ? 'bg-transparent' : ''">
        <a :href="target.url" :aria-label="`${kind === 'game' ? t.gameName : t.clientName} — ${platform === 'windows' ? 'Windows' : 'Linux'} — ${t.directDownload}`">{{ t.directDownload }}<ArrowDown class="h-4 w-4" /></a>
      </Button>
      <Button v-else disabled variant="outline" class="h-11 w-full px-5 sm:col-start-2 sm:row-span-2 sm:row-start-1 sm:w-auto lg:col-auto lg:row-auto" :aria-label="`${kind === 'game' ? t.gameName : t.clientName} — ${platform === 'windows' ? 'Windows' : 'Linux'} — ${t.development}`">{{ t.development }}</Button>
    </div>
  </li>
</template>
