// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { renderMarkdown } from './markdown'

describe('on-site documentation renderer', () => {
  it('maps relative guides and GitHub documentation to the current locale', () => {
    const { html } = renderMarkdown('[IOS](IOS_GUIDE.md#configuration) [Linux](https://github.com/nrf24l01/sys-admin-sym/blob/main/docs/LINUX_GUIDE.md) [Download](/#downloads)', 'ru')
    expect(html).toContain('href="/ru/docs/ios#configuration"')
    expect(html).toContain('href="/ru/docs/linux"')
    expect(html).toContain('href="/ru#downloads"')
    expect(html).not.toContain('href="https://github.com')
  })
  it('keeps all GitHub browse links on site, while allowing direct release files', () => {
    const { html } = renderMarkdown('[Build](https://github.com/nrf24l01/sys-admin-sym/actions/workflows/build-game.yml) [Source](https://github.com/nrf24l01/sys-admin-sym/blob/main/crates/sim/src/lib.rs) [ZIP](https://github.com/nrf24l01/sys-admin-sym/releases/download/v1/game.zip)', 'en')
    expect(html).toContain('href="/en#downloads"')
    expect(html).not.toContain('href="https://github.com/nrf24l01/sys-admin-sym/blob')
    expect(html).toContain('href="https://github.com/nrf24l01/sys-admin-sym/releases/download/v1/game.zip"')
  })
  it('strips executable HTML and unsafe Markdown links', () => {
    const { html } = renderMarkdown('<script>alert(1)</script>\n\n<img src=x onerror="alert(1)">\n\n[click](javascript:alert(1))\n\n<iframe src="https://bad.test"></iframe>', 'en')
    expect(html).not.toMatch(/<script|onerror|javascript:|<iframe/)
  })
  it('preserves manufacturer image-source links instead of treating them as bundled screenshots', () => {
    const href = 'https://dl.dell.com/content/guides/public/Html/r360_ism/images/GUID-1EA46641-4637-4621-BA2C-16A366798F5F-low.jpg'
    const { html } = renderMarkdown(`[Original photograph](${href})`, 'en')
    expect(html).toContain(`href="${href}"`)
    expect(html).not.toContain('<img')
  })
  it('provides unique heading anchors and maps bundled images', () => {
    const { html, headings } = renderMarkdown('# Guide\n## Ports\n## Ports\n![Ports](screenshots/dell-server-interfaces-en.png)', 'en')
    expect(headings.map(heading => heading.id)).toEqual(['guide', 'ports', 'ports-2'])
    expect(html).toContain('id="ports-2"')
    expect(html).toContain('src="/docs-assets/screenshots/dell-server-interfaces-en.png"')
  })
})
