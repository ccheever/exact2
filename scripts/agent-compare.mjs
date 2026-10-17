// `screenshot <png> compare <reference.png>` (LLP 1012, "A screenshot compared with a reference"): the screen against a
// picture of the original app, in text — how much matches, where the content sits higher, lower, left or right of the
// reference and which node's box grew or which gap widened to put it there, and what else differs, named by the nodes
// under it — and a picture of the differences beside the capture. An agent porting an app reads this instead of
// eyeballing two screenshots.
//
// Everything is measured on a grid of one cell a point, each cell the area mean of the pixels it covers, and two cells
// match when each lies within INK of the other's 3×3 neighbourhood, so anti-aliasing and sub-point offsets do not
// count. Shifts are an alignment: each row of the reference is matched to a row of the screen by a Viterbi pass over
// vertical offsets that charges for every change of offset, so a block of content that moved is one offset and a box
// that grew is where the offset changes. Inside each region the rows leave differing, the columns are aligned the same
// way. What neither alignment explains is reported with the colours on each side.
import { MalformedOperation } from './agent-keys.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { decodePng, encodePng, shrink } from './png.mjs';

const INK = 40; // a channel difference (of 255) beyond the other image's neighbourhood that makes a cell differ
const SOFT = 64; // an alignment line's cost: each cell's luminance difference, capped here, as a fraction
const REACH = 64; // the largest offset searched, in points
const REGIONS = 6; // regions reported, largest first

/** A decoded image on the point grid: `w`×`h` cells, RGB (0–255) and luminance per cell, and each channel's least and
 *  greatest over the cell's 3×3 neighbourhood (`lo`, `hi`), against which the other image's cell is matched. */
