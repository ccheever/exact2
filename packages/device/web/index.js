// @exact/device on the web (LLP 1133.031 D1, D2, D3): the page's own
// `navigator.getBattery()` and `navigator.connection`, in the interface's
// shapes. The page runtime (host/web-js/modules.js) calls it outside the
// agent; under the agent the runner's substitute answers.
//
// - Both exist only in Chromium. Where one is missing its source fails
//   `NotSupportedError`, so `failed(…)` is the feature test (D14).
// - `level` is rounded to hundredths, as on every target (D1).
// - Each source subscribes to its object's change events at its first read
//   and announces its topic only when an answered field changes (D5); the
//   last reader's release unsubscribes.
// - `powerState` has no web API (D3): `NotSupportedError`.

const round = (level) => Math.round(level * 100) / 100;
const battery = (b) => ({ charging: !!b.charging, level: round(b.level) });
const TYPES = new Set(['bluetooth', 'cellular', 'ethernet', 'mixed', 'none', 'other', 'unknown', 'wifi', 'wimax']);
const network = (c) => ({ type: TYPES.has(c.type) ? c.type : 'unknown', saveData: !!c.saveData });
const same = (a, b) => a != null && b != null && Object.keys(a).every((k) => a[k] === b[k]);

export function implementation({ changed }) {
  const nav = typeof navigator === 'object' ? navigator : {};
  // Each source's subscription, made at the first read: its last answer and how to stop it.
  const watching = {};

  /** Subscribe `name` to `target`'s `events`, announcing when `read(target)` changes. */
  function watch(name, target, events, read) {
    if (watching[name]) return watching[name];
    const entry = { last: read(target) };
    const heard = () => {
      const now = read(target);
      if (same(now, entry.last)) return;
      entry.last = now;
      changed(`device.${name}`);
    };
    for (const e of events) target.addEventListener(e, heard);
    entry.stop = () => { for (const e of events) target.removeEventListener(e, heard); };
    return (watching[name] = entry);
  }

  return {
    async getBattery() {
      if (typeof nav.getBattery !== 'function') return { error: 'NotSupportedError' };
      let manager;
      try {
        manager = await nav.getBattery();
      } catch (e) {
        // A Permissions Policy that does not allow `battery` (D1).
        return { error: e?.name === 'NotAllowedError' ? 'NotAllowedError' : 'NotSupportedError', message: e?.message };
      }
      const entry = watch('getBattery', manager, ['levelchange', 'chargingchange'], battery);
      return (entry.last = battery(manager));
    },
    async connection() {
      const c = nav.connection;
      if (!c) return { error: 'NotSupportedError' };
      const entry = watch('connection', c, ['change'], network);
      return (entry.last = network(c));
    },
    async powerState() {
      return { error: 'NotSupportedError' };
    },
    /** The last reader of `name` is gone: stop listening. */
    release(name) {
      watching[name]?.stop?.();
      delete watching[name];
    },
  };
}
