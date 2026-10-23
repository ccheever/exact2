#!/usr/bin/env bun
// What a host draws for a padding and a corner radius, against what it should
// (LLP 1121 stage 11 C): a plan driven on each host by the agent, its boxes
// read with `layout` and its corners from `screenshot`. A padding is where a
// named child sits in its box, on both axes. A radius is fitted to where the
// box's fill begins along its top-left corner's first rows (antialiased
// coverage reaching one half, at the screenshot's own scale, from the box's
// exact edges). A radius over half the box's shorter side is drawn as CSS
// reduces it (a pill), so that is what is expected. A probe that cannot read
// its corner, or a box with no expectation for the host, fails: it is never
// taken for a pass. The box's top-left corner must show its fill over the
// background alone (no border, shadow or neighbour there).
//
// usage: bun scripts/scale-check.mjs <plan.contract> <expect.json> [--host <h> …] [--app <name>] [--out <dir>] [--json]
//   <expect.json>: {"background": "#ffffff", "boxes": [{"testId": "pill", "radius": 9999, "fill": "#0000ff"},
//                   {"testId": "padded", "child": "inner", "padding": 16}]} — a value per box, or per host:
//                   {"padding": {"ios": 16, "macos": 10, "default": 16}}. A box may also expect a `gap`
//                   between two children (`"between": ["a", "b"]`) and a `height`. A value may name a length
//                   (`"radius": "radius.full"`): a base name from kernel/tables/schema.json's scale in the host's
//                   column, or the plan's own design's token (`contract tamber ir`), which wins. `"then": [{"steps":
//                   ["tap flip"], "boxes": […]}]` checks boxes again after steps, each from a fresh launch.
//   --host: web, linux, macos, ios, android (as scripts/agent.mjs names them), or terminal (the terminal
//   host headless, its own `layout` and `plan` verbs: corners read from its reply); default linux. The Android
//   reader runs an app's APK with the plan pushed to it: `--app` names one whose data the plan does not ask
//   (unsaid), built with `bun scripts/agent-android.mjs build <app> --apk`.
//   Each drive starts from a fresh scratch store (`--storage scale-check --reset-storage`).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { decodePng } from './png.mjs';

const ROOT = resolve(import.meta.dir, '..');
/** The corner's rows read, below the box's top edge. */
const ROWS = 6;

/** `#rrggbb` as [r, g, b]. */
export const rgb = hex => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));

/** How much of `fill` over `ground` the pixel is, 0 to 1 (its projection on
 * the line between them); null when they do not differ, or the pixel is no
 * blend of the two (a border, a shadow, a neighbour: then the corner is not
 * read rather than read wrong). */
export function coverage(px, fill, ground) {
  const d = [0, 1, 2].map(c => fill[c] - ground[c]), p = [0, 1, 2].map(c => px[c] - ground[c]);
  const span = d.reduce((s, v) => s + v * v, 0);
  if (span < 32 * 32) return null;
  const t = p.reduce((s, v, c) => s + v * d[c], 0) / span;
  const off = Math.sqrt(p.reduce((s, v, c) => s + (v - t * d[c]) ** 2, 0));
  return off > 24 ? null : Math.min(1, Math.max(0, t));
}

/** Where the fill begins on each of the first ROWS pixel rows wholly inside
 * the box (`box` in device pixels, fractional): `{dy, dx}`, each the distance
 * from the box's top and left edges to the row's centre and to where its
 * coverage reaches one half. null when a row has no such place within the
 * box's left half, or the box is not wholly in the image. */