export function grid(png, w, h) {
  const img = png.width === w && png.height === h ? png : shrink(png, w, h);
  const n = w * h, rgb = new Float32Array(n * 3), lum = new Float32Array(n), lo = new Float32Array(n * 3), hi = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const a = img.data[i * 4 + 3] / 255; // a transparent pixel over white, as a viewer shows it
    for (let c = 0; c < 3; c++) rgb[i * 3 + c] = img.data[i * 4 + c] * a + 255 * (1 - a);
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

/** One axis aligned by Viterbi: `n` lines of the reference against `m` of the screen. `cost(i, d)` is the price of
 *  matching reference line i with screen line i+d; a line past the screen's edge is free to keep (content pushed off
 *  the screen) but never jumped to; `content(i)` says whether line i has anything on it, `penalty` is the price of a
 *  change of offset. Then each run of one offset must earn its place: `wrong(a, b, d)`, the cells of lines a…b that
 *  differ at offset d, must be clearly fewer at the run's offset than at a neighbour's, or the run takes the
 *  neighbour's. Returns the offset per line and the changes `{at, from, to, lo, hi}`: `lo…hi` are the reference lines
 *  the change could equally have been placed between (the uniform run around it). */
export function align(n, reach, cost, content, penalty, wrong, m = n) {
  const K = 2 * reach + 1, C = new Float32Array(n * K), back = new Int16Array(n * K), past = new Uint8Array(n * K);
  for (let i = 0; i < n; i++) for (let k = 0; k < K; k++) {
    const d = k - reach, ahead = Math.min(n - 1, i + reach) + d;
    // Past the edge now or within `reach` lines: a jump there would make what follows free.
    if (i + d < 0 || ahead >= m) past[i * K + k] = i + d < 0 || i + d >= m ? 1 : 2;
    // A small pull toward no offset on lines with content breaks ties.
    C[i * K + k] = past[i * K + k] === 1 ? 0 : cost(i, d) + (content(i) ? Math.abs(d) * 0.01 : 0);
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
  for (let round = 0; wrong && round < 32; round++) {
    const spans = [];
    for (let i = 0; i < n;) { let j = i; while (j < n && off[j] === off[i]) j++; spans.push([i, j - 1, off[i]]); i = j; }
    let changed = false;
    for (let s = 0; s < spans.length && !changed; s++) {
      const [a, b, d] = spans[s], own = wrong(a, b, d);
      const alt = [spans[s - 1]?.[2], spans[s + 1]?.[2]].filter((v) => v !== undefined).map((v) => [v, wrong(a, b, v)]).sort((p, q) => p[1] - q[1])[0];
      if (alt && alt[1] <= own * 1.25 + 2) { off.fill(alt[0], a, b + 1); changed = true; }
    }
    if (!changed) break;
  }
  const changes = [], tie = 0.5;
  for (let i = 1; i < n; i++) if (off[i] !== off[i - 1]) {
    const from = off[i - 1], to = off[i], kf = from + reach, kt = to + reach;
    let lo = i; while (lo > 1 && off[lo - 1] === from && C[(lo - 1) * K + kt] - C[(lo - 1) * K + kf] <= tie) lo--;
    let hi = i; while (hi < n - 1 && off[hi] === to && C[hi * K + kf] - C[hi * K + kt] <= tie) hi++;
    changes.push({ at: i, from, to, lo, hi });
  }
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
/** A box's fill (its commonest colour) and the ink among the cells `pick` takes (their commonest colour beyond INK of
 *  the fill), as hex at 8 levels a channel; `row` maps a reference row to the image's. */
function inkOf(G, box, row, pick) {
  const all = new Map(), picked = [];
  for (let y = box.y; y < box.y + box.h; y++) {
    const yy = row(y);
    if (yy < 0 || yy >= G.h) continue;
    for (let x = box.x; x < box.x + box.w; x++) {
      const i = yy * G.w + x, key = hex8(G, i);
      all.set(key, (all.get(key) ?? 0) + 1);
      if (pick(x, y)) picked.push(i);
    }
  }
  const mode = (m) => [...m.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
  const fill = mode(all), f = fill && [1, 2, 3].map((k) => parseInt(fill[k], 16) * 17), inks = new Map();
  for (const i of picked) if (!f || [0, 1, 2].some((c) => Math.abs(G.rgb[i * 3 + c] - f[c]) > INK)) inks.set(hex8(G, i), (inks.get(hex8(G, i)) ?? 0) + 1);
  return { fill, ink: mode(inks) };
}

/** Runs of one offset (`off`) inside `[a, b)`, each `{from, to, d}`, the zero ones left out. */
function runs(off, a, b) {
  const out = [];
  for (let i = a; i < b;) { let j = i; while (j < b && off[j] === off[i]) j++; if (off[i]) out.push({ from: i, to: j - 1, d: off[i] }); i = j; }
  return out;
}

/** The comparison, on two grids of one size, the top `top` rows (a status bar) left out. Returns the plain and the
 *  aligned match, the vertical offset of each reference row (`dy`) and its changes, and the regions the alignment
 *  leaves: each in reference and screen points, with its columns' offsets, their changes, how much of it they explain,
 *  and the colours of its differing cells on each side. */
export function compareGrids(A, B, { top = 0 } = {}) {
  const { w, h } = A, reach = Math.min(REACH, Math.floor(h / 4)), considered = (h - top) * w;
  let raw = 0;
  for (let y = top; y < h; y++) for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, y * w + x)) raw++;
  // A row has content when a cell of it differs from its first by more than half INK.
  const has = (y) => { const f = A.lum[y * w]; for (let x = 1; x < w; x++) if (Math.abs(A.lum[y * w + x] - f) > INK / 2) return true; return false; };
  const contentA = Uint8Array.from({ length: h }, (_, y) => y >= top && has(y));
  const rowCost = (y, d) => {
    if (y < top) return d ? Infinity : 0;
    const yy = y + d;
    if (yy >= 0 && yy < top) return w / 4; // the status bar is never the content's
    const b = Math.min(h - 1, Math.max(0, yy)) * w;
    let s = 0;
    for (let x = 0; x < w; x++) s += Math.min(SOFT, Math.abs(A.lum[y * w + x] - B.lum[b + x]));
    return s / SOFT;
  };
  const wrongRows = (a, b, d) => {
    let k = 0;
    for (let y = Math.max(a, top); y <= b; y++) {
      const yy = y + d;
      if (yy < 0 || yy >= h) { k += w / 4; continue; }
      for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, yy * w + x)) k++;
    }
    return k;
  };
  const vertical = align(h, reach, rowCost, (y) => contentA[y], w / 2, wrongRows);
  const dy = vertical.off, mask = new Uint8Array(w * h);
  let aligned = 0, pushed = 0;
  for (let y = top; y < h; y++) {
    const yy = y + dy[y];
    if (yy < 0 || yy >= h) { pushed += w; continue; } // pushed off the screen: the shift says so
    for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, yy * w + x)) { mask[y * w + x] = 1; aligned++; }
  }
  const changes = vertical.changes.map((c) => ({ ...c, screen: [c.lo + c.from, c.hi + c.to] }));
  const found = regions(mask, w, h), out = [];
  for (const r of found.slice(0, REGIONS)) {
    // The region's columns aligned over its rows (each at its screen row), across the whole width.
    const rows = []; for (let y = r.y; y < r.y + r.h; y++) if (y + dy[y] >= 0 && y + dy[y] < h) rows.push(y);
    const colCost = (x, d) => {
      const xb = Math.min(w - 1, Math.max(0, x + d));
      let s = 0;
      for (const y of rows) s += Math.min(SOFT, Math.abs(A.lum[y * w + x] - B.lum[(y + dy[y]) * w + xb]));
      return s / SOFT;
    };
    const colContent = (x) => rows.some((y) => Math.abs(A.lum[y * w + x] - A.lum[rows[0] * w + x]) > INK / 2);
    const wrongCols = (a, b, d) => {
      let k = 0;
      for (let x = a; x <= b; x++) {
        const xx = x + d;
        if (xx < 0 || xx >= w) { k += rows.length / 4; continue; }
        for (const y of rows) if (differs(A, y * w + x, B, (y + dy[y]) * w + xx)) k++;
      }
      return k;
    };
    const horizontal = align(w, Math.min(REACH, Math.floor(w / 4)), colCost, colContent, Math.max(24, rows.length * 2), wrongCols);
    const dx = horizontal.off;
    let left = 0;
    for (const y of rows) for (let x = r.x; x < r.x + r.w; x++) {
      const xx = x + dx[x];
      if (xx >= 0 && xx < w && mask[y * w + x] && differs(A, y * w + x, B, (y + dy[y]) * w + xx)) left++;
    }
    const y0 = Math.max(0, r.y + dy[r.y]), y1 = Math.min(h - 1, r.y + r.h - 1 + dy[r.y + r.h - 1]);
    const pick = (x, y) => mask[y * w + x] === 1, onScreen = (y) => y + dy[y];
    out.push({
      reference: { x: r.x, y: r.y, w: r.w, h: r.h },
      screen: { x: r.x, y: y0, w: r.w, h: Math.max(1, y1 - y0 + 1) },
      count: r.count,
      runs: runs(dx, 0, w).filter((u) => u.to >= r.x && u.from < r.x + r.w),
      shifts: horizontal.changes.map((c) => ({ ...c, screen: [c.lo + c.from, c.hi + c.to] })),
      explained: r.count ? 1 - left / r.count : 0,
      colours: { screen: inkOf(B, r, onScreen, pick), reference: inkOf(A, r, (y) => y, pick) },
    });
  }
  return { w, h, top, match: 1 - raw / considered, alignedMatch: 1 - aligned / Math.max(1, considered - pushed), dy, changes, regions: out, more: Math.max(0, found.length - REGIONS) };
}

