// @ref LLP 1133.029 D7 — `orientationlock` on the JS target, bundled only
// for a plan that says it. After each commit's tree update the lock follows
// the counted holders, by orientationlock-glue.js's rules over the same DOM
// the wasm host projects; a computed word outside the table is journaled as
// the runner's bridge journals it, and holds nothing.
// `state.orientationlock` reads it.
import { After, journal, clock, viewId } from './rt.js';
import { WORDS, hold, session, state } from './orientationlock-glue.js';

const say = line => journal.push(`t=${clock.now} ${line}`);
const root = () => document.getElementById('exact-root');
// Each element's last computed value (`$orient`, written by its binding).
const last = new WeakMap();

export function install() {
  After.push(() => {
    const r = root();
    if (!r) return;
    for (const el of r.querySelectorAll('[data-exact-orientationlock]')) {
      const v = '$orient' in el ? el.$orient : el.getAttribute('data-exact-orientationlock'), shown = JSON.stringify(v);
      if (last.get(el) === shown) continue;
      last.set(el, shown);
      if (!WORDS.includes(v)) say(`view ${viewId(el)}: invalid orientationlock value ${shown}; unset`);
    }
    hold(session(r), say, viewId);
  });
  (globalThis.exact ??= {}).orientationlock = () => state(root(), viewId);
}
