// `screenshot <png> compare <reference.png>` (LLP 1012, "A screenshot compared with a reference"): the screen against a
// picture of the original app, in text, saying only what the pixels show. The regions are where the two pictures differ,
// cell by cell, merged into boxes; each is named by the node under its centre, with the colours on each side where they
// differ, and a hint where its content plainly sits elsewhere in the reference. A box whose edges both pictures show
// is given at its measured size in each. A picture of the differences is written beside the capture, one of the
// regions alone, each cut from both pictures at their own resolution, and the two pictures side by side at half size.
//
// Everything is measured on a grid of one cell a point, each cell the area mean of the pixels it covers (flattened on
// white first). Two cells match when each lies within INK of the other's 3×3 neighbourhood, so anti-aliasing and
// sub-point offsets do not count.
import { MalformedOperation } from './agent-keys.mjs';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, dirname, relative, resolve } from 'node:path';
import { crop, decodePng, encodePng, shrink } from './png.mjs';
import { scaledLines } from './agent-inspect.mjs';

const INK = 40; // a channel difference (of 255) beyond the other image's neighbourhood that makes a cell differ
const EDGE = 12; // the least channel difference across a box's edge
const REACH = 64; // the farthest a hint or a box's edge is looked for, in points
const REGIONS = 6; // regions reported, largest first
const BOXES = 4; // measured boxes reported

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

/** A decoded image on the point grid: `w`×`h` cells, RGB (0–255) per cell, and each channel's least and greatest over
 *  the cell's 3×3 neighbourhood (`lo`, `hi`), against which the other image's cell is matched. */
export function grid(png, w, h) {
  const flat = flatten(png), img = png.width === w && png.height === h ? flat : shrink(flat, w, h);
  const n = w * h, rgb = new Float32Array(n * 3), lo = new Float32Array(n * 3), hi = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) for (let c = 0; c < 3; c++) rgb[i * 3 + c] = img.data[i * 4 + c];
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) for (let c = 0; c < 3; c++) {
    let l = 255, g = 0;
    for (let yy = Math.max(0, y - 1); yy <= Math.min(h - 1, y + 1); yy++) for (let xx = Math.max(0, x - 1); xx <= Math.min(w - 1, x + 1); xx++) {
      const v = rgb[(yy * w + xx) * 3 + c]; if (v < l) l = v; if (v > g) g = v;
    }
    lo[(y * w + x) * 3 + c] = l; hi[(y * w + x) * 3 + c] = g;
  }
  return { w, h, rgb, lo, hi };
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
/** Whether colour v (cell `i`) is a mix of the colours of cells `a` and `b`: one fraction t of the way from a to b, in
 *  (0.1, 0.9), in each channel where they differ (within 0.15), and a's own value, within INK/2, in each where they
 *  do not. A colour off that line (a grey border between white and blue) is no mix of them. */
function mixOf(G, a, i, b) {
  let t = null;
  for (let c = 0; c < 3; c++) {
    const from = G.rgb[a * 3 + c], span = G.rgb[b * 3 + c] - from, v = G.rgb[i * 3 + c];
    if (Math.abs(span) <= INK / 2) { if (Math.abs(v - from) > INK / 2) return false; continue; }
    const at = (v - from) / span;
    if (at <= 0.1 || at >= 0.9 || (t !== null && Math.abs(at - t) > 0.15)) return false;
    t ??= at;
  }
  return t !== null;
}

/** Whether cell i is an edge's anti-aliasing: on a run of at most three cells across or down, between two cells whose
 *  colours are present in the region (`present`), every cell of the run, i among them, a mix of theirs (`mixOf`). A
 *  1 pt border whose colour is off the line between its two sides, or beside a cell that is no mix of them (a strip
 *  of another colour), is its own colour; one on it (grey between white and black) reads as a blend, which the
 *  pixels cannot tell apart. */
export function blended(G, i, present) {
  const { w, h } = G, x = i % w, y = (i - x) / w, step = (dx, dy) => dy * w + dx;
  for (const [dx, dy] of [[1, 0], [0, 1]]) for (let d1 = 1; d1 <= 3; d1++) for (let d2 = 1; d1 + d2 - 1 <= 3; d2++) {
    const xa = x - dx * d1, ya = y - dy * d1, xb = x + dx * d2, yb = y + dy * d2;
    if (xa < 0 || ya < 0 || xb >= w || yb >= h) continue;
    const a = ya * w + xa, b = yb * w + xb;
    if (!present(a) || !present(b)) continue;
    let run = true;
    for (let k = a + step(dx, dy); k !== b && run; k += step(dx, dy)) run = mixOf(G, a, k, b);
    if (run) return true;
  }
  return false;
}