// ---- naming what is under a box -------------------------------------------------------------------------------

/** The view nodes in screen points, with what names them: `layout`'s boxes moved by `origin` (where the viewport sits
 *  in the capture), and `tree`'s type, testId, text and depth. */
export function viewNodes(layout, tree, origin = [0, 0]) {
  const by = new Map((tree?.nodes ?? []).map((n) => [n.id, n]));
  return (layout?.nodes ?? []).filter((n) => Number.isFinite(n.x) && n.w > 0 && n.h > 0).map((n) => {
    const t = by.get(n.id), text = t?.props?.text ?? t?.props?.accessibilityLabel ?? t?.props?.label, map = n.sourceMap;
    return { id: n.id, type: t?.type ?? n.type, testId: t?.props?.testId ?? n.testId, text: typeof text === 'string' && text.trim() ? text : undefined, depth: t?.depth ?? 0, inactive: !!t?.inactive,
      at: map?.status === 'compatible' && map.file ? `${basename(map.file)}:${map.line}` : undefined, x: n.x + origin[0], y: n.y + origin[1], w: n.w, h: n.h };
  }).filter((n) => !n.inactive); // a covered screen's (under a sheet) or an unselected tab's
}
const r0 = (v) => Math.round(v);
const boxText = (b) => `[${r0(b.x)},${r0(b.y)} ${r0(b.w)}×${r0(b.h)}]`;
/** A node as an agent names it: its testId, else its type and text; then its box. */
export function nodeName(n) {
  const text = n.text ? ` ${JSON.stringify(n.text.length > 28 ? n.text.slice(0, 27) + '…' : n.text)}` : '';
  return `${n.testId != null ? n.testId : n.type ?? 'view'}${n.testId != null && text ? ` (${n.type}${text})` : n.testId == null ? text : ''} ${boxText(n)}${n.at ? ` ${n.at}` : ''}`;
}
const area = (b) => b.w * b.h;
const overlap = (a, b) => Math.max(0, Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x)) * Math.max(0, Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y));

