// `screenshot <png> compare <reference.png>` (LLP 1012, "A screenshot compared with a reference"): the screen against a
// picture of the original app, in text — how much matches, where the content sits higher, lower, left or right of the
// reference and, where the pixels show it, which box grew or which gap widened to put it there, and what else differs,
// named by the nodes under it — and a picture of the differences beside the capture. An agent porting an app reads this
// instead of eyeballing two screenshots. It claims no more than the pixels show: a size it did not measure in the
// reference is not given, a shift that does not leave clearly fewer differences than none is not taken, and content on
// one side only is always a region.
//
// Everything is measured on a grid of one cell a point, each cell the area mean of the pixels it covers (flattened on
// white first), and two cells match when each lies within INK of the other's 3×3 neighbourhood, so anti-aliasing and
// sub-point offsets do not count. Shifts are an alignment: each row of the reference is matched to a row of the screen
// by a Viterbi pass over vertical offsets that charges for every change of offset; then each run of one offset must
// earn its place against no offset, counting the reference's cells that still differ and the screen's cells no
// reference row claims that are unlike the rows around them. Inside each region left differing, the columns are
// aligned the same way.
import { MalformedOperation } from './agent-keys.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { decodePng, encodePng, shrink } from './png.mjs';

const INK = 40; // a channel difference (of 255) beyond the other image's neighbourhood that makes a cell differ
const SOFT = 64; // a Viterbi line cost: each cell's luminance difference, capped here, as a fraction
const REACH = 64; // the largest offset searched, in points
const REGIONS = 6; // regions reported, largest first
const EDGE = 12; // the least channel difference across a box's edge

/** A PNG's pixels over white, opaque: what a viewer shows, before any averaging (averaging unassociated colour with its
 *  alpha would tint a half-transparent edge). */
export function flatten({ width, height, data }) {
  const out = new Uint8Array(data.length);
  for (let i = 0; i < data.length; i += 4) {
    const a = data[i + 3] / 255;
    for (let c = 0; c < 3; c++) out[i + c] = Math.round(data[i + c] * a + 255 * (1 - a));
    out[i + 3] = 255;
  }
  return { width, height, data: out };
}

/** A decoded image on the point grid: `w`×`h` cells, RGB (0–255) and luminance per cell, and each channel's least and
 *  greatest over the cell's 3×3 neighbourhood (`lo`, `hi`), against which the other image's cell is matched. */
export function grid(png, w, h) {
  const flat = flatten(png), img = png.width === w && png.height === h ? flat : shrink(flat, w, h);
  const n = w * h, rgb = new Float32Array(n * 3), lum = new Float32Array(n), lo = new Float32Array(n * 3), hi = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    for (let c = 0; c < 3; c++) rgb[i * 3 + c] = img.data[i * 4 + c];
    lum[i] = 0.2126 * rgb[i * 3] + 0.7152 * rgb[i * 3 + 1] + 0.0722 * rgb[i * 3 + 2];
  }
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) for (let c = 0; c < 3; c++) {
    let l = 255, g = 0;
    for (let yy = Math.max(0, y - 1); yy <= Math.min(h - 1, y + 1); yy++) for (let xx = Math.max(0, x - 1); xx <= Math.min(w - 1, x + 1); xx++) {
      const v = rgb[(yy * w + xx) * 3 + c]; if (v < l) l = v; if (v > g) g = v;
    }
    lo[(y * w + x) * 3 + c] = l; hi[(y * w + x) * 3 + c] = g;
  }
  return { w, h, rgb, lum, lo, hi };
}

/** Whether cell i of A and cell j of B differ: either lies beyond INK of everything around the other in some channel.
 *  Edges a fraction of a point apart match; a point or more apart do not. */
export function differs(A, i, B, j) {
  for (let c = 0; c < 3; c++) {
    const a = A.rgb[i * 3 + c], b = B.rgb[j * 3 + c];
    if (a < B.lo[j * 3 + c] - INK || a > B.hi[j * 3 + c] + INK || b < A.lo[i * 3 + c] - INK || b > A.hi[i * 3 + c] + INK) return true;
  }
  return false;
}

/** Whether screen cell `i` (on a line no reference line claims) is content the reference lacks: no neighbour explains
 *  it. `near` holds [j, k] pairs: a screen cell j on a claimed line beside it and the reference cell k matched there; j
 *  explains i when i is like j and j is like k (a stretch of what matched: a box grown, a gap widened). With no
 *  neighbour, i is judged against reference cell `same` at its own place. */
export function unexplained(A, B, i, near, same) {
  if (!near.length) return differs(B, i, A, same);
  return near.every(([j, k]) => differs(B, i, B, j) || differs(A, k, B, j));
}

/** Runs of one offset in `off` over `[a, b)`: `{from, to, d}`, the zero ones left out unless `all`. */
function runs(off, a, b, all = false) {
  const out = [];
  for (let i = a; i < b;) { let j = i; while (j < b && off[j] === off[i]) j++; if (off[i] || all) out.push({ from: i, to: j - 1, d: off[i] }); i = j; }
  return out;
}

/** One axis aligned: `n` lines of the reference against `m` of the screen, lines before `start` held at no offset.
 *  `cost(i, d)` prices reference line i against screen line i+d for the Viterbi pass (a line past the screen's edge may
 *  keep an offset but is never jumped to), `content(i)` says whether line i has anything on it, `penalty` prices a
 *  change of offset. Then each run is judged by `wrong(i, d)`, the cells of reference line i that differ from screen
 *  line i+d (those with content, past the edge), and `fresh(s, p, q, cp, cq)`, the cells of screen line s that no
 *  reference line claims and that no claimed line around it (p, q, or null; claimed by reference lines cp, cq) explains
 *  (`unexplained`): a run takes a neighbour's
 *  offset when that does about as well, and no offset unless it leaves at most 0.4 of the differences no offset leaves,
 *  and at least `least` fewer. Returns the offset per line and the changes `{at, from, to, lo, hi}`, `lo…hi` the
 *  reference lines the change could equally have been placed between; an offset from the first line is a change at it. */
