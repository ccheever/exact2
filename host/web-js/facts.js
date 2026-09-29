// The runner's reserved sources on the JS target, each answered before the
// app's (`data.reserved`, as the entry answers `exactTime`) by the declared
// fields' names, and re-answered in one commit when the fact changes.
// Imported by an app's module only for the sources its plan declares.
// - `exactViewport` (LLP 1039 D1–D4, 1061 D4; runner/src/viewport.rs): the
//   page's own readings (`navigation.js`), on every resize (glue.js
//   `mediaChanged`).
// - `exactPage` (LLP 1069.000 D2; runner/src/page.rs): visibility, online,
//   share sheet, from the web host's own `pageReporter`; under the agent the
//   drive's (`prefer page`).
// - `exactDelivery` (LLP 1030 D7; runner/src/delivery.rs): what the build
//   baked; a JS build links no update store, so nothing is ever staged.
// - `exactSurface` (LLP 1047 D3; runner/src/surface_record.rs): a GPU
//   surface's published record, decoded against each reader's shape.
import { data, Resources, commit, R, Refusal } from "./rt.js";
import { preferences, onPreferences, pageReporter } from "./navigation.js";

const again = (source, why) => () => commit(() => { for (const r of Resources) if (r.source === source) R(r); }, why);
// `readers`: resource name → [its fields, in its shape's order, …].
const byName = (readers, name, v) => readers[name][0].map(n => v[n]);

const CONTRAST = ["no-preference", "more", "less", "custom"];
export function viewport(readers) {
  (data.reserved ??= {}).exactViewport = (_, a, name) => {
    const p = preferences();
    return byName(readers, name, { width: innerWidth, height: innerHeight, prefersReducedMotion: !!(p & 1), prefersReducedTransparency: !!(p & 2),
      prefersContrast: CONTRAST[(p >> 2) & 3], prefersColorScheme: p & 16 ? "dark" : "light" });
  };
  if (typeof addEventListener !== "function") return;
  const changed = again("exactViewport", "viewport");
  addEventListener("resize", changed);
  onPreferences(changed);
}

export function page(readers) {
  const agent = typeof location === "object" && new URLSearchParams(location.search).has("agent");
  const facts = typeof document === "object" && document.createElement ? pageReporter(agent) : null;
  (data.reserved ??= {}).exactPage = (_, a, name) => {
    // A render has no page: the bake's answer (runner/src/page.rs `Page::default`).
    const f = facts ? facts.read() : { "visibility-state": "visible", online: true, "can-share": false };
    return byName(readers, name, { visibilityState: f["visibility-state"], onLine: f.online, canShare: f["can-share"] });
  };
  if (!facts) return;
  const changed = again("exactPage", "page");
  facts.onChange(changed);
  // The agent's `prefer` page group (LLP 1069.000 D6).
  (globalThis.exact ??= {}).page = { prefer: p => { facts.prefer(p); changed(); return facts.read(); } };
}

/** Each reader's second entry: the value the build baked, if it baked one
 * (a plan built from source did not): the embedded answer. */
export function delivery(readers) {
  const embedded = { stream: "embedded", seq: 0, embeddedSeq: 0, staged: false, sunset: "", interpreted: [], compatibilityId: "" };
  (data.reserved ??= {}).exactDelivery = (_, a, name) => readers[name][1] ?? byName(readers, name, embedded);
}

/** `readers`: resource name → [surface name, the shape by field name
 * (names.js `types`)]. A record arrives from the GPU module (`gpu-glue.js`
 * calls the wasm host's `exact_surface_record`); the host's bytes are
 * `name` alone (disposed) or `name\0json`. */
export function surfaces(readers) {
  const records = new Map();
  (data.reserved ??= {}).exactSurface = (_, args, name) => {
    const [surface, shape] = readers[name];
    const text = records.get(surface);
    let json;
    if (text != null) {
      if (text.length > 65536) throw new Refusal(`${name}: record exceeds 64 KiB`);
      try { json = JSON.parse(text); } catch (e) { throw new Refusal(`${name}: ${e.message}`); }
      if (!json || typeof json !== "object" || Array.isArray(json)) throw new Refusal(`${name}: record: expected JSON object`);
    }
    return shaped(shape, json, "record", name);
  };
  const x = globalThis.exact ??= {};
  x.surfaceRecord = (surface, json) => {
    const was = records.get(surface) ?? null;
    if (was === (json ?? null)) return;
    const put = v => v == null ? records.delete(surface) : records.set(surface, v);
    put(json);
    // A record a reader refuses leaves the last one standing.
    if (!commit(() => { for (const r of Resources) if (r.source === "exactSurface" && readers[r.name]?.[0] === surface) R(r); }, `surface ${surface}`)) put(was);
  };
}
// surface_record.rs `shaped`: absent fields are their type's zero, extra keys
// are ignored, anything else of the wrong kind is refused by its path.
function shaped(t, v, path, name) {
  const fail = () => { throw new Refusal(`${name}: ${path}: expected ${Array.isArray(t) ? (t[0] === "?" ? "Option" : "List") : typeof t === "string" ? KIND[t] : "Record"}`); };
  if (typeof t === "string") {
    if (v === undefined) return t === "n" ? 0 : t === "b" ? false : t === "s" ? "" : null;
    if (t === "u") return v === null ? null : fail();
    return (t === "n" && typeof v === "number") || (t === "b" && typeof v === "boolean") || (t === "s" && typeof v === "string") ? v : fail();
  }
  if (t[0] === "?") return v == null ? null : shaped(t[1], v, path, name);
  if (Array.isArray(t)) {
    if (v === undefined) return [];
    return Array.isArray(v) ? v.map((x, i) => shaped(t[1], x, `${path}[${i}]`, name)) : fail();
  }
  if (v !== undefined && (v === null || typeof v !== "object" || Array.isArray(v))) fail();
  return Object.keys(t).map(k => shaped(t[k], v?.[k], `${path}.${k}`, name));
}
const KIND = { n: "Number", b: "Bool", s: "String", u: "Unit" };
