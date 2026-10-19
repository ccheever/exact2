#!/usr/bin/env bun
// What a host draws for a padding and a corner radius, against what it should
// (LLP 1121 stage 11 C): a plan driven on each host by the agent, its boxes
// read with `layout` and its corners from `screenshot`. A padding is the
// offset of a box's first child; a radius is fitted to where the box's fill
// begins along its top-left corner's first rows (antialiased coverage, at the
// screenshot's own scale). A radius over half the box's shorter side is drawn
// as CSS reduces it (a pill), so that is what is expected.
//
// usage: bun scripts/scale-check.mjs <plan.contract> <expect.json> [--host <h> …] [--out <dir>] [--json]
//   <expect.json>: {"background": "#ffffff", "boxes": [{"testId": "pill", "radius": 9999, "fill": "#0000ff"},
//                   {"testId": "padded", "padding": 16}]} — a value per box, or per host:
//                   {"padding": {"ios": 16, "macos": 10, "default": 16}}.
//   --host: web, linux, macos, ios (as scripts/agent.mjs names them); default linux.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { decodePng } from './png.mjs';

const ROOT = resolve(import.meta.dir, '..');

/** `#rrggbb` as [r, g, b]. */
export const rgb = hex => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));

/** How much of `fill` over `ground` the pixel is, 0 to 1, by its strongest channel. */
export function coverage(px, fill, ground) {
  let best = 0, spread = 0;
  for (let c = 0; c < 3; c++) {
    const d = fill[c] - ground[c];
    if (Math.abs(d) > spread) { spread = Math.abs(d); best = (px[c] - ground[c]) / d; }
  }
  return Math.min(1, Math.max(0, best));
}

/** Where, in rows 0..rows-1 below a box's top edge, its fill begins: the
 * column (from the box's left edge, in pixels) at which coverage reaches one
 * half, interpolated. */
export function edges(image, box, fill, ground, rows = 3) {
  const at = (x, y) => { const i = (y * image.width + x) * 4; return [image.data[i], image.data[i + 1], image.data[i + 2]]; };
  const out = [];
  for (let row = 0; row < rows; row++) {
    const y = box.y + row;
    let prev = 0;
    for (let dx = 0; dx < box.w / 2; dx++) {
      const c = coverage(at(box.x + dx, y), fill, ground);
      if (c >= 0.5) { out.push(dx - 1 + (0.5 - prev) / Math.max(c - prev, 1e-6) + 0.5); break; }
      prev = c;
    }
  }
  return out;
}

/** The radius whose circle crosses each row's centre where `starts` says. */
export function fitRadius(starts) {
  const inset = (r, row) => { const dy = r - (row + 0.5); return dy <= 0 ? 0 : r - Math.sqrt(Math.max(0, r * r - dy * dy)); };
  let best = 0, err = Infinity;
  for (let r = 0; r <= 400; r += 0.05) {
    const e = starts.reduce((s, x, row) => s + (inset(r, row) - x) ** 2, 0);
    if (e < err) { err = e; best = r; }
  }
  return best;
}

/** A box's expected radius once CSS reduces it to the box. */
export const reduced = (radius, w, h) => Math.min(radius, w / 2, h / 2);

const pick = (v, host) => (v && typeof v === 'object' ? v[host] ?? v.default : v);

function drive(host, plan, png) {
  const run = spawnSync(process.execPath, [join(ROOT, 'scripts/agent.mjs'), host, '--plan', plan, '--storage', 'scale-check', '--json', 'layout', `screenshot ${png}`], { cwd: ROOT, encoding: 'utf8', timeout: 900_000 });
  const line = (run.stdout ?? '').split('\n').find(l => l.startsWith('{') && l.includes('"nodes"'));
  if (run.status !== 0 || !line) throw new Error(`${host}: the drive failed\n${run.stdout}${run.stderr}`);
  return JSON.parse(line);
}

/** Each box's padding and radius on `host`, against what `expect` says. */
export function check(host, layout, image, expect) {
  const scale = image.width / layout.viewport.w;
  const ground = rgb(expect.background ?? '#ffffff');
  const nodes = layout.nodes;
  return expect.boxes.map(want => {
    const box = nodes.find(n => n.testId === want.testId);
    if (!box) return { host, testId: want.testId, ok: false, why: 'not in the layout' };
    const row = { host, testId: want.testId, ok: true };
    const padding = pick(want.padding, host);
    if (padding !== undefined) {
      const child = nodes.find(n => n.id > box.id && n.x >= box.x && n.y >= box.y);
      row.padding = child ? Math.min(child.x - box.x, child.y - box.y) : null;
      row.ok &&= row.padding !== null && Math.abs(row.padding - padding) <= 0.5;
    }
    const radius = pick(want.radius, host);
    if (radius !== undefined) {
      const px = { x: Math.round(box.x * scale), y: Math.round(box.y * scale), w: Math.round(box.w * scale), h: Math.round(box.h * scale) };
      const starts = edges(image, px, rgb(want.fill), ground);
      row.radius = Math.round((fitRadius(starts) / scale) * 10) / 10;
      row.expected = reduced(radius, box.w, box.h);
      row.ok &&= Math.abs(row.radius - row.expected) <= 1;
    }
    return row;
  });
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const hosts = [], flag = name => { const i = args.indexOf(name); return i < 0 ? null : args.splice(i, 2)[1]; };
  for (let h; (h = flag('--host'));) hosts.push(h);
  const out = flag('--out') ?? '/tmp/exact-scale-check';
  const json = args.includes('--json') && args.splice(args.indexOf('--json'), 1);
  const [source, expectFile] = args;
  if (!source || !expectFile) { console.error('usage: bun scripts/scale-check.mjs <plan.contract> <expect.json> [--host <h> …] [--out <dir>] [--json]'); process.exit(2); }
  mkdirSync(out, { recursive: true });
  const plan = join(out, 'scale.plan');
  const built = spawnSync('cargo', ['run', '-q', '-p', 'contract', '--', 'build', resolve(source), '-o', plan, '--errors-only'], { cwd: ROOT, encoding: 'utf8' });
  if (built.status !== 0) { console.error(built.stdout + built.stderr); process.exit(1); }
  const expect = JSON.parse(readFileSync(expectFile, 'utf8'));
  const rows = [];
  for (const host of hosts.length ? hosts : ['linux']) {
    const png = join(out, `${host}.png`);
    rows.push(...check(host, drive(host, plan, png), decodePng(readFileSync(png)), expect));
  }
  if (json) console.log(JSON.stringify(rows));
  else for (const r of rows) console.log(`${r.ok ? 'ok  ' : 'FAIL'} ${r.host} ${r.testId}${r.padding !== undefined ? ` padding ${r.padding}` : ''}${r.radius !== undefined ? ` radius ${r.radius} (expected ${r.expected})` : ''}${r.why ? ` ${r.why}` : ''}`);
  process.exit(rows.every(r => r.ok) ? 0 : 1);
}
