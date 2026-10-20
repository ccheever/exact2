// @ref LLP 1125 D11 — `wakelock` on the JS target, bundled only for a plan
// that says it. After each commit's tree update the lock follows the
// displayed holders, by the wasm host's rule over the same DOM
// (wakelock-glue.js); a computed word outside the two is journaled as the
// runner's bridge journals it, and holds nothing. `state.wakelock` reads it.
import { After, journal, clock, viewId } from './rt.js';
import { hold, holders } from './wakelock-glue.js';

const say = line => journal.push(`t=${clock.now} ${line}`);
const root = () => document.getElementById('exact-root');
// Each element's last computed value (`$wake`, written by its binding): a
// word outside the two is journaled when the value changes to it, as the
// runner journals a binding whose value changed (runner/src/instance.rs).
const last = new WeakMap();

export function install() {
  After.push(() => {
    const r = root();
    if (!r) return;
    for (const el of r.querySelectorAll('[data-exact-wakelock]')) {
      const v = '$wake' in el ? el.$wake : el.getAttribute('data-exact-wakelock'), shown = JSON.stringify(v);
      if (last.get(el) === shown) continue;
      last.set(el, shown);
      if (v !== 'screen' && v !== 'none') say(`view ${viewId(el)}: invalid wakelock value ${shown}; unset`);
    }
    hold(holders(r).length > 0, say);
  });
  (globalThis.exact ??= {}).wakelock = () => { const by = holders(root()).map(viewId); return { held: by.length > 0, by }; };
}