export function edges(image, box, fill, ground) {
  const top = Math.ceil(box.y), left = Math.floor(box.x);
  if (box.x < 0 || box.y < 0 || box.x + box.w > image.width || box.y + box.h > image.height) return null;
  if (box.h < ROWS + 2 || box.w < 4) return null;
  const at = (x, y) => { const i = (y * image.width + x) * 4; return [image.data[i], image.data[i + 1], image.data[i + 2]]; };
  const start = y => {
    let prev = 0, found = null;
    for (let x = left; x < box.x + box.w / 2; x++) {
      const c = coverage(at(x, y), fill, ground);
      if (c === null) return null;
      if (c >= 0.5) {
        // Between this pixel's centre and the last one's, where it was one half.
        const centre = x + 0.5, before = x - 0.5;
        found = x === left ? centre - (c - 0.5) : before + (0.5 - prev) / Math.max(c - prev, 1e-6);
        break;
      }
      prev = c;
    }
    return found === null ? null : found - box.x;
  };
  // The row through the box's middle is past any corner (a radius is at most
  // half the box): where its fill begins is the straight edge, which the
  // corner's rows are read from.
  const edge = start(Math.floor(box.y + box.h / 2));
  // Not at the box's own left edge: something covers it there.
  if (edge === null || Math.abs(edge) > 1) return null;
  const out = [];
  for (let row = 0; row < ROWS; row++) {
    const y = top + row, dx = start(y);
    if (dx === null) return null;
    out.push({ dy: y + 0.5 - box.y, dx: Math.max(0, dx - edge) });
  }
  return out;
}

/** The radius (in the same pixels) whose circle, tangent to the box's top and
 * left edges, crosses each row's centre where `starts` says; at most `limit`.
 * Of radii that fit as well, to a few thousandths of a pixel, the least: one
 * under the first row's depth draws no row differently from a square corner. */
export function fitRadius(starts, limit) {
  const inset = (r, dy) => { const d = r - dy; return d <= 0 ? 0 : r - Math.sqrt(Math.max(0, r * r - d * d)); };
  let best = 0, err = Infinity;
  for (let r = 0; r <= limit; r += 0.05) {
    const e = starts.reduce((s, { dx, dy }) => s + (inset(r, dy) - dx) ** 2, 0);
    if (e < err - 1e-5) { err = e; best = r; }
  }
  return best;
}

/** A box's expected radius once CSS reduces it to the box. */
export const reduced = (radius, w, h) => Math.min(radius, w / 2, h / 2);
// A length in whole cells as the terminal paints it (LLP 1121 stage 11 (c)):
// to the nearest cell, a tie toward zero.
export const cellsOf = (px, cell) => (Math.sign(px) * Math.max(0, Math.ceil(Math.abs(px / cell) - 0.5))) || 0;

/** A host's column in the base scale's table (LLP 1121 stage 11 B): every
 * web build and the Linux desktop draw the neutral look; `terminal` is the
 * terminal host, driven here (`exact-terminal`), not through the agent. */
const COLUMN = { web: 'neutral', linux: 'neutral', ios: 'ios', macos: 'mac', android: 'material', terminal: 'terminal' };
/** A column's cell in a scale row, and in a metrics row (the Material and
 * terminal cells, where a row has none, are the web's first look's). */
const SCALE_CELL = { ios: 1, mac: 2, neutral: 3, material: 4, terminal: 5 };
const METRIC_CELL = { ios: 2, mac: 3, neutral: 5, material: 6, terminal: 7 };
/** radius.full's one oversized length (`style::scale::RADIUS_FULL`). */
export const RADIUS_FULL = 9999;

/** Length `name`'s value on `host`, in points: the design's own token
 * (`overrides`), else a base name's in the kernel's table, in the host's
 * column (a metric's reference read in that column, a stretch's too; radius.full
 * [`RADIUS_FULL`]; the terminal's cells by `grid`'s width); undefined for a
 * name neither has, a host with no column, or cells with no grid. */
export function baseLength(schema, overrides, name, host, grid = null) {
  if (Object.hasOwn(overrides, name)) return overrides[name];
  const row = schema.scale.find(r => r[0] === name), column = COLUMN[host];
  if (!row || !column) return undefined;
  let cell = row[SCALE_CELL[column]];
  // A stretch reads its metric as `=metric` does (stage 13 D5).
  if (typeof cell?.stretch === 'string') cell = cell.stretch;
  if (typeof cell === 'string' && cell.startsWith('=')) {
    const metric = schema.metrics.find(r => r[0] === cell.slice(1));
    if (!metric) return undefined;
    // A row of three cells is the neutral look's own value.
    cell = metric.length === 3 ? metric[2] : metric[METRIC_CELL[column]] ?? metric[4];
  }
  if (cell === 'full') return RADIUS_FULL;
  if (typeof cell === 'number') return cell;
  // The terminal's spacings are cells: a `ch`, the cell's width (`grid`, the
  // terminal's own reply).
  if (typeof cell?.cells === 'number') return grid?.w ? cell.cells * grid.w : undefined;
  return typeof cell?.px === 'number' ? cell.px : undefined;
}

