// Grid values as the browser normalizes them, refused where the kernel's layout can't follow (Taffy).
import { normal, say } from "./rt.js";
function withoutLines(s) { let o = "", bracket = 0; for (let i = 0; i < s.length; i++) { if (s[i] === "\\") { if (!bracket) o += s[i]; if (++i < s.length && !bracket) o += s[i]; } else if (s[i] === "[") bracket++; else if (s[i] === "]" && bracket) bracket--; else if (!bracket) o += s[i]; } return o; }
// Check authored units and function names before Chrome folds calculations.
function trackSource(s) {
  return withoutLines(s.replace(/\/\*[\s\S]*?\*\//g, ' ')).replace(/\\(?:([0-9a-f]{1,6})[ \t\n\r\f]?|([^\n\r\f]))/gi, (_, hex, char) => {
    const n = hex && parseInt(hex, 16);
    return hex ? String.fromCodePoint(n && n <= 0x10ffff ? n : 0xfffd) : char;
  }).toLowerCase();
}
function trackCount(s) { let count = 0; for (let i = 0; i < s.length;) { while (/\s/.test(s[i])) i++; if (i >= s.length) break; if (s[i] === "[") { for (i++; i < s.length && s[i] !== "]"; i += s[i] === "\\" ? 2 : 1); i++; continue; } const start = i; let depth = 0; for (; i < s.length; i++) { if (s[i] === "(") depth++; else if (s[i] === ")") depth--; else if (!depth && /\s/.test(s[i])) break; } const part = s.slice(start, i); if (/^repeat\(/i.test(part)) { const comma = part.indexOf(","), n = part.slice(7, comma).trim(); count += (/^\d+$/.test(n) ? Number(n) : 1) * trackCount(part.slice(comma + 1, -1)); } else count++; } return count; }
export function gridValue(kind, value) {
  if (value == null) return value;
  // A design token's property (LLP 1121 D7): each tuple's value was checked as this row's when it was built.
  // A base name's (LLP 1121 stage 11 B2): one length of the kernel's scale, exactly these nine (base_scale.rs).
  if (/^\s*var\((?:--x-[\w-]+|--exact-(?:space-(?:xs|sm|md|lg|xl)|radius-(?:sm|md|lg|full)))\)\s*$/.test(String(value))) return value;
  const prop = kind === "tracks" ? "grid-template-columns" : kind === "placement" ? "grid-column" : kind === "flow" ? "grid-auto-flow" : "justify-items", v = normal(prop, String(value));
  const refuse = why => { say(`unset ${prop}: ${JSON.stringify(value)} (${why})`); return "unset"; };
  if (!v) return refuse("the browser rejected it");
  const lower = v.toLowerCase(), wide = /^(?:inherit|initial|unset|revert|revert-layer)$/;
  if (wide.test(lower)) return refuse("CSS-wide values have no kernel cascade");
  if (kind === "tracks") {
    const sizes = withoutLines(lower), source = trackSource(String(value));
    if (/^subgrid(?:\s|\[|$)/.test(lower)) return refuse("Taffy has no value for it");
    for (const m of source.matchAll(/([a-z-]+)\s*\(/g)) if (!/^(?:calc|min|max|clamp|minmax|repeat|fit-content)$/.test(m[1])) return refuse("unsupported track function");
    // Consume the whole number even without a unit, so an exponent cannot backtrack into one.
    for (const m of source.matchAll(/(?:^|[^\w.])[+-]?(?:\d*\.)?\d+(?:e[+-]?\d+)?(?:([a-z]+)|%)?/g)) if (m[1] && !/^(?:px|fr)$/.test(m[1])) return refuse("Taffy has no value for its unit");
    if (/\bfit-content\(\s*(?:calc|min|max|clamp)\(/.test(sizes)) return refuse("calculated fit-content() limits are unsupported");
    if (trackCount(lower) > 10000) return refuse("Taffy supports at most 10000 explicit tracks");
  }
  if (kind === "placement" && lower.split(/[ \/]+/).some(x => /^-?\d+$/.test(x) && Math.abs(Number(x)) > 10000)) return refuse("Taffy supports grid indexes and spans through 10000");
  if (kind === "justify" && /^(?:last baseline|legacy(?: (?:left|right|center))?|(?:left|right|center) legacy)$/.test(lower)) return refuse("Taffy has no such alignment mode");
  return v;
}