export function align({ n, m = n, start = 0, reach, cost, content, penalty, wrong, fresh, least }) {
  const K = 2 * reach + 1, C = new Float32Array(n * K), back = new Int16Array(n * K), past = new Uint8Array(n * K);
  for (let i = 0; i < n; i++) for (let k = 0; k < K; k++) {
    const d = k - reach, ahead = Math.min(n - 1, i + reach) + d;
    // Past the edge now (1), or within `reach` lines (2): a jump there would make what follows free.
    if (i + d < 0 || ahead >= m) past[i * K + k] = i + d < 0 || i + d >= m ? 1 : 2;
    C[i * K + k] = i < start ? (d ? Infinity : 0) : past[i * K + k] === 1 ? 0 : cost(i, d) + (content(i) ? Math.abs(d) * 0.01 : 0);
  }
  let acc = new Float64Array(K), next = new Float64Array(K);
  for (let k = 0; k < K; k++) acc[k] = past[k] ? Infinity : C[k];
  for (let i = 1; i < n; i++) {
    let best = 0;
    for (let k = 1; k < K; k++) if (acc[k] < acc[best]) best = k;
    for (let k = 0; k < K; k++) {
      const stay = acc[k], jump = past[i * K + k] ? Infinity : acc[best] + penalty;
      if (stay <= jump) { next[k] = stay + C[i * K + k]; back[i * K + k] = k; } else { next[k] = jump + C[i * K + k]; back[i * K + k] = best; }
    }
    [acc, next] = [next, acc];
  }
  const off = new Int16Array(n);
  let k = reach;
  for (let j = 0; j < K; j++) if (acc[j] < acc[k]) k = j;
  for (let i = n - 1; i >= 0; i--) { off[i] = k - reach; k = back[i * K + k]; }
  // The differences the lines a…b leave under `o`, with every screen line they could reach that no line claims.
  const local = (o, a, b) => {
    const lo = Math.max(0, a - reach), hi = Math.min(m - 1, b + reach), claim = new Int32Array(hi - lo + 1).fill(-1);
    for (let i = Math.max(0, a - 2 * reach); i <= Math.min(n - 1, b + 2 * reach); i++) { const s = i + o[i]; if (s >= lo && s <= hi && claim[s - lo] < 0) claim[s - lo] = i; }
    let total = 0;
    for (let i = a; i <= b; i++) if (i >= start) total += wrong(i, o[i]);
    for (let s = Math.max(lo, start); s <= hi; s++) {
      if (claim[s - lo] >= 0) continue;
      let p = s - 1; while (p >= lo && claim[p - lo] < 0) p--;
      let q = s + 1; while (q <= hi && claim[q - lo] < 0) q++;
      const pp = p >= Math.max(lo, start) ? p : null, qq = q <= hi ? q : null;
      total += fresh(s, pp, qq, pp === null ? null : claim[pp - lo], qq === null ? null : claim[qq - lo]);
    }
    return total;
  };
  const tried = (o, a, b, v) => { const t = Int16Array.from(o); t.fill(v, a, b + 1); return local(t, a, b); };
  for (let round = 0; round < 64; round++) {
    let changed = false;
    const spans = runs(off, start, n, true);
    for (let s = 0; s < spans.length && !changed; s++) {
      const { from: a, to: b, d } = spans[s];
      if (!d) continue;
      const own = local(off, a, b);
      const near = [spans[s - 1]?.d, spans[s + 1]?.d].filter((v) => v !== undefined && v !== 0).map((v) => [v, tried(off, a, b, v)]).sort((p, q) => p[1] - q[1])[0];
      if (near && near[1] <= own * 1.05 + 2) { off.fill(near[0], a, b + 1); changed = true; continue; }
      const none = tried(off, a, b, 0);
      if (!(own <= 0.4 * none && none - own >= least)) { off.fill(0, a, b + 1); changed = true; }
    }
    if (!changed) break;
  }
  const changes = [], tie = 0.5;
  for (let i = Math.max(1, start); i < n; i++) if (off[i] !== off[i - 1]) {
    const from = off[i - 1], to = off[i], kf = from + reach, kt = to + reach;
    let lo = i; while (lo > Math.max(1, start) && off[lo - 1] === from && C[(lo - 1) * K + kt] - C[(lo - 1) * K + kf] <= tie) lo--;
    let hi = i; while (hi < n - 1 && off[hi] === to && C[hi * K + kf] - C[hi * K + kt] <= tie) hi++;
    changes.push({ at: i, from, to, lo, hi });
  }
  if (start === 0 && off[0]) { let hi = 0; while (hi < n - 1 && off[hi + 1] === off[0] && !content(hi)) hi++; changes.unshift({ at: 0, from: 0, to: off[0], lo: 0, hi }); }
  return { off, changes };
}

/** Boxes on one band (overlapping by half the shorter's height) less than `gap` apart, merged. */
export function mergeBoxes(boxes, gap = 24) {
  const out = boxes.map((b) => ({ ...b }));
  for (let again = true; again;) {
    again = false;
    for (let i = 0; i < out.length && !again; i++) for (let j = i + 1; j < out.length && !again; j++) {
      const a = out[i], b = out[j];
      const vo = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y), hgap = Math.max(a.x, b.x) - Math.min(a.x + a.w, b.x + b.w);
      if (vo >= 0.5 * Math.min(a.h, b.h) && hgap <= gap) {
        const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y);
        out[i] = { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y, count: a.count + b.count };
        out.splice(j, 1); again = true;
      }
    }
  }
  return out.sort((a, b) => b.count - a.count);
}

/** The connected regions of a mask (`w`×`h`, 1 = differs): cells with no differing neighbour dropped, the rest dilated
 *  by `gap` and labelled, then merged along their bands; each box is its own cells', with their count. Largest first. */
export function regions(mask, w, h, { gap = 4, least = 6 } = {}) {
  const kept = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    if (!mask[y * w + x]) continue;
    let near = 0;
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
      const yy = y + dy, xx = x + dx;
      if ((dx || dy) && yy >= 0 && yy < h && xx >= 0 && xx < w && mask[yy * w + xx]) near++;
    }
    if (near) kept[y * w + x] = 1;
  }
  // Dilation by a square, separably: rows then columns.
  const rows = new Uint8Array(w * h), grown = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) { let last = -1e9; for (let x = 0; x < w; x++) { if (kept[y * w + x]) last = x; if (x - last <= gap) rows[y * w + x] = 1; } last = 1e9; for (let x = w - 1; x >= 0; x--) { if (kept[y * w + x]) last = x; if (last - x <= gap) rows[y * w + x] = 1; } }
  for (let x = 0; x < w; x++) { let last = -1e9; for (let y = 0; y < h; y++) { if (rows[y * w + x]) last = y; if (y - last <= gap) grown[y * w + x] = 1; } last = 1e9; for (let y = h - 1; y >= 0; y--) { if (rows[y * w + x]) last = y; if (last - y <= gap) grown[y * w + x] = 1; } }
  const label = new Int32Array(w * h), out = [], stack = [];
  for (let s = 0; s < w * h; s++) {
    if (!grown[s] || label[s]) continue;
    const r = { x0: w, y0: h, x1: -1, y1: -1, count: 0 }, id = out.length + 1;
    label[s] = id; stack.push(s);
    while (stack.length) {
      const p = stack.pop(), x = p % w, y = (p - x) / w;
      if (kept[p]) { r.count++; if (x < r.x0) r.x0 = x; if (x > r.x1) r.x1 = x; if (y < r.y0) r.y0 = y; if (y > r.y1) r.y1 = y; }
      for (const q of [x > 0 ? p - 1 : -1, x < w - 1 ? p + 1 : -1, y > 0 ? p - w : -1, y < h - 1 ? p + w : -1]) if (q >= 0 && grown[q] && !label[q]) { label[q] = id; stack.push(q); }
    }
    if (r.count >= least) out.push({ x: r.x0, y: r.y0, w: r.x1 - r.x0 + 1, h: r.y1 - r.y0 + 1, count: r.count });
  }
  return mergeBoxes(out);
}

