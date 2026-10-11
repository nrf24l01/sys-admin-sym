import { afterEach, describe, expect, it, vi } from 'vitest'
import { requestJson } from './client'

afterEach(() => vi.unstubAllGlobals())
describe('release request boundary', () => {
  it('preserves HTTP failures and support request IDs', async () => {
    vi.stubGlobal('fetch', () => Promise.resolve(new Response('{"message":"API rate limit exceeded"}', {
      status: 403, headers: { 'x-github-request-id': 'ABCD:1234' },
    })))
    await expect(requestJson('https://api.github.com/example')).rejects.toMatchObject({ status: 403, traceId: 'ABCD:1234' })
  })
  it('rejects malformed JSON instead of treating it as an empty release list', async () => {
    vi.stubGlobal('fetch', () => Promise.resolve(new Response('<html>Proxy failure</html>')))
    await expect(requestJson('https://api.github.com/example')).rejects.toThrow()
  })
})
