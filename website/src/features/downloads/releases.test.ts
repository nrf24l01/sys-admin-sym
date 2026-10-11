import { describe, expect, it } from 'vitest'
import { detectPlatform, resolveDownloads, safeDownloadUrl } from './releases'

const base = 'https://github.com/nrf24l01/sys-admin-sym/releases'
const release = (version: string, assets: string[], extras = {}) => ({
  tag_name: version, draft: false, prerelease: false, published_at: '2026-10-01T12:00:00Z',
  html_url: `${base}/tag/${version}`,
  assets: assets.map(name => ({ name, size: 12582912, browser_download_url: `${base}/download/${version}/${name}` })),
  ...extras,
})
const windowsGame = 'cloud-provider-sim-x86_64-pc-windows-msvc.zip'
const linuxGame = 'cloud-provider-sim-x86_64-unknown-linux-gnu.zip'
const windowsClient = 'game-ssh-x86_64-pc-windows-msvc.zip'

describe('published downloads', () => {
  it('finds the game and independently versioned client across releases', () => {
    const result = resolveDownloads([
      release('game-ssh-v0.2', [windowsClient, `${windowsClient}.sha256`]),
      release('v0.1', [windowsGame, linuxGame]),
    ])
    expect(result['game-windows']?.url).toBe(`${base}/download/v0.1/${windowsGame}`)
    expect(result['game-linux']?.version).toBe('v0.1')
    expect(result['client-windows']?.version).toBe('game-ssh-v0.2')
    expect(result['client-windows']?.checksumUrl).toBe(`${base}/download/game-ssh-v0.2/${windowsClient}.sha256`)
    expect(result['client-linux']).toBeUndefined()
  })
  it('skips drafts, prereleases and newer releases lacking the required platform', () => {
    const result = resolveDownloads([
      release('beta', [windowsGame], { prerelease: true }),
      release('draft', [windowsGame], { draft: true }),
      release('v0.3', [linuxGame], { published_at: '2026-10-03T12:00:00Z' }),
      release('v0.2', [windowsGame]),
    ])
    expect(result['game-windows']?.version).toBe('v0.2')
    expect(result['game-linux']?.version).toBe('v0.3')
  })
  it('rejects release-page URLs masquerading as downloadable ZIPs', () => {
    const result = resolveDownloads([release('v0.1', [windowsGame], { assets: [{ name: windowsGame, size: 1, browser_download_url: `${base}/tag/v0.1` }] })])
    expect(result).toEqual({})
  })
  it('sorts publication dates instead of trusting release API ordering', () => {
    const result = resolveDownloads([
      release('v0.1', [windowsGame], { published_at: '2026-09-01T12:00:00Z' }),
      release('v0.2', [windowsGame]),
    ])
    expect(result['game-windows']?.version).toBe('v0.2')
  })
  it('ignores unrelated filenames, malformed metadata and hostile asset URLs', () => {
    expect(resolveDownloads([null, {}, release('v0.1', ['game-x86_64-pc-windows-msvc.zip']),
      release('v0.2', [windowsGame], { assets: [{ name: windowsGame, size: 1, browser_download_url: 'javascript:alert(1)' }] }),
      release('v0.3', [windowsGame], { html_url: 'https://evil.test/release' }),
    ])).toEqual({})
    expect(resolveDownloads({ message: 'API rate limit exceeded' })).toEqual({})
    expect(resolveDownloads([])).toEqual({})
  })
})

describe('platform and configured download links', () => {
  it('recognizes supported desktops without assigning Linux to macOS or mobile', () => {
    expect(detectPlatform('Mozilla/5.0 (Windows NT 10.0; Win64; x64)')).toBe('windows')
    expect(detectPlatform('Mozilla/5.0 (X11; Linux x86_64)')).toBe('linux')
    expect(detectPlatform('Mozilla/5.0 (Macintosh; Intel Mac OS X)')).toBeNull()
    expect(detectPlatform('Mozilla/5.0 (Linux; Android 14)')).toBeNull()
  })
  it('accepts configured HTTPS downloads and rejects executable protocols or URL credentials', () => {
    expect(safeDownloadUrl(' https://downloads.example.com/game.zip ')).toBe('https://downloads.example.com/game.zip')
    expect(safeDownloadUrl('javascript:alert(1)')).toBeUndefined()
    expect(safeDownloadUrl('http://example.com/game.zip')).toBeUndefined()
    expect(safeDownloadUrl('https://name:password@example.com/game.zip')).toBeUndefined()
  })
})