/** A box's fill (the commonest colour of its cells) and ink (the commonest colour of `picked` beyond INK of the fill,
 *  an edge's anti-aliasing left out), as hex at 8 levels a channel. When every such cell was anti-aliasing, `edges`
 *  and `blend`, their commonest colour: the picture has no colour of its own there (a box edge a point or two from the
 *  other picture's). A colour is present when 3 or more cells of the box, grown by 3 on each side, have it. */
export function inkOf(G, box, picked) {
  const tally = (cells) => { const m = new Map(); for (const i of cells) m.set(hex8(G, i), (m.get(hex8(G, i)) ?? 0) + 1); return m; };
  const mode = (m) => [...m.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
  const cells = (grow) => {
    const out = [];
    for (let y = Math.max(0, box.y - grow); y < Math.min(G.h, box.y + box.h + grow); y++) for (let x = Math.max(0, box.x - grow); x < Math.min(G.w, box.x + box.w + grow); x++) out.push(y * G.w + x);
    return out;
  };
  const fill = mode(tally(cells(0))), f = fill && [1, 2, 3].map((k) => parseInt(fill[k], 16) * 17), counts = tally(cells(3));
  const present = (k) => (counts.get(hex8(G, k)) ?? 0) >= 3;
  const unlike = picked.filter((i) => !f || [0, 1, 2].some((c) => Math.abs(G.rgb[i * 3 + c] - f[c]) > INK));
  const own = unlike.filter((i) => !blended(G, i, present));
  return { fill, ink: mode(tally(own)), ...(unlike.length && !own.length ? { edges: true, blend: mode(tally(unlike)) } : {}) };
}

/** Where a region's content sits in the reference, when that is plain: the one vertical or horizontal offset (up to
 *  REACH) at which the region's cells and the reference's match both ways — the screen's cells against the reference's
 *  moved, and the reference's against the screen's moved back — leaving at most a tenth of the differences, with no
 *  other offset (beyond a point either side) within three times as few, and at least 2 pt (a point is within the
 *  match's tolerance). Returns `{axis, d}` (the content is d points
 *  lower or right in the reference) or null. */
export function hint(A, B, r) {
  const { w, h } = A, step = Math.max(1, Math.round(Math.sqrt((r.w * r.h) / 20000)));
  const cells = [];
  for (let y = r.y; y < r.y + r.h; y += step) for (let x = r.x; x < r.x + r.w; x += step) cells.push([x, y]);
  const wrong = (dx, dy) => {
    let k = 0;
    for (const [x, y] of cells) {
      const xx = x + dx, yy = y + dy, xb = x - dx, yb = y - dy;
      if (xx < 0 || xx >= w || yy < 0 || yy >= h || differs(B, y * w + x, A, yy * w + xx)) k++;
      if (xb < 0 || xb >= w || yb < 0 || yb >= h || differs(A, y * w + x, B, yb * w + xb)) k++;
    }
    return k;
  };
  const none = wrong(0, 0);
  if (!none) return null;
  let best = null;
  for (const axis of ['y', 'x']) {
    const scores = [];
    for (let d = -REACH; d <= REACH; d++) scores.push(d ? wrong(axis === 'x' ? d : 0, axis === 'y' ? d : 0) : none);
    // The best offsets: within a point of each other (the match tolerates a point), their middle; else no hint.
    const score = Math.min(...scores), lows = scores.flatMap((v, i) => (v === score ? [i] : []));
    if (lows.at(-1) - lows[0] > 2) continue;
    const at = Math.round((lows[0] + lows.at(-1)) / 2);
    const rival = Math.min(...scores.filter((_, i) => i < lows[0] - 1 || i > lows.at(-1) + 1));
    if (Math.abs(at - REACH) >= 2 && score <= 0.1 * none && rival >= 3 * score + 4 && (!best || score < best.score)) best = { axis, d: at - REACH, score };
  }
  return best && { axis: best.axis, d: best.d };
}

/** The comparison on two grids of one size, the top `top` and bottom `bottom` rows (a status bar, a home indicator)
 *  never scored: the match, the differing cells, and the regions they make, the REGIONS largest each with its colours
 *  on each side and its hint. */
export function compareGrids(A, B, { top = 0, bottom = 0 } = {}) {
  const { w, h } = A, end = h - bottom, mask = new Uint8Array(w * h);
  let differing = 0;
  for (let y = top; y < end; y++) for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, y * w + x)) { mask[y * w + x] = 1; differing++; }
  const found = regions(mask, w, h);
  const out = found.slice(0, REGIONS).map((r) => {
    const picked = [];
    for (let y = r.y; y < r.y + r.h; y++) for (let x = r.x; x < r.x + r.w; x++) if (mask[y * w + x]) picked.push(y * w + x);
    return { screen: { x: r.x, y: r.y, w: r.w, h: r.h }, count: r.count, colours: { screen: inkOf(B, r, picked), reference: inkOf(A, r, picked) }, hint: hint(A, B, r) };
  });
  return { w, h, top, bottom, match: 1 - differing / Math.max(1, (end - top) * w), mask, regions: out, more: Math.max(0, found.length - REGIONS) };
}

