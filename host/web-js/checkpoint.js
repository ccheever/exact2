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
