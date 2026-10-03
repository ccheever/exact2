// Native modules on the JS runtime (LLP 1024 D3–D5, D7; LLP 1067 D5; LLP
// 1071 §7): the wasm host's own `native-glue.js`, unchanged, over the app's
// module artifact (`modules/index.js`, its `modules/web/`). A loaded chunk,
// fetched after first paint by the first module element (rt.js `nm`) or a
// TypeScript source's `native` (ts-data.js); an app without modules never
// has it.
import './native-glue.js';

// The runner's dispatch codes (native-glue.js `DISPATCH`) as the runtime's
// handler kinds; hover's over and off are its true and false.
const KINDS = [['press'], ['change'], ['hover', true], ['hover', false], ['focus'], ['blur'], ['key'], ['submit'], ['load'], ['message']];
let views = null, page = null;

/** The module elements' host: a module's event is an `exact-native` event
 * on its element, which the handlers take (rt.js `on`). */
export function viewHost(say) {
  const x = globalThis.exact;
  // The glue dispatches once the page is ready, which it awaits.
  if (typeof x.ready?.then !== 'function') x.ready = Promise.resolve(x.ready);
  return views ??= x.nativeHost({ log: say, dispatch(el, code, text) {
    const [kind, value] = KINDS[code] ?? [];
    if (kind) el.dispatchEvent(new CustomEvent('exact-native', { detail: { kind, value: value ?? text } }));
  } });
}

/** The app's page module itself: its `element` hooks (hooks.js). */
export const pageTable = () => globalThis.exact.nativeModule();

/** `native.later` and device topics (LLP 1067 D5, 1016.002): the artifact's
 * `later(request)`, and its `connect` announcements, each re-asking the
 * resources whose answers watched the topic (`changed`). */
export function pageModule({ changed, agent, now }) {
  page ??= globalThis.exact.pageNative(true, { changed, ready: () => true, generation: () => 0, agent, now });
  return page.then(p => ({
    later: request => p.later(btoa(String.fromCharCode(...new TextEncoder().encode(JSON.stringify(request))))).then(JSON.parse),
  }));
}
