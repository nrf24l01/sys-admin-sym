<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import gsap from 'gsap'
import { ScrollTrigger } from 'gsap/ScrollTrigger'
import HeroSection from '@/features/showcase/HeroSection.vue'
import GameplaySection from '@/features/showcase/GameplaySection.vue'
import TerminalSection from '@/features/showcase/TerminalSection.vue'
import DownloadSection from '@/features/downloads/DownloadSection.vue'
import { watch } from 'vue'
import { useLocale } from '@/shared/locale'
import { useDownloads } from '@/features/downloads/useDownloads'

const root = ref<HTMLElement>()
const downloads = useDownloads()
const { locale } = useLocale()
watch(locale, () => { document.title = `Cloud Provider Simulator — ${locale.value === 'ru' ? 'Скачать игру' : 'Download the game'}` }, { immediate: true })
let media: gsap.MatchMedia | undefined
onMounted(() => {
  gsap.registerPlugin(ScrollTrigger)
  media = gsap.matchMedia()
  media.add('(prefers-reduced-motion: no-preference)', () => {
    gsap.from('[data-hero-copy], [data-hero-image]', { y: 18, opacity: 0, duration: 0.65, stagger: 0.12, ease: 'power2.out', clearProps: 'all' })
    root.value?.querySelectorAll('[data-reveal]').forEach(element => {
      gsap.from(element, { y: 14, opacity: 0, duration: 0.5, ease: 'power2.out', clearProps: 'all', scrollTrigger: { trigger: element, start: 'top 93%', once: true } })
    })
  }, root.value)
})
onUnmounted(() => media?.revert())
</script>

<template>
  <main id="main" ref="root"><HeroSection /><GameplaySection /><TerminalSection /><DownloadSection :state="downloads" /></main>
</template>