const hex8 = (G, i) => '#' + [0, 1, 2].map((c) => Math.round(Math.round(G.rgb[i * 3 + c] / 255 * 7) * 15 / 7).toString(16)).join('');
/** A set of cells' fill (the commonest colour of `all`) and ink (the commonest colour of `picked` beyond INK of the
 *  fill), as hex at 8 levels a channel. */
function inkOf(G, all, picked) {
  const tally = (cells) => { const m = new Map(); for (const i of cells) m.set(hex8(G, i), (m.get(hex8(G, i)) ?? 0) + 1); return m; };
  const mode = (m) => [...m.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
  const fill = mode(tally(all)), f = fill && [1, 2, 3].map((k) => parseInt(fill[k], 16) * 17);
  return { fill, ink: mode(tally(picked.filter((i) => !f || [0, 1, 2].some((c) => Math.abs(G.rgb[i * 3 + c] - f[c]) > INK)))) };
}

/** The comparison, on two grids of one size, the top `top` and bottom `bottom` rows (a status bar, a home indicator
 *  the capture lacks) left out. Returns the plain and the aligned match, each screen row's reference row (`claim`, -1 for
 *  none) and the vertical changes, and the regions left differing, in screen points: each with its reference rows, its
 *  columns' offsets and changes, how much of it they explain, and the fill and ink of its differing cells on each side. */
export function compareGrids(A, B, { top = 0, bottom = 0 } = {}) {
  const { w, h } = A, end = h - bottom, reach = Math.min(REACH, Math.floor(h / 4)), considered = Math.max(1, (end - top) * w);
  let raw = 0;
  for (let y = top; y < end; y++) for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, y * w + x)) raw++;
  // A line has content when a cell of it differs from its first by more than half INK.
  const uneven = (G, y) => { const f = G.lum[y * w]; for (let x = 1; x < w; x++) if (Math.abs(G.lum[y * w + x] - f) > INK / 2) return true; return false; };
  const contentA = Uint8Array.from({ length: h }, (_, y) => y >= top && y < end && uneven(A, y));
  const inks = (G, y) => { let k = 0; for (let x = 0; x < w; x++) if (Math.abs(G.lum[y * w + x] - G.lum[y * w]) > INK) k++; return k; };
  const rowCost = (y, d) => {
    const yy = y + d;
    if (yy >= 0 && (yy < top || yy >= end)) return w / 4; // never the status bar or a home indicator
    const b = Math.min(h - 1, Math.max(0, yy)) * w;
    let s = 0;
    for (let x = 0; x < w; x++) s += Math.min(SOFT, Math.abs(A.lum[y * w + x] - B.lum[b + x]));
    return s / SOFT;
  };
  const rowWrong = (y, d) => {
    if (y >= end) return 0;
    const s = y + d;
    if (s < top || s >= end) return inks(A, y); // its content off the screen
    let k = 0;
    for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, s * w + x)) k++;
    return k;
  };
  const rowFresh = (s, p, q, cp, cq) => {
    if (s >= end) return 0;
    let k = 0;
    for (let x = 0; x < w; x++) {
      const near = [];
      if (p !== null) near.push([p * w + x, cp * w + x]);
      if (q !== null) near.push([q * w + x, cq * w + x]);
      if (unexplained(A, B, s * w + x, near, s * w + x)) k++;
    }
    return k;
  };
  const vertical = align({ n: h, m: end, start: top, reach, cost: rowCost, content: (y) => contentA[y], penalty: w / 2, wrong: rowWrong, fresh: rowFresh, least: Math.max(8, w / 8) });
  const dy = vertical.off, claim = new Int32Array(h).fill(-1), mask = new Uint8Array(w * h);
  for (let y = top; y < end; y++) { const s = y + dy[y]; if (s >= top && s < end && claim[s] < 0) claim[s] = y; }
  let pushed = 0;
  for (let y = top; y < end; y++) {
    const s = y + dy[y];
    if (s < top || s >= end) { pushed += w; continue; }
    for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, s * w + x)) mask[s * w + x] = 1;
  }
  // Screen rows no reference row claims: what is unlike the claimed rows either side is content the reference lacks.
  for (let s = top; s < end; s++) {
    if (claim[s] >= 0) continue;
    let p = s - 1; while (p >= top && claim[p] < 0) p--;
    let q = s + 1; while (q < end && claim[q] < 0) q++;
    const pp = p >= top ? p : null, qq = q < end ? q : null;
    for (let x = 0; x < w; x++) {
      const i = s * w + x, near = [];
      if (pp !== null) near.push([pp * w + x, claim[pp] * w + x]);
      if (qq !== null) near.push([qq * w + x, claim[qq] * w + x]);
      if (unexplained(A, B, i, near, i)) mask[i] = 1;
    }
  }
  let aligned = 0;
  for (let i = 0; i < mask.length; i++) aligned += mask[i];
  // A screen row's reference row, where none claims it the nearest one's neighbour.
  const inv = new Int32Array(h);
  for (let s = 0, last = 0; s < h; s++) { if (claim[s] >= 0) last = claim[s] - s; inv[s] = Math.min(h - 1, Math.max(0, s + last)); }
  const changes = vertical.changes.map((c) => ({ ...c, screen: [c.lo + c.from, c.hi + c.to] }));
  const found = regions(mask, w, h), out = [];
  for (const r of found.slice(0, REGIONS)) out.push(columns(A, B, r, claim, mask, reach));
  return { w, h, top, bottom, match: 1 - raw / considered, alignedMatch: 1 - aligned / Math.max(1, considered - pushed), dy, claim, inv, changes, regions: out, more: Math.max(0, found.length - REGIONS) };
}

