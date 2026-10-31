// Location and pattern matching shared by the JS router and web servers.
const HEX = "0123456789ABCDEF", utf8 = new TextEncoder();
export const enc = (s, esc) => { let o = ""; for (const b of utf8.encode(s)) o += esc(b) ? "%" + HEX[b >> 4] + HEX[b & 15] : String.fromCharCode(b); return o; };
export const dec = (s, plus) => { const out = []; for (let i = 0; i < s.length; i++) { const c = s.charCodeAt(i); if (c === 37 && /^[0-9a-f]{2}$/i.test(s.substr(i + 1, 2))) { out.push(parseInt(s.substr(i + 1, 2), 16)); i += 2; } else if (plus && c === 43) out.push(32); else out.push(...utf8.encode(s[i])); } return new TextDecoder("utf-8", { ignoreBOM: true }).decode(new Uint8Array(out)); };
const clean = s => s.replace(/^[\0- ]+|[\0- ]+$/g, "").replace(/[\t\n\r]/g, "");
/** `canonical` (route/src/location.rs): path and query, dot segments resolved, escaped. */
export function canonical(location) {
  let input = clean(location[0] === "/" ? location : "/" + location).split("#")[0];
  let [path, ...q] = input.split("?"); const query = q.join("?");
  path = path.replace(/\\/g, "/").replace(/^\//, "");
  const segs = [], parts = path.split("/");
  parts.forEach((seg, i) => {
    const d = seg.toLowerCase(), last = i === parts.length - 1;
    if (d === "." || d === "%2e") { if (last) segs.push(""); }
    else if (["..", ".%2e", "%2e.", "%2e%2e"].includes(d)) { segs.pop(); if (last) segs.push(""); }
    else segs.push(enc(seg, b => b < 0x21 || b > 0x7e || '"#<>?^`{}|'.includes(String.fromCharCode(b))));
  });
  return "/" + segs.join("/") + (query ? "?" + enc(query, b => b < 0x21 || b > 0x7e || "\"#<>'".includes(String.fromCharCode(b))) : "");
}
/** `fragment_of` (route/src/location.rs): what follows the first `#`, the fragment percent-encode set applied (LLP 1153 D5). */
export const fragmentOf = location => { const s = clean(location), at = s.indexOf("#"); return at < 0 ? "" : enc(s.slice(at + 1), b => b < 0x21 || b > 0x7e || '"<>`'.includes(String.fromCharCode(b))); };
/** `location_of` (route/src/location.rs): an incoming URL uses the router's path serializer, its fragment kept. */
export function locationOf(href) {
  const input = clean(href), scheme = /^([A-Za-z][A-Za-z0-9+.-]*):([\s\S]*)$/.exec(input), fragment = fragmentOf(input), hash = fragment ? "#" + fragment : "";
  if (!scheme) return canonical(input) + hash;
  if (!/^https?$/i.test(scheme[1])) return canonical('/' + scheme[2].replace(/^\/\//, '')) + hash;
  const after = scheme[2].replace(/^[/\\]+/, ''), end = after.search(/[/\\?#]/);
  // A leading backslash folds to `/` first, as path_query folds before it strips one `/`.
  const rest = end < 0 ? '' : after.slice(end);
  return canonical(rest[0] === '\\' ? '/' + rest.slice(1) : rest) + hash;
}
const segments = p => p === "/" ? [] : p.replace(/^\//, "").split("/");
/** Pattern-only matching (route/src/table.rs): notfound never declares a path.
 * `url` is canonical; escaped slashes stay inside their segment. */
export function matchPattern(table, url, bind = () => {}) {
  const path = url.split("?")[0], parts = segments(path);
  if (path === "/" || !path.endsWith("/")) for (let i = 0; i < table.length; i++) {
    const r = table[i]; if (r.notfound) continue;
    const pat = segments(canonical(r.pattern));
    if (pat.length !== parts.length || !pat.every((s, k) => s[0] === ":" ? parts[k] !== "" : s === parts[k])) continue;
    pat.forEach((s, k) => { if (s[0] === ":") bind(s.slice(1), dec(parts[k], false)); });
    return i;
  }
  return -1;
}
