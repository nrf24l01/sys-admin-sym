export class RequestError extends Error {
  constructor(public readonly status: number, public readonly traceId?: string) {
    super('Unable to load published releases')
    this.name = 'RequestError'
  }
}

export async function requestJson(url: string, signal?: AbortSignal): Promise<unknown> {
  const controller = new AbortController()
  const cancel = () => controller.abort()
  if (signal?.aborted) cancel()
  signal?.addEventListener('abort', cancel, { once: true })
  const timeout = setTimeout(cancel, 10_000)
  try {
    const response = await fetch(url, {
      signal: controller.signal,
      headers: { Accept: 'application/vnd.github+json' },
    })
    if (!response.ok) throw new RequestError(response.status, response.headers.get('x-github-request-id') ?? undefined)
    return await response.json()
  } finally {
    clearTimeout(timeout)
    signal?.removeEventListener('abort', cancel)
  }
}
