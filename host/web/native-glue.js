// Native modules on the web (@ref LLP 1024 D2–D5, D7): the host half of the
// app's module table. Loaded after first paint, only when a plan names a
// module tag; it then injects the app's own module (`./modules/index.js`, the app's `modules/web/`), whose
// exports are the table's names:
//
//   export const abi = 1;
//   export const roster = { "photo-editor": { snapshot: false } };
//   export function create(tag, element, propsJson, event, reply) → handle
//   export function setProps(handle, propsJson)   // throws: rejected, last kept
//   export function snapshot(handle, token)       // optional; unused here: the
//                                                 // page capture has every element
//   export function destroy(handle)
//
// `event(kind, payload)` takes the kernel's EventKind ordinals — press 0,
// change 1, hover 2, focus 3, blur 4, key 5, submit 6, load 7, message 8 —
// with a string payload for change, key and message and a boolean for hover.
//
// The node is the real custom element: this file defines each roster tag
// once (a reload reuses the definitions), and the module renders into the
// element it is given. Events are copied, checked against the instance's
// nonce, and dispatched after the batch that caused them, never inside it.
const ABI = 1;
// The runner's dispatch codes for EventKind ordinals 0–8 (hover: over 2, off 3).
const DISPATCH = [0, 1, 2, 4, 5, 6, 7, 8, 9];
const NAMES = ['press', 'change', 'hover', 'focus', 'blur', 'key', 'submit', 'load', 'message'];

globalThis.exact.nativeHost = ({ dispatch, log }) => {
  let table = null, failure = null, nonce = 0;
  const waiting = new Set();
  const defines = (globalThis.exact.nativeDefines ??= { count: 0 });
  const say = (line) => { log(`native ${line}`); };
  const url = new URL('./modules/index.js', document.baseURI).href;
  const loaded = new Promise((resolve, reject) => {
    // An injected module script, never an `import()` (LLP 1024 D3): the
    // inline script's static import keeps the module's exports its names.
    const script = document.createElement('script');
    script.type = 'module';
    globalThis.exact.nativeTable = resolve;
    script.textContent = `import * as table from ${JSON.stringify(url)}; globalThis.exact.nativeTable(table);`;
    script.onerror = () => reject(new Error(`the module artifact ${url} did not load`));
    addEventListener('error', (e) => { if (e.filename === url) reject(new Error(`${url}: ${e.message}`)); }, { once: true });
    setTimeout(() => reject(new Error(`the module artifact ${url} did not load in 10 s`)), 10000);
    document.head.append(script);
  }).then((m) => {
    if (m.abi !== ABI) throw Object.assign(new Error(`module ABI ${m.abi}, host ABI ${ABI}`), { state: 'unavailable' });
    if (!m.roster || typeof m.create !== 'function' || typeof m.setProps !== 'function' || typeof m.destroy !== 'function') {
      throw Object.assign(new Error(`${url} is not a module table (abi, roster, create, setProps, destroy)`), { state: 'unavailable' });
    }
    for (const tag of Object.keys(m.roster)) {
      if (!customElements.get(tag)) { customElements.define(tag, class extends HTMLElement {}); defines.count += 1; }
    }
    table = m;
    say(`loaded ${url}: ${Object.keys(m.roster).join(', ')} (abi ${m.abi})`);
  }).catch((error) => {
    failure = error;
    say(`unavailable: ${error.message}`);
  }).finally(() => { for (const el of waiting) attachNow(el); waiting.clear(); });

  function fail(st, state, error) {
    st.state = state; st.error = error;
    say(`${st.name} #${st.id}: ${state}: ${error}`);
  }
  function attachNow(el) {
    const st = el.exactNative;
    if (!st || st.destroyed) return;
    if (failure) return fail(st, failure.state ?? 'unavailable', failure.message);
    // The name is a table key, checked again on plan bytes (D2, D5).
    if (!/^[a-z][a-z0-9_]*(-[a-z][a-z0-9_]*)+$/.test(st.name)) return fail(st, 'error', `refused module name ${JSON.stringify(st.name)}`);
    if (!Object.hasOwn(table.roster, st.name)) return fail(st, 'error', `the module artifact has no factory for ${st.name}`);
    const mine = ++nonce;
    st.nonce = mine;
    const live = () => st.nonce === mine && !st.destroyed;
    const event = (kind, payload) => {
      const text = payload == null ? null : String(payload);
      if (!live()) { say(`${st.name} #${st.id}: dropped ${NAMES[kind] ?? kind} from nonce ${mine} after destroy`); return; }
      if (!Number.isInteger(kind) || kind < 0 || kind > 8) { say(`${st.name} #${st.id}: refused event kind ${kind}`); return; }
      const code = kind === 2 ? (text === 'true' ? 2 : 3) : DISPATCH[kind];
      // After the batch in flight: a module never re-enters `apply`.
      queueMicrotask(() => {
        if (!live()) { say(`${st.name} #${st.id}: dropped ${NAMES[kind]} from nonce ${mine} after destroy`); return; }
        globalThis.exact.ready.then(() => { if (live()) dispatch(el, code, kind === 1 || kind === 5 || kind === 8 ? (text ?? '') : null); }, () => {});
      });
    };
    // The browser composites every module element into the page capture, so
    // the web never asks for a snapshot; a reply is accepted and unused.
    const reply = () => {};
    const props = el.getAttribute('data-nativeviewprops') ?? '{}';
    try { st.handle = table.create(st.name, el, props, event, reply); }
    catch (error) { st.nonce = 0; return fail(st, 'error', `create refused: ${error?.message ?? error}`); }
    st.props = props; st.state = 'ready'; delete st.error;
    say(`${st.name} #${st.id}: ready`);
    // Props replace the whole aggregate; a refusal keeps the last accepted.
    st.observer = new MutationObserver(() => {
      const next = el.getAttribute('data-nativeviewprops') ?? '{}';
      if (next === st.props || !live()) return;
      try { table.setProps(st.handle, next); st.props = next; if (st.state === 'error') { st.state = 'ready'; delete st.error; } }
      catch (error) { fail(st, 'error', `props refused: ${error?.message ?? error}`); }
    });
    st.observer.observe(el, { attributes: true, attributeFilter: ['data-nativeviewprops'] });
  }
  return {
    attach(el) { if (table || failure) attachNow(el); else waiting.add(el); },
    destroy(el) {
      const st = el.exactNative;
      waiting.delete(el);
      if (!st || !st.nonce) return;
      st.nonce = 0; st.observer?.disconnect();
      try { table.destroy(st.handle); } catch (error) { say(`${st.name} #${st.id}: destroy threw ${error?.message ?? error}`); }
      say(`${st.name} #${st.id}: destroyed`);
    },
    loaded,
  };
};
