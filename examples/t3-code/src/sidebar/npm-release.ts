// npmRelease.ts, T3 Code 6d450a4284 (MIT; see LICENSE-T3). The native fetch has no
// browser tracing middleware, so it sends the same plain GET without tracing headers.
//
// The probe is the npmRelease resource's answer (shell-members.contract), keyed by the version
// and its five-minute window (npmReleaseAtom's refresh), so nothing lets it go before npm replies.
// Not the shell's answer: an answer waits on the fetches it makes, the runner lets the shell's go
// on each revision and each 500 ms tick, and its fetch then rejects as Aborted (let-go.ts).
import { letGo } from '../client/let-go';
import type { Native } from '../client/protocol';

const releases = new Map<string, boolean>();
/** The key npm last answered: when it comes back (a server reconnecting) it reads that answer, as the kept atom does. */
let answered = '';
/** An npm manifest of one version is about 2 KB. */
const MANIFEST_BYTES = 1 << 20;
/** Whether npm has `version`, or null when the answer that asked was let go before npm replied. */
export async function npmReleasePublished(version: string, request: typeof fetch = fetch): Promise<boolean | null> {
  try {
    // Independent of the ordered fetch queue: the reply depends on no other, as the browser's fetch upstream.
    const response = await request(`https://registry.npmjs.org/t3/${encodeURIComponent(version)}`,
      { exactIndependentHttp: { maxResponseBytes: MANIFEST_BYTES } } as RequestInit);
    if (!response.ok) return false;
    const body = await response.json() as { version?: unknown };
    return body !== null && typeof body === 'object' && body.version === version;
  } catch (error) { return letGo(error) ? null : false; }
}
export const cachedReleasePublished = (version: string): boolean => releases.get(version) ?? false;
/** The resource's key: `<version>|<window>` while a server waits for that version on npm, else ''. */
export const npmReleaseKey = (version: string | null, now: number): string => version ? `${version}|${Math.floor(now / 300_000)}` : '';
/**
 * The npmRelease resource: asks npm once per key; a failure reads as not published until the next window. A changed
 * answer asks the window to read again (sidebarNotify), as the pill and the update dialog read the cached one.
 */
export async function npmReleaseAnswer(key: string, native: Native | null | undefined, request?: typeof fetch): Promise<boolean> {
  const version = key.slice(0, Math.max(0, key.indexOf('|')));
  if (!version) return false;
  if (answered === key) return cachedReleasePublished(version);
  const published = await npmReleasePublished(version, request);
  if (published === null) return cachedReleasePublished(version);
  answered = key;
  if (published !== cachedReleasePublished(version)) {
    releases.set(version, published);
    if (native?.available) try { await native.later({ op: 'sidebarNotify' }); } catch { /* the next read shows it */ }
  }
  return published;
}
