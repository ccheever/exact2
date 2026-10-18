#!/usr/bin/env bun
// Measure the control-role table's fidelity on one host (LLP 1121 D9, stage
// 8 C): the fixture (scripts/fixtures/control-roles) built twice, once
// as written (an authored accent and field roles) and once with neither,
// each control's frame read from both screenshots and compared.
//
//   bun scripts/measure-controls.mjs <web|macos|ios|linux|android> [--json] [--keep <dir>] [--scratch <worktree>]
//
// For each (control, parameter), over the control's pixels:
// - exact: at least 4 pixels within ΔE00 3 of the authored colour where
//   the plain build's pixel is not;
// - best-effort: none so close, but 4 pixels further than ΔE00 3 from the
//   authored colour yet nearer it than the plain build's pixel there (moved
//   by more than ΔE00 1, of its hue (within 30°, chroma 10 or more; an
//   authored grey by ΔE00 alone), and not another colour the fixture asks for,
//   blended over the pixel or the control's ground): theme2's ruling;
// - none: else (the platform's own colour stands);
// - unmeasured: fewer than 16 pixels in the frame, or frames that differ
//   between the builds (the run fails, naming it).
// Pixels that move between two looks in either build, the app's clock
// held (an animation the clock does not drive), are left out first (theme2's 08:42 ruling, as is the hue).
// Run each host's prerequisites first (a web dist, a native build of the
// host fixture, a simulator or emulator of your own, Android's reader APK:
// with --scratch for the reader), as agent.mjs says.
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { decodePng } from './png.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const FIXTURE = resolve(ROOT, 'scripts/fixtures/control-roles');
const argv = process.argv.slice(2);
const host = argv[0];
const keep = argv.includes('--keep') ? resolve(argv[argv.indexOf('--keep') + 1]) : null;
const asJson = argv.includes('--json');
const scratch = argv.includes('--scratch') ? resolve(argv[argv.indexOf('--scratch') + 1]) : null;

// What each row measures: the control's testId, the parameter, and the
// colour the authored build asks for there.
const ACCENT = '#e0218a';
export const ROWS = [
  ['switch', 'accent-color', 'switch', ACCENT],
  ['switch:disabled', 'accent-color', 'switch-disabled', ACCENT],
  ['checkbox', 'accent-color', 'checkbox', ACCENT],
  ['checkbox:disabled', 'accent-color', 'checkbox-disabled', ACCENT],
  ['radio', 'accent-color', 'radio', ACCENT],
  ['radio:disabled', 'accent-color', 'radio-disabled', ACCENT],
  ['range', 'accent-color', 'range', ACCENT],
  ['range:disabled', 'accent-color', 'range-disabled', ACCENT],
  ['progress', 'accent-color', 'progress', ACCENT],
  ['spinner', 'accent-color', 'spinner', ACCENT],
  ['select', 'accent-color', 'select', ACCENT],
  ['button-prominent', 'accent-color', 'button-prominent', ACCENT],
  ['button-prominent:disabled', 'accent-color', 'button-prominent-disabled', ACCENT],
  ['field', 'Field', 'field', '#fff1a8'],
  ['field', 'FieldText', 'field', '#0b8a3a'],
  ['field', 'placeholder', 'field-placeholder', '#d9480f'],
  ['textarea', 'Field', 'textarea', '#fff1a8'],
  ['textarea', 'FieldText', 'textarea', '#0b8a3a'],
  ['textarea', 'placeholder', 'textarea-placeholder', '#d9480f'],
];

