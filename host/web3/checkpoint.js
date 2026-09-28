// A render's checkpoint (LLP 1048.000 D4), as `exact_web::document::checkpoint`
// writes it: the answers the document used, each a record of (name,
// source, arguments, value) in the plan's canonical value bytes, base64.
const utf8 = new TextEncoder();
function encode(out, v, t) {
  const num = n => { const b = new Uint8Array(8); new DataView(b.buffer).setFloat64(0, n, true); out.push(...b); };
  const u32 = n => { const b = new Uint8Array(4); new DataView(b.buffer).setUint32(0, n, true); out.push(...b); };
  if (t === 'n') { out.push(0); num(v); }
  else if (t === 'b') out.push(1, v ? 1 : 0);
  else if (t === 's') { const b = utf8.encode(v); out.push(2); u32(b.length); out.push(...b); }
  else if (t === 'u') out.push(3);
  else if (Array.isArray(t) && t[0] === '?') { if (v == null) out.push(4); else { out.push(5); encode(out, v, t[1]); } }
  else if (Array.isArray(t)) { out.push(6); u32(v.length); for (const x of v) encode(out, x, t[1]); }
  else { const ks = Object.keys(t); out.push(7); u32(ks.length); ks.forEach((k, i) => encode(out, v[i], t[k])); }
}
export function answers(resources, types, sourceTypes) {
  const out = [6, 0, 0, 0, 0];
  let n = 0;
  resources.forEach((r, i) => {
    if (r.settled === undefined) return;
    const params = sourceTypes[r.source]?.[0] ?? [];
    out.push(7, 4, 0, 0, 0);
    encode(out, r.name, 's'); encode(out, r.source, 's');
    out.push(6); const len = new Uint8Array(4); new DataView(len.buffer).setUint32(0, r.settled.length, true); out.push(...len);
    r.settled.forEach((a, k) => encode(out, a, params[k] ?? 's'));
    encode(out, r.value, types[i]);
    n++;
  });
  new DataView(Uint8Array.from(out).buffer);
  const bytes = Uint8Array.from(out);
  new DataView(bytes.buffer).setUint32(1, n, true);
  let bin = ''; for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}