/** The nodes under a region: the named one (a testId or text) whose box is most nearly the region's, when one is,
 *  then the smallest named ones that it covers most of or that cover most of it, at most `max`, none holding another
 *  chosen. */
export function under(nodes, box, max = 3) {
  const cands = nodes.filter((n) => { const o = overlap(n, box); return o > 0 && (o >= 0.5 * area(n) || o >= 0.5 * area(box)) && area(n) <= 6 * area(box) + 400; })
    .sort((a, b) => area(a) - area(b) || b.depth - a.depth);
  const named = cands.filter((n) => n.testId != null || n.text), out = [];
  const fit = (n) => overlap(n, box) ** 2 / (area(n) * area(box)), best = named.filter((n) => fit(n) >= 0.5).sort((a, b) => fit(b) - fit(a))[0];
  if (best) out.push(best);
  for (const n of named.length ? named : cands) {
    if (out.length >= max) break;
    if (out.some((o) => overlap(o, n) >= 0.9 * Math.min(area(o), area(n)))) continue;
    out.push(n);
  }
  return out;
}

/** Where a change of offset falls along an axis (`lo…hi` in screen points; `x` true for columns), among the nodes that
 *  cross `band` on the other axis: the smallest box that holds it and could have grown or shrunk by `delta`, with its
 *  look-alike siblings, or the boxes either
 *  side when it falls in the space between them (in a container's padding or gap). */
export function spanning(nodes, [lo, hi], x, band, delta = 0) {
  const [p, s, q, t] = x ? ['x', 'w', 'y', 'h'] : ['y', 'h', 'x', 'w'];
  const across = nodes.filter((n) => !band || (n[q] < band[1] && n[q] + n[t] > band[0]));
  // A box that grew by the change is at most twice the reference's, or shrank to no less than half of it.
  const holds = across.filter((n) => n[p] <= lo + 0.5 && n[p] + n[s] >= hi - 0.5 && delta <= n[s] / 2 && -delta <= n[s]).sort((a, b) => a[s] - b[s] || b.depth - a.depth);
  const node = holds[0];
  const inner = node ? across.filter((n) => n !== node && n[p] >= node[p] - 0.5 && n[p] + n[s] <= node[p] + node[s] + 0.5 && n[s] < node[s]) : across;
  const before = inner.filter((n) => n[p] + n[s] <= lo + 1).sort((a, b) => (b[p] + b[s]) - (a[p] + a[s]) || area(a) - area(b))[0];
  const after = inner.filter((n) => n[p] >= hi - 1).sort((a, b) => a[p] - b[p] || area(a) - area(b))[0];
  // The change falls between two boxes the holder holds: their gap, not the holder, is what differs.
  if (before && after && (!node || (before[s] < node[s] && after[s] < node[s]))) return { between: [before, after], node };
  if (!node) return {};
  const alike = across.filter((n) => n !== node && n.type === node.type && Math.abs(n.w - node.w) < 0.6 && Math.abs(n.h - node.h) < 0.6 && n.depth === node.depth).length;
  return { node, alike };
}