/** A region's columns aligned over its claimed rows, judged as rows are; what that leaves still differing, and the fill
 *  and ink of those cells on each side. */
function columns(A, B, r, claim, mask, reach) {
  const { w } = A, pairs = [];
  for (let s = r.y; s < r.y + r.h; s++) if (claim[s] >= 0) pairs.push([claim[s], s]);
  let dx = new Int16Array(w), changes = [];
  if (pairs.length) {
    const colCost = (x, d) => {
      const xb = Math.min(w - 1, Math.max(0, x + d));
      let s = 0;
      for (const [y, yy] of pairs) s += Math.min(SOFT, Math.abs(A.lum[y * w + x] - B.lum[yy * w + xb]));
      return s / SOFT;
    };
    const colContent = (x) => pairs.some(([y]) => Math.abs(A.lum[y * w + x] - A.lum[pairs[0][0] * w + x]) > INK / 2);
    const colWrong = (x, d) => {
      let k = 0;
      for (const [y, yy] of pairs) { const c = x + d; if (c < 0 || c >= w ? Math.abs(A.lum[y * w + x] - A.lum[y * w]) > INK : differs(A, y * w + x, B, yy * w + c)) k++; }
      return k;
    };
    const colFresh = (c, p, q, cp, cq) => {
      let k = 0;
      for (const [y, yy] of pairs) {
        const near = [];
        if (p !== null) near.push([yy * w + p, y * w + cp]);
        if (q !== null) near.push([yy * w + q, y * w + cq]);
        if (unexplained(A, B, yy * w + c, near, y * w + c)) k++;
      }
      return k;
    };
    ({ off: dx, changes } = align({ n: w, reach: Math.min(reach, Math.floor(w / 4)), cost: colCost, content: colContent, penalty: Math.max(24, pairs.length * 2), wrong: colWrong, fresh: colFresh, least: Math.max(6, pairs.length) }));
  }
  // What still differs: each differing cell, at the column offsets (a screen column no reference column claims is
  // judged against the columns either side).
  const owner = new Int32Array(w).fill(-1);
  for (let x = 0; x < w; x++) { const c = x + dx[x]; if (c >= 0 && c < w && owner[c] < 0) owner[c] = x; }
  const cells = [], stillCells = [], refCells = [], allScreen = [], allRef = [];
  let left = 0;
  for (let s = r.y; s < r.y + r.h; s++) for (let c = r.x; c < r.x + r.w; c++) {
    const i = s * w + c, y = claim[s];
    allScreen.push(i); if (y >= 0) allRef.push(y * w + c);
    if (!mask[i]) continue;
    cells.push(i);
    let still = true;
    if (y >= 0 && owner[c] >= 0) still = differs(A, y * w + owner[c], B, i);
    else if (y >= 0) {
      let p = c - 1; while (p >= 0 && owner[p] < 0) p--;
      let q = c + 1; while (q < w && owner[q] < 0) q++;
      still = unexplained(A, B, i, [...(p >= 0 ? [[s * w + p, y * w + owner[p]]] : []), ...(q < w ? [[s * w + q, y * w + owner[q]]] : [])], y * w + c);
    }
    // The colours are those of what the columns' offsets leave differing, beside the reference's cell matched there.
    if (still) { left++; stillCells.push(i); if (y >= 0) refCells.push(y * w + (owner[c] >= 0 ? owner[c] : c)); }
  }
  const ys = pairs.map((p) => p[0]);
  return {
    screen: { x: r.x, y: r.y, w: r.w, h: r.h },
    reference: ys.length ? { x: r.x, y: Math.min(...ys), w: r.w, h: Math.max(...ys) - Math.min(...ys) + 1 } : null,
    count: r.count,
    runs: runs(dx, 0, w).filter((u) => u.to >= r.x && u.from < r.x + r.w),
    shifts: changes.map((c) => ({ ...c, screen: [c.lo + c.from, c.hi + c.to] })),
    explained: cells.length ? 1 - left / cells.length : 0,
    colours: { screen: inkOf(B, allScreen, stillCells), reference: allRef.length ? inkOf(A, allRef, refCells) : { fill: null, ink: null } },
  };
}

// ---- naming what is under a box -------------------------------------------------------------------------------

const area = (b) => Math.max(0, b.w) * Math.max(0, b.h);
const overlap = (a, b) => Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)) * Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));
const meet = (a, b) => { const x = Math.max(a.x, b.x), y = Math.max(a.y, b.y); return { x, y, w: Math.min(a.x + a.w, b.x + b.w) - x, h: Math.min(a.y + a.h, b.y + b.h) - y }; };
const holds = (a, b) => a.x <= b.x + 0.5 && a.y <= b.y + 0.5 && a.x + a.w >= b.x + b.w - 0.5 && a.y + a.h >= b.y + b.h - 0.5;
const CLIPS = /^(hidden|clip|scroll|auto)$/;

/** The view nodes in capture points, as far as they show: `layout`'s boxes moved by `origin` (where the viewport sits in
 *  the capture), `tree`'s type, testId, text and depth, the source line, and `vis`, the part of the box that paints: cut
 *  by every ancestor that clips (a scroller, `overflow` other than visible). A node hidden (`visibility: hidden`
 *  unless it says `visible`, `opacity: 0`, `hidden`) or clipped away is left out, and so is one of a covered screen
 *  (`inactive`) whose box lies inside a box of the shown screen that is not its ancestor: a sheet over it. One of a
 *  covered screen that still shows (above a sheet) is kept, `inactive`. */
