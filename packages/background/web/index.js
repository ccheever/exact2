// @exact/background on the web (LLP 1133.017 D17): `sync` while the page is
// open, `periodicsync` not at all. The page runtime (host/web-js/modules.js)
// calls it outside the agent; under the agent the runner's substitute answers
// and this is never loaded.
//
// - A registration fires at once when `navigator.onLine`, else at the page's
//   `online` event (D2): the specification's `register` fires "if the user
//   agent is currently online", and its service worker is not built here.
// - Tags are kept in `localStorage` for the origin, so one registered while
//   offline and not fired before the page closed fires at the next open.
// - Retrying is the runtime's rules (D1): a handler that registers its tag
//   again keeps it, eligible after 5 minutes, doubling to an hour; the third
//   attempt in a row is `lastChance`, after which it is retired unless
//   registered again during it.
// - `backgroundSyncStatus().state` is `denied` where the Permissions API's
//   `background-sync` reads so (Chromium; `registerSync` is then
//   `NotAllowedError`), `granted` where it reads anything else, and `unknown`
//   where the browser has no such permission. `periodic` is false, and
//   `registerPeriodicSync` is `NotSupportedError`.

const KEY = 'exact.background.sync';
const BACKOFF = 5 * 60_000, LONGEST = 60 * 60_000, ATTEMPTS = 3;
const bytes = s => new TextEncoder().encode(s).length;

export function implementation({ changed, wakes, log }) {
  const store = (() => { try { return globalThis.localStorage ?? null; } catch { return null; } })();
  /** tag → { registered, attempts, notBefore, firing, again } */
  const tags = new Map();
  try { for (const [tag, r] of JSON.parse(store?.getItem(KEY) ?? '[]')) tags.set(tag, { ...r, firing: false, again: false }); } catch { /* none kept */ }
  const keep = () => {
    try { store?.setItem(KEY, JSON.stringify([...tags].map(([tag, { registered, attempts, notBefore }]) => [tag, { registered, attempts, notBefore }]))); } catch { /* storage blocked: tags live as long as the page */ }
  };
  // The browser's word where it has the permission. Nothing asks for it, so a `prompt` (Chromium's in a profile
  // with no setting) is not withheld: the page fires its tags all the same.
  const permission = async () => {
    try { return (await navigator.permissions.query({ name: 'background-sync' })).state === 'denied' ? 'denied' : 'granted'; } catch { return 'unknown'; }
  };
  const online = () => typeof navigator !== 'object' || navigator.onLine !== false;
  let timer = 0;

  const fire = async tag => {
    const r = tags.get(tag);
    if (!r || r.firing) return;
    r.firing = true;
    r.again = false;
    const lastChance = r.attempts + 1 >= ATTEMPTS;
    const outcome = await wakes.raise('sync', [tag, { tag, lastChance }], { id: `sync ${tag}` });
    r.firing = false;
    if (tags.get(tag) !== r) return;
    if (outcome === 'completed' && !r.again) tags.delete(tag);
    else if (outcome === 'unheard') tags.delete(tag);
    else {
      r.attempts++;
      if (r.attempts >= ATTEMPTS && !r.again) tags.delete(tag);
      else {
        if (r.attempts >= ATTEMPTS) r.attempts = 0;
        r.notBefore = Date.now() + (r.attempts === 0 ? LONGEST : Math.min(BACKOFF * 2 ** (r.attempts - 1), LONGEST));
      }
    }
    if (!tags.has(tag)) log?.(`background.sync ${tag}: retired (${outcome})`);
    keep();
    changed('background.syncTags');
    schedule();
  };
  /** Fire each eligible tag while online, and look again when the soonest backoff ends. */
  function schedule() {
    clearTimeout(timer);
    timer = 0;
    if (!online()) return;
    const now = Date.now();
    let soonest = Infinity;
    for (const [tag, r] of tags) {
      if (r.firing) continue;
      if ((r.notBefore ?? 0) <= now) queueMicrotask(() => fire(tag));
      else soonest = Math.min(soonest, r.notBefore);
    }
    if (soonest < Infinity) timer = setTimeout(schedule, soonest - now);
  }
  if (typeof addEventListener === 'function') addEventListener('online', schedule);
  if (tags.size) queueMicrotask(schedule);

  return {
    async registerSync({ tag }) {
      if (typeof tag !== 'string' || bytes(tag) > 64) return { error: 'TypeError' };
      if (await permission() === 'denied') return { error: 'NotAllowedError' };
      const r = tags.get(tag);
      if (r?.firing) r.again = true;
      else if (!r) tags.set(tag, { registered: Date.now(), attempts: 0, notBefore: null, firing: false, again: false });
      keep();
      changed('background.syncTags');
      if (!r?.firing) schedule();
      return null;
    },
    async registerPeriodicSync() { return { error: 'NotSupportedError' }; },
    async unregisterPeriodicSync() { return null; },
    async syncTags() { return [...tags.keys()].map(tag => ({ tag, minInterval: null, lastRun: null })); },
    async periodicSyncTags() { return []; },
    async backgroundSyncStatus() {
      return { available: true, state: await permission(), periodic: false, sync: true };
    },
    release() {},
    dispose() { clearTimeout(timer); if (typeof removeEventListener === 'function') removeEventListener('online', schedule); },
  };
}
