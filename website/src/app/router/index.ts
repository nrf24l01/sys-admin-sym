import { createRouter, createWebHistory } from 'vue-router'
import { setLocale, useLocale } from '@/shared/locale'
import HomePage from '@/pages/HomePage.vue'
export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: to => ({ path: `/${useLocale().locale.value}`, hash: to.hash, query: to.query }) },
    { path: '/:locale(en|ru)', name: 'home', component: HomePage },
    { path: '/:locale(en|ru)/docs/:slug?', name: 'docs', component: () => import('@/pages/DocsPage.vue') },
    { path: '/:pathMatch(.*)*', redirect: () => `/${useLocale().locale.value}/docs` },
  ],
  scrollBehavior(to, from, saved) {
    if (saved) return saved
    // A language change keeps the reader's current position.
    if (to.name === from.name && to.params.slug === from.params.slug && to.params.locale !== from.params.locale) return false
    if (to.hash) return new Promise(resolve => setTimeout(() => resolve({ el: to.hash, top: 30 }), 100))
    return { top: 0 }
  },
})
router.beforeEach(to => { setLocale(to.params.locale === 'ru' ? 'ru' : 'en') })