// ---- nodes --------------------------------------------------------------------------------------------------------

const area = (b) => Math.max(0, b.w) * Math.max(0, b.h);
const overlap = (a, b) => Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)) * Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));
const r0 = (v) => Math.round(v);
const boxText = (b) => `[${r0(b.x)},${r0(b.y)} ${r0(b.w)}×${r0(b.h)}]`;

/** The view nodes in capture points: `layout`'s boxes moved by `origin` (where the viewport sits in the capture), with
 *  `tree`'s type, testId, text, depth and `inactive` (a covered screen's or an unselected tab's), and the source line. */
export function viewNodes(layout, tree, origin = [0, 0]) {
  const by = new Map((tree?.nodes ?? []).map((n) => [n.id, n]));
  return (layout?.nodes ?? []).filter((n) => Number.isFinite(n.x) && n.w > 0 && n.h > 0).map((n) => {
    const t = by.get(n.id), text = t?.props?.text ?? t?.props?.accessibilityLabel ?? t?.props?.label, map = n.sourceMap;
    return { id: n.id, type: t?.type ?? n.type, testId: t?.props?.testId ?? n.testId, text: typeof text === 'string' && text.trim() ? text : undefined, depth: t?.depth ?? 0, inactive: !!t?.inactive,
      at: map?.status === 'compatible' && map.file ? `${basename(map.file)}:${map.line}` : undefined, x: n.x + origin[0], y: n.y + origin[1], w: n.w, h: n.h };
  });
}
/** A node as an agent names it: its testId, else its type and text; then its box and source line. */
export function nodeName(n) {
  const text = n.text ? ` ${JSON.stringify(n.text.length > 28 ? n.text.slice(0, 27) + '…' : n.text)}` : '';
  return `${n.testId != null ? n.testId : n.type ?? 'view'}${n.testId != null && text ? ` (${n.type}${text})` : n.testId == null ? text : ''} ${boxText(n)}${n.at ? ` ${n.at}` : ''}`;
}

/** The nodes under a box's centre: the innermost whose box holds it (the deepest, then the smallest), and the smallest
 *  named one (a testId or text) holding it when that is another; a shown screen's before a covered one's. */
export function under(nodes, box) {
  const cx = box.x + box.w / 2, cy = box.y + box.h / 2;
  const holding = nodes.filter((n) => n.x <= cx && n.x + n.w >= cx && n.y <= cy && n.y + n.h >= cy);
  const shown = holding.filter((n) => !n.inactive), pool = shown.length ? shown : holding;
  const inner = [...pool].sort((a, b) => b.depth - a.depth || area(a) - area(b))[0];
  const named = pool.filter((n) => n.testId != null || n.text).sort((a, b) => area(a) - area(b))[0];
  return [inner, named].filter((n, i, all) => n && all.indexOf(n) === i);
}

// ---- boxes --------------------------------------------------------------------------------------------------------

/** Where a node's visible box (a fill or border unlike what is around it) lies in the reference: each edge of its
 *  screen box whose inside and outside differ by EDGE, each side mostly one colour, is looked for in the reference
 *  within REACH, as the column (or row) where the same outside and inside meet. Returns the reference box when all four
 *  edges were found there, at between half and twice the screen's size, else null. */
export function boxIn(A, B, n) {
  const { w, h } = B, x0 = Math.round(n.x), y0 = Math.round(n.y), x1 = Math.round(n.x + n.w), y1 = Math.round(n.y + n.h);
  if (n.w < 8 || n.h < 8 || x0 < 3 || y0 < 3 || x1 > w - 3 || y1 > h - 3) return null;
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
    let found = null;
    for (let i = 0; i < scores.length;) {
      if (scores[i] < best - 0.05) { i++; continue; }
      let j = i; while (j + 1 < scores.length && scores[j + 1] >= best - 0.05) j++;
      const mid = from + Math.round((i + j) / 2);
      if (found === null || Math.abs(mid - expect) < Math.abs(found - expect)) found = mid;
      i = j + 1;
    }
    return found;
  };
  const rows = [], cols = [];
  for (let y = Math.round(y0 + n.h * 0.2); y < y1 - n.h * 0.2; y++) rows.push(y);
  for (let x = Math.round(x0 + n.w * 0.2); x < x1 - n.w * 0.2; x++) cols.push(x);
  const cell = (x, y) => (x >= 0 && x < w && y >= 0 && y < h ? y * w + x : null);
  const pair = (a, b) => (a !== null && b !== null ? [a, b] : null);
  const left = edge(rows.map((y) => [y * w + x0 - 2, y * w + x0 + 1]), (p, k) => pair(cell(p - 2, rows[k]), cell(p + 1, rows[k])), x0 - REACH, x0 + REACH, x0);
  const right = edge(rows.map((y) => [y * w + x1 + 1, y * w + x1 - 2]), (p, k) => pair(cell(p + 1, rows[k]), cell(p - 2, rows[k])), x1 - REACH, x1 + REACH, x1);
  const topE = edge(cols.map((x) => [(y0 - 2) * w + x, (y0 + 1) * w + x]), (p, k) => pair(cell(cols[k], p - 2), cell(cols[k], p + 1)), y0 - REACH, y0 + REACH, y0);
  const bottom = edge(cols.map((x) => [(y1 + 1) * w + x, (y1 - 2) * w + x]), (p, k) => pair(cell(cols[k], p + 1), cell(cols[k], p - 2)), y1 - REACH, y1 + REACH, y1);
  if ([left, right, topE, bottom].includes(null)) return null;
  const rw = right - left, rh = bottom - topE;
  if (rw < (x1 - x0) / 2 || rw > 2 * (x1 - x0) || rh < (y1 - y0) / 2 || rh > 2 * (y1 - y0)) return null;
  return { x: left, y: topE, w: rw, h: rh };
}

