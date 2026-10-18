// A scrub's frame patched into a design (LLP 1121 D13): the dev loops set
// the rows the design server's resident fold answers (`scrub-fold`'s
// `values`) on the committed design, compiling nothing. The same rule as
// host/web/src/dev.rs `Session::patch`, for a design a loop holds as JSON
// (`{structure, shape, digest, tokens: [{name, value, slices}]}`).
import { createHash } from 'node:crypto';

/** The digest of a design's values, as exact_runner::design::digest makes it. */
export function designDigest(tokens) {
  const hash = createHash('sha256');
  const u32 = (n) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
  const text = (t) => { const b = Buffer.from(t, 'utf8'); hash.update(u32(b.length)); hash.update(b); };
  for (const t of tokens) {
    text(t.name);
    text(t.value);
    for (const s of t.slices) text(s);
    hash.update(u32(t.slices.length));
  }
  return hash.digest('hex');
}

/** `design` with each of `values` (`[{name, css}]`, the CSS text per full
 * app tuple) set on its row, or null when a row would gain or lose its
 * slices (a structure change: the caller compiles the frame instead). A
 * name with no row changes nothing the builds read. */
export function patchDesign(design, values) {
  const tokens = design.tokens.map((t) => ({ name: t.name, value: t.value, slices: t.slices }));
  for (const { name, css } of values) {
    const t = tokens.find((t) => t.name === name);
    if (!t) continue;
    if (!css.length) return null;
    const varies = css.some((c) => c !== css[0]);
    if (varies !== (t.slices.length > 0) || (varies && css.length !== t.slices.length)) return null;
    t.value = css[0];
    if (varies) t.slices = [...css];
  }
  return { ...design, tokens, digest: designDigest(tokens) };
}
