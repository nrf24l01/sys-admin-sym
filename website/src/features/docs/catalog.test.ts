// @vitest-environment jsdom
import { existsSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { documents, loadDocument } from './catalog'
import { renderMarkdown } from './markdown'

describe('bundled bilingual documentation', () => {
  for (const locale of ['en', 'ru'] as const) {
    it(`loads every ${locale} article with valid local links and image files`, async () => {
      for (const entry of documents) {
        const source = await loadDocument(entry, locale)
        expect(source.length, entry.file).toBeGreaterThan(100)
        const { html, headings } = renderMarkdown(source, locale)
        expect(headings.length, entry.file).toBeGreaterThan(0)
        const container = document.createElement('div')
        container.innerHTML = html
        for (const anchor of container.querySelectorAll('a[href]')) {
          const href = anchor.getAttribute('href')!
          if (href.startsWith(`/${locale}/docs/`)) expect(documents.some(doc => href.split('#')[0] === `/${locale}/docs/${doc.slug}`), `${entry.file}: ${href}`).toBe(true)
          if (href.startsWith('https://github.com')) expect(href, entry.file).toContain('/releases/download/')
        }
        for (const img of container.querySelectorAll('img')) expect(existsSync(`public${img.getAttribute('src')}`), `${entry.file}: ${img.src}`).toBe(true)
      }
    })
  }
  it('keeps canonical anchors while showing translated headings', async () => {
    const entry = documents.find(doc => doc.slug === 'game-ssh')!
    const en = renderMarkdown(await loadDocument(entry, 'en'), 'en')
    const ru = renderMarkdown(await loadDocument(entry, 'ru'), 'ru', en.headings)
    expect(ru.headings.map(heading => heading.id)).toEqual(en.headings.map(heading => heading.id))
    expect(ru.html).toContain('id="connect">Подключение')
  })
})