/** The design's own lengths, from the compiler's reading of it (`contract
 * tamber ir`, which finds the design as a build does): each length token whose
 * value is one px length in every tuple. One whose value varies, or is not px,
 * is refused when it is a base name (one length is what a box is held to). */
export function overridesOf(ir, names) {
  const out = {};
  for (const t of ir.tokens ?? []) {
    if (t.kind !== 'length') continue;
    const values = [...new Set(t.values ?? [])];
    const px = values.length === 1 ? /^(-?\d+(?:\.\d+)?)px$/.exec(values[0]) : null;
    if (px) out[t.name] = Number(px[1]);
    else if (names.includes(t.name)) throw new Error(`the design's ${t.name} is not one length in px (${values.join(', ')})`);
  }
  return out;
}

/** What `want` says for `host`: its own, else `default`; `missing` when it
 * names hosts and not this one. */
const pick = (v, host) => (v && typeof v === 'object' ? (host in v ? v[host] : 'default' in v ? v.default : 'missing') : v);

/** Where the page's top is in a screenshot taller than the page (the whole
 * screen, its bars too: the Android reader's), in its pixels: found down the
 * middle of the topmost box expected to show a fill (of `boxes`, every
 * phase's first frame's), where a run of fill as tall as the box first
 * begins (no fill box is above it), its edge to the fraction of a pixel, less
 * that box's top. 0 when the screenshot is the page. A page offset across
 * (side bars, a cutout) is refused: its pixels a point are not the
 * screenshot's width over the page's. */
export function pageTop(image, layout, expect, scale, boxes = expect.boxes) {
  if (image.height <= layout.viewport.h * scale + 2) return 0;
  const ground = rgb(expect.background ?? '#ffffff');
  const [want, box] = boxes
    .map(w => [w, w.fill && layout.nodes.find(n => n.testId === w.testId)])
    .filter(([, b]) => b)
    .sort(([, a], [, b]) => a.y - b.y)[0] ?? [];
  if (!box) return 0;
  const fill = rgb(want.fill);
  const ink = (x, y) => {
    if (x < 0 || y < 0 || x >= image.width || y >= image.height) return 0;
    const i = (y * image.width + x) * 4;
    return coverage([image.data[i], image.data[i + 1], image.data[i + 2]], fill, ground) ?? 0;
  };
  // An edge where a run of `length` inked pixels begins, along `at(k)`: the
  // run's first pixel, less the fraction the pixel before it is inked, plus
  // the fraction its own is not.
  const edge = (count, at, length) => {
    for (let k = 0, run = 0; k < count; k++) {
      run = at(k) >= 0.5 ? run + 1 : 0;
      if (run >= length) {
        const first = k - run + 1;
        return first + (1 - at(first)) - at(first - 1);
      }
    }
    return null;
  };
  const x = Math.round((box.x + box.w / 2) * scale);
  const top = edge(image.height, y => ink(x, y), Math.floor(box.h * scale * 0.9));
  if (top === null) return 0;
  const y = Math.round(top + (box.h / 2) * scale);
  // Across the box's middle row, its left and right edges where the page's
  // pixels a point say: a page offset or narrowed across the screen (side
  // bars, a cutout, on either side) puts one elsewhere.
  const row = k => ink(k, y);
  const left = edge(image.width, row, Math.floor(box.w * scale * 0.9));
  // Where that run ends: the first pixel less than half inked after it, less
  // the fraction the pixel before is not, plus the fraction its own is.
  let right = null;
  if (left !== null) {
    let k = Math.ceil(left);
    while (k < image.width && row(k) >= 0.5) k++;
    right = k - (1 - row(k - 1)) + row(k);
  }
  const across = left === null ? Infinity : Math.max(Math.abs(left - box.x * scale), Math.abs(right - (box.x + box.w) * scale));
  if (!(across <= 1)) throw new Error(`the page is not the screenshot's width (a box's edges ${across === Infinity ? 'not found' : `${across.toFixed(1)} px off`}: side bars, a cutout): not read`);
  return top - box.y * scale;
}