/** The nodes over the regions whose visible boxes both pictures show, measured in each, that differ by 2 pt or more in
 *  size or place: `{node, ref, alike}`, the largest first, look-alike siblings counted once. */
export function boxDiffs(A, B, result, nodes) {
  const keys = new Set(), found = [];
  const cands = nodes.filter((n) => n.type !== 'Text' && !n.inactive && result.regions.some((r) => overlap(n, { x: r.screen.x - 8, y: r.screen.y - 8, w: r.screen.w + 16, h: r.screen.h + 16 }) > 0))
    .sort((a, b) => area(b) - area(a) || b.depth - a.depth);
  for (const n of cands.slice(0, 200)) {
    const key = [n.x, n.y, n.w, n.h].map(r0).join(',');
    if (keys.has(key)) continue;
    keys.add(key);
    const here = boxIn(B, B, n), ref = here && boxIn(A, B, n);
    if (!ref) continue;
    if ([ref.x - here.x, ref.y - here.y, ref.w - here.w, ref.h - here.h].every((d) => Math.abs(d) < 2)) continue;
    const twin = found.find((f) => f.node.type === n.type && f.here.w === here.w && f.here.h === here.h && f.ref.w === ref.w && f.ref.h === ref.h);
    if (twin) twin.alike++; else found.push({ node: n, here, ref, alike: 0 });
  }
  return found.slice(0, BOXES);
}

// ---- the report -------------------------------------------------------------------------------------------------

const hintText = ({ axis, d }, unit) => `its content sits ${Math.abs(d)} ${unit} ${axis === 'y' ? (d > 0 ? 'lower' : 'higher') : (d > 0 ? 'right' : 'left')} in the reference`;

/** The text `compare` prints: a head line, a few lines a region, a line a measured box. */
export function report(result, nodes, { reference, diff, side = null, close = null, unit = 'pt', boxes = [], painted = false }) {
  const pct = (v) => `${(Math.floor(v * 1000) / 10).toFixed(1)}%`, out = [];
  const left = [result.top ? `the top ${result.top} (status bar)` : '', result.bottom ? `the bottom ${result.bottom} (home indicator)` : ''].filter(Boolean).join(' and ');
  const n = result.regions.length, near = close && n ? `; up close, ${close} (${n === 1 ? 'region 1' : `a row a region, 1 to ${n}`})` : '';
  // A menu the agent painted moves the score and reads as the app's bug: said with the score, the line a reader keeps.
  const menu = painted ? ', with the open menu or popover painted by the agent, not UIKit\'s (--touch platform shows UIKit\'s)' : '';
  const both = side ? `; side by side at half size, ${side} (${SIDE_PANELS.join(' | ')}, the regions outlined)` : '';
  out.push(`compare ${reference}: ${pct(result.match)} matches (${result.w}×${result.h} ${unit}${left ? `; ${left} not scored` : ''})${menu}; diff ${diff} (${PANELS.join(' | ')})${both}${near}`);
  result.regions.forEach((r, i) => {
    const names = under(nodes, r.screen), { screen: sc, reference: rc } = r.colours;
    const colour = (c) => (c.ink ? `${c.ink} on ${c.fill}` : c.edges ? `${c.fill}, its edge's anti-aliasing ${c.blend} beside it` : c.fill ?? 'nothing');
    // Content that moved puts unlike cells side by side: their colours say nothing, so the hint is said instead. Both
    // clauses are said when either side's differing cells are only an edge's anti-aliasing.
    const said = r.hint ? [hintText(r.hint, unit)] : sc.fill === rc.fill && sc.ink === rc.ink && !sc.edges && !rc.edges ? [] : [`here ${colour(sc)}, in the reference ${colour(rc)}`];
    out.push(`  ${i + 1} ${boxText(r.screen)} (${r.count} ${unit}²)${names.length ? ` under ${names.map(nodeName).join(', in ')}` : ''}${said.length ? `: ${said.join('; ')}` : ''}`);
  });
  for (const b of boxes) out.push(`  box    ${nodeName({ ...b.node, ...b.here })}: ${b.ref.w}×${b.ref.h} at ${b.ref.x},${b.ref.y} in the reference${b.alike ? ` (and ${b.alike} like it)` : ''}`);
  if (result.more) out.push(`  … and ${result.more} smaller region${result.more === 1 ? '' : 's'}, in the diff`);
  if (!result.regions.length) out.push('  nothing differs');
  return out.join('\n');
}

