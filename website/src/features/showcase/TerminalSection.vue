<script setup lang="ts">
import { onUnmounted, ref } from 'vue'
import { ArrowUpRight, Check, Copy } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/shared/locale'
const { t, locale } = useLocale()
const copyState = ref<'idle' | 'copied' | 'error'>('idle')
let resetTimer: ReturnType<typeof setTimeout> | undefined
async function copyCommands() {
  try {
    await navigator.clipboard.writeText('game-ssh --list\ngame-ssh 1')
    copyState.value = 'copied'
    clearTimeout(resetTimer)
    resetTimer = setTimeout(() => { copyState.value = 'idle' }, 2500)
  } catch { copyState.value = 'error' }
}
onUnmounted(() => clearTimeout(resetTimer))
</script>

<template>
  <section id="terminal" aria-labelledby="terminal-heading" class="bg-equipment text-paper">
    <div class="mx-auto grid max-w-[1440px] gap-10 px-5 py-16 md:grid-cols-2 md:gap-14 md:px-10 md:py-20 lg:px-14" data-reveal>
      <div>
        <p class="mb-5 font-mono text-[10px] uppercase tracking-widest text-[#78a99e]">{{ t.sshLabel }}</p>
        <h2 id="terminal-heading" class="whitespace-pre-line font-display text-[clamp(2.2rem,4.2vw,3.8rem)] font-medium uppercase leading-[1.15] tracking-tight">{{ t.sshTitle }}</h2>
        <p class="mt-6 max-w-[410px] text-sm leading-[1.85] text-white/60">{{ t.sshText }}</p>
        <ul class="mt-6 flex flex-wrap gap-x-5 gap-y-2 font-mono text-[10px] text-white/65"><li v-for="feature in t.sshFeatures" :key="feature" class="flex items-center gap-2"><span class="h-1 w-1 bg-[#78a99e]" />{{ feature }}</li></ul>
        <Button as-child variant="outline" class="mt-8 h-11 border-white/30 bg-transparent px-5 text-paper hover:bg-white/10 hover:text-white"><a href="#client-download">{{ t.sshDownload }}<ArrowUpRight class="ml-3 h-4 w-4" /></a></Button>
        <p class="mt-4 max-w-[400px] text-[11px] leading-[1.7] text-white/55">{{ t.sshNote }}</p>
      </div>
      <div class="min-w-0 self-center border border-white/20 bg-[#090d0f]">
        <div class="flex flex-wrap items-center justify-between gap-3 border-b border-white/15 px-5 py-3"><span class="font-mono text-[10px] text-white/50">{{ locale === 'ru' ? 'ТЕРМИНАЛ' : 'TERMINAL' }} / game-ssh</span><Button variant="ghost" size="sm" class="h-7 px-2 text-[10px] text-white/60 hover:bg-white/10 hover:text-white" @click="copyCommands"><Check v-if="copyState === 'copied'" class="h-3 w-3" /><Copy v-else class="h-3 w-3" />{{ copyState === 'copied' ? t.copied : t.copy }}</Button></div>
        <div class="overflow-x-auto p-5 font-mono text-xs leading-[2.3] sm:p-7 sm:text-[13px]">
          <p class="text-white/50"># {{ locale === 'ru' ? 'Подключитесь к запущенной игре' : 'Connect to a running game' }}</p>
          <p><span class="mr-3 text-[#78a99e]">$</span><code>game-ssh --list</code></p>
          <p><span class="mr-3 text-[#78a99e]">$</span><code>game-ssh 1</code></p>
          <div class="my-5 border-t border-white/10" />
          <p class="text-white/50"># {{ locale === 'ru' ? 'Настройте игровой коммутатор' : 'Configure a simulated switch' }}</p>
          <p><span class="mr-2 text-[#78a99e]">Switch&gt;</span>enable</p>
          <p><span class="mr-2 text-[#78a99e]">Switch#</span>configure terminal</p>
          <p><span class="mr-2 text-[#78a99e]">Switch(config)#</span>vlan 10</p>
          <p><span class="mr-2 text-[#78a99e]">Switch(config-vlan)#</span>name management</p>
        </div>
        <div class="border-t border-white/15 px-5 py-3 font-mono text-[9px] text-white/50">{{ locale === 'ru' ? 'КОНСОЛИ LINUX И В СТИЛЕ IOS' : 'LINUX / IOS-STYLE CONSOLES' }}</div>
        <p v-if="copyState !== 'idle'" role="status" class="border-t border-white/15 px-5 py-3 text-xs text-[#a0c4b9]">{{ copyState === 'copied' ? t.copied : t.copyFailed }}</p>
      </div>
    </div>
  </section>
</template>