/** Each box's padding and radius on `host`, against what `expect` says: a
 * number, or a base name (`space.md`), which `base(name, host)` gives. */
export function check(host, layout, image, expect, base = () => undefined) {
  const named = v => (typeof v === 'string' ? (base(v, host, layout.cell) ?? v) : v);
  const scale = image ? image.width / layout.viewport.w : 1;
  const top = image ? pageTop(image, layout, expect, scale, expect.calibrate) : 0;
  const ground = rgb(expect.background ?? '#ffffff');
  const nodes = layout.nodes;
  return expect.boxes.map(want => {
    const box = typeof want.testId === 'string' && want.testId ? nodes.find(n => n.testId === want.testId) : null;
    if (!box) return { host, testId: want.testId, ok: false, why: 'not in the layout' };
    // Its box as the host laid it out (and the cells it paints, in a grid).
    const row = { host, testId: want.testId, ok: true, why: [], box: { x: box.x, y: box.y, w: box.w, h: box.h, ...(box.cells ? { cells: box.cells } : {}) } };
    const fail = why => { row.ok = false; row.why.push(why); };
    // The terminal paints in cells: a reply that gives no grid cannot say
    // which rows it paints.
    const grid = layout.cell && Number.isFinite(layout.cell.w) && layout.cell.w > 0 && Number.isFinite(layout.cell.h) && layout.cell.h > 0;
    if (host === 'terminal' && !grid) fail('its reply gives no cell grid: the cells it paints cannot be read');
    const padding = named(pick(want.padding, host));
    if (padding === 'missing') fail(`no padding expected for ${host}`);
    else if (padding !== undefined && !Number.isFinite(padding)) fail(`padding ${JSON.stringify(padding)} is no number`);
    else if (padding !== undefined) {
      const child = typeof want.child === 'string' && want.child ? nodes.find(n => n.testId === want.child) : null;
      if (!child) fail(`its child ${want.child ?? '(unnamed)'} is not in the layout`);
      else {
        row.padding = [child.x - box.x, child.y - box.y];
        // In a grid of cells (the terminal's), the kernel lays each space out
        // in whole cells on its axis (LLP 1121 stage 11 (c)): across, then down.
        const wantPx = layout.cell ? [cellsOf(padding, layout.cell.w) * layout.cell.w, cellsOf(padding, layout.cell.h) * layout.cell.h] : [padding, padding];
        row.expectedPadding = layout.cell ? wantPx : padding;
        if (row.padding.some((p, i) => Math.abs(p - wantPx[i]) > 0.5)) fail(`padding ${row.padding.join(', ')}, expected ${wantPx.join(', ')}`);
        // In a grid of cells (the terminal's): the padding as the painter
        // rounds it; for a box `even` (a child filling it, but for its
        // padding), the same whole number of rows above and below, of
        // columns left and right (no half-row drift between edges).
        const whole = r => r && ['x', 'y', 'w', 'h'].every(k => Number.isInteger(r[k]));
        // (Only a host that paints in cells says them: the terminal.)
        if (layout.cell && !(whole(box.cells) && whole(child.cells))) fail('its cells and its child\'s are not whole cells in the reply: the rows it paints cannot be read');
        else if (box.cells && child.cells) {
          const [b, c] = [box.cells, child.cells];
          row.margins = { top: c.y - b.y, bottom: b.y + b.h - (c.y + c.h), left: c.x - b.x, right: b.x + b.w - (c.x + c.w) };
          if (want.even && (row.margins.top !== row.margins.bottom || row.margins.left !== row.margins.right)) fail(`its padding paints ${row.margins.top}/${row.margins.right}/${row.margins.bottom}/${row.margins.left} cells (top/right/bottom/left): not equal rows above and below and columns left and right`);
          // Each edge a length in whole cells on its axis (stage 11 (c)):
          // the leading edges always, the far ones for a box `even`.
          if (layout.cell) {
            const [down, across] = [cellsOf(padding, layout.cell.h), cellsOf(padding, layout.cell.w)];
            const m = row.margins;
            if (m.top !== down || m.left !== across || (want.even && (m.bottom !== down || m.right !== across))) fail(`its padding paints ${m.top}/${m.right}/${m.bottom}/${m.left} cells (top/right/bottom/left), expected ${down} rows and ${across} columns`);
          }
        }
      }
    }
    const gap = named(pick(want.gap, host));
    if (gap === 'missing') fail(`no gap expected for ${host}`);
    else if (gap !== undefined && !Number.isFinite(gap)) fail(`gap ${JSON.stringify(gap)} is no number`);
    else if (gap !== undefined) {
      // Between its two named children, across whichever axis they are apart on.
      const [a, b] = (Array.isArray(want.between) ? want.between : []).map(id => (typeof id === 'string' && id ? nodes.find(n => n.testId === id) : null));
      if (!a || !b) fail(`its children ${JSON.stringify(want.between)} are not both in the layout`);
      else {
        // Across the axis it names, or the one its children are aligned on.
        const axis = want.axis ?? (Math.abs(b.y - a.y) < 0.5 ? 'row' : Math.abs(b.x - a.x) < 0.5 ? 'column' : null);
        if (axis !== 'row' && axis !== 'column') fail('its children are aligned on neither axis: name its `axis` (row or column)');
        else {
          row.gap = axis === 'row' ? b.x - (a.x + a.w) : b.y - (a.y + a.h);
          const cell = layout.cell && (axis === 'row' ? layout.cell.w : layout.cell.h);
          const wantGap = cell ? cellsOf(gap, cell) * cell : gap;
          if (Math.abs(row.gap - wantGap) > 0.5) fail(`gap ${row.gap}, expected ${wantGap}`);
          // In a grid of cells, the gap as a length in whole cells.
          const whole = r => r && ['x', 'y', 'w', 'h'].every(k => Number.isInteger(r[k]));
          if (layout.cell && !(whole(a.cells) && whole(b.cells))) fail('its children\'s cells are not whole cells in the reply: the gap it paints cannot be read');
          else if (layout.cell) {
            const cells = axis === 'row' ? b.cells.x - (a.cells.x + a.cells.w) : b.cells.y - (a.cells.y + a.cells.h);
            const wantCells = cellsOf(gap, axis === 'row' ? layout.cell.w : layout.cell.h);
            row.gapCells = cells;
            if (cells !== wantCells) fail(`its gap paints ${cells} cells, expected ${wantCells}`);
          }
        }
      }
    }
    const height = named(pick(want.height, host));
    if (height === 'missing') fail(`no height expected for ${host}`);
    else if (height !== undefined && !Number.isFinite(height)) fail(`height ${JSON.stringify(height)} is no number`);
    else if (height !== undefined) {
      row.height = box.h;
      if (Math.abs(box.h - height) > 0.5) fail(`height ${box.h}, expected ${height}`);
    }
    const radius = named(pick(want.radius, host));
    if (radius === 'missing') fail(`no radius expected for ${host}`);
    else if (radius !== undefined && !Number.isFinite(radius)) fail(`radius ${JSON.stringify(radius)} is no number`);
    else if (radius !== undefined && !image) {
      // No picture (the terminal, which draws no corner a picture could be
      // measured by): the corners its reply resolves.
      row.expected = reduced(radius, box.w, box.h);
      if (!Array.isArray(box.radius) || box.radius.length !== 4 || !box.radius.every(Number.isFinite)) fail(`its reply's corner radii are not four lengths (${JSON.stringify(box.radius)}), and there is no picture`);
      else {
        row.radius = Math.max(...box.radius);
        if (box.radius.some(r => Math.abs(r - row.expected) > 0.5)) fail(`radii ${box.radius.join(', ')}, expected ${row.expected}`);
      }
    } else if (radius !== undefined) {
      const px = { x: box.x * scale, y: box.y * scale + top, w: box.w * scale, h: box.h * scale };
      const starts = edges(image, px, rgb(want.fill ?? '#000000'), ground);
      row.expected = reduced(radius, box.w, box.h);
      if (!starts) fail('its corner could not be read (off the image, too small, no fill there, or another colour in it)');
      else {
        const measured = fitRadius(starts, Math.max(px.w, px.h) / 2 + 1) / scale;
        row.radius = Math.round(measured * 10) / 10;
        // A radius no deeper than the first row read draws that row as a
        // square corner does: two such are the same reading.
        const unseen = starts[0].dy / scale;
        if (Math.abs(measured - row.expected) > 1 && !(measured <= unseen && row.expected <= unseen)) fail(`radius ${row.radius}, expected ${row.expected}`);
      }
    }
    row.why = row.why.join('; ');
    return row;
  });
}

