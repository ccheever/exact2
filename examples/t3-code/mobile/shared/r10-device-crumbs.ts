// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r10-device-crumbs.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r10-device: where the Files breadcrumbs settle when the file preview mounts with its file (MIT
// reference, see LICENSE-T3: FilePreviewPanel.tsx scrolls the current crumb into view whenever the file
// changes). r9-device-crumbs.ts settles a file opened for the first time in a session 36pt short of the
// trail's end, as the reference does when a file is picked in an open preview (its word-wrap toggle
// arrives after the scroll). When the preview mounts with its file already chosen (a relaunch that
// restores the panel, a thread whose panel shows a file, the panel reopened), the reference settles at
// the end even though nothing is cached: measured on the HEAD oracle at 840 × 620, scrollLeft 5 of 41
// for a first pick, 41 after two relaunches (lanes/r10-device/evidence/ref/crumbs-*).
const mounts = new WeakMap<object, string>();

/** Records that the Files preview is showing for `thread`; true when it was not showing for it before. */
export function crumbsMounting(owner: object, thread: string): boolean {
  const mounting = mounts.get(owner) !== thread;
  mounts.set(owner, thread);
  return mounting;
}

/** The Files preview is not showing (another surface, the panel closed): the next view mounts it. */
export function crumbsHidden(owner: object): void { mounts.delete(owner); waiting.delete(owner); }

// A file restored with the panel is revealed from the view's own answer (the panel is not opened by a
// command at launch). A view answer superseded while it waits never sees its replies, so a folder it asked
// for can stay unlisted with the tree's "Loading files…" up. A reveal still missing a folder after two
// seconds asks again and gives back the abandoned request's count.
type Reveal = { dirs: ReadonlyMap<string, unknown>; errors: ReadonlyMap<string, unknown>; expanded: ReadonlySet<string>; loading: number };
const missingSince = new WeakMap<object, Map<string, number>>();

/** The folders `path`'s reveal still lacks (ancestors the tree shows expanded, the root first). */
export function missingFolders(state: Reveal, path: string): string[] {
  if (!path || path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path)) return [];
  const segments = path.split('/'), out: string[] = [];
  for (let index = 0; index < segments.length; index++) {
    const folder = segments.slice(0, index).join('/');
    if ((index === 0 || state.expanded.has(folder)) && !state.dirs.has(folder) && !state.errors.has(folder)) out.push(folder);
  }
  return out;
}

/** True when the reveal has waited long enough to ask again (`now` is the window clock; 0 never retries). */
export function revealStale(state: Reveal, path: string, now: number): boolean {
  const missing = missingFolders(state, path);
  let seen = missingSince.get(state);
  if (!seen) { seen = new Map(); missingSince.set(state, seen); }
  for (const key of [...seen.keys()]) if (!missing.includes(key)) seen.delete(key);
  if (!missing.length || now <= 0) return false;
  for (const folder of missing) if (!seen.has(folder)) seen.set(folder, now);
  const stale = missing.some(folder => now - seen!.get(folder)! >= 2000);
  if (stale) for (const folder of missing) seen.set(folder, now);
  return stale;
}

// The tree's "Loading files…" counts requests by what they ask for (a folder, a file, the search), so an
// answer abandoned with its request in flight is cleared by the next request for the same thing (the
// retry above, Refresh) instead of holding the count up for the session.
const inflight = new WeakMap<object, Set<string>>();
export function loadBegin(state: { loading: number }, key: string): void {
  let keys = inflight.get(state);
  if (!keys) { keys = new Set(); inflight.set(state, keys); }
  keys.add(key); state.loading = keys.size;
}
export function loadEnd(state: { loading: number }, key: string): void {
  const keys = inflight.get(state);
  keys?.delete(key); state.loading = keys?.size ?? 0;
}

/** Whether the Files reveal shown last still lacks a folder (the shell keeps its clock ticking so it retries). */
const waiting = new WeakMap<object, boolean>();
export function noteReveal(owner: object, missing: boolean): void { waiting.set(owner, missing); }
export function revealWaiting(owner: object): boolean { return waiting.get(owner) === true; }