/** sRGB hex or [r, g, b] 0–255 → CIE Lab (D65). */
export function lab(c) {
  const [r, g, b] = (typeof c === 'string' ? [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16)) : c).map(v => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  const xyz = [
    (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047,
    0.2126 * r + 0.7152 * g + 0.0722 * b,
    (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883,
  ].map(t => (t > 216 / 24389 ? Math.cbrt(t) : (24389 / 27 * t + 16) / 116));
  return [116 * xyz[1] - 16, 500 * (xyz[0] - xyz[1]), 200 * (xyz[1] - xyz[2])];
}

/** CIEDE2000 between two Lab colours. */
export function deltaE00([L1, a1, b1], [L2, a2, b2]) {
  const rad = Math.PI / 180;
  const C1 = Math.hypot(a1, b1), C2 = Math.hypot(a2, b2), Cm = (C1 + C2) / 2;
  const G = 0.5 * (1 - Math.sqrt(Cm ** 7 / (Cm ** 7 + 25 ** 7)));
  const ap1 = (1 + G) * a1, ap2 = (1 + G) * a2;
  const Cp1 = Math.hypot(ap1, b1), Cp2 = Math.hypot(ap2, b2);
  const h = (a, b) => (a === 0 && b === 0 ? 0 : (Math.atan2(b, a) / rad + 360) % 360);
  const hp1 = h(ap1, b1), hp2 = h(ap2, b2);
  const dL = L2 - L1, dC = Cp2 - Cp1;
  let dh = hp2 - hp1;
  if (Cp1 * Cp2 === 0) dh = 0; else if (dh > 180) dh -= 360; else if (dh < -180) dh += 360;
  const dH = 2 * Math.sqrt(Cp1 * Cp2) * Math.sin((dh / 2) * rad);
  const Lm = (L1 + L2) / 2, Cpm = (Cp1 + Cp2) / 2;
  let hm = hp1 + hp2;
  if (Cp1 * Cp2 !== 0) hm = Math.abs(hp1 - hp2) > 180 ? (hp1 + hp2 + (hp1 + hp2 < 360 ? 360 : -360)) / 2 : (hp1 + hp2) / 2;
  const T = 1 - 0.17 * Math.cos((hm - 30) * rad) + 0.24 * Math.cos(2 * hm * rad) + 0.32 * Math.cos((3 * hm + 6) * rad) - 0.2 * Math.cos((4 * hm - 63) * rad);
  const SL = 1 + (0.015 * (Lm - 50) ** 2) / Math.sqrt(20 + (Lm - 50) ** 2), SC = 1 + 0.045 * Cpm, SH = 1 + 0.015 * Cpm * T;
  const RT = -2 * Math.sqrt(Cpm ** 7 / (Cpm ** 7 + 25 ** 7)) * Math.sin(60 * Math.exp(-(((hm - 275) / 25) ** 2)) * rad);
  return Math.sqrt((dL / SL) ** 2 + (dC / SC) ** 2 + (dH / SH) ** 2 + RT * (dC / SC) * (dH / SH));
}

/** A fixture build: the app as written, or with no accent and no design. */
function build(dir, authored) {
  cpSync(resolve(FIXTURE, 'app.contract'), join(dir, 'app.contract'));
  if (authored) cpSync(resolve(FIXTURE, 'app.tamber'), join(dir, 'app.tamber'));
  else writeFileSync(join(dir, 'app.contract'), readFileSync(join(dir, 'app.contract'), 'utf8').replace(' accent-color="accent.authored"', ''));
  const plan = join(dir, 'app.plan');
  const c = spawnSync('cargo', ['run', '-q', '--profile', 'host-dev', '-p', 'contract', '--', 'build', join(dir, 'app.contract'), '-o', plan], { cwd: ROOT, encoding: 'utf8' });
  if (c.status !== 0) throw new Error(`the fixture does not build: ${c.stderr}`);
  return plan;
}

/** Each control's frame (view points) and the screenshot, on `host`.
 * Android is its reader (D9's column: the APK's Canvas reader, not the
 * Linux host under adb), which boots the plan its APK bakes: the fixture
 * goes in as the app of a scratch checkout (`--scratch <worktree>`, whose
 * `apps/tamber-host` it overwrites) and the APK is built there. */
function capture(plan, png, dir) {
  const ids = [...new Set(ROWS.map(r => r[2]))];
  let root = ROOT, target = ['--plan', plan];
  if (host === 'android') {
    if (!scratch) throw new Error('android: --scratch <a throwaway git worktree of this checkout>, whose apps/tamber-host is the fixture while it runs');
    // The fixture names no module: the app's own Swatch (its hatches read its design's params) goes.
    const manifest = join(scratch, 'apps/tamber-host/app.json');
    writeFileSync(manifest, JSON.stringify({ ...JSON.parse(readFileSync(manifest, 'utf8')), modules: [] }, null, 2));
    rmSync(join(scratch, 'apps/tamber-host/modules'), { recursive: true, force: true });
    for (const f of ['app.contract', 'app.tamber']) rmSync(join(scratch, 'apps/tamber-host', f), { force: true });
    cpSync(join(dir, 'app.contract'), join(scratch, 'apps/tamber-host/app.contract'));
    if (existsSync(join(dir, 'app.tamber'))) cpSync(join(dir, 'app.tamber'), join(scratch, 'apps/tamber-host/app.tamber'));
    const b = spawnSync(process.execPath, [join(scratch, 'scripts/agent-android.mjs'), 'build', 'tamber-host', '--apk'], { cwd: scratch, encoding: 'utf8', maxBuffer: 1 << 28 });
    if (b.status !== 0) throw new Error(`the reader's APK does not build: ${(b.stderr || b.stdout).slice(-2000)}`);
    root = scratch;
    target = ['--reader', '--app', 'tamber-host'];
  }
  const r = spawnSync(process.execPath, [resolve(root, 'scripts/agent.mjs'), host, ...target, '--json', 'clock settle', ...ids.map(id => `layout ${id}`), `screenshot ${png}`, `screenshot ${png.replace(/\.png$/, '.again.png')}`], { cwd: root, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error(`agent ${host}: ${r.stderr || r.stdout}`);
  const replies = r.stdout.split('\n').filter(l => l.startsWith('{')).map(l => JSON.parse(l));
  const frames = {};
  for (const reply of replies) if (reply.node?.testId) frames[reply.node.testId] = reply.node.space;
  const shot = replies.findLast(x => x.screenshot), image = decodePng(readFileSync(png));
  // A second look, the app's clock held: a pixel that moved is an animation the clock does not drive.
  const again = decodePng(readFileSync(png.replace(/\.png$/, '.again.png')));
  // A phone's screenshot is its whole screen: the viewport's width, and the
  // status bar above the app's first row (the reader draws under no other).
  if (shot.device) {
    const points = replies.find(x => x.viewport)?.viewport.w;
    const bars = spawnSync('adb', ['shell', 'dumpsys', 'window'], { encoding: 'utf8', maxBuffer: 1 << 26 }).stdout ?? '';
    const top = Number(bars.match(/type=statusBars frame=\[\d+,\d+\]\[\d+,(\d+)\]/)?.[1] ?? NaN);
    if (!points || !Number.isFinite(top)) throw new Error(`${host}: the screenshot's scale or the status bar's height is unknown`);
    return { frames, image, again, points, top };
  }
  return { frames, image, again, points: shot.w, top: 0 };
}

/** The pixels of a frame: [lab, rgb, x, y] for each. */
function pixels({ image, points, top }, space, from = image) {
  const scale = image.width / points;
  const v = space.viewport, out = [];
  // Inset one point: the frame's own edge is antialiased into the page.
  for (let y = Math.ceil((v.y + 1) * scale) + top; y < Math.floor((v.y + v.h - 1) * scale) + top; y++) {
    for (let x = Math.ceil((v.x + 1) * scale); x < Math.floor((v.x + v.w - 1) * scale); x++) {
      if (x < 0 || y < 0 || x >= image.width || y >= image.height) continue;
      const i = (y * image.width + x) * 4, rgb = [from.data[i], from.data[i + 1], from.data[i + 2]];
      out.push([lab(rgb), rgb, x, y]);
    }
  }
  return out;
}

/** One row's fidelity, from the two builds' pixels of its control. */
export function judge(authoredPx, plainPx, target, others = []) {
  const n = Math.min(authoredPx.length, plainPx.length);
  if (n < 16 || authoredPx.length !== plainPx.length || [...authoredPx, ...plainPx].some(p => p[0].some(v => !Number.isFinite(v)))) {
    return { fidelity: 'unmeasured', deltaE: null, pixels: n, why: `${n} pixels in its frame, or the two builds' frames differ` };
  }
  const want = lab(target), hex = c => [1, 3, 5].map(i => parseInt(c.slice(i, i + 2), 16));
  // A rival's colour blended over a pixel, as antialiasing draws it.
  const blend = (rival, under, t) => rival.map((v, k) => v * t + under[k] * (1 - t));
  const rivals = others.map(hex);
  // The plain control's ground: its most common colour.
  const counts = new Map();
  for (const px of plainPx) { const k = px[1].join(); counts.set(k, (counts.get(k) ?? 0) + 1); }
  const ground = [...counts].sort((x, y) => y[1] - x[1])[0][0].split(',').map(Number);
  // A pixel another colour the fixture asks for explains, blended
  // over the plain pixel or the control's ground, is that one's (the field's
  // text and its antialiasing, not its ground).
  const theirs = i => rivals.some(r => [plainPx[i][1], ground].some(under =>
    [...Array(20).keys()].some(k => deltaE00(authoredPx[i][0], lab(blend(r, under, (k + 1) / 20))) <= 3)));
  // Of the authored colour's hue (within 30°, chroma 10 or more): a tint
  // of it, not a grey that a moving picture or a blend left nearer.
  const hue = ([, a, b]) => (Math.atan2(b, a) * 180) / Math.PI;
  // An authored grey (chroma under 10) has no hue: judged by ΔE00 alone.
  const grey = Math.hypot(want[1], want[2]) < 10;
  const tinted = px => grey || (Math.hypot(px[1], px[2]) >= 10 && Math.abs(((hue(px) - hue(want) + 540) % 360) - 180) <= 30);
  const near = authoredPx.map(p => deltaE00(p[0], want)), was = plainPx.map(p => deltaE00(p[0], want));
  // Exact: within ΔE00 3 of the authored colour where the plain build was not.
  const close = near.filter((d, i) => d <= 3 && was[i] > 3).length;
  const best = Math.min(...near);
  if (close >= 4) return { fidelity: 'exact', deltaE: best, pixels: close };
  // Best-effort (theme2's ruling): further than ΔE00 3 from the authored
  // colour, yet nearer it than the platform's pixel there.
  let nearer = 0, nearest = Infinity;
  for (let i = 0; i < n; i++) {
    if (near[i] > 3 && near[i] < was[i] && deltaE00(authoredPx[i][0], plainPx[i][0]) > 1 && tinted(authoredPx[i][0]) && !theirs(i)) { nearer++; nearest = Math.min(nearest, near[i]); }
  }
  if (nearer >= 4) return { fidelity: 'best-effort', deltaE: nearest, pixels: nearer };
  return { fidelity: 'none', deltaE: best, pixels: 0 };
}

if (import.meta.main) {
  if (!['web', 'macos', 'ios', 'linux', 'android'].includes(host)) {
    console.error('usage: bun scripts/measure-controls.mjs <web|macos|ios|linux|android> [--json] [--keep <dir>]');
    process.exit(2);
  }
  const dir = keep ?? mkdtempSync(join(tmpdir(), 'exact-measure-controls-'));
  try {
    const [a, p] = ['authored', 'plain'].map(side => {
      const at = join(dir, side);
      spawnSync('mkdir', ['-p', at]);
      const plan = build(at, side === 'authored');
      return capture(plan, join(at, `${host}.png`), at);
    });
    const results = ROWS.map(([control, parameter, id, target]) => {
      if (!a.frames[id] || !p.frames[id]) return { control, parameter, fidelity: 'unmeasured', why: `no frame for ${id}` };
      // The two builds' pixels are compared one for one: their frames must agree.
      const [fa, fp] = [a.frames[id].viewport, p.frames[id].viewport];
      if (['x', 'y', 'w', 'h'].some(k => Math.abs(fa[k] - fp[k]) > 0.5)) return { control, parameter, fidelity: 'unmeasured', why: `${id}'s frame differs between the builds` };
      // A frame either screenshot does not hold whole is not measured.
      const outside = (c, f) => {
        const k = c.image.width / c.points, tall = (c.image.height - c.top) / k;
        return f.x < -0.5 || f.y < -0.5 || f.x + f.w > c.points + 0.5 || f.y + f.h > tall + 0.5;
      };
      if (outside(a, fa) || outside(p, fp)) return { control, parameter, fidelity: 'unmeasured', why: `${id} runs past the screenshot` };
      // Every other colour the fixture asks for (a field's ground paints its placeholder's box too).
      const others = [...new Set(ROWS.map(o => o[3]).filter(c => c !== target))];
      // Pixels either build moved between its two looks (animation) decide nothing.
      const [pa, pa2, pp, pp2] = [pixels(a, a.frames[id]), pixels(a, a.frames[id], a.again), pixels(p, p.frames[id]), pixels(p, p.frames[id], p.again)];
      const still = pa.map((_, i) => deltaE00(pa[i][0], pa2[i][0]) <= 1 && deltaE00(pp[i]?.[0] ?? [0, 0, 0], pp2[i]?.[0] ?? [0, 0, 0]) <= 1);
      const r = judge(pa.filter((_, i) => still[i]), pp.filter((_, i) => still[i]), target, others);
      return { control, parameter, ...r, deltaE: r.deltaE == null ? null : Math.round(r.deltaE * 10) / 10 };
    });
    process.exitCode = results.some(r => r.fidelity === 'unmeasured') ? 1 : 0;
    if (asJson) console.log(JSON.stringify({ host, results }));
    else for (const r of results) console.log(`${host}\t${r.control}\t${r.parameter}\t${r.fidelity}\tΔE00 ${r.deltaE ?? '-'}\t${r.pixels ?? 0} px${r.why ? `\t${r.why}` : ''}`);
  } finally {
    if (!keep) rmSync(dir, { recursive: true, force: true });
  }
}
