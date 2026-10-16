// @ref LLP 1125 D11 — `wakelock` on the JS target, bundled only for a plan
// that says it. After each commit's tree update the lock follows the
// displayed holders, by the wasm host's rule over the same DOM
// (wakelock-glue.js); a computed word outside the two is journaled as the
// runner's bridge journals it, and holds nothing. `state.wakelock` reads it.
import { After, journal, clock, viewId } from './rt.js';
import { hold, holders } from './wakelock-glue.js';

const say = line => journal.push(`t=${clock.now} ${line}`);
const root = () => document.getElementById('exact-root');
const told = new WeakMap();

export function install() {
  After.push(() => {
    const r = root();
    if (!r) return;
    for (const el of r.querySelectorAll('[data-exact-wakelock]')) {
      const v = el.getAttribute('data-exact-wakelock');
      if (v !== 'screen' && v !== 'none' && told.get(el) !== v) { told.set(el, v); say(`view ${viewId(el)}: invalid wakelock value ${JSON.stringify(v)}; unset`); }
    }
    hold(holders(r).length > 0, say);
  });
  (globalThis.exact ??= {}).wakelock = () => { const by = holders(root()).map(viewId); return { held: by.length > 0, by }; };
}