// ---- the picture --------------------------------------------------------------------------------------------------

/** The diff picture's panels, left to right, as their labels name them. */
export const PANELS = ['reference', 'screen', 'differences'];

// A 5×7 capital for each letter the labels use, a row a string ('#' is ink).
const GLYPHS = {
  C: ['.###.', '#...#', '#....', '#....', '#....', '#...#', '.###.'],
  D: ['####.', '#...#', '#...#', '#...#', '#...#', '#...#', '####.'],
  E: ['#####', '#....', '#....', '####.', '#....', '#....', '#####'],
  F: ['#####', '#....', '#....', '####.', '#....', '#....', '#....'],
  I: ['.###.', '..#..', '..#..', '..#..', '..#..', '..#..', '.###.'],
  N: ['#...#', '##..#', '#.#.#', '#..##', '#...#', '#...#', '#...#'],
  R: ['####.', '#...#', '#...#', '####.', '#.#..', '#..#.', '#...#'],
  S: ['.####', '#....', '#....', '.###.', '....#', '....#', '####.'],
};

const AMBER = [240, 170, 0], GREY = [128, 128, 128];

/** A canvas of panels `w` wide and `h` tall, 4 apart, under a band naming each (`labels`), at twice the glyphs' size
 *  where the widest fits a panel, else at their size: `put(x, y, colour)` in the panels' pixels (the band above y 0). */
function panels(labels, w, h) {
  const W = labels.length * (w + 4) - 4, k = labels.every((l) => l.length * 12 + 8 <= w) ? 2 : 1, T = 7 * k + 8, H = h + T;
  const data = new Uint8Array(W * H * 4).fill(255);
  const put = (x, y, c) => { y += T; if (x < 0 || y < 0 || x >= W || y >= H) return; const i = (y * W + x) * 4; data[i] = c[0]; data[i + 1] = c[1]; data[i + 2] = c[2]; };
  labels.forEach((label, p) => {
    [...label.toUpperCase()].forEach((ch, n) => GLYPHS[ch].forEach((row, gy) => [...row].forEach((c, gx) => {
      if (c !== '#') return;
      for (let dy = 0; dy < k; dy++) for (let dx = 0; dx < k; dx++) put(p * (w + 4) + 4 + n * 6 * k + gx * k + dx, 4 - T + gy * k + dy, [0, 0, 0]);
    })));
    if (p) for (let y = -T; y < h; y++) for (let x = 0; x < 4; x++) put(p * (w + 4) - 4 + x, y, GREY);
  });
  for (let x = 0; x < W; x++) put(x, -1, GREY);
  return { put, image: { width: W, height: H, data } };
}

/** Region `n`'s box (`x0`…`x1`, `y0`…`y1` inclusive, in a panel's pixels from `ox`) outlined in amber just outside it,
 *  with n + 1 ticks above its corner. */
function outline(put, ox, x0, y0, x1, y1, n) {
  for (let x = x0 - 1; x <= x1 + 1; x++) { put(ox + x, y0 - 1, AMBER); put(ox + x, y1 + 1, AMBER); }
  for (let y = y0 - 1; y <= y1 + 1; y++) { put(ox + x0 - 1, y, AMBER); put(ox + x1 + 1, y, AMBER); }
  for (let k = 0; k <= n; k++) for (let y = y0 - 6; y < y0 - 2; y++) { put(ox + x0 + k * 3, y, AMBER); put(ox + x0 + k * 3 + 1, y, AMBER); }
}

/** Three panels a point a pixel under a band naming each (PANELS): the reference, the screen, and the screen faded
 *  with what differs in red and each region boxed in amber with its number in ticks above its corner. */