// ---- boxes --------------------------------------------------------------------------------------------------------

/** Where a node's visible box (a fill or border unlike what is around it) lies in the reference: each edge of its
 *  screen box whose inside and outside differ is looked for in the reference within REACH, as the column (or row)
 *  where the same outside and inside meet; `inv` maps a screen row to the reference's. Returns the reference box, its
 *  edges not found taken from the screen's (shifted), or null when fewer than two opposite edges were found. */
export function boxIn(A, B, n, inv) {
  const { w, h } = B, x0 = Math.round(n.x), y0 = Math.round(n.y), x1 = Math.round(n.x + n.w), y1 = Math.round(n.y + n.h);
  if (n.w < 8 || n.h < 8 || x0 < 3 || y0 < 3 || x1 > w - 3 || y1 > h - 3) return null;
  const near = (G, i, H, j) => [0, 1, 2].every((c) => Math.abs(G.rgb[i * 3 + c] - H.rgb[j * 3 + c]) <= INK);
  const apart = (i, j) => !near(B, i, B, j);
  // One edge: `samples` are [outside, inside] screen cells along it; `at(p, k)` the reference cells [outside, inside] for
  // candidate position p and sample k. Returns the best position, or null.
  const edge = (samples, at, from, to, expect) => {
    // An edge: outside and inside differ along most of it, and each side is mostly one colour (a fill, not glyphs).
    const even = (cells) => cells.some((c) => cells.filter((d) => near(B, c, B, d)).length >= 0.6 * cells.length);
    if (samples.filter(([o, i]) => apart(o, i)).length < 0.6 * samples.length || !even(samples.map((p) => p[0])) || !even(samples.map((p) => p[1]))) return null;
    const scores = [];
    for (let p = from; p <= to; p++) {
      let k = 0;
      samples.forEach(([o, i], s) => { const r = at(p, s); if (r && near(A, r[0], B, o) && near(A, r[1], B, i)) k++; });
      scores.push(k / samples.length);
    }
    const top = Math.max(...scores);
    if (top < 0.6) return null;
    // Of the runs of positions scoring within 0.05 of the best, the one nearest where the screen has the edge; its middle
    // (the samples straddle the edge by a cell or two, so a run of a few positions is one edge).
    let best = null;
    for (let i = 0; i < scores.length;) {
      if (scores[i] < top - 0.05) { i++; continue; }
      let j = i; while (j + 1 < scores.length && scores[j + 1] >= top - 0.05) j++;
      const mid = from + Math.round((i + j) / 2);
      if (best === null || Math.abs(mid - expect) < Math.abs(best - expect)) best = mid;
      i = j + 1;
    }
    return best;
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
  // A pair of edges found counts when the box between them is between half and twice the screen's.
  const fx = left !== null && right !== null && right - left >= (x1 - x0) / 2 && right - left <= 2 * (x1 - x0);
  const fy = topE !== null && bottom !== null && bottom - topE >= (y1 - y0) / 2 && bottom - topE <= 2 * (y1 - y0);
  if (!fx && !fy) return null;
  return { x: fx ? left : x0, y: fy ? topE : ty, w: fx ? right - left : x1 - x0, h: fy ? bottom - topE : y1 - y0, found: { x: fx, y: fy } };
}

/** The nodes under the regions and the shifts whose visible boxes differ in size or across from the reference's by
 *  2 pt or more: `{node, ref, alike}`, the largest first, look-alike siblings counted once. */
export function boxDiffs(A, B, result, nodes, max = 4) {
  const { w, h, dy } = result, inv = new Int32Array(h);
  // A screen row's reference row: the nearest reference row the alignment put there.
  inv.fill(-1);
  for (let y = 0; y < h; y++) { const s = y + dy[y]; if (s >= 0 && s < h && inv[s] < 0) inv[s] = y; }
  for (let y = 0, last = 0; y < h; y++) { if (inv[y] >= 0) last = inv[y] - y; else inv[y] = Math.min(h - 1, Math.max(0, y + last)); }
  const areas = [...result.regions.map((r) => r.screen), ...result.changes.filter((c) => Math.abs(c.to - c.from) >= 2).map((c) => ({ x: 0, y: c.screen[0], w, h: Math.max(1, c.screen[1] - c.screen[0]) }))];
  const seen = new Set(), found = [];
  // A node whose box, grown by 8 pt (a difference often lies just outside a box that moved), covers much of an area.
  const grown = (n) => ({ x: n.x - 8, y: n.y - 8, w: n.w + 16, h: n.h + 16 });
  const cands = nodes.filter((n) => n.type !== 'Text' && areas.some((a) => overlap(n, a) >= 0.3 * area(n) || overlap(grown(n), a) >= 0.3 * area(a))).sort((a, b) => area(b) - area(a) || b.depth - a.depth);
  for (const n of cands.slice(0, 80)) {
    const key = [n.x, n.y, n.w, n.h].map(r0).join(',');
    if (seen.has(key)) continue;
    seen.add(key);
    const ref = boxIn(A, B, n, inv);
    if (!ref) continue;
    const dw = ref.found.x ? ref.w - r0(n.w) : 0, dh = ref.found.y ? ref.h - r0(n.h) : 0, dx = ref.found.x ? ref.x - r0(n.x) : 0;
    if (Math.abs(dw) < 2 && Math.abs(dh) < 2 && Math.abs(dx) < 2) continue;
    const twin = found.find((f) => f.node.type === n.type && r0(f.node.w) === r0(n.w) && r0(f.node.h) === r0(n.h) && f.ref.w === ref.w && f.ref.h === ref.h);
    if (twin) twin.alike++; else found.push({ node: n, ref, alike: 0 });
  }
  return found.slice(0, max);
}

// ---- the report -------------------------------------------------------------------------------------------------

const moved = (d, x, unit) => (d ? `${Math.abs(d)} ${unit} ${x ? (d > 0 ? 'right of' : 'left of') : (d > 0 ? 'lower than' : 'higher than')}` : 'level with');
/** A change of offset explained: the box that grew or shrank by it, or the space between two boxes. */
function cause(c, nodes, x, band, unit) {
  const delta = c.to - c.from, at = spanning(nodes, c.screen, x, band, delta);
  if (at.between) return `the space between ${nodeName(at.between[0])} and ${nodeName(at.between[1])} is ${Math.abs(delta)} ${unit} ${delta > 0 ? 'wider' : 'narrower'} than the reference's`;
  if (at.node) {
    const n = at.node, ref = x ? `${r0(n.w - delta)}×${r0(n.h)}` : `${r0(n.w)}×${r0(n.h - delta)}`;
    return `${nodeName(n)} is ${r0(n.w)}×${r0(n.h)}, ${ref} in the reference${at.alike ? ` (and ${at.alike} like it)` : ''}`;
  }
  return '';
}

/** The text `compare` prints: a head line, a line a vertical shift, a few lines a region, a line a box. */
export function report(result, nodes, { reference, diff, unit = 'pt', boxes = [] }) {
  const pct = (v) => `${(Math.floor(v * 1000) / 10).toFixed(1)}%`, out = [];
  // A change of a point is as likely rounding as layout: aligned, but not said.
  const said = result.changes.filter((c) => Math.abs(c.to - c.from) >= 2);
  out.push(`compare ${reference}: ${pct(result.match)} matches${result.changes.length ? `, ${pct(result.alignedMatch)} once aligned` : ''} (${result.w}×${result.h} ${unit}${result.top ? `, the status bar's ${result.top} ${unit} left out` : ''}); diff ${diff}`);
  for (const c of said.slice(0, 6)) {
    const why = cause(c, nodes, false, null, unit);
    out.push(`  shift  from y≈${r0(c.screen[1])} down: ${moved(c.to, false, unit)} the reference${c.from ? ` (above it: ${moved(c.from, false, unit).replace(/ (than|with)$/, '')})` : ''}${why ? `; ${why}` : ''}`);
  }
  if (said.length > 6) out.push(`  … and ${said.length - 6} more shifts (the content may differ more than it moved)`);
  result.regions.forEach((r, i) => {
    const names = under(nodes, r.screen), band = [r.screen.y, r.screen.y + r.screen.h];
    out.push(`  ${i + 1} ${boxText(r.screen)}${names.length ? ' ' + names.map(nodeName).join(', ') : ''}`);
    // A long run that explains little is the alignment reaching for something to match, not a move.
    for (const run of r.runs.filter((u) => Math.abs(u.d) <= 16 || r.explained >= 0.6).slice(0, 2)) {
      const near = (c) => c.screen[1] >= r.screen.x - 8 && c.screen[0] <= r.screen.x + r.screen.w + 8;
      const why = [r.shifts.find((s) => s.at === run.from), r.shifts.find((s) => s.at === run.to + 1)].filter((c) => c && near(c)).map((c) => cause(c, nodes, true, band, unit)).filter(Boolean).slice(0, 1);
      out.push(`     x ${Math.max(run.from, r.screen.x)}–${Math.min(run.to, r.screen.x + r.screen.w - 1)}: ${moved(run.d, true, unit)} the reference${why.length ? `; ${why[0]}` : ''}`);
    }
    if (r.explained < 0.6) {
      const { screen: sc, reference: rc } = r.colours, colour = (c) => (c.ink ? `${c.ink} on ${c.fill}` : c.fill ?? 'nothing');
      out.push(`     ${r.runs.length ? 'still differs' : 'differs'} (${r.count} ${unit}²)${sc.fill === rc.fill && sc.ink === rc.ink ? '' : `: here ${colour(sc)}, in the reference ${colour(rc)}`}`);
    }
  });
  for (const b of boxes) {
    const { node: n, ref } = b, sized = `${ref.found.x ? ref.w : r0(n.w)}×${ref.found.y ? ref.h : r0(n.h)}`;
    out.push(`  box    ${nodeName(n)}: in the reference ${sized} at ${ref.x},${ref.y}${b.alike ? ` (and ${b.alike} like it)` : ''}`);
  }
  if (result.more) out.push(`  … and ${result.more} smaller region${result.more === 1 ? '' : 's'}, in the diff`);
  if (!said.length && !result.regions.length) out.push('  nothing differs');
  return out.join('\n');
}

// ---- the picture --------------------------------------------------------------------------------------------------

/** Three panels a point a pixel: the reference, the screen, and the screen faded with what still differs in red, each
 *  vertical shift as a dashed blue line, and each region boxed in amber with its number in ticks above its corner. */
export function diffImage(A, B, result) {
  const { w, h, dy } = result, W = w * 3 + 8, data = new Uint8Array(W * h * 4).fill(255);
  const put = (x, y, c) => { if (x < 0 || y < 0 || x >= W || y >= h) return; const i = (y * W + x) * 4; data[i] = c[0]; data[i + 1] = c[1]; data[i + 2] = c[2]; };
  const ox = 2 * w + 8, amber = [240, 170, 0];
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 3, l = 155 + B.lum[y * w + x] * 0.39;
    put(x, y, [A.rgb[i], A.rgb[i + 1], A.rgb[i + 2]]);
    put(w + 4 + x, y, [B.rgb[i], B.rgb[i + 1], B.rgb[i + 2]]);
    put(ox + x, y, [l, l, l]);
  }
  for (let y = 0; y < h; y++) for (let x = 0; x < 4; x++) { put(w + x, y, [128, 128, 128]); put(2 * w + 4 + x, y, [128, 128, 128]); }
  for (let y = result.top; y < h; y++) {
    const yy = y + dy[y];
    if (yy >= 0 && yy < h) for (let x = 0; x < w; x++) if (differs(A, y * w + x, B, yy * w + x)) put(ox + x, yy, [230, 30, 30]);
  }
  for (const c of result.changes) for (let x = 0; x < w; x++) if (x % 6 < 3) put(ox + x, c.screen[1], [30, 110, 230]);
  result.regions.forEach((r, n) => {
    const b = r.screen;
    for (let x = b.x - 1; x <= b.x + b.w; x++) { put(ox + x, b.y - 1, amber); put(ox + x, b.y + b.h, amber); }
    for (let y = b.y - 1; y <= b.y + b.h; y++) { put(ox + b.x - 1, y, amber); put(ox + b.x + b.w, y, amber); }
    for (let k = 0; k <= n; k++) for (let y = b.y - 6; y < b.y - 2; y++) { put(ox + b.x + k * 3, y, amber); put(ox + b.x + k * 3 + 1, y, amber); }
  });
  return { width: W, height: h, data };
}

// ---- the operation ------------------------------------------------------------------------------------------------

/** The reference against the screen's size in points: one shape, at a pixel a point or more. Returns its scale. */
export function referenceScale(ref, w, h) {
  const sx = ref.width / w, sy = ref.height / h;
  if (Math.abs(sx - sy) / sx > 0.01) throw new MalformedOperation(`compare: the reference is ${ref.width}×${ref.height} px and the screen ${w}×${h} pt, not one shape (another device, or a capture without its status bar?)`);
  if (sx < 0.999) throw new MalformedOperation(`compare: the reference is ${ref.width}×${ref.height} px, under a pixel a point of the ${w}×${h}-pt screen`);
  return sx;
}

/** The CLI's `screenshot <png> [window [status]] compare <reference.png>` (`args` after `screenshot`). On an iOS
 *  simulator the capture is the window with its status bar, as a device screenshot is, whether or not the words say so. */
export function compareScreenshot(s, args) {
  const at = args.indexOf('compare'), [path, ...form] = args.slice(0, at), reference = args[at + 1];
  const ios = ['ios', 'host-ios'].includes(s.host);
  if (!path || at < 1 || !reference || args.length !== at + 2 || !(form.length === 0 || (form[0] === 'window' && (form.length === 1 || (form.length === 2 && form[1] === 'status' && ios)))))
    throw new MalformedOperation(`screenshot <png> [window${ios ? ' [status]' : ''}] compare <reference.png>`);
  const window = ios || form[0] === 'window';
  return compareOp(s, resolve(reference), () => s.screenshot(path, window, ios ? 'status' : undefined));
}

/** `screenshot <png> [window [status]] compare <reference.png>`: `take()` captures (the driver's screenshot), then the
 *  comparison against the reference, with the nodes from `layout` and `tree`; the diff picture is written beside the
 *  capture as `<png>.diff.png`. Replies the capture's reply with `compare`, `diff` and `text`. */
export async function compareOp(s, referencePath, take) {
  let ref;
  try { ref = decodePng(readFileSync(referencePath)); } catch (e) { throw new MalformedOperation(`compare ${referencePath}: ${e.code === 'ENOENT' ? 'no such file' : e.message}`); }
  const shot = await take();
  const w = Math.round(shot.w), h = Math.round(shot.h);
  referenceScale(ref, w, h);
  const png = decodePng(readFileSync(shot.screenshot));
  if (png.width < w) throw new Error(`compare: the capture is ${png.width} px wide for ${w} pt; take it at a pixel a point or more`);
  const layout = await s.layout(), tree = await s.tree();
  // A window capture holds the viewport where the screen has it (iOS); any other capture is the viewport.
  const origin = shot.window && layout.screen ? [layout.screen.x ?? 0, layout.screen.y ?? 0] : [0, 0];
  const top = shot.window ? Math.ceil(shot.statusBarHeight ?? 0) : 0; // the status bar's own clock and battery
  const A = grid(ref, w, h), B = grid(png, w, h), result = compareGrids(A, B, { top });
  const diff = shot.screenshot.replace(/\.png$/i, '') + '.diff.png';
  writeFileSync(diff, encodePng(diffImage(A, B, result)));
  const nodes = viewNodes(layout, tree, origin), boxes = boxDiffs(A, B, result, nodes);
  const text = report(result, nodes, { reference: referencePath, diff, unit: s.host === 'web' ? 'px' : 'pt', boxes });
  const { dy, ...kept } = result;
  return { ...shot, compare: { ...kept, reference: referencePath }, diff, text };
}
