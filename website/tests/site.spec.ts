import { mkdir } from 'node:fs/promises'
import { expect, test, type Page } from '@playwright/test'

const releaseEndpoint = /https:\/\/api\.github\.com\/repos\/.*\/releases/
const base = 'https://github.com/nrf24l01/sys-admin-sym/releases'
const assets = ['cloud-provider-sim-x86_64-pc-windows-msvc.zip', 'cloud-provider-sim-x86_64-unknown-linux-gnu.zip', 'game-ssh-x86_64-pc-windows-msvc.zip']
const published = [{ tag_name: 'v0.1.0', draft: false, prerelease: false, published_at: '2026-10-01T12:00:00Z', html_url: `${base}/tag/v0.1.0`, assets: [...assets, `${assets[0]}.sha256`].map(name => ({ name, size: 12582912, browser_download_url: `${base}/download/v0.1.0/${name}` })) }]

async function loadImagesForScreenshot(page: Page) {
  for (const image of await page.locator('main img').all()) {
    await image.scrollIntoViewIfNeeded()
    await expect.poll(() => image.evaluate(element => (element as HTMLImageElement).naturalWidth)).toBeGreaterThan(0)
  }
  await page.evaluate(() => window.scrollTo(0, 0))
}

test.beforeEach(async ({ page }) => {
  await page.route(releaseEndpoint, route => route.fulfill({ json: [] }))
})

test('shows real imagery, release-only downloads, and an operable rack view', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto('/')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Configure the network.')
  await page.getByRole('button', { name: 'Rear', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Rear', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByAltText('PowerEdge R360 / 01')).toHaveAttribute('src', '/images/server-rear.png')
  await page.getByRole('button', { name: 'Front', exact: true }).click()
  await page.getByRole('link', { name: 'Download the game', exact: true }).click()
  await expect(page).toHaveURL(/#downloads$/)
  await expect(page.getByText('Published ZIPs are not available yet.', { exact: false })).toBeVisible()
  await expect(page.locator('#downloads a[href*="/actions/"]')).toHaveCount(0)
  await page.getByRole('tab', { name: 'Windows' }).click()
  await expect(page.getByRole('button', { name: 'Cloud Provider Simulator — Windows — Not available' })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'game-ssh — Windows — Not available' })).toBeDisabled()
  await page.getByRole('button', { name: 'Linux installation' }).click()
  await expect(page.getByText('chmod +x cloud-provider-sim', { exact: false })).toBeVisible()
  await expect(page.locator('main img')).toHaveCount(8)
  const imagesLoaded = await page.locator('main img').evaluateAll(images => images.every(image => (image as HTMLImageElement).naturalWidth > 0))
  expect(imagesLoaded).toBe(true)
  expect(errors).toEqual([])
})

test('selects platform-specific ZIPs and disables a missing client', async ({ page }) => {
  await page.unroute(releaseEndpoint)
  await page.route(releaseEndpoint, route => route.fulfill({ json: published }))
  await page.goto('/#downloads')
  await page.getByRole('tab', { name: 'Windows' }).click()
  await expect(page.getByRole('link', { name: 'Cloud Provider Simulator — Windows — Download ZIP' })).toHaveAttribute('href', `${base}/download/v0.1.0/${assets[0]}`)
  await expect(page.getByRole('link', { name: 'SHA-256' })).toHaveAttribute('href', `${base}/download/v0.1.0/${assets[0]}.sha256`)
  await page.getByRole('tab', { name: 'Linux' }).click()
  await expect(page.getByRole('link', { name: 'Cloud Provider Simulator — Linux — Download ZIP' })).toHaveAttribute('href', `${base}/download/v0.1.0/${assets[1]}`)
  await expect(page.getByRole('button', { name: 'game-ssh — Linux — Not available' })).toBeDisabled()
  await expect(page.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', /trigger-linux$/)
})

test('recovers from API failure and enables release downloads after retry', async ({ page }) => {
  await page.unroute(releaseEndpoint)
  let attempts = 0
  await page.route(releaseEndpoint, route => {
    attempts += 1
    return route.fulfill(attempts === 1 ? { status: 403, headers: { 'x-github-request-id': 'TEST:RATE' }, json: { message: 'API rate limit exceeded' } } : { json: published })
  })
  await page.goto('/#downloads')
  await expect(page.getByText('Couldn’t check published releases.', { exact: false })).toBeVisible()
  await expect(page.getByRole('button', { name: /Cloud Provider Simulator.*Not available/ })).toBeDisabled()
  await page.getByRole('button', { name: /Try again/ }).click()
  await expect(page.getByRole('link', { name: /Cloud Provider Simulator.*Download ZIP/ })).toBeVisible()
  await expect(page.getByRole('button', { name: /Try again/ })).toHaveCount(0)
})

test('persists Russian copy and keeps OS controls keyboard accessible', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Switch to Russian' }).click()
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Настройте сеть.')
  await page.reload()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Станьте провайдером.')
  await page.getByRole('tab', { name: 'Windows' }).focus()
  await page.keyboard.press('ArrowRight')
  await expect(page.getByRole('tab', { name: 'Linux' })).toHaveAttribute('aria-selected', 'true')
})

