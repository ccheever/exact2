// `screenshot <png> compare <reference.png>` (LLP 1012, "A screenshot compared with a reference"): the screen against a
// picture of the original app, in text, saying only what the pixels show. The regions are where the two pictures differ,
// cell by cell, merged into boxes; each is named by the node under its centre, with the colours on each side where they
// differ, and a hint where its content plainly sits elsewhere in the reference. A box whose edges both pictures show
// is given at its measured size in each. A picture of the differences is written beside the capture.
//
// Everything is measured on a grid of one cell a point, each cell the area mean of the pixels it covers (flattened on
// white first). Two cells match when each lies within INK of the other's 3×3 neighbourhood, so anti-aliasing and
// sub-point offsets do not count.
import { MalformedOperation } from './agent-keys.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { decodePng, encodePng, shrink } from './png.mjs';
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
/** A box's fill (the commonest colour of its cells) and ink (the commonest colour of `picked` beyond INK of the fill),
 *  as hex at 8 levels a channel. */
function inkOf(G, box, picked) {
  const tally = (cells) => { const m = new Map(); for (const i of cells) m.set(hex8(G, i), (m.get(hex8(G, i)) ?? 0) + 1); return m; };
  const mode = (m) => [...m.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
  const all = [];
  for (let y = box.y; y < box.y + box.h; y++) for (let x = box.x; x < box.x + box.w; x++) all.push(y * G.w + x);
  const fill = mode(tally(all)), f = fill && [1, 2, 3].map((k) => parseInt(fill[k], 16) * 17);
  return { fill, ink: mode(tally(picked.filter((i) => !f || [0, 1, 2].some((c) => Math.abs(G.rgb[i * 3 + c] - f[c]) > INK)))) };
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
export function report(result, nodes, { reference, diff, unit = 'pt', boxes = [] }) {
  const pct = (v) => `${(Math.floor(v * 1000) / 10).toFixed(1)}%`, out = [];
  const left = [result.top ? `the top ${result.top} (status bar)` : '', result.bottom ? `the bottom ${result.bottom} (home indicator)` : ''].filter(Boolean).join(' and ');
  out.push(`compare ${reference}: ${pct(result.match)} matches (${result.w}×${result.h} ${unit}${left ? `; ${left} not scored` : ''}); diff ${diff}`);
  result.regions.forEach((r, i) => {
    const names = under(nodes, r.screen), { screen: sc, reference: rc } = r.colours, colour = (c) => (c.ink ? `${c.ink} on ${c.fill}` : c.fill ?? 'nothing');
    // Content that moved puts unlike cells side by side: their colours say nothing, so the hint is said instead.
    const said = r.hint ? [hintText(r.hint, unit)] : sc.fill === rc.fill && sc.ink === rc.ink ? [] : [`here ${colour(sc)}, in the reference ${colour(rc)}`];
    out.push(`  ${i + 1} ${boxText(r.screen)} (${r.count} ${unit}²)${names.length ? ` under ${names.map(nodeName).join(', in ')}` : ''}${said.length ? `: ${said.join('; ')}` : ''}`);
  });
  for (const b of boxes) out.push(`  box    ${nodeName({ ...b.node, ...b.here })}: ${b.ref.w}×${b.ref.h} at ${b.ref.x},${b.ref.y} in the reference${b.alike ? ` (and ${b.alike} like it)` : ''}`);
  if (result.more) out.push(`  … and ${result.more} smaller region${result.more === 1 ? '' : 's'}, in the diff`);
  if (!result.regions.length) out.push('  nothing differs');
  return out.join('\n');
}

// ---- the picture --------------------------------------------------------------------------------------------------

/** Three panels a point a pixel: the reference, the screen, and the screen faded with what differs in red and each
 *  region boxed in amber with its number in ticks above its corner. */
export function diffImage(A, B, result) {
  const { w, h, mask } = result, W = w * 3 + 8, data = new Uint8Array(W * h * 4).fill(255);
  const put = (x, y, c) => { if (x < 0 || y < 0 || x >= W || y >= h) return; const i = (y * W + x) * 4; data[i] = c[0]; data[i + 1] = c[1]; data[i + 2] = c[2]; };
  const ox = 2 * w + 8, amber = [240, 170, 0];
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 3, l = 155 + (0.2126 * B.rgb[i] + 0.7152 * B.rgb[i + 1] + 0.0722 * B.rgb[i + 2]) * 0.39;
    put(x, y, [A.rgb[i], A.rgb[i + 1], A.rgb[i + 2]]);
    put(w + 4 + x, y, [B.rgb[i], B.rgb[i + 1], B.rgb[i + 2]]);
    put(ox + x, y, mask[y * w + x] ? [230, 30, 30] : [l, l, l]);
  }
  for (let y = 0; y < h; y++) for (let x = 0; x < 4; x++) { put(w + x, y, [128, 128, 128]); put(2 * w + 4 + x, y, [128, 128, 128]); }
  result.regions.forEach((r, n) => {
    const b = r.screen;
    for (let x = b.x - 1; x <= b.x + b.w; x++) { put(ox + x, b.y - 1, amber); put(ox + x, b.y + b.h, amber); }
    for (let y = b.y - 1; y <= b.y + b.h; y++) { put(ox + b.x - 1, y, amber); put(ox + b.x + b.w, y, amber); }
    for (let k = 0; k <= n; k++) for (let y = b.y - 6; y < b.y - 2; y++) { put(ox + b.x + k * 3, y, amber); put(ox + b.x + k * 3 + 1, y, amber); }
  });
  return { width: W, height: h, data };
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
  for (const k of ['note', 'paintedNote', 'chromeNote', 'unshownNote']) if (typeof shot[k] === 'string') out.push(shot[k]);
  if (shot.notShown?.windows?.length) out.push(`not in the capture, as windows of their own: ${shot.notShown.windows.join(', ')} (the reference may show them)`);
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
  const diff = shot.screenshot.replace(/\.png$/i, '') + '.diff.png';
  writeFileSync(diff, encodePng(diffImage(A, B, result)));
  let text = report(result, nodes, { reference: referencePath, diff, unit: s.host === 'web' ? 'px' : 'pt', boxes: boxDiffs(A, B, result, nodes) });
  if (notes.length) text += '\n' + notes.map((n) => `  capture: ${n}`).join('\n');
  // A floating sheet's content is drawn scaled by the platform, the reference's too: a box measured smaller is that.
  for (const line of scaledLines(layout)) text += `\n  note: ${line}`;
  const { mask, ...kept } = result;
  return { ...shot, compare: { ...kept, reference: referencePath }, diff, text };
}
