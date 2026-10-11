export type Platform = 'windows' | 'linux'
export type Package = 'game' | 'client'
export interface DownloadAsset {
  name: string
  url: string
  size: number
  version: string
  date: string
  notes: string
  checksumUrl?: string
}
export type DownloadKey = `${Package}-${Platform}`
export type DownloadCatalog = Partial<Record<DownloadKey, DownloadAsset>>

const filenames: Record<DownloadKey, string> = {
  'game-windows': 'cloud-provider-sim-x86_64-pc-windows-msvc.zip',
  'game-linux': 'cloud-provider-sim-x86_64-unknown-linux-gnu.zip',
  'client-windows': 'game-ssh-x86_64-pc-windows-msvc.zip',
  'client-linux': 'game-ssh-x86_64-unknown-linux-gnu.zip',
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

export function safeDownloadUrl(value: string | undefined): string | undefined {
  if (!value) return undefined
  try {
    const url = new URL(value.trim())
    return url.protocol === 'https:' && !url.username && !url.password ? url.href : undefined
  } catch { return undefined }
}

function releaseUrl(value: unknown, repository: string): string | undefined {
  if (typeof value !== 'string') return undefined
  const safe = safeDownloadUrl(value)
  return safe?.startsWith(`https://github.com/${repository}/releases/`) ? safe : undefined
}

function assetUrl(value: unknown, repository: string, filename: string): string | undefined {
  const safe = releaseUrl(value, repository)
  if (!safe) return undefined
  const url = new URL(safe)
  try {
    return url.pathname.startsWith(`/${repository}/releases/download/`) && decodeURIComponent(url.pathname.split('/').at(-1) ?? '') === filename ? safe : undefined
  } catch { return undefined }
}

export function resolveDownloads(data: unknown, repository = 'nrf24l01/sys-admin-sym'): DownloadCatalog {
  if (!Array.isArray(data)) return {}
  const catalog: DownloadCatalog = {}
  const releases = data.filter(record).filter(item =>
    item.draft === false && item.prerelease === false && typeof item.tag_name === 'string'
    && typeof item.published_at === 'string' && Number.isFinite(Date.parse(item.published_at))
    && Array.isArray(item.assets) && releaseUrl(item.html_url, repository),
  ).sort((a, b) => Date.parse(String(b.published_at)) - Date.parse(String(a.published_at)))

  for (const release of releases) {
    const assets = (release.assets as unknown[]).filter(record)
    for (const key of Object.keys(filenames) as DownloadKey[]) {
      if (catalog[key]) continue
      const asset = assets.find(item => item.name === filenames[key])
      const url = assetUrl(asset?.browser_download_url, repository, filenames[key])
      if (!asset || !url || typeof asset.size !== 'number' || !Number.isSafeInteger(asset.size) || asset.size <= 0) continue
      const checksum = assets.find(item => item.name === `${filenames[key]}.sha256`)
      catalog[key] = {
        name: filenames[key], url, size: asset.size, version: String(release.tag_name),
        date: String(release.published_at), notes: typeof release.body === 'string' ? release.body : '',
        checksumUrl: assetUrl(checksum?.browser_download_url, repository, `${filenames[key]}.sha256`),
      }
    }
  }
  return catalog
}

export function detectPlatform(userAgent: string): Platform | null {
  if (/Android|iPhone|iPad|Mobile/i.test(userAgent)) return null
  if (/Windows/i.test(userAgent)) return 'windows'
  if (/Linux.*(?:x86_64|amd64)/i.test(userAgent)) return 'linux'
  return null
}