/** One drive of `plan` on `host`: `steps`, then its layout and screenshot,
 * once the host says it runs `plan` (its `perf` names the plan by its bytes'
 * SHA-256): a host that runs another, as the Android reader would on its own
 * bake were the push not read, fails loudly. The reader runs an app's APK
 * (`app`), which the drive gives the plan to (pushed, in place of its bake). */
export function drive(host, plan, png, steps = [], app = null) {
  const launch = host === 'android' ? ['--reader', '--app', app, '--plan', plan] : ['--plan', plan];
  // The terminal host headless, its own verbs: `plan` (the running plan's
  // SHA-256), the steps, then `layout` (its reply carries each corner's
  // radius: it draws none a picture could be measured by, so there is none).
  const run = host === 'terminal'
    ? spawnSync('cargo', ['run', '-q', '-p', 'exact-terminal', '--', plan, '--size', '64x64', 'plan', ...steps.flatMap(terminalVerb), 'layout'], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 })
    : spawnSync(process.execPath, [join(ROOT, 'scripts/agent.mjs'), host, ...launch, '--storage', 'scale-check', '--reset-storage', '--json', 'perf', ...steps, 'layout', `screenshot ${png}`], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 });
  const ran = (run.stdout ?? '').split('\n').map(l => /"plan":"([0-9a-f]{64})"/.exec(l)?.[1]).find(Boolean);
  const want = createHash('sha256').update(readFileSync(plan)).digest('hex');
  if (run.status === 0 && ran !== want) throw new Error(`${host} runs ${ran ? `another plan (${ran.slice(0, 12)}…)` : 'a plan it does not name'}, not ${want.slice(0, 12)}…: not measured`);
  // The last layout: the one after the steps; any a step asked for before
  // it, in order, as `earlier`.
  const layouts = (run.stdout ?? '').split('\n').filter(l => l.startsWith('{') && l.includes('"nodes"'));
  const line = layouts.at(-1);
  if (run.error || run.status !== 0 || !line) throw new Error(`the drive failed${run.error ? ` (${run.error.message})` : run.signal ? ` (${run.signal})` : ''}\n${run.stdout ?? ''}${run.stderr ?? ''}`);
  return Object.assign(JSON.parse(line), { earlier: layouts.slice(0, -1).map(l => JSON.parse(l)) });
}

