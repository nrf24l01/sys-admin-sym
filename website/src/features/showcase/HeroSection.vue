<script setup lang="ts">
import { ArrowDown, ArrowUpRight } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/shared/locale'
import RackEquipment from './RackEquipment.vue'
const { t } = useLocale()
</script>

<template>
  <section aria-labelledby="hero-heading" class="mx-auto max-w-[1440px] px-5 pb-12 pt-10 md:px-10 md:pt-16 lg:px-14 lg:pb-16 lg:pt-18">
    <div class="grid items-center gap-10 lg:grid-cols-[1.06fr_1fr] lg:gap-14">
      <div class="min-w-0" data-hero-copy>
        <p class="mb-7 flex items-center gap-3 font-mono text-[10px] uppercase tracking-[0.12em] text-muted-ink"><span class="h-[7px] w-[7px] shrink-0 bg-teal" />{{ t.eyebrow }}</p>
        <h1 id="hero-heading" class="font-display text-[clamp(2.5rem,6.4vw,5.5rem)] font-medium uppercase leading-[1.12] tracking-[-0.045em]"><span v-for="(line, index) in t.heroLines" :key="index" class="block" :class="index === 2 ? 'text-teal' : ''">{{ line }}</span></h1>
        <p class="mb-8 mt-7 max-w-[405px] text-[15px] leading-[1.8] text-muted-ink">{{ t.intro }}</p>
        <Button as-child class="h-13 min-w-52 justify-between gap-7 px-6 text-sm"><a href="#downloads">{{ t.downloadGame }}<ArrowDown class="h-4 w-4" /></a></Button>
        <p class="mt-3 font-mono text-[10px] text-muted-ink">{{ t.platforms }}<br /><span class="mt-1 inline-block">{{ t.includes }}</span></p>
        <a href="#gameplay" class="mt-8 inline-flex items-center gap-2 border-b border-ink/30 pb-1 text-xs hover:border-teal hover:text-teal">{{ t.seeGameplay }}<ArrowUpRight class="h-3 w-3" /></a>
      </div>
      <div data-hero-image><RackEquipment /></div>
    </div>
  </section>
  <div class="border-y border-rule">
    <div class="mx-auto grid max-w-[1440px] grid-cols-1 divide-y divide-rule px-5 font-mono text-[10px] uppercase tracking-[0.07em] text-muted-ink sm:grid-cols-3 sm:divide-x sm:divide-y-0 md:px-10 lg:px-14">
      <div v-for="(label, index) in t.strip" :key="index" class="flex items-center gap-4 py-4 sm:justify-center"><span class="text-teal">0{{ index + 1 }}</span>{{ label }}</div>
    </div>
  </div>
</template>