test('copies supported commands and reports denied clipboard access', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write'])
  await page.goto('/#terminal')
  await page.getByRole('button', { name: 'Copy commands' }).click()
  await expect(page.getByRole('button', { name: 'Copied', exact: true })).toBeVisible()
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('game-ssh --list\ngame-ssh 1')
  await page.evaluate(() => { Object.defineProperty(navigator.clipboard, 'writeText', { configurable: true, value: () => Promise.reject(new Error('Denied')) }) })
  await page.getByRole('button', { name: 'Copied', exact: true }).click()
  await expect(page.getByText('Select and copy the commands manually.')).toBeVisible()
})

test('has no horizontal overflow at small mobile widths in either language', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 800 })
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  for (const language of ['en', 'ru']) {
    if (language === 'ru') await page.getByRole('button', { name: 'Switch to Russian' }).click()
    const dimensions = await page.evaluate(() => ({ content: document.documentElement.scrollWidth, viewport: innerWidth }))
    expect(dimensions.content).toBeLessThanOrEqual(dimensions.viewport)
    await expect(page.getByRole('link', { name: language === 'en' ? 'Download the game' : 'Скачать игру', exact: true })).toBeVisible()
  }
})

test('respects reduced motion and captures the desktop and mobile designs', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(() => document.fonts.ready)
  await expect(page.locator('[data-hero-copy]')).toHaveCSS('opacity', '1')
  await expect(page.locator('[data-hero-copy]')).toHaveCSS('transform', 'none')
  await mkdir('docs/screenshots', { recursive: true })
  await loadImagesForScreenshot(page)
  await page.screenshot({ path: 'docs/screenshots/desktop.png', fullPage: true })
  await page.screenshot({ path: 'docs/screenshots/desktop-first-screen.png' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.reload()
  await page.evaluate(() => document.fonts.ready)
  await loadImagesForScreenshot(page)
  await page.screenshot({ path: 'docs/screenshots/mobile.png', fullPage: true })
  await page.screenshot({ path: 'docs/screenshots/mobile-first-screen.png' })
})

test('keeps the page usable when browser preference storage is unavailable', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(Storage.prototype, 'getItem', { value: () => { throw new Error('Storage blocked') } })
    Object.defineProperty(Storage.prototype, 'setItem', { value: () => { throw new Error('Storage blocked') } })
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'Switch to Russian' }).click()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Настройте сеть.')
  await page.getByRole('button', { name: 'Переключить на английский' }).click()
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Configure the network.')
})

test('reveals content with GSAP and cleans up motion when the preference changes', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' })
  await page.goto('/')
  await expect(page.locator('[data-hero-copy]')).toHaveCSS('opacity', '1')
  await page.locator('#downloads').scrollIntoViewIfNeeded()
  await expect(page.locator('#downloads [data-reveal]')).toHaveCSS('opacity', '1')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await expect(page.locator('[data-hero-copy]')).toHaveCSS('transform', 'none')
})