/** An agent step as the terminal's verbs take it: a verb's text tail (`type
 * <id> <text…>`, `paste <text…>`, `until <text…>`) one argument, spaces and
 * all; the rest a word each. */
export function terminalVerb(step) {
  const text = step.trim(), words = text.split(/\s+/);
  const tail = { type: 2, paste: 1, until: 1 }[words[0]];
  if (tail === undefined) return words;
  // (A verb short of its words would take the next verb, `layout`, as one.)
  if (words.length <= tail) throw new Error(`the step \`${text}\` is short of its words for the terminal`);
  let rest = text;
  for (let k = 0; k < tail; k++) rest = rest.replace(/^\S+\s+/, '');
  return [...words.slice(0, tail), rest];
}

/** The command line: {source, expect, hosts, out, json}, or a reason it is not one. */
export function parse(argv) {
  const opts = { hosts: [], out: '/tmp/exact-scale-check', json: false, files: [], app: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--host' || a === '--out' || a === '--app') {
      const v = argv[++i];
      if (v === undefined || v.startsWith('--')) return `${a} needs a value`;
      if (a === '--host') opts.hosts.push(v); else if (a === '--app') opts.app = v; else opts.out = v;
    } else if (a === '--json') opts.json = true;
    else if (a.startsWith('--')) return `unknown flag ${a}`;
    else opts.files.push(a);
  }
  if (opts.files.length !== 2) return 'a plan and an expectation file';
  opts.out = resolve(opts.out);
  // The agent's `screenshot <png>` is one word.
  if (/\s/.test(opts.out)) return `--out ${opts.out}: a path with no spaces`;
  const hosts = opts.hosts.length ? opts.hosts : ['linux'];
  // The reader runs an app's APK, the plan given to it.
  if (hosts.includes('android') && !opts.app) return '--host android needs --app <an app whose APK the reader runs: one with no data, as unsaid>';
  return { source: resolve(opts.files[0]), expect: resolve(opts.files[1]), hosts, out: opts.out, json: opts.json, app: opts.app };
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/scale-check.mjs <plan.contract> <expect.json> [--host <h> …] [--app <name>] [--out <dir>] [--json]`); process.exit(2); }
  mkdirSync(opts.out, { recursive: true });
  const plan = join(opts.out, 'scale.plan');
  const built = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', opts.source, '-o', plan, '--errors-only'], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 });
  if (built.error || built.status !== 0) { console.error(`${built.error?.message ?? ''}${built.stdout}${built.stderr}`); process.exit(1); }
  const expect = JSON.parse(readFileSync(opts.expect, 'utf8'));
  // A length named is read from the kernel's table and the plan's own design,
  // as the compiler reads it, never a copy.
  const schema = JSON.parse(readFileSync(join(ROOT, 'kernel/tables/schema.json'), 'utf8'));
  const ir = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'tamber', 'ir', opts.source], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 });
  let overrides = {};
  try {
    // (A plan with no design has none of its own.)
    if (ir.status === 0) overrides = overridesOf(JSON.parse(ir.stdout), schema.scale.map(r => r[0]));
    else if (!/^contract tamber: \S+ has no design \(an app\.tamber, or the file app\.json's "tamber" names\)$/m.test(`${ir.stdout}${ir.stderr}`)) throw new Error(`contract tamber ir: ${ir.stdout}${ir.stderr}`);
  } catch (e) { console.error(e.message); process.exit(1); }
  const base = (name, host, grid) => baseLength(schema, overrides, name, host, grid);
  // The first frame, then each `then`'s steps from a fresh launch.
  const phases = [{ steps: [], boxes: expect.boxes }, ...(expect.then ?? [])];
  const rows = [];
  for (const host of opts.hosts) {
    for (const [k, phase] of phases.entries()) {
      const png = join(opts.out, `${host}${k ? `-${k}` : ''}.png`);
      try {
        const layout = drive(host, plan, png, phase.steps ?? [], opts.app);
        const image = host === 'terminal' ? null : decodePng(readFileSync(png));
        rows.push(...check(host, layout, image, { ...expect, boxes: phase.boxes, calibrate: expect.boxes }, base).map(r => (k ? { ...r, testId: `${r.testId} (after ${phase.steps.join(', ')})` } : r)));
      }
      catch (e) { rows.push({ host, testId: '*', ok: false, why: e.message }); }
    }
  }
  if (opts.json) console.log(JSON.stringify(rows));
  else for (const r of rows) console.log(`${r.ok ? 'ok  ' : 'FAIL'} ${r.host} ${r.testId}${r.padding ? ` padding ${r.padding.join(', ')}` : ''}${r.box?.cells ? ` [cells ${r.box.cells.x},${r.box.cells.y} ${r.box.cells.w}x${r.box.cells.h}${r.margins ? `, padding ${r.margins.top}/${r.margins.right}/${r.margins.bottom}/${r.margins.left}` : ''}]` : ''}${r.gap !== undefined ? ` gap ${r.gap}` : ''}${r.height !== undefined ? ` height ${r.height}` : ''}${r.radius !== undefined ? ` radius ${r.radius} (expected ${r.expected})` : ''}${r.why ? `: ${r.why}` : ''}`);
  process.exit(rows.every(r => r.ok) ? 0 : 1);
}
