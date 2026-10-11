import { computed, ref } from 'vue'
import { downloadUrl } from '@/shared/config'
import { detectPlatform } from './releases'
import type { DownloadKey, Package, Platform } from './releases'

export interface DownloadTarget {
  url?: string
  checksumUrl?: string
  source: 'configured' | 'unavailable'
}

export function useDownloads() {
  const detected = detectPlatform(navigator.userAgent)
  const platform = ref<Platform>(detected ?? 'windows')

  function target(kind: Package, selected = platform.value): DownloadTarget {
    const key: DownloadKey = `${kind}-${selected}`
    const url = downloadUrl(key)
    return {
      url,
      checksumUrl: url ? downloadUrl(key, true) : undefined,
      source: url ? 'configured' : 'unavailable',
    }
  }

  return {
    platform,
    detected,
    game: computed(() => target('game')),
    client: computed(() => target('client')),
  }
}

export type DownloadsState = ReturnType<typeof useDownloads>
