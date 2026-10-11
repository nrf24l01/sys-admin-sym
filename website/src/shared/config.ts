import type { DownloadKey } from '@/features/downloads/releases'

declare global {
  interface Window {
    __APP_CONFIG__?: {
      downloadBaseUrl?: string
    }
  }
}

const filenames: Record<DownloadKey, string> = {
  'game-windows': 'cloud-provider-sim-x86_64-pc-windows-msvc.zip',
  'game-linux': 'cloud-provider-sim-x86_64-unknown-linux-gnu.zip',
  'client-windows': 'game-ssh-x86_64-pc-windows-msvc.zip',
  'client-linux': 'game-ssh-x86_64-unknown-linux-gnu.zip',
}

function normalizedDownloadBaseUrl(value: string | undefined): string | undefined {
  if (!value?.trim()) return undefined
  try {
    const url = new URL(value.trim())
    if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) return undefined
    return url.href.replace(/\/+$/, '')
  } catch {
    return undefined
  }
}

const downloadBaseUrl = normalizedDownloadBaseUrl(
  typeof window === 'undefined' ? undefined : window.__APP_CONFIG__?.downloadBaseUrl,
)

export function downloadUrl(key: DownloadKey, checksum = false): string | undefined {
  if (!downloadBaseUrl) return undefined
  return `${downloadBaseUrl}/resources/${filenames[key]}${checksum ? '.sha256' : ''}`
}