export function viewNodes(layout, tree, origin = [0, 0]) {
  const by = new Map((tree?.nodes ?? []).map((n) => [n.id, n])), boxes = new Map();
  for (const n of layout?.nodes ?? []) if (Number.isFinite(n.x) && Number.isFinite(n.w)) boxes.set(n.id, n);
  const prop = (t, k) => t?.props?.[k] ?? t?.props?.style?.[k];
  const shown = new Map();
  const state = (id) => {
    if (shown.has(id)) return shown.get(id);
    const t = by.get(id), b = boxes.get(id), up = t?.parent != null && by.has(t.parent) ? state(t.parent) : null;
    const vis = String(prop(t, 'visibility') ?? '');
    const hidden = prop(t, 'hidden') === true || String(prop(t, 'opacity')) === '0' || vis === 'hidden' || vis === 'collapse' || (!!up?.hidden && vis !== 'visible');
    let clip = up?.clip ?? null;
    const own = b ? { x: b.x + origin[0], y: b.y + origin[1], w: b.w, h: b.h } : null;
    if (up?.clips && up.box) clip = clip ? meet(clip, up.box) : up.box;
    const clips = !!b && (b.sx != null || b.sy != null || CLIPS.test(String(prop(t, 'overflow') ?? '')) || CLIPS.test(String(prop(t, 'overflowX') ?? '')) || CLIPS.test(String(prop(t, 'overflowY') ?? '')));
    const r = { hidden, clip, clips, box: own };
    shown.set(id, r);
    return r;
  };
  const ancestors = (id) => { const s = new Set(); for (let p = by.get(id)?.parent; p != null && !s.has(p); p = by.get(p)?.parent) s.add(p); return s; };
  const out = [];
  for (const [id, b] of boxes) {
    if (!(b.w > 0 && b.h > 0)) continue;
    const t = by.get(id), st = state(id);
    if (st.hidden) continue;
    const vis = st.clip ? meet(st.box, st.clip) : st.box;
    if (!(vis.w > 0 && vis.h > 0)) continue;
    const text = t?.props?.text ?? t?.props?.accessibilityLabel ?? t?.props?.label, map = b.sourceMap;
    out.push({ id, type: t?.type ?? b.type, testId: t?.props?.testId ?? b.testId, text: typeof text === 'string' && text.trim() ? text : undefined, depth: t?.depth ?? 0, inactive: !!t?.inactive,
      at: map?.status === 'compatible' && map.file ? `${basename(map.file)}:${map.line}` : undefined, ...st.box, vis });
  }
  const active = out.filter((n) => !n.inactive);
  return out.filter((n) => !n.inactive || !active.some((a) => area(a.vis) >= area(n.vis) && holds(a.vis, n.vis) && !ancestors(n.id).has(a.id)));
}
const r0 = (v) => Math.round(v);
const boxText = (b) => `[${r0(b.x)},${r0(b.y)} ${r0(b.w)}×${r0(b.h)}]`;
/** A node as an agent names it: its testId, else its type and text; then its box and source line. */
export function nodeName(n) {
  const text = n.text ? ` ${JSON.stringify(n.text.length > 28 ? n.text.slice(0, 27) + '…' : n.text)}` : '';
  return `${n.testId != null ? n.testId : n.type ?? 'view'}${n.testId != null && text ? ` (${n.type}${text})` : n.testId == null ? text : ''} ${boxText(n)}${n.at ? ` ${n.at}` : ''}`;
}
const seen = (n) => n.vis ?? n;

/** The nodes under a region: the named one (a testId or text) whose painted box is most nearly the region's, when one
 *  is, then the smallest named ones that it covers most of or that cover most of it, at most `max`, none holding
 *  another chosen; a shown screen's before a covered one's. */
export function under(nodes, box, max = 3) {
  const cands = nodes.filter((n) => { const v = seen(n), o = overlap(v, box); return o > 0 && (o >= 0.5 * area(v) || o >= 0.5 * area(box)) && area(v) <= 6 * area(box) + 400; })
    .sort((a, b) => (a.inactive ? 1 : 0) - (b.inactive ? 1 : 0) || area(seen(a)) - area(seen(b)) || b.depth - a.depth);
  const named = cands.filter((n) => n.testId != null || n.text), out = [];
  const fit = (n) => overlap(seen(n), box) ** 2 / (area(seen(n)) * area(box)), best = named.filter((n) => !n.inactive && fit(n) >= 0.5).sort((a, b) => fit(b) - fit(a))[0];
  if (best) out.push(best);
  for (const n of named.length ? named : cands) {
    if (out.length >= max) break;
    if (out.some((o) => overlap(seen(o), seen(n)) >= 0.9 * Math.min(area(seen(o)), area(seen(n))))) continue;
    out.push(n);
  }
  return out;
}

/** Where a change of offset falls along an axis (`lo…hi` in screen points; `x` true for columns), among the nodes that
 *  cross `band` on the other axis: the boxes either side when it falls in the space between two (a container's gap or
 *  padding), else the smallest box that holds it, no more than four times the change's extent and the move's, and the
 *  others that do, smallest first (`holders`). */
export function spanning(nodes, [lo, hi], x, band, delta = 0) {
  const [p, s, q, t] = x ? ['x', 'w', 'y', 'h'] : ['y', 'h', 'x', 'w'];
  const across = nodes.filter((n) => !band || (n[q] < band[1] && n[q] + n[t] > band[0]));
  const holders = across.filter((n) => n[p] <= lo + 0.5 && n[p] + n[s] >= hi - 0.5).sort((a, b) => a[s] - b[s] || b.depth - a.depth);
  const node = holders[0];
  const inner = node ? across.filter((n) => n !== node && n[p] >= node[p] - 0.5 && n[p] + n[s] <= node[p] + node[s] + 0.5 && n[s] < node[s]) : across;
  const before = inner.filter((n) => n[p] + n[s] <= lo + 1).sort((a, b) => (b[p] + b[s]) - (a[p] + a[s]) || area(a) - area(b))[0];
  const after = inner.filter((n) => n[p] >= hi - 1).sort((a, b) => a[p] - b[p] || area(a) - area(b))[0];
  if (before && after) return { between: [before, after] };
  const fit = holders.filter((n) => n[s] <= 4 * (hi - lo + Math.abs(delta) + 8));
  if (!fit.length) return {};
  const alike = (m) => across.filter((n) => n !== m && n.type === m.type && Math.abs(n.w - m.w) < 0.6 && Math.abs(n.h - m.h) < 0.6 && n.depth === m.depth).length;
  return { node: fit[0], alike: alike(fit[0]), holders: fit.map((n) => ({ node: n, alike: alike(n) })) };
}

// ---- boxes --------------------------------------------------------------------------------------------------------

/** Where a node's visible box (a fill or border unlike what is around it) lies in the reference: each edge of its
 *  screen box whose inside and outside differ, each side mostly one colour, is looked for in the reference within
 *  REACH, as the column (or row) where the same outside and inside meet; `inv` maps a screen row to the reference's.
 *  Returns the reference box, `found.x`/`found.y` for the axes whose two edges were found (the other axis the
 *  screen's), or null when neither was. */
