// Grid values as the browser normalizes them, refused where the kernel's layout can't follow (Taffy).
import { normal, say } from "./rt.js";
function withoutLines(s) { let o = "", bracket = 0; for (let i = 0; i < s.length; i++) { if (s[i] === "\\") { if (!bracket) o += s[i]; if (++i < s.length && !bracket) o += s[i]; } else if (s[i] === "[") bracket++; else if (s[i] === "]" && bracket) bracket--; else if (!bracket) o += s[i]; } return o; }
function trackCount(s) { let count = 0; for (let i = 0; i < s.length;) { while (/\s/.test(s[i])) i++; if (i >= s.length) break; if (s[i] === "[") { for (i++; i < s.length && s[i] !== "]"; i += s[i] === "\\" ? 2 : 1); i++; continue; } const start = i; let depth = 0; for (; i < s.length; i++) { if (s[i] === "(") depth++; else if (s[i] === ")") depth--; else if (!depth && /\s/.test(s[i])) break; } const part = s.slice(start, i); if (/^repeat\(/i.test(part)) { const comma = part.indexOf(","), n = part.slice(7, comma).trim(); count += (/^\d+$/.test(n) ? Number(n) : 1) * trackCount(part.slice(comma + 1, -1)); } else count++; } return count; }
export function gridValue(kind, value) {
  if (value == null) return value;
  const prop = kind === "tracks" ? "grid-template-columns" : kind === "placement" ? "grid-column" : kind === "flow" ? "grid-auto-flow" : "justify-items", v = normal(prop, String(value));
  const refuse = why => { say(`unset ${prop}: ${JSON.stringify(value)} (${why})`); return null; };
  if (!v) return refuse("the browser rejected it");
  const lower = v.toLowerCase(), wide = /^(?:inherit|initial|unset|revert|revert-layer)$/;
  if (wide.test(lower)) return refuse("CSS-wide values have no kernel cascade");
  if (kind === "tracks") { const sizes = withoutLines(lower); if (/^subgrid(?:\s|\[|$)/.test(lower) || /\b(?:calc|min|max|clamp|var|env)\s*\(/.test(sizes)) return refuse("Taffy has no value for it"); for (const m of sizes.matchAll(/(?:^|[^\w.-])(?:\d*\.)?\d+([a-z]+)\b/g)) if (!/^(?:px|fr)$/.test(m[1])) return refuse("Taffy has no value for its unit"); if (trackCount(lower) > 10000) return refuse("Taffy supports at most 10000 explicit tracks"); }
  if (kind === "placement" && lower.split(/[ \/]+/).some(x => /^-?\d+$/.test(x) && Math.abs(Number(x)) > 10000)) return refuse("Taffy supports grid indexes and spans through 10000");
  if (kind === "justify" && /^(?:last baseline|legacy(?: (?:left|right|center))?|(?:left|right|center) legacy)$/.test(lower)) return refuse("Taffy has no such alignment mode");
  return v;
}
