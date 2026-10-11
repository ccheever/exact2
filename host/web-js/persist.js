// Persisted state (LLP 1116 D5): `state … persist` kept in the page's
// `localStorage` under "exact.state.<name>", as the runner keeps it
// (runner/src/runner/persist.rs): JSON text, a number as JSON's (non-finite
// ones as "NaN", "Infinity", "-Infinity", -0 as `-0`), an option as `null`
// or its value, a list as an array. Read as the slot is made, before the
// first commit; after every commit that stands the store holds each
// persisted slot's value. Types are the emitter's `type_json`: "n", "s",
// "b", ["?", t], ["[", t].
import { After, say } from './rt.js';

const LIMIT = 16384, KEY = 'exact.state.', Kept = [];
const NONFINITE = { NaN: NaN, Infinity: Infinity, '-Infinity': -Infinity };
const bytes = text => new TextEncoder().encode(text).length;
const name = t => typeof t === 'string' ? { n: 'number', s: 'string', b: 'bool' }[t] : `${t[0] === '?' ? 'option' : 'list'}<${name(t[1])}>`;

/** `v`, parsed JSON, as a value of type `t`; `undefined` when it is not one. */
function revive(v, t) {
  if (t === 'n') return typeof v === 'number' ? v : typeof v === 'string' && v in NONFINITE ? NONFINITE[v] : undefined;
  if (t === 's') return typeof v === 'string' ? v : undefined;
  if (t === 'b') return typeof v === 'boolean' ? v : undefined;
  if (t[0] === '?') return v === null ? null : revive(v, t[1]);
  if (t[0] !== '[' || !Array.isArray(v)) return undefined;
  const out = v.map(x => revive(x, t[1]));
  return out.includes(undefined) ? undefined : out;
}

function encode(v, t) {
  if (t === 'n') return Number.isFinite(v) ? (Object.is(v, -0) ? '-0' : JSON.stringify(v)) : `"${v}"`;
  if (typeof t === 'string') return JSON.stringify(v);
  if (t[0] === '?') return v == null ? 'null' : encode(v, t[1]);
  return `[${v.map(x => encode(x, t[1])).join(',')}]`;
}

/** Persisted slot `slot`'s first value: the stored one where it fits type `t`, else `initial`. */
export function stored(slot, initial, t) {
  let text = null;
  try { text = localStorage.getItem(KEY + slot); } catch {}
  if (text == null) return initial;
  let v;
  try { if (bytes(text) <= LIMIT) v = revive(JSON.parse(text), t); } catch {}
  if (v !== undefined) return v;
  say(`persisted state ${slot}: the stored value does not fit ${name(t)}: the initial value stands`);
  return initial;
}

/** Keep signal `s` (persisted slot `slot` of type `t`) in the store after each commit. */
export function keep(s, slot, t) {
  let last = null;
  try { last = localStorage.getItem(KEY + slot); } catch {}
  Kept.push({ s, slot, t, last });
  if (Kept.length === 1) After.push(flush);
}

function flush() {
  for (const k of Kept) {
    const text = encode(k.s.n.v, k.t);
    if (text === k.last) continue;
    if (bytes(text) > LIMIT) { say(`persisted state ${k.slot}: ${bytes(text)} bytes is over the ${LIMIT} a persisted value may hold: not kept (the store keeps its last value)`); k.last = text; continue; }
    try { localStorage.setItem(KEY + k.slot, text); k.last = text; say(`store ${KEY}${k.slot}`); } catch {}
  }
}