export function boxIn(A, B, n, inv) {
  const { w, h } = B, x0 = Math.round(n.x), y0 = Math.round(n.y), x1 = Math.round(n.x + n.w), y1 = Math.round(n.y + n.h);
  if (n.w < 8 || n.h < 8 || x0 < 3 || y0 < 3 || x1 > w - 3 || y1 > h - 3) return null;
  // A pale card on a pale page differs by less than INK: an edge needs EDGE across it, and a reference cell matches a
  // side when it is nearer that side than half the contrast across the edge.
  const gap = (G, i, H, j) => Math.max(...[0, 1, 2].map((c) => Math.abs(G.rgb[i * 3 + c] - H.rgb[j * 3 + c])));
  const near = (G, i, H, j, within = EDGE / 2) => gap(G, i, H, j) <= within;
  const edge = (samples, at, from, to, expect) => {
    const even = (cells) => cells.some((c) => cells.filter((d) => near(B, c, B, d)).length >= 0.6 * cells.length);
    if (samples.filter(([o, i]) => gap(B, o, B, i) >= EDGE).length < 0.6 * samples.length || !even(samples.map((p) => p[0])) || !even(samples.map((p) => p[1]))) return null;
    const scores = [];
    for (let p = from; p <= to; p++) {
      let k = 0;
      samples.forEach(([o, i], s) => { const r = at(p, s), half = gap(B, o, B, i) / 2; if (r && near(A, r[0], B, o, half) && near(A, r[1], B, i, half)) k++; });
      scores.push(k / samples.length);
    }
    const best = Math.max(...scores);
    if (best < 0.6) return null;
    // Of the runs of positions within 0.05 of the best, the one nearest where the screen has the edge; its middle (the
    // samples straddle the edge by a cell or two, so a run of a few positions is one edge).
    let at0 = null;
    for (let i = 0; i < scores.length;) {
      if (scores[i] < best - 0.05) { i++; continue; }
      let j = i; while (j + 1 < scores.length && scores[j + 1] >= best - 0.05) j++;
      const mid = from + Math.round((i + j) / 2);
      if (at0 === null || Math.abs(mid - expect) < Math.abs(at0 - expect)) at0 = mid;
      i = j + 1;
    }
    return at0;
  };
  const rows = [], cols = [];
  for (let y = Math.round(y0 + n.h * 0.2); y < y1 - n.h * 0.2; y++) rows.push(y);
  for (let x = Math.round(x0 + n.w * 0.2); x < x1 - n.w * 0.2; x++) cols.push(x);
  const cell = (x, y) => (x >= 0 && x < w && y >= 0 && y < h ? y * w + x : null);
  const pair = (a, b) => (a !== null && b !== null ? [a, b] : null);
  const left = edge(rows.map((y) => [y * w + x0 - 2, y * w + x0 + 1]), (p, k) => pair(cell(p - 2, inv[rows[k]]), cell(p + 1, inv[rows[k]])), x0 - REACH, x0 + REACH, x0);
  const right = edge(rows.map((y) => [y * w + x1 + 1, y * w + x1 - 2]), (p, k) => pair(cell(p + 1, inv[rows[k]]), cell(p - 2, inv[rows[k]])), x1 - REACH, x1 + REACH, x1);
  const ty = inv[y0], by = inv[Math.min(h - 1, y1)];
  const topE = edge(cols.map((x) => [(y0 - 2) * w + x, (y0 + 1) * w + x]), (p, k) => pair(cell(cols[k], p - 2), cell(cols[k], p + 1)), ty - REACH, ty + REACH, ty);
  const bottom = edge(cols.map((x) => [(y1 + 1) * w + x, (y1 - 2) * w + x]), (p, k) => pair(cell(cols[k], p + 1), cell(cols[k], p - 2)), by - REACH, by + REACH, by);
  // A pair of edges counts when the box between them is between half and twice the screen's.
  const fx = left !== null && right !== null && right - left >= (x1 - x0) / 2 && right - left <= 2 * (x1 - x0);
  const fy = topE !== null && bottom !== null && bottom - topE >= (y1 - y0) / 2 && bottom - topE <= 2 * (y1 - y0);
  if (!fx && !fy) return null;
  return { x: fx ? left : x0, y: fy ? topE : ty, w: fx ? right - left : x1 - x0, h: fy ? bottom - topE : y1 - y0, found: { x: fx, y: fy } };
}

/** The nodes under the regions and the shifts whose visible boxes differ in size or across from the reference's by
 *  2 pt or more: `{node, ref, alike}`, the largest first, look-alike siblings counted once; `skip` the nodes already
 *  said, and their look-alikes. */
export function boxDiffs(A, B, result, nodes, { max = 4, skip = new Set() } = {}) {
  const { w } = result;
  const areas = [...result.regions.map((r) => r.screen), ...said(result).map((c) => ({ x: 0, y: c.screen[0], w, h: Math.max(1, c.screen[1] - c.screen[0]) }))];
  const keys = new Set(), found = [];
  // A node whose box, grown by 8 pt (a difference often lies just outside a box that moved), covers much of an area.
  const grown = (n) => ({ x: n.x - 8, y: n.y - 8, w: n.w + 16, h: n.h + 16 });
  const known = (n) => [...skip].some((t) => t.id === n.id || (t.type === n.type && r0(t.w) === r0(n.w) && r0(t.h) === r0(n.h)));
  const cands = nodes.filter((n) => n.type !== 'Text' && !n.inactive && !known(n) && areas.some((a) => overlap(n, a) >= 0.3 * area(n) || overlap(grown(n), a) >= 0.3 * area(a))).sort((a, b) => area(b) - area(a) || b.depth - a.depth);
  for (const n of cands.slice(0, 200)) {
    const key = [n.x, n.y, n.w, n.h].map(r0).join(',');
    if (keys.has(key)) continue;
    keys.add(key);
    const ref = boxIn(A, B, n, result.inv);
    if (!ref) continue;
    const dw = ref.found.x ? ref.w - r0(n.w) : 0, dh = ref.found.y ? ref.h - r0(n.h) : 0, dx = ref.found.x ? ref.x - r0(n.x) : 0;
    if (Math.abs(dw) < 2 && Math.abs(dh) < 2 && Math.abs(dx) < 2) continue;
    const twin = found.find((f) => f.node.type === n.type && r0(f.node.w) === r0(n.w) && r0(f.node.h) === r0(n.h) && f.ref.w === ref.w && f.ref.h === ref.h);
    if (twin) twin.alike++; else found.push({ node: n, ref, alike: 0 });
  }
  return found.slice(0, max);
}

// ---- the report -------------------------------------------------------------------------------------------------

/** The vertical changes worth saying: a point's is as likely rounding as layout. */
const said = (result) => result.changes.filter((c) => Math.abs(c.to - c.from) >= 2);
const moved = (d, x, unit) => (d ? `${Math.abs(d)} ${unit} ${x ? (d > 0 ? 'right of' : 'left of') : (d > 0 ? 'lower than' : 'higher than')}` : 'level with');

/** A change of offset explained as far as the pixels show it: the two boxes it falls between, whose space changed by
 *  it; or the box it falls in, with its size in the reference where its edges were found there; else nothing. */
