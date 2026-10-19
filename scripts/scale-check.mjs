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
// usage: bun scripts/scale-check.mjs <plan.contract> <expect.json> [--host <h> …] [--out <dir>] [--json]
//   <expect.json>: {"background": "#ffffff", "boxes": [{"testId": "pill", "radius": 9999, "fill": "#0000ff"},
//                   {"testId": "padded", "child": "inner", "padding": 16}]} — a value per box, or per host:
//                   {"padding": {"ios": 16, "macos": 10, "default": 16}}.
//   --host: web, linux, macos, ios (as scripts/agent.mjs names them); default linux.
//   Each drive starts from a fresh scratch store (`--storage scale-check --reset-storage`).
import { spawnSync } from 'node:child_process';
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

/** What `want` says for `host`: its own, else `default`; `missing` when it
 * names hosts and not this one. */
const pick = (v, host) => (v && typeof v === 'object' ? (host in v ? v[host] : 'default' in v ? v.default : 'missing') : v);

/** Each box's padding and radius on `host`, against what `expect` says. */
export function check(host, layout, image, expect) {
  const scale = image.width / layout.viewport.w;
  const ground = rgb(expect.background ?? '#ffffff');
  const nodes = layout.nodes;
  return expect.boxes.map(want => {
    const box = nodes.find(n => n.testId === want.testId);
    if (!box) return { host, testId: want.testId, ok: false, why: 'not in the layout' };
    const row = { host, testId: want.testId, ok: true, why: [] };
    const fail = why => { row.ok = false; row.why.push(why); };
    const padding = pick(want.padding, host);
    if (padding === 'missing') fail(`no padding expected for ${host}`);
    else if (padding !== undefined && !Number.isFinite(padding)) fail(`padding ${JSON.stringify(padding)} is no number`);
    else if (padding !== undefined) {
      const child = typeof want.child === 'string' && want.child ? nodes.find(n => n.testId === want.child) : null;
      if (!child) fail(`its child ${want.child ?? '(unnamed)'} is not in the layout`);
      else {
        row.padding = [child.x - box.x, child.y - box.y];
        row.expectedPadding = padding;
        if (row.padding.some(p => Math.abs(p - padding) > 0.5)) fail(`padding ${row.padding.join(', ')}, expected ${padding}`);
      }
    }
    const radius = pick(want.radius, host);
    if (radius === 'missing') fail(`no radius expected for ${host}`);
    else if (radius !== undefined && !Number.isFinite(radius)) fail(`radius ${JSON.stringify(radius)} is no number`);
    else if (radius !== undefined) {
      const px = { x: box.x * scale, y: box.y * scale, w: box.w * scale, h: box.h * scale };
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

function drive(host, plan, png) {
  const run = spawnSync(process.execPath, [join(ROOT, 'scripts/agent.mjs'), host, '--plan', plan, '--storage', 'scale-check', '--reset-storage', '--json', 'layout', `screenshot ${png}`], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 });
  const line = (run.stdout ?? '').split('\n').find(l => l.startsWith('{') && l.includes('"nodes"'));
  if (run.error || run.status !== 0 || !line) throw new Error(`the drive failed${run.error ? ` (${run.error.message})` : run.signal ? ` (${run.signal})` : ''}\n${run.stdout ?? ''}${run.stderr ?? ''}`);
  return JSON.parse(line);
}

/** The command line: {source, expect, hosts, out, json}, or a reason it is not one. */
export function parse(argv) {
  const opts = { hosts: [], out: '/tmp/exact-scale-check', json: false, files: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--host' || a === '--out') {
      const v = argv[++i];
      if (v === undefined || v.startsWith('--')) return `${a} needs a value`;
      if (a === '--host') opts.hosts.push(v); else opts.out = v;
    } else if (a === '--json') opts.json = true;
    else if (a.startsWith('--')) return `unknown flag ${a}`;
    else opts.files.push(a);
  }
  if (opts.files.length !== 2) return 'a plan and an expectation file';
  opts.out = resolve(opts.out);
  // The agent's `screenshot <png>` is one word.
  if (/\s/.test(opts.out)) return `--out ${opts.out}: a path with no spaces`;
  return { source: resolve(opts.files[0]), expect: resolve(opts.files[1]), hosts: opts.hosts.length ? opts.hosts : ['linux'], out: opts.out, json: opts.json };
}

if (import.meta.main) {
  const opts = parse(process.argv.slice(2));
  if (typeof opts === 'string') { console.error(`${opts}\nusage: bun scripts/scale-check.mjs <plan.contract> <expect.json> [--host <h> …] [--out <dir>] [--json]`); process.exit(2); }
  mkdirSync(opts.out, { recursive: true });
  const plan = join(opts.out, 'scale.plan');
  const built = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', opts.source, '-o', plan, '--errors-only'], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 });
  if (built.error || built.status !== 0) { console.error(`${built.error?.message ?? ''}${built.stdout}${built.stderr}`); process.exit(1); }
  const expect = JSON.parse(readFileSync(opts.expect, 'utf8'));
  const rows = [];
  for (const host of opts.hosts) {
    const png = join(opts.out, `${host}.png`);
    try { rows.push(...check(host, drive(host, plan, png), decodePng(readFileSync(png)), expect)); }
    catch (e) { rows.push({ host, testId: '*', ok: false, why: e.message }); }
  }
  if (opts.json) console.log(JSON.stringify(rows));
  else for (const r of rows) console.log(`${r.ok ? 'ok  ' : 'FAIL'} ${r.host} ${r.testId}${r.padding ? ` padding ${r.padding.join(', ')}` : ''}${r.radius !== undefined ? ` radius ${r.radius} (expected ${r.expected})` : ''}${r.why ? `: ${r.why}` : ''}`);
  process.exit(rows.every(r => r.ok) ? 0 : 1);
}