test('reads bundled documentation on site and preserves the article when switching language', async ({ page }) => {
  await page.goto('/en/docs/game-ssh')
  await expect(page.getByRole('heading', { level: 1 })).toContainText('game-ssh')
  await expect(page.locator('.doc-prose')).toContainText('sha256sum -c')
  await expect(page.locator('a[href*="github.com"]').filter({ hasText: /GitHub|workflow|Releases/ })).toHaveCount(0)
  await page.getByRole('button', { name: 'Switch to Russian' }).click()
  await expect(page).toHaveURL(/\/ru\/docs\/game-ssh$/)
  await expect(page.locator('.doc-prose')).toContainText('Подключение')
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('lang', 'ru')
  await page.getByRole('link', { name: 'Настройка сети провайдера', exact: true }).first().click()
  await expect(page).toHaveURL(/\/ru\/docs\/provider-network$/)
  await expect(page.locator('.doc-prose')).toContainText('10.10.0.1/24')
})

test('searches guides, follows local document links and renders documentation images', async ({ page }) => {
  await page.goto('/en/docs')
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Documentation')
  await page.getByRole('searchbox', { name: 'Search documentation' }).fill('optical')
  await page.getByRole('link', { name: 'Optical networking', exact: true }).first().click()
  await expect(page).toHaveURL(/\/en\/docs\/optical-networking$/)
  await page.goto('/en/docs/server-hardware')
  await expect(page.locator('.doc-prose img')).not.toHaveCount(0)
  await loadImagesForScreenshot(page)
  await expect(page.locator('.doc-prose img')).not.toHaveCount(0)
  await expect(page.locator('.doc-prose a[href*="github.com"]')).toHaveCount(0)
  await page.getByRole('link', { name: 'Documentation', exact: true }).first().click()
  await expect(page).toHaveURL(/\/en\/docs$/)
  await page.goBack()
  await expect(page).toHaveURL(/\/en\/docs\/server-hardware$/)
})

test('shows release notes on site instead of opening the release page', async ({ page }) => {
  await page.unroute(releaseEndpoint)
  await page.route(releaseEndpoint, route => route.fulfill({ json: [{ ...published[0], body: '## Improvements\nNew optical cables. [Guide](docs/OPTICAL_NETWORKING.md)' }] }))
  await page.goto('/en#downloads')
  await page.getByRole('button', { name: 'Release notes' }).first().click()
  await expect(page.locator('#downloads')).toContainText('New optical cables.')
  await expect(page.getByRole('link', { name: 'Guide', exact: true })).toHaveAttribute('href', '/en/docs/optical-networking')
  await expect(page.locator('a[href*="/tag/"]')).toHaveCount(0)
})


test('keeps heading deep links across languages and mobile guides within the viewport', async ({ page }) => {
  await page.goto('/en/docs/game-ssh#connect')
  await expect(page.locator('#connect')).toContainText('Connect')
  await page.getByRole('button', { name: 'Switch to Russian' }).click()
  await expect(page).toHaveURL(/\/ru\/docs\/game-ssh#connect$/)
  await expect(page.locator('#connect')).toContainText('Подключение')
  await page.reload()
  await expect(page.locator('#connect')).toBeInViewport()
  await page.setViewportSize({ width: 320, height: 800 })
  for (const path of ['/ru/docs', '/ru/docs/ios', '/en/docs/hosting']) {
    await page.goto(path)
    await expect(page.locator('main h1')).toBeVisible()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(320)
  }
  await page.goto('/ru/docs/unknown-guide')
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Руководство не найдено')
})

test('sanitizes untrusted release notes before displaying them', async ({ page }) => {
  await page.unroute(releaseEndpoint)
  const body = '<img src=x onerror="window.siteInjected=true">\n\n<script>window.siteInjected=true</script>\n\n[Unsafe](javascript:alert(1))\n\n<a href="https://github.com/nrf24l01/sys-admin-sym/actions">Builds</a>'
  await page.route(releaseEndpoint, route => route.fulfill({ json: [{ ...published[0], body }] }))
  await page.goto('/en#downloads')
  await page.getByRole('button', { name: 'Release notes' }).first().click()
  await expect(page.locator('#downloads .doc-prose script, #downloads .doc-prose [onerror]')).toHaveCount(0)
  expect(await page.evaluate(() => 'siteInjected' in window)).toBe(false)
  await expect(page.getByRole('link', { name: 'Builds', exact: true })).toHaveAttribute('href', '/en#downloads')
})