function cause(c, nodes, x, band, unit, A, B, inv, told) {
  const delta = c.to - c.from, at = spanning(nodes, c.screen, x, band, delta);
  if (at.between) return `the space between ${nodeName(at.between[0])} and ${nodeName(at.between[1])} is ${Math.abs(delta)} ${unit} ${delta > 0 ? 'wider' : 'narrower'} than the reference's`;
  if (!at.node) return '';
  // The smallest holder whose edges the reference shows.
  const axis = x ? 'x' : 'y';
  for (const { node: n, alike } of at.holders.slice(0, 6)) {
    const ref = A && B ? boxIn(A, B, n, inv) : null;
    if (!ref?.found[axis]) continue;
    told?.add(n);
    const measured = x ? `${ref.w}×${r0(n.h)}` : `${r0(n.w)}×${ref.h}`;
    return `${nodeName(n)} is ${r0(n.w)}×${r0(n.h)}, ${measured} in the reference${alike ? ` (and ${alike} like it)` : ''}`;
  }
  return `it falls inside ${nodeName(at.node)}, whose size in the reference was not measured`;
}

/** The text `compare` prints: a head line, a line a vertical shift, a few lines a region, a line a box. `A` and `B`
 *  (the grids) let a shift's box be measured in the reference; `boxes` are `boxDiffs`'. */
export function report(result, nodes, { reference, diff, unit = 'pt', boxes = [], A = null, B = null, told = new Set() }) {
  const pct = (v) => `${(Math.floor(v * 1000) / 10).toFixed(1)}%`, out = [], shifts = said(result);
  const left = [result.top ? `the status bar's ${result.top} ${unit}` : '', result.bottom ? `the bottom ${result.bottom} ${unit} (a home indicator the capture lacks)` : ''].filter(Boolean).join(' and ');
  out.push(`compare ${reference}: ${pct(result.match)} matches${result.changes.length ? `, ${pct(result.alignedMatch)} once aligned` : ''} (${result.w}×${result.h} ${unit}${left ? `, ${left} left out` : ''}); diff ${diff}`);
  for (const c of shifts.slice(0, 6)) {
    const why = cause(c, nodes, false, null, unit, A, B, result.inv, told);
    out.push(`  shift  ${c.at === 0 || c.at === result.top ? 'from the top' : `from y≈${r0(c.screen[1])}`} down: ${moved(c.to, false, unit)} the reference${c.from ? ` (above it: ${moved(c.from, false, unit).replace(/ (than|with)$/, '')})` : ''}${why ? `; ${why}` : ''}`);
  }
  if (shifts.length > 6) out.push(`  … and ${shifts.length - 6} more shifts (the content may differ more than it moved)`);
  result.regions.forEach((r, i) => {
    const names = under(nodes, r.screen), band = [r.screen.y, r.screen.y + r.screen.h];
    out.push(`  ${i + 1} ${boxText(r.screen)}${names.length ? ' ' + names.map(nodeName).join(', ') : ''}${r.reference ? '' : ' (no row of the reference is here: content only the screen has)'}`);
    for (const run of r.runs.slice(0, 2)) {
      const near = (c) => c.screen[1] >= r.screen.x - 8 && c.screen[0] <= r.screen.x + r.screen.w + 8;
      const why = [r.shifts.find((s) => s.at === run.from), r.shifts.find((s) => s.at === run.to + 1)].filter((c) => c && near(c)).map((c) => cause(c, nodes, true, band, unit, A, B, result.inv, told)).filter(Boolean).slice(0, 1);
      out.push(`     x ${Math.max(run.from, r.screen.x)}–${Math.min(run.to, r.screen.x + r.screen.w - 1)}: ${moved(run.d, true, unit)} the reference${why.length ? `; ${why[0]}` : ''}`);
    }
    if (r.explained < 0.6) {
      const { screen: sc, reference: rc } = r.colours, colour = (c) => (c.ink ? `${c.ink} on ${c.fill}` : c.fill ?? 'nothing');
      out.push(`     ${r.runs.length ? 'still differs' : 'differs'} (${r.count} ${unit}²)${sc.fill === rc.fill && sc.ink === rc.ink ? '' : `: here ${colour(sc)}, in the reference ${colour(rc)}`}${r.runs.length ? '' : '; cause unclear'}`);
    }
  });
  for (const b of boxes) {
    const { node: n, ref } = b, sized = `${ref.found.x ? ref.w : r0(n.w)}×${ref.found.y ? ref.h : r0(n.h)}`;
    out.push(`  box    ${nodeName(n)}: in the reference ${sized} at ${ref.x},${ref.y}${b.alike ? ` (and ${b.alike} like it)` : ''}`);
  }
  if (result.more) out.push(`  … and ${result.more} smaller region${result.more === 1 ? '' : 's'}, in the diff`);
  if (!shifts.length && !result.regions.length) out.push('  nothing differs');
  return out.join('\n');
}

// ---- the picture --------------------------------------------------------------------------------------------------

/** Three panels a point a pixel: the reference, the screen, and the screen faded with what still differs in red, each
 *  vertical shift as a dashed blue line, and each region boxed in amber with its number in ticks above its corner. */
export function diffImage(A, B, result, mask = residual(A, B, result)) {
  const { w, h } = result, W = w * 3 + 8, data = new Uint8Array(W * h * 4).fill(255);
  const put = (x, y, c) => { if (x < 0 || y < 0 || x >= W || y >= h) return; const i = (y * W + x) * 4; data[i] = c[0]; data[i + 1] = c[1]; data[i + 2] = c[2]; };
  const ox = 2 * w + 8, amber = [240, 170, 0];
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 3, l = 155 + B.lum[y * w + x] * 0.39;
    put(x, y, [A.rgb[i], A.rgb[i + 1], A.rgb[i + 2]]);
    put(w + 4 + x, y, [B.rgb[i], B.rgb[i + 1], B.rgb[i + 2]]);
    put(ox + x, y, mask[y * w + x] ? [230, 30, 30] : [l, l, l]);
  }
  for (let y = 0; y < h; y++) for (let x = 0; x < 4; x++) { put(w + x, y, [128, 128, 128]); put(2 * w + 4 + x, y, [128, 128, 128]); }
  for (const c of said(result)) for (let x = 0; x < w; x++) if (x % 6 < 3) put(ox + x, c.screen[1], [30, 110, 230]);
  result.regions.forEach((r, n) => {
    const b = r.screen;
    for (let x = b.x - 1; x <= b.x + b.w; x++) { put(ox + x, b.y - 1, amber); put(ox + x, b.y + b.h, amber); }
    for (let y = b.y - 1; y <= b.y + b.h; y++) { put(ox + b.x - 1, y, amber); put(ox + b.x + b.w, y, amber); }
    for (let k = 0; k <= n; k++) for (let y = b.y - 6; y < b.y - 2; y++) { put(ox + b.x + k * 3, y, amber); put(ox + b.x + k * 3 + 1, y, amber); }
  });
  return { width: W, height: h, data };
}
/** The screen cells the alignment leaves differing, as `compareGrids` counts them. */
function residual(A, B, { w, h, top, bottom, claim }) {
  const mask = new Uint8Array(w * h);
  for (let s = top; s < h - bottom; s++) {
    const y = claim[s];
    if (y >= 0) { for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, s * w + x)) mask[s * w + x] = 1; continue; }
    let p = s - 1; while (p >= top && claim[p] < 0) p--;
    let q = s + 1; while (q < h - bottom && claim[q] < 0) q++;
    for (let x = 0; x < w; x++) {
      const i = s * w + x, near = [];
      if (p >= top) near.push([p * w + x, claim[p] * w + x]);
      if (q < h - bottom) near.push([q * w + x, claim[q] * w + x]);
      if (unexplained(A, B, i, near, i)) mask[i] = 1;
    }
  }
  return mask;
}

