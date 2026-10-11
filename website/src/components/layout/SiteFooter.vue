<script setup lang="ts">
import { useLocale } from '@/shared/locale'
import { RouterLink } from 'vue-router'
const { t, locale } = useLocale()
</script>

<template>
  <footer class="border-t border-rule">
    <div class="mx-auto max-w-[1440px] px-5 py-9 md:px-10 lg:px-14">
      <div class="flex flex-wrap items-start justify-between gap-6"><div><p class="font-display text-2xl font-medium uppercase">{{ t.footerLine }}</p><p class="mt-2 text-[11px] text-muted-ink">{{ t.footerNote }}</p></div><div class="flex flex-wrap gap-6 text-xs"><RouterLink :to="`/${locale}/docs`" class="hover:text-teal">{{ t.docs }}</RouterLink><RouterLink :to="`/${locale}/docs/attribution`" class="hover:text-teal">{{ locale === 'ru' ? 'Источники' : 'Credits' }}</RouterLink></div></div>
      <div class="mt-7 flex flex-wrap justify-between gap-3 border-t border-rule pt-4 font-mono text-[9px] text-muted-ink"><span>CLOUD PROVIDER SIMULATOR</span><span>{{ t.rights }}</span></div>
    </div>
  </footer>
</template>
