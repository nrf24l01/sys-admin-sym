import { Marked } from 'marked'
import DOMPurify from 'dompurify'
import { docPath } from './catalog'
import type { Locale } from './catalog'

export interface Heading { id: string; text: string; depth: number }
const escape = (text: string) => text.replace(/[&<>"']/g, char => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[char]!)

function localAsset(href: string): string | undefined {
  if (/^(?:https?:)?\/\//i.test(href)) return undefined
  const match = href.match(/(?:^|\/)(images|screenshots)\/([\w.-]+\.(?:png|jpg|webp))$/i)
  return match ? `/docs-assets/${match[1]}/${match[2]}` : undefined
}
export function linkDestination(href: string, locale: Locale): string | undefined {
  if (href.startsWith('#')) return href
  const asset = localAsset(href)
  if (asset) return asset
  if (/^\/(?:en|ru)?\/?#/.test(href)) return `/${locale}${href.slice(href.indexOf('#'))}`
  const [path = '', fragment] = href.split('#')
  if (/\.md$/i.test(path)) return `${docPath(path, locale)}${fragment ? `#${fragment}` : ''}`
  if (/^\/(en|ru)(?:\/docs(?:\/[\w-]+)?)?(?:#.*)?$/.test(href)) return href.replace(/^\/(en|ru)/, `/${locale}`)
  try {
    const url = new URL(href)
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password) return undefined
    if (url.hostname === 'github.com' || url.hostname === 'www.github.com') {
      if (/\/releases\/download\//.test(url.pathname)) return url.href
      return /\/actions\b|\/releases\b/.test(url.pathname) ? `/${locale}#downloads` : `/${locale}/docs/architecture`
    }
    return url.href
  } catch { return undefined }
}

export function renderMarkdown(source: string, locale: Locale, anchors: Heading[] = []): { html: string; headings: Heading[] } {
  const headings: Heading[] = []
  const counts = new Map<string, number>()
  const parser = new Marked({ gfm: true, async: false })
  parser.use({ renderer: {
    heading({ tokens, depth }) {
      const inline = this.parser.parseInline(tokens)
      const text = DOMPurify.sanitize(inline, { ALLOWED_TAGS: [] })
      const base = text.toLowerCase().replace(/[^\p{L}\p{N}_\s-]/gu, '').trim().replace(/\s+/g, '-') || 'section'
      const count = (counts.get(base) ?? 0) + 1
      counts.set(base, count)
      const id = anchors[headings.length]?.id ?? (count === 1 ? base : `${base}-${count}`)
      headings.push({ id, text, depth })
      return `<h${depth} id="${escape(id)}">${inline}</h${depth}>`
    },
    link({ href, tokens }) {
      const label = this.parser.parseInline(tokens)
      const destination = linkDestination(href, locale)
      if (!destination) return label
      const asset = localAsset(href)
      const content = asset ? `<img src="${escape(asset)}" alt="${escape(DOMPurify.sanitize(label, { ALLOWED_TAGS: [] }))}" loading="lazy"><span>${label}</span>` : label
      const external = /^https?:/.test(destination) ? ' target="_blank" rel="noopener noreferrer"' : ''
      return `<a href="${escape(destination)}"${external}${asset ? ' class="doc-screenshot"' : ''}>${content}</a>`
    },
    image({ href, text }) {
      const src = localAsset(href)
      return src ? `<img src="${escape(src)}" alt="${escape(text)}" loading="lazy">` : escape(text)
    },
  } })
  const html = DOMPurify.sanitize(parser.parse(source) as string, {
    USE_PROFILES: { html: true }, FORBID_TAGS: ['style', 'iframe', 'form', 'input', 'button'],
    FORBID_ATTR: ['style'], ADD_ATTR: ['target'],
  })
  // Apply the same navigation policy to any raw HTML links in Markdown.
  const container = document.createElement('div')
  container.innerHTML = html
  container.querySelectorAll<HTMLAnchorElement>('a').forEach(anchor => {
    const href = linkDestination(anchor.getAttribute('href') ?? '', locale)
    if (href) anchor.setAttribute('href', href)
    else anchor.removeAttribute('href')
    if (href?.startsWith('http')) { anchor.target = '_blank'; anchor.rel = 'noopener noreferrer' }
  })
  container.querySelectorAll<HTMLImageElement>('img').forEach(img => {
    if (!img.getAttribute('src')?.startsWith('/docs-assets/')) img.remove()
  })
  return { html: container.innerHTML, headings }
}
