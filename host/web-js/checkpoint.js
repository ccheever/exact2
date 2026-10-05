import { carryRouter, clock, launchLocation, res, sig } from './rt.js';

// A render's checkpoint answers (LLP 1048.000 D4), as
// `exact_web::document::checkpoint` writes them: JSON text, one
// `[name, source, [args…], value]` per answer the document used, each value
// typed as `push_value` (host/web/src/page.rs) spells it — a list an array,
// a record `{"r":[…]}`, `none` `{}`, `some(v)` `{"s":v}`, unit `null`.
function encode(v, t) {
  if (t === 'n') return Object.is(v, -0) ? '-0' : Number.isFinite(v) ? JSON.stringify(v) : `{"n":"${v}"}`;
  if (t === 'b') return v ? 'true' : 'false';
  if (t === 's') return JSON.stringify(v);
  if (t === 'u') return 'null';
  if (Array.isArray(t) && t[0] === '?') return v == null ? '{}' : `{"s":${encode(v, t[1])}}`;
  if (Array.isArray(t)) return `[${v.map(x => encode(x, t[1])).join(',')}]`;
  return `{"r":[${Object.keys(t).map((k, i) => encode(v[i], t[k])).join(',')}]}`;
}
export function answers(resources, types, sourceTypes) {
  const out = [];
  resources.forEach((r, i) => {
    if (r.settled === undefined) return;
    const params = sourceTypes[r.source]?.[0] ?? [];
    out.push(`[${JSON.stringify(r.name)},${JSON.stringify(r.source)},[${r.settled.map((a, k) => encode(a, params[k] ?? 's')).join(',')}],${encode(r.value, types[i])}]`);
  });
  return `[${out.join(',')}]`;
}

// The JS dev loop's one-load checkpoint. Its separate MIME type keeps rt.js's
// rendered-document checkpoint — including an agent drive's zero clock —
// unchanged. Answers here are validated by source and full result type;
// slots and focus are development-only fields.
let Dev;
const decode = v => v === null || typeof v !== 'object' ? v : Array.isArray(v) ? v.map(decode) : '$exactNumber' in v ? Number(v.$exactNumber) : Object.fromEntries(Object.entries(v).map(([k, x]) => [k, decode(x)]));
const shape = (a, b) => JSON.stringify(a) === JSON.stringify(b);
function dev() {
  if (Dev !== undefined) return Dev;
  const el = typeof document === 'object' && document.querySelector('script[type="application/vnd.exact.dev-checkpoint"]');
  if (!el) return Dev = null;
  Dev = JSON.parse(el.textContent);
  clock.now = Number.isFinite(Dev.time) ? Dev.time : 0;
  return Dev;
}

/** A root slot carried by authored name and full declared shape. */
export function devSignal(name, initial, type, declared, router = false) {
  const cp = dev(), kept = cp?.slots?.find(s => s[0] === name);
  const value = kept && (router || shape(kept[1], declared)) ? (router ? carryRouter(decode(kept[2]), launchLocation()) : decode(kept[2])) : initial;
  const out = sig(value, type, type === "s" ? name : undefined); out.n.devName = name; out.n.devType = declared; return out;
}

/** A settled resource carried only for the same source, arguments and type. */
export function devResource(name, source, args, initial, initialArgs, type, placeholder, declared, elseRow = false) {
  const cp = dev(), kept = cp?.carryAnswers === false ? null : cp?.answers?.find(a => a[0] === name && a[1] === source && shape(a[4], declared));
  const out = res(name, source, args, kept ? decode(kept[3]) : initial, kept ? decode(kept[2]) : initialArgs, type, placeholder, !!kept || elseRow);
  out.r.devType = declared; if (kept?.[5]) out.r.store = true; return out;
}

/** Suppress boot autofocus during a carried restart, then restore by the
 * old tree path and kernel node type exactly as the wasm page does. */
export function prepareDev() {
  const cp = dev();
  if (!cp) return () => {};
  const focus = HTMLElement.prototype.focus;
  HTMLElement.prototype.focus = function () {};
  return () => {
    HTMLElement.prototype.focus = focus;
    if (!cp.focus) return;
    // A virtualized row wrapper is a logical View in the wasm tree even
    // though it is host-created and has no plan-node carry attribute.
    const children = e => [...(e?.children ?? [])].filter(x => x.hasAttribute('data-carry-type') || x.hasAttribute('data-listitemkey'));
    let el = document.getElementById('exact-root');
    for (const at of cp.focus.path) el = children(el)[at];
    if (el?.getAttribute('data-carry-type') === cp.focus.type && el.getClientRects().length && !el.matches(':disabled') && !el.closest('[inert]')) focus.call(el, { preventScroll: true });
  };
}
