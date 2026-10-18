// npmRelease.ts, T3 Code 9c161462c9 (MIT; see LICENSE-T3). The native fetch has no
// browser tracing middleware, so it sends the same plain GET without tracing headers.
const releases = new Map<string, { checkedAt: number; published: boolean; pending: boolean }>();
export async function npmReleasePublished(version: string, request: typeof fetch = fetch): Promise<boolean> {
  try {
    const response = await request(`https://registry.npmjs.org/t3/${encodeURIComponent(version)}`);
    if (!response.ok) return false;
    const body = await response.json() as { version?: unknown };
    return body !== null && typeof body === 'object' && body.version === version;
  } catch { return false; }
}
export const cachedReleasePublished = (version: string): boolean => releases.get(version)?.published ?? false;
/** A publication probe never holds up the sidebar; failed/not-yet-published versions retry in five minutes. */
export function releasePublished(version: string, now: number, notify?: () => void): boolean {
  let state = releases.get(version);
  if (!state || !state.pending && now - state.checkedAt >= 300_000) {
    state = { checkedAt: now, published: state?.published ?? false, pending: true };
    releases.set(version, state);
    const current = state;
    void npmReleasePublished(version).then(published => { current.published = published; current.pending = false; notify?.(); });
  }
  return state.published;
}
