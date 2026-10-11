<script setup lang="ts">
import { Button } from '@/components/ui/button'
import { useLocale } from '@/shared/locale'
import { RouterLink, useRoute, useRouter } from 'vue-router'
const route = useRoute()
const router = useRouter()
const { t, locale } = useLocale()
function toggle() {
  const next = locale.value === 'en' ? 'ru' : 'en'
  void router.push({ path: route.path.replace(/^\/(en|ru)/, `/${next}`), hash: route.hash, query: route.query })
}
</script>

<template>
  <a href="#main" class="sr-only fixed left-4 top-4 z-50 bg-ink p-3 text-paper focus:not-sr-only">{{ t.skip }}</a>
  <header class="border-b border-rule">
    <div class="mx-auto flex max-w-[1440px] flex-wrap items-center justify-between gap-x-5 gap-y-4 px-5 py-5 md:px-10 lg:px-14 lg:py-7">
      <RouterLink :to="`/${locale}`" aria-label="Cloud Provider Simulator — Home" class="flex items-center gap-3">
        <svg class="h-9 w-8" viewBox="0 0 32 36" aria-hidden="true"><path fill="currentColor" d="M0 0h32v9H0zm0 13h32v9H0zm0 13h32v9H0z"/><path fill="#eceae3" d="M4 4h2v2H4zm0 13h2v2H4zm0 13h2v2H4z"/><path fill="#287568" d="M24 4h4v2h-4zm0 13h4v2h-4zm0 13h4v2h-4z"/></svg>
        <span class="text-[13px] font-semibold uppercase leading-[1.25] tracking-[0.07em]">Cloud Provider<br /><span class="font-normal tracking-[0.17em]">Simulator</span></span>
      </RouterLink>
      <nav :aria-label="locale === 'en' ? 'Main navigation' : 'Главная навигация'" class="order-3 flex w-full items-center justify-between gap-3 border-t border-rule pt-3 text-[13px] md:order-none md:w-auto md:gap-7 md:border-0 md:pt-0">
        <RouterLink :to="`/${locale}#gameplay`" class="transition-colors hover:text-teal">{{ t.gameplay }}</RouterLink>
        <RouterLink :to="`/${locale}#terminal`" class="font-mono text-xs transition-colors hover:text-teal">game-ssh</RouterLink>
        <RouterLink :to="`/${locale}/docs`" class="inline-flex items-center gap-1 transition-colors hover:text-teal">{{ t.docs }}</RouterLink>
      </nav>
      <div class="flex items-center gap-3 md:gap-5">
        <Button variant="ghost" class="h-8 px-1 font-mono text-xs" :aria-label="locale === 'en' ? 'Switch to Russian' : 'Переключить на английский'" @click="toggle">{{ locale === 'en' ? 'RU' : 'EN' }}</Button>
        <Button as-child class="h-9 px-4 text-xs"><RouterLink :to="`/${locale}#downloads`">{{ t.downloads }}<span aria-hidden="true">↓</span></RouterLink></Button>
      </div>
    </div>
  </header>
</template>