export function diffImage(A, B, result) {
  const { w, h, mask } = result, { put, image } = panels(PANELS, w, h), ox = 2 * w + 8;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 3, l = 155 + (0.2126 * B.rgb[i] + 0.7152 * B.rgb[i + 1] + 0.0722 * B.rgb[i + 2]) * 0.39;
    put(x, y, [A.rgb[i], A.rgb[i + 1], A.rgb[i + 2]]);
    put(w + 4 + x, y, [B.rgb[i], B.rgb[i + 1], B.rgb[i + 2]]);
    put(ox + x, y, mask[y * w + x] ? [230, 30, 30] : [l, l, l]);
  }
  result.regions.forEach(({ screen: b }, n) => outline(put, ox, b.x, b.y, b.x + b.w - 1, b.y + b.h - 1, n));
  return image;
}

/** The side-by-side picture's panels, left to right. */
export const SIDE_PANELS = ['reference', 'screen'];

/** The reference and the screen at half a pixel a point, side by side under a band naming each (SIDE_PANELS), each
 *  region outlined in amber on both with its number in ticks above its corner: the two pictures in one image a
 *  twelfth the pixels of the diff's, small enough to view at every step (the euxy porters composed it themselves). */
export function sideImage(A, B, result) {
  const { w, h } = result, pw = Math.max(1, Math.round(w / 2)), ph = Math.max(1, Math.round(h / 2));
  const { put, image } = panels(SIDE_PANELS, pw, ph);
  const half = (G) => {
    const full = new Uint8Array(w * h * 4);
    for (let i = 0; i < w * h; i++) full.set([G.rgb[i * 3], G.rgb[i * 3 + 1], G.rgb[i * 3 + 2], 255], i * 4);
    return shrink({ width: w, height: h, data: full }, pw, ph).data;
  };
  [A, B].forEach((G, p) => {
    const px = half(G), ox = p * (pw + 4);
    for (let y = 0; y < ph; y++) for (let x = 0; x < pw; x++) put(ox + x, y, px.subarray((y * pw + x) * 4, (y * pw + x) * 4 + 3));
    // On `shrink`'s own scale, which an odd size makes other than a half.
    const sx = pw / w, sy = ph / h;
    result.regions.forEach(({ screen: b }, n) => outline(put, ox, Math.floor(b.x * sx), Math.floor(b.y * sy), Math.ceil((b.x + b.w) * sx) - 1, Math.ceil((b.y + b.h) * sy) - 1, n));
  });
  return image;
}

const CLOSE_WIDTH = 1600, CLOSE_HEIGHT = 2400; // the largest a close-up sheet is drawn, in pixels
const PAD = 8; // the points around a region a close-up shows

/** The regions up close: a row each, top to bottom, of the reference, the screen and the differences (the screen faded,
 *  what differs in red, the region boxed in amber), each cut from its picture at its own resolution (`ref` and `png`,
 *  decoded; `result` on the `w`×`h`-point grid) and drawn at one scale, at most a picture's own pixels a point. A band
 *  of amber ticks above each row counts its region's number. Null when nothing differs. */