// ---- the operation ------------------------------------------------------------------------------------------------

/** The reference against the screen's size in points: one shape, at a pixel a point or more on each axis. Returns its
 *  scale. */
export function referenceScale(ref, w, h) {
  const sx = ref.width / w, sy = ref.height / h;
  if (Math.abs(sx - sy) / sx > 0.01) throw new MalformedOperation(`compare: the reference is ${ref.width}×${ref.height} px and the screen ${w}×${h} pt, not one shape (another device, or a capture without its status bar?)`);
  if (sx < 0.999 || sy < 0.999) throw new MalformedOperation(`compare: the reference is ${ref.width}×${ref.height} px, under a pixel a point of the ${w}×${h}-pt screen`);
  return sx;
}

/** The CLI's `screenshot <png> compare <reference.png>` (`args` after `screenshot`). On an iOS simulator the capture is
 *  the window with its status bar, as a device screenshot is (`window` and `window status` may be said); elsewhere the
 *  viewport, the only capture whose points the layout's boxes are in. */
export function compareScreenshot(s, args) {
  const at = args.indexOf('compare'), [path, ...form] = args.slice(0, at), reference = args[at + 1];
  const ios = ['ios', 'host-ios'].includes(s.host);
  if (!path || at < 1 || !reference || args.length !== at + 2 || !(form.length === 0 || (ios && form[0] === 'window' && (form.length === 1 || (form.length === 2 && form[1] === 'status')))))
    throw new MalformedOperation(ios ? 'screenshot <png> [window [status]] compare <reference.png>' : `screenshot <png> compare <reference.png> (the viewport: ${s.host} compares no window capture)`);
  return compareOp(s, resolve(reference), () => s.screenshot(path, ios, ios ? 'status' : undefined));
}

/** What a capture's reply says it left out or could not do, a line each. */
function captureNotes(shot) {
  const out = [];
  if (shot.statusBar === false || typeof shot.statusBar === 'number') out.push('the status bar is not in the capture: its rows are left out');
  if (shot.homeIndicator > 0 && !shot.homeIndicatorDrawn) out.push('the home indicator is not in the capture: its rows are left out');
  for (const k of ['note', 'paintedNote', 'chromeNote', 'unshownNote']) if (typeof shot[k] === 'string') out.push(shot[k]);
  if (shot.imagesPending) out.push(`${shot.imagesPending} image${shot.imagesPending === 1 ? ' was' : 's were'} still loading`);
  return out;
}

/** `screenshot <png> … compare <reference.png>`: `take()` captures (the driver's screenshot), then the comparison against
 *  the reference, with the nodes from `layout` and `tree` (on the web read where the picture shows a transition under
 *  way); the diff picture is written beside the capture as `<png>.diff.png`. Replies the capture's reply with
 *  `compare`, `diff` and `text`. */
export async function compareOp(s, referencePath, take) {
  let ref;
  try { ref = decodePng(readFileSync(referencePath)); } catch (e) { throw new MalformedOperation(`compare ${referencePath}: ${e.code === 'ENOENT' ? 'no such file' : e.message}`); }
  const shot = await take();
  const w = Math.round(shot.w), h = Math.round(shot.h);
  referenceScale(ref, w, h);
  const png = decodePng(readFileSync(shot.screenshot));
  if (png.width < w || png.height < h) throw new Error(`compare: the capture is ${png.width}×${png.height} px for ${w}×${h} pt; take it at a pixel a point or more`);
  const layout = await s.layout(undefined, undefined, s.host === 'web' ? { picture: true } : {}), tree = await s.tree();
  const notes = captureNotes(shot);
  // A window capture (iOS) holds the viewport where the window has it: read from a node's own spaces.
  let origin = [0, 0], placed = true;
  if (shot.window) {
    const probe = (layout.nodes ?? []).find((n) => n.w > 0 && n.h > 0);
    const space = probe ? (await s.op({ op: 'layout', id: probe.id }).catch(() => null))?.node?.space : null;
    if (space?.window && space?.viewport) origin = [space.window.x - space.viewport.x, space.window.y - space.viewport.y];
    else { placed = false; notes.push('no node is named: the host gave no node\'s place in the window'); }
  }
  const nodes = placed ? viewNodes(layout, tree, origin) : [];
  const top = shot.window ? Math.ceil(shot.statusBarHeight ?? (typeof shot.statusBar === 'number' ? shot.statusBar : 0)) : 0;
  const bottom = shot.window && shot.homeIndicator > 0 && !shot.homeIndicatorDrawn ? Math.ceil(shot.homeIndicator) : 0;
  const A = grid(ref, w, h), B = grid(png, w, h), result = compareGrids(A, B, { top, bottom });
  const diff = shot.screenshot.replace(/\.png$/i, '') + '.diff.png';
  writeFileSync(diff, encodePng(diffImage(A, B, result)));
  const unit = s.host === 'web' ? 'px' : 'pt', told = new Set();
  let text = report(result, nodes, { reference: referencePath, diff, unit, A, B, told, boxes: [] });
  const boxes = boxDiffs(A, B, result, nodes, { skip: told });
  if (boxes.length) text = report(result, nodes, { reference: referencePath, diff, unit, A, B, boxes });
  if (notes.length) text += '\n' + notes.map((n) => `  capture: ${n}`).join('\n');
  const { dy, claim, inv, ...kept } = result;
  return { ...shot, compare: { ...kept, reference: referencePath }, diff, text };
}
