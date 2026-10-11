<script setup lang="ts">
import { computed } from 'vue'
import { useLocale } from '@/shared/locale'
const { t } = useLocale()
const stories = computed(() => [
  { title: t.value.buildTitle, text: t.value.buildText, detail: t.value.buildDetail, image: 'equipment-shop.png', caption: t.value.shopCaption, alt: 'Game equipment shop showing compatible server components' },
  { title: t.value.networkTitle, text: t.value.networkText, detail: t.value.networkDetail, image: 'cabling.png', caption: t.value.cablingCaption, alt: 'Actual gameplay rear rack view with physical Ethernet and power cables' },
  { title: t.value.hostingTitle, text: t.value.hostingText, detail: t.value.hostingDetail, image: 'server-inspector.png', caption: t.value.inspectorCaption, alt: 'Actual gameplay server inspector with installed components, power status and console' },
])
</script>

<template>
  <section id="gameplay" aria-labelledby="gameplay-heading" class="mx-auto max-w-[1440px] px-5 py-16 md:px-10 md:py-22 lg:px-14">
    <div class="mb-12 grid gap-7 md:grid-cols-2 md:items-end" data-reveal>
      <div><p class="mb-5 font-mono text-[10px] uppercase tracking-widest text-teal">{{ t.sectionLabel }}</p><h2 id="gameplay-heading" class="whitespace-pre-line font-display text-[clamp(2.1rem,4vw,3.5rem)] font-medium uppercase leading-[1.15] tracking-tight">{{ t.sectionTitle }}</h2></div>
      <p class="max-w-[360px] text-sm leading-[1.8] text-muted-ink md:justify-self-end">{{ t.sectionIntro }}</p>
    </div>
    <article v-for="(story, index) in stories" :key="index" class="grid items-center gap-7 border-t border-rule py-9 md:grid-cols-[0.78fr_1.22fr] md:gap-14 md:py-12" data-reveal>
      <div :class="index === 1 ? 'md:order-2 md:pl-8' : ''">
        <p class="mb-4 font-mono text-[10px] text-teal">/ 0{{ index + 1 }}</p>
        <h3 class="mb-4 max-w-[330px] font-display text-3xl font-medium uppercase leading-[1.18] tracking-tight lg:text-4xl">{{ story.title }}</h3>
        <p class="max-w-[365px] text-sm leading-[1.85] text-muted-ink">{{ story.text }}</p>
        <p class="mt-6 max-w-[350px] font-mono text-[10px] leading-[1.8] text-muted-ink">{{ story.detail }}</p>
      </div>
      <figure class="min-w-0" :class="index === 1 ? 'md:order-1' : ''">
        <div class="relative aspect-[16/10] overflow-hidden bg-equipment">
          <img :src="`/images/${story.image}`" :alt="story.alt" class="h-full w-full object-cover object-top" width="1600" height="1000" loading="lazy" />
          <span class="absolute bottom-3 left-3 bg-equipment/90 px-2 py-1 font-mono text-[8px] uppercase tracking-wider text-white/65">{{ t.actualGameplay }}</span>
        </div>
        <figcaption class="mt-3 flex gap-3 text-[11px] leading-relaxed text-muted-ink"><span class="shrink-0 font-mono text-[9px] text-teal">FIG. 0{{ index + 2 }}</span>{{ story.caption }}</figcaption>
      </figure>
    </article>
  </section>
</template>