export function closeUp(ref, png, result) {
  const { w, h, mask, regions: found } = result;
  if (!found.length) return null;
  const boxes = found.map(({ screen: b }) => {
    const x = Math.max(0, b.x - PAD), y = Math.max(0, b.y - PAD);
    return { x, y, w: Math.min(w, b.x + b.w + PAD) - x, h: Math.min(h, b.y + b.h + PAD) - y, region: b };
  });
  const GAP = 4, BAND = 10, widest = Math.max(...boxes.map((b) => b.w)), tall = boxes.reduce((sum, b) => sum + b.h, 0);
  // Each axis by its own scale: a reference may be up to 1% another shape (`referenceScale`).
  const k = Math.max(0.25, Math.min(ref.width / w, ref.height / h, png.width / w, png.height / h, (CLOSE_WIDTH - 2 * GAP) / (3 * widest), (CLOSE_HEIGHT - BAND * boxes.length) / tall));
  const W = Math.round(3 * widest * k) + 2 * GAP;
  const rows = boxes.map((b) => ({ ...b, pw: Math.max(1, Math.round(b.w * k)), ph: Math.max(1, Math.round(b.h * k)) }));
  const H = rows.reduce((sum, r) => sum + BAND + r.ph, 0);
  const data = new Uint8Array(W * H * 4).fill(255);
  const put = (x, y, c) => { if (x < 0 || y < 0 || x >= W || y >= H) return; const i = (y * W + x) * 4; data[i] = c[0]; data[i + 1] = c[1]; data[i + 2] = c[2]; data[i + 3] = 255; };
  // A box's pixels from a picture of the `w`×`h`-point screen, at `pw`×`ph`, over white; inside the picture however
  // its scale rounds.
  const cut = (img, b, pw, ph) => {
    const sx = img.width / w, sy = img.height / h;
    const x = Math.min(img.width - 1, Math.round(b.x * sx)), y = Math.min(img.height - 1, Math.round(b.y * sy));
    const cw = Math.max(1, Math.min(img.width - x, Math.round(b.w * sx))), ch = Math.max(1, Math.min(img.height - y, Math.round(b.h * sy)));
    const piece = flatten(crop(img, x, y, cw, ch));
    return pw <= cw && ph <= ch ? shrink(piece, pw, ph) : piece;
  };
  let top = 0;
  rows.forEach((r, n) => {
    for (let t = 0; t <= n; t++) for (let y = 2; y < BAND - 2; y++) { put(2 + t * 4, top + y, AMBER); put(3 + t * 4, top + y, AMBER); }
    top += BAND;
    const left = cut(ref, r, r.pw, r.ph), mid = cut(png, r, r.pw, r.ph), panel = Math.round(widest * k) + GAP;
    for (let y = 0; y < r.ph; y++) for (let x = 0; x < r.pw; x++) {
      const at = (img) => { const xx = Math.min(img.width - 1, Math.floor(x * img.width / r.pw)), yy = Math.min(img.height - 1, Math.floor(y * img.height / r.ph)), i = (yy * img.width + xx) * 4; return [img.data[i], img.data[i + 1], img.data[i + 2]]; };
      const a = at(left), b = at(mid), cx = Math.min(w - 1, Math.floor(r.x + x / k)), cy = Math.min(h - 1, Math.floor(r.y + y / k));
      const l = 155 + (0.2126 * b[0] + 0.7152 * b[1] + 0.0722 * b[2]) * 0.39;
      put(x, top + y, a); put(panel + x, top + y, b);
      put(2 * panel + x, top + y, mask[cy * w + cx] ? [230, 30, 30] : [l, l, l]);
    }
    for (let y = top - BAND; y < top + r.ph; y++) for (let g = 0; g < GAP; g++) { put(panel - GAP + g, y, GREY); put(2 * panel - GAP + g, y, GREY); }
    const q = r.region, bx = 2 * panel + Math.round((q.x - r.x) * k), by = top + Math.round((q.y - r.y) * k), bw = Math.round(q.w * k), bh = Math.round(q.h * k);
    for (let x = bx - 1; x <= bx + bw; x++) { put(x, by - 1, AMBER); put(x, by + bh, AMBER); }
    for (let y = by - 1; y <= by + bh; y++) { put(bx - 1, y, AMBER); put(bx + bw, y, AMBER); }
    top += r.ph;
  });
  return { width: W, height: H, data };
}

// ---- the operation ------------------------------------------------------------------------------------------------

/** The reference against the screen's size in points: one shape (within 1%), and at least a pixel a point on each axis.
 *  Returns its scale. */
export function referenceScale(ref, w, h) {
  const sx = ref.width / w, sy = ref.height / h;
  if (Math.abs(sx - sy) / sx > 0.01) throw new MalformedOperation(`compare: the reference is ${ref.width}×${ref.height} px and the screen ${w}×${h} pt, not one shape (another device, or a capture without its status bar?)`);
  if (ref.width < w || ref.height < h) throw new MalformedOperation(`compare: the reference is ${ref.width}×${ref.height} px, under a pixel a point of the ${w}×${h}-pt screen`);
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
  for (const k of ['note', 'chromeNote', 'unshownNote']) if (typeof shot[k] === 'string') out.push(shot[k]);
  if (shot.notShown?.windows?.length) out.push(`not in the capture, as windows of their own: ${shot.notShown.windows.join(', ')} (the reference may show them)`);
  if (shot.imagesPending) out.push(`${shot.imagesPending} image${shot.imagesPending === 1 ? ' was' : 's were'} still loading`);
  return out;
}

/** `screenshot <png> … compare <reference.png>`: `take()` captures (the driver's screenshot), then the comparison against
 *  the reference, with the nodes from `layout` and `tree` (on the web read where the picture shows a transition under
 *  way); the diff picture is written beside the capture as `<png>.diff.png`, the two pictures at half size as
 *  `<png>.side.png` (`sideImage`), and its regions up close as `<png>.regions.png` (`closeUp`). Replies the capture's
 *  reply with `compare`, `diff`, `side`, `close` and `text`. */
export async function compareOp(s, referencePath, take) {
  let ref;
  try { ref = decodePng(readFileSync(referencePath)); } catch (e) { throw new MalformedOperation(`compare ${referencePath}: ${e.code === 'ENOENT' ? 'no such file' : e.message}`); }
  let shot = await take();
  const w = Math.round(shot.w);
  // A web page's reference as wide as the viewport and taller, a design's capture of the whole page: the viewport
  // takes the reference's height for the capture and the nodes read beside it, and its own back after.
  const tall = s.host === 'web' && !shot.window ? Math.round(ref.height * w / ref.width) : 0, back = Math.round(shot.h);
  if (tall > back + 1) { await s.resize(w, tall); shot = await take(); }
  const h = Math.round(shot.h);
  let layout, tree, png;
  try {
    referenceScale(ref, w, h);
    png = decodePng(readFileSync(shot.screenshot));
    if (png.width < w || png.height < h) throw new Error(`compare: the capture is ${png.width}×${png.height} px for ${w}×${h} pt; take it at a pixel a point or more`);
    layout = await s.layout(undefined, undefined, s.host === 'web' ? { picture: true } : {}); tree = await s.tree();
  } finally {
    if (h !== back) await s.resize(w, back);
  }
  const notes = captureNotes(shot);
  // A window capture (iOS) holds the viewport where the window has it, read from a node's own spaces; a viewport drawn
  // smaller than its points (`--size`) has no such offset.
  let origin = [0, 0], placed = true;
  if (shot.window) {
    const probe = (layout.nodes ?? []).find((n) => n.w > 0 && n.h > 0);
    const space = probe ? (await s.op({ op: 'layout', id: probe.id }).catch(() => null))?.node?.space : null;
    if (space?.window && space?.viewport && (Math.abs(space.window.w - space.viewport.w) > 0.5 || Math.abs(space.window.h - space.viewport.h) > 0.5))
      throw new MalformedOperation('compare: the viewport is drawn at another size than its points (--size): compare at the device\'s own size');
    if (space?.window && space?.viewport) origin = [space.window.x - space.viewport.x, space.window.y - space.viewport.y];
    else { placed = false; notes.push('no node is named: the host gave no node\'s place in the window'); }
  }
  const nodes = placed ? viewNodes(layout, tree, origin) : [];
  // The rows other processes draw on a device (its status bar and home indicator), never scored: the window's safe area.
  const top = shot.window ? Math.ceil(Math.max(shot.safeArea?.top ?? 0, typeof shot.statusBar === 'number' ? shot.statusBar : 0)) : 0;
  const bottom = shot.window ? Math.ceil(shot.safeArea?.bottom ?? 0) : 0;
  if (shot.window && !shot.safeArea) notes.push('the host gave no safe area: only a status bar it named is left out');
  const A = grid(ref, w, h), B = grid(png, w, h), result = compareGrids(A, B, { top, bottom });
  const base = shot.screenshot.replace(/\.png$/i, ''), diff = `${base}.diff.png`, side = `${base}.side.png`, near = closeUp(ref, png, result);
  writeFileSync(diff, encodePng(diffImage(A, B, result)));
  writeFileSync(side, encodePng(sideImage(A, B, result)));
  const close = near ? `${base}.regions.png` : null;
  if (near) writeFileSync(close, encodePng(near));
  let text = report(result, nodes, { reference: referencePath, diff, side, close, unit: s.host === 'web' ? 'px' : 'pt', boxes: boxDiffs(A, B, result, nodes), painted: typeof shot.paintedNote === 'string' });
  if (notes.length) text += '\n' + notes.map((n) => `  capture: ${n}`).join('\n');
  // A floating sheet's content is drawn scaled by the platform, the reference's too: a box measured smaller is that.
  for (const line of scaledLines(layout)) text += `\n  note: ${line}`;
  const { mask, ...kept } = result;
  return { ...shot, compare: { ...kept, reference: referencePath }, diff, side, ...(close ? { close } : {}), text };
}

/** A drive that takes a plain `screenshot` while the app keeps reference pictures (a `ref/`, `reference/`, `design/` or
 *  `screenshots/` folder, or a design's `screenshots/`, in the app, here or beside the app) is told of `compare`, which
 *  says where the screen differs in text, so neither picture is viewed. */
export function referenceNote(ops, appDir, say = console.error) {
  const shots = ops.map((line) => line.trim().split(/\s+/)).filter(([op, png, ...rest]) => op === 'screenshot' && png && !rest.includes('compare') && rest[0] !== 'over' && rest[1] !== 'save');
  if (!shots.length) return;
  const names = ['ref', 'reference', 'references', 'design', 'designs', 'screenshots'];
  const dirs = [appDir, process.cwd(), appDir && dirname(appDir)].filter(Boolean)
    .flatMap((base) => names.flatMap((name) => [resolve(base, name), resolve(base, name, 'screenshots')]));
  // A file of that name, or a folder that cannot be read, gives no note rather than ending the drive.
  const pngsIn = (d) => { try { return readdirSync(d).filter((f) => f.endsWith('.png')); } catch { return []; } };
  const dir = dirs.find((d) => pngsIn(d).length);
  if (!dir) return;
  const shown = relative(process.cwd(), dir) || '.', pngs = pngsIn(dir);
  const [, png] = shots.find(([, png]) => pngs.includes(basename(png))) ?? shots[0], same = pngs.includes(basename(png)) ? basename(png) : '<name>.png';
  say(`note: ${shown}/ holds ${pngs.length} reference picture${pngs.length === 1 ? '' : 's'}: "screenshot ${png} compare ${shown}/${same}" says in text where the screen differs from one, with no picture to view`);
}
