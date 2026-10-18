// The Material oracle (LLP 1115.001.001 D5): each specimen drawn by Compose Material 3 (the reference, `:reference`)
// and by Exact (the specimen door, `apps/material-specimens`), compared in the layers that need no phone: geometry,
// colour at probes, text's appearance, and pixels. Then the detection test (§3 O1): each specimen against itself
// passes, and each planted defect fails.
//
//   bun host/android/reference/oracle.mjs [--out <dir>] [--compare]
//
// Without --compare it draws both sides first: the reference through Gradle (JAVA_HOME a JDK 21, ANDROID_HOME the
// SDK) and Exact through Cargo, given the reference's own fonts (Robolectric's, from Gradle's cache). It writes
// report.json and report.html into the out directory (default: host/android/reference/build/oracle) and exits 1
// when the detection test fails. Until calibration (O2) the comparison of the two sides reports and does not fail.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { crop, decodePng, encodePng } from '../../../scripts/png.mjs';

const here = dirname(new URL(import.meta.url).pathname);
const root = resolve(here, '../../..');
const manifest = JSON.parse(readFileSync(join(here, 'specimens.json'), 'utf8'));
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const option = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : null);
const out = resolve(option('--out') ?? join(here, 'build/oracle'));

/** The starting thresholds (D4: calibration on a phone replaces them). */
const LIMIT = {
  geometry: 0.5, // dp, any box edge or corner radius
  probe: 2, // ΔE2000
  ink: 5, // ΔE2000, a text's ink
  coverage: 0.2, // a text's ink, as a share of the larger
  pixels: 0.005, // share of a crop's pixels past ΔE2000 5
};
/** Around a component's layout box, room for its shadow. */
const MARGIN = 16;

let schemes;

function main() {
  if (!flag('--compare')) draw();
  schemes = JSON.parse(readFileSync(join(out, 'schemes.json'), 'utf8'));
  const report = { limits: LIMIT, comparisons: [], detection: [] };
  for (const s of manifest.specimens) {
    for (const scheme of manifest.schemes) {
      report.comparisons.push(compare(`${s.id}.${scheme}`, s, scheme, ['compose', 'exact']));
    }
  }
  for (const s of manifest.specimens) {
    for (const side of ['compose', 'exact']) {
      const c = compare(`${s.id}.light`, s, 'light', [side, side]);
      report.detection.push({ kind: 'self', name: `${side}: ${s.id}`, expect: 'pass', caught: failed(c), ok: !failed(c), comparison: c });
    }
  }
  for (const d of manifest.defects) {
    const s = manifest.specimens.find((x) => x.id === d.of);
    const c = compare(`${d.of}.light`, s, 'light', ['compose', 'compose'], `${d.id}.light`);
    report.detection.push({ kind: 'defect', name: `${d.id}: ${d.planted}`, expect: 'fail', caught: failed(c), ok: failed(c), comparison: c });
  }
  writeFileSync(join(out, 'report.json'), JSON.stringify(report, null, 2));
  writeFileSync(join(out, 'report.html'), html(report));
  const missed = report.detection.filter((d) => !d.ok);
  const failing = report.comparisons.filter(failed);
  console.log(`detection: ${report.detection.length - missed.length} of ${report.detection.length} as expected`);
  for (const d of missed) console.log(`  NOT AS EXPECTED: ${d.name} (expected to ${d.expect})`);
  console.log(`Compose against Exact: ${failing.length} of ${report.comparisons.length} specimens differ (reported, not failed, until O2)`);
  for (const c of failing) console.log(`  ${c.name}: ${summary(c)}`);
  console.log(`report: ${relative(process.cwd(), join(out, 'report.html'))}`);
  process.exit(missed.length ? 1 : 0);
}

function draw() {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  run('./gradlew', [':reference:draw', `-Preference.out=${out}`, '-q'], join(root, 'host/android'));
  const fonts = join(out, 'fonts');
  mkdirSync(fonts, { recursive: true });
  const jar = findJar(join(homedir(), '.gradle/caches/modules-2/files-2.1/org.robolectric/nativeruntime-dist-compat'));
  run('unzip', ['-q', '-j', '-o', jar, 'fonts/*', '-d', fonts], root);
  sansSerif(fonts);
  run('cargo', ['run', '-q', '-p', 'material-specimens-linux', '--', join(here, 'specimens.json'),
    join(root, 'apps/material-specimens/specimens'), out, join(out, 'exact')], root,
  { EXACT_FONTS: fonts, EXACT_FONT: 'Roboto' });
}

/**
 * Android finds `sans-serif` by `fonts.xml`, file by file and weight by weight, never by a font's own names; these
 * Roboto files name their family three ways ("Roboto21382017", "Roboto Medium21382017", and "Roboto" as the
 * typographic family of some), so a catalogue that reads the names would find no 400 in "Roboto" and draw Medium.
 * Each file of `fonts.xml`'s `sans-serif` is renamed "Roboto" (name IDs 1 and 16), as Android sees it.
 */
function sansSerif(dir) {
  const xml = readFileSync(join(dir, 'fonts.xml'), 'utf8');
  const family = xml.match(/<family name="sans-serif">([\s\S]*?)<\/family>/)?.[1];
  if (!family) throw new Error('fonts.xml names no sans-serif family');
  for (const [, file] of family.matchAll(/<font [^>]*>\s*([^<\s]+)\s*<\/font>/g)) {
    const path = join(dir, file);
    writeFileSync(path, renamed(readFileSync(path), 'Roboto'));
  }
}

/** An sfnt with its family names (IDs 1 and 16) `name`, its other names kept, a new `name` table appended. */
function renamed(font, name) {
  const view = new DataView(font.buffer, font.byteOffset, font.byteLength);
  const tables = view.getUint16(4);
  let entry = -1;
  for (let i = 0; i < tables; i++) if (font.toString('latin1', 12 + 16 * i, 16 + 16 * i) === 'name') entry = 12 + 16 * i;
  if (entry < 0) throw new Error('a font with no name table');
  const at = view.getUint32(entry + 8);
  const count = view.getUint16(at + 2), strings = at + view.getUint16(at + 4);
  const records = [];
  for (let i = 0; i < count; i++) {
    const r = at + 6 + 12 * i;
    const [platform, encoding, language, id, length, offset] = [0, 2, 4, 6, 8, 10].map((k) => view.getUint16(r + k));
    let bytes = font.subarray(strings + offset, strings + offset + length);
    if (id === 1 || id === 16) {
      bytes = platform === 1 ? Buffer.from(name, 'latin1') : Buffer.from(name, 'utf16le').swap16();
    }
    records.push({ platform, encoding, language, id, bytes });
  }
  const head = Buffer.alloc(6 + 12 * records.length);
  head.writeUInt16BE(0, 0);
  head.writeUInt16BE(records.length, 2);
  head.writeUInt16BE(head.length, 4);
  let offset = 0;
  records.forEach((r, i) => {
    [r.platform, r.encoding, r.language, r.id, r.bytes.length, offset].forEach((v, k) => head.writeUInt16BE(v, 6 + 12 * i + 2 * k));
    offset += r.bytes.length;
  });
  const table = Buffer.concat([head, ...records.map((r) => r.bytes)]);
  const padded = Buffer.concat([table, Buffer.alloc((4 - (table.length % 4)) % 4)]);
  let sum = 0;
  for (let i = 0; i < padded.length; i += 4) sum = (sum + padded.readUInt32BE(i)) >>> 0;
  const start = font.length + ((4 - (font.length % 4)) % 4);
  const out = Buffer.concat([font, Buffer.alloc(start - font.length), padded]);
  out.writeUInt32BE(sum, entry + 4);
  out.writeUInt32BE(start, entry + 8);
  out.writeUInt32BE(table.length, entry + 12);
  return out;
}

function run(cmd, argv, cwd, env = {}) {
  const r = spawnSync(cmd, argv, { cwd, stdio: 'inherit', env: { ...process.env, ...env } });
  if (r.status !== 0) throw new Error(`${cmd} ${argv.join(' ')} failed (${r.status ?? r.error?.message})`);
}

function findJar(dir) {
  const stack = [dir];
  while (stack.length) {
    const d = stack.pop();
    if (!existsSync(d)) continue;
    for (const e of readdirSync(d, { withFileTypes: true })) {
      if (e.isDirectory()) stack.push(join(d, e.name));
      else if (e.name.endsWith('.jar') && !e.name.endsWith('-sources.jar')) return join(d, e.name);
    }
  }
  throw new Error(`no Robolectric native runtime jar under ${dir}: draw the reference once first`);
}

// ---------------------------------------------------------------------------------------------------- one comparison

function compare(name, specimen, scheme, [refSide, candSide], candName = name) {
  const ref = load(refSide, name);
  const cand = load(candSide, candName);
  const density = manifest.density;
  const ground = argb(schemes[scheme].surface);
  const layers = {};
  const rv = visual(ref, ground, density);
  const cv = visual(cand, ground, density);
  // Geometry: the layout box, the visual box and its corners, each text's box.
  const geo = [];
  const edge = (what, a, b) => geo.push({ what, reference: round(a), candidate: round(b), ok: a != null && b != null && Math.abs(a - b) <= LIMIT.geometry });
  for (const k of ['x', 'y', 'w', 'h']) edge(`layout ${k}`, ref.facts.layout[k], cand.facts.layout[k]);
  if (!cv) geo.push({ what: 'visual box', reference: rv, candidate: null, ok: false, note: 'nothing drawn' });
  else {
    for (const k of ['x', 'y', 'w', 'h']) edge(`visual ${k}`, rv[k], cv[k]);
    if (rv.corners || cv.corners) {
      ['top-left', 'top-right', 'bottom-right', 'bottom-left'].forEach((c, i) => edge(`corner ${c}`, rv.corners?.[i] ?? null, cv.corners?.[i] ?? null));
    }
  }
  const texts = [];
  for (const t of ref.facts.texts ?? []) {
    const other = (cand.facts.texts ?? []).find((u) => u.text === t.text);
    if (!other) {
      geo.push({ what: `text "${t.text}"`, reference: t.box, candidate: null, ok: false, note: 'missing' });
      continue;
    }
    // Where both sides name a text's lines, its height is the line model's (Compose's line height, Exact's
    // paragraph centred on its line) and what places it is the baseline.
    const lines = t.lines?.length && other.lines?.length;
    for (const k of lines ? ['x', 'w'] : ['x', 'y', 'w', 'h']) edge(`text "${t.text}" ${k}`, t.box[k], other.box[k]);
    if (lines) {
      edge(`text "${t.text}" lines`, t.lines.length, other.lines.length);
      t.lines.forEach((l, i) => edge(`text "${t.text}" baseline ${i + 1}`, l.baseline, other.lines[i]?.baseline ?? null));
    }
    texts.push({ text: t.text, ref: t.box, cand: other.box });
  }
  layers.geometry = { ok: geo.every((g) => g.ok), items: geo };
  // Colour at probes, each in each side's own visual box.
  const probes = (specimen.probes ?? []).map((p) => {
    const a = rv && sample(ref.png, at(rv, p), density);
    const b = cv && sample(cand.png, at(cv, p), density);
    const d = a && b ? de2000(lab(a), lab(b)) : null;
    return { what: p.name, reference: a && hex(a), candidate: b && hex(b), deltaE: round(d), ok: d != null && d <= LIMIT.probe };
  });
  layers.probes = { ok: probes.every((p) => p.ok), items: probes };
  // Text's appearance: its ink and how much of it there is.
  const inks = texts.map((t) => {
    const a = ink(ref.png, t.ref, density);
    const b = ink(cand.png, t.cand, density);
    const d = de2000(lab(a.ink), lab(b.ink));
    const share = Math.abs(a.coverage - b.coverage) / Math.max(a.coverage, b.coverage, 1e-6);
    return { what: `text "${t.text}"`, inkReference: hex(a.ink), inkCandidate: hex(b.ink), deltaE: round(d),
      coverageReference: round(a.coverage), coverageCandidate: round(b.coverage),
      ok: d <= LIMIT.ink && share <= LIMIT.coverage };
  });
  layers.text = { ok: inks.every((t) => t.ok), items: inks };
  // Pixels, over each side's crop round its layout box, text masked.
  const px = pixels(ref, cand, texts, density, `${refSide}-${candSide}.${candName}`);
  layers.pixels = { ok: px.share <= LIMIT.pixels, share: round(px.share, 4), heat: px.heat, crops: px.crops, gates: false };
  return { name: candName === name ? name : `${name} ← ${candName}`, sides: [refSide, candSide], layers };
}

function failed(c) {
  return !c.layers.geometry.ok || !c.layers.probes.ok || !c.layers.text.ok || (c.sides[0] === c.sides[1] && !c.layers.pixels.ok);
}

function summary(c) {
  const bad = [];
  for (const [k, l] of Object.entries(c.layers)) {
    if (k === 'pixels') { if (!l.ok) bad.push(`pixels ${(l.share * 100).toFixed(2)}%`); continue; }
    const items = l.items.filter((i) => !i.ok).map((i) => i.what);
    if (items.length) bad.push(`${k}: ${items.slice(0, 4).join(', ')}${items.length > 4 ? ` (+${items.length - 4})` : ''}`);
  }
  return bad.join('; ');
}

function load(side, name) {
  const png = decodePng(readFileSync(join(out, side, `${name}.png`)));
  const facts = JSON.parse(readFileSync(join(out, side, `${name}.json`), 'utf8'));
  return { png, facts };
}

// ------------------------------------------------------------------------------------------------------------ boxes

/**
 * The visual box: the bounds of what differs from the page's ground inside the layout box and its margin, in dp, and
 * each corner's radius, read along the diagonal: a corner of radius r leaves r(1 − 1/√2) of ground on each axis before
 * its fill.
 */
function visual({ png, facts }, ground, density) {
  const region = grow(facts.layout, MARGIN);
  const [x0, y0, x1, y1] = pxRect(region, density, png);
  const g = lab(ground);
  const inked = (x, y) => de76(lab(rgbAt(png, x, y)), g) > 3;
  let [l, t, r, b] = [Infinity, Infinity, -1, -1];
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
    if (!inked(x, y)) continue;
    if (x < l) l = x; if (x > r) r = x; if (y < t) t = y; if (y > b) b = y;
  }
  if (r < 0) return null;
  // Where the diagonal crosses half coverage, between pixels: the edge's anti-aliasing places it to a fraction of a
  // pixel, where the first inked pixel alone is 1.4 dp of radius at a phone's density.
  const corner = (cx, cy, dx, dy) => {
    let n = 0;
    while (n < 200 && !inked(cx + dx * n, cy + dy * n)) n++;
    const fill = lab(rgbAt(png, cx + dx * (n + 3), cy + dy * (n + 3)));
    const full = de76(fill, g) || 1;
    const cover = (k) => Math.min(1, de76(lab(rgbAt(png, cx + dx * k, cy + dy * k)), g) / full);
    while (n < 200 && cover(n) < 0.5) n++;
    const before = n > 0 ? cover(n - 1) : 0, at = cover(n);
    const crossing = n - 1 + (0.5 - before) / Math.max(at - before, 1e-6);
    return Math.max(0, crossing + 0.5) / density / (1 - Math.SQRT1_2);
  };
  // Corners are a container's: a box mostly of ground (a bar the page's colour, a text alone) has none to read.
  let filled = 0;
  for (let y = t; y <= b; y++) for (let x = l; x <= r; x++) filled += inked(x, y);
  const container = filled / ((r - l + 1) * (b - t + 1)) > 0.6;
  return { x: l / density, y: t / density, w: (r - l + 1) / density, h: (b - t + 1) / density,
    corners: container ? [corner(l, t, 1, 1), corner(r, t, -1, 1), corner(r, b, -1, -1), corner(l, b, 1, -1)].map((c) => round(c)) : null };
}

const grow = (b, m) => ({ x: b.x - m, y: b.y - m, w: b.w + 2 * m, h: b.h + 2 * m });
const shift = (b, dx, dy) => ({ ...b, x: b.x + dx, y: b.y + dy });

function pxRect(b, density, png) {
  const c = (v, max) => Math.max(0, Math.min(max, Math.round(v * density)));
  return [c(b.x, png.width), c(b.y, png.height), c(b.x + b.w, png.width), c(b.y + b.h, png.height)];
}

/** A probe's point: dp from the visual box's corner, a negative from its far edge, "mid" its middle. */
function at(v, p) {
  const one = (q, start, size) => (q === 'mid' ? start + size / 2 : q < 0 ? start + size + q : start + q);
  return [one(p.x, v.x, v.w), one(p.y, v.y, v.h)];
}

/** The mean colour of the 3 × 3 pixels round a point given in dp. */
function sample(png, [x, y], density) {
  const cx = Math.floor(x * density), cy = Math.floor(y * density);
  const sum = [0, 0, 0];
  for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
    const c = rgbAt(png, cx + dx, cy + dy);
    for (let i = 0; i < 3; i++) sum[i] += c[i];
  }
  return sum.map((v) => v / 9);
}

/**
 * A text's ink and coverage in its box: the ground is the box's commonest colour (the container behind the text), the
 * ink the pixel farthest from it, and the coverage the summed distance of each pixel from the ground towards the ink,
 * in dp² of full ink.
 */
function ink(png, box, density) {
  const [x0, y0, x1, y1] = pxRect(box, density, png);
  const counts = new Map();
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
    const k = rgbAt(png, x, y).join(',');
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  const ground = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0].split(',').map(Number) ?? [0, 0, 0];
  const g = lab(ground);
  let far = ground, farD = 0;
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
    const c = rgbAt(png, x, y), d = de76(lab(c), g);
    if (d > farD) { far = c; farD = d; }
  }
  let coverage = 0;
  if (farD > 0) for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) coverage += Math.min(1, de76(lab(rgbAt(png, x, y)), g) / farD);
  return { ink: far, coverage: coverage / density / density };
}

// ----------------------------------------------------------------------------------------------------------- pixels

function pixels(ref, cand, texts, density, file) {
  const region = (side) => pxRect(grow(side.facts.layout, MARGIN), density, side.png);
  const [ax, ay, ax1, ay1] = region(ref);
  const [bx, by, bx1, by1] = region(cand);
  const w = Math.min(ax1 - ax, bx1 - bx), h = Math.min(ay1 - ay, by1 - by);
  const a = crop(ref.png, ax, ay, w, h), b = crop(cand.png, bx, by, w, h);
  const masked = new Uint8Array(w * h);
  for (const t of texts) for (const [box, ox, oy] of [[t.ref, ax, ay], [t.cand, bx, by]]) {
    const [x0, y0, x1, y1] = pxRect(box, density, { width: 1e9, height: 1e9 });
    for (let y = Math.max(0, y0 - oy); y < Math.min(h, y1 - oy); y++) for (let x = Math.max(0, x0 - ox); x < Math.min(w, x1 - ox); x++) masked[y * w + x] = 1;
  }
  const heat = new Uint8Array(w * h * 4);
  let over = 0, counted = 0;
  for (let i = 0; i < w * h; i++) {
    const ca = [a.data[i * 4], a.data[i * 4 + 1], a.data[i * 4 + 2]], cb = [b.data[i * 4], b.data[i * 4 + 1], b.data[i * 4 + 2]];
    const grey = (ca[0] + ca[1] + ca[2]) / 3 * 0.25 + 191;
    if (masked[i]) { heat.set([grey, grey, 255, 255], i * 4); continue; }
    counted++;
    const d = de2000(lab(ca), lab(cb));
    if (d > 5) { over++; heat.set([255, Math.max(0, 200 - d * 4), 0, 255], i * 4); } else heat.set([grey, grey, grey, 255], i * 4);
  }
  mkdirSync(join(out, 'crops'), { recursive: true });
  const names = ['a', 'b', 'heat'].map((k) => `crops/${file}.${k}.png`);
  writeFileSync(join(out, names[0]), encodePng(a));
  writeFileSync(join(out, names[1]), encodePng(b));
  writeFileSync(join(out, names[2]), encodePng({ width: w, height: h, data: heat }));
  return { share: counted ? over / counted : 0, heat: names[2], crops: names.slice(0, 2) };
}

// ------------------------------------------------------------------------------------------------------------ colour

const rgbAt = (png, x, y) => {
  const i = (Math.max(0, Math.min(png.height - 1, y)) * png.width + Math.max(0, Math.min(png.width - 1, x))) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
};
const argb = (v) => [(v >>> 16) & 255, (v >>> 8) & 255, v & 255];
const hex = (c) => '#' + c.map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
const round = (v, p = 2) => (v == null ? null : Math.round(v * 10 ** p) / 10 ** p);

/** sRGB (D65) to CIELAB. */
function lab([r, g, b]) {
  const lin = (c) => ((c /= 255) <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const [R, G, B] = [lin(r), lin(g), lin(b)];
  const X = (0.4124 * R + 0.3576 * G + 0.1805 * B) / 0.95047, Y = 0.2126 * R + 0.7152 * G + 0.0722 * B,
    Z = (0.0193 * R + 0.1192 * G + 0.9505 * B) / 1.08883;
  const f = (t) => (t > 216 / 24389 ? Math.cbrt(t) : (24389 / 27 * t + 16) / 116);
  return [116 * f(Y) - 16, 500 * (f(X) - f(Y)), 200 * (f(Y) - f(Z))];
}
const de76 = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

/** CIEDE2000 (Sharma, Wu and Dalal, 2005). */
function de2000([L1, a1, b1], [L2, a2, b2]) {
  const rad = Math.PI / 180, C1 = Math.hypot(a1, b1), C2 = Math.hypot(a2, b2), Cm = (C1 + C2) / 2;
  const G = 0.5 * (1 - Math.sqrt(Cm ** 7 / (Cm ** 7 + 25 ** 7)));
  const a1p = (1 + G) * a1, a2p = (1 + G) * a2, C1p = Math.hypot(a1p, b1), C2p = Math.hypot(a2p, b2);
  const h = (b, a) => (b === 0 && a === 0 ? 0 : (Math.atan2(b, a) / rad + 360) % 360);
  const h1 = h(b1, a1p), h2 = h(b2, a2p);
  const dL = L2 - L1, dC = C2p - C1p;
  let dh = C1p * C2p === 0 ? 0 : h2 - h1;
  if (dh > 180) dh -= 360; else if (dh < -180) dh += 360;
  const dH = 2 * Math.sqrt(C1p * C2p) * Math.sin((dh / 2) * rad);
  const Lm = (L1 + L2) / 2, Cmp = (C1p + C2p) / 2;
  let hm = h1 + h2;
  if (C1p * C2p !== 0) hm = Math.abs(h1 - h2) > 180 ? (h1 + h2 + (h1 + h2 < 360 ? 360 : -360)) / 2 : (h1 + h2) / 2;
  const T = 1 - 0.17 * Math.cos((hm - 30) * rad) + 0.24 * Math.cos(2 * hm * rad) + 0.32 * Math.cos((3 * hm + 6) * rad) - 0.2 * Math.cos((4 * hm - 63) * rad);
  const SL = 1 + (0.015 * (Lm - 50) ** 2) / Math.sqrt(20 + (Lm - 50) ** 2), SC = 1 + 0.045 * Cmp, SH = 1 + 0.015 * Cmp * T;
  const RT = -2 * Math.sqrt(Cmp ** 7 / (Cmp ** 7 + 25 ** 7)) * Math.sin(60 * Math.exp(-(((hm - 275) / 25) ** 2)) * rad);
  return Math.sqrt((dL / SL) ** 2 + (dC / SC) ** 2 + (dH / SH) ** 2 + RT * (dC / SC) * (dH / SH));
}

// ------------------------------------------------------------------------------------------------------------ report

function html(r) {
  const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
  const row = (c, label) => {
    const bad = summary(c);
    const p = c.layers.pixels;
    return `<section class="${bad ? 'bad' : 'good'}"><h3>${esc(label ?? c.name)}</h3>
<p>${bad ? esc(bad) : 'every layer within its limit'} · pixels ${(p.share * 100).toFixed(2)}%</p>
<div class="imgs"><figure><img src="${p.crops[0]}"><figcaption>${c.sides[0]}</figcaption></figure><figure><img src="${p.crops[1]}"><figcaption>${c.sides[1]}</figcaption></figure><figure><img src="${p.heat}"><figcaption>past ΔE 5 (text masked)</figcaption></figure></div>
<details><summary>layers</summary><pre>${esc(JSON.stringify(Object.fromEntries(Object.entries(c.layers).map(([k, l]) => [k, l.items ?? { share: l.share }])), null, 1))}</pre></details></section>`;
  };
  return `<!doctype html><meta charset="utf-8"><title>Material oracle</title>
<style>body{font:14px system-ui;margin:16px;background:#fff;color:#111}section{border-left:4px solid #2a2;padding:4px 12px;margin:12px 0}section.bad{border-color:#d33}.imgs{display:flex;gap:8px;flex-wrap:wrap}img{max-width:420px;border:1px solid #ccc}figure{margin:0}figcaption{font-size:12px;color:#555}pre{font-size:11px}</style>
<h1>Material oracle</h1><p>Limits: ${esc(JSON.stringify(r.limits))}. Compose against Exact reports and does not fail until calibration (LLP 1115.001.001 O2).</p>
<h2>Detection test</h2>${r.detection.map((d) => row(d.comparison, `${d.ok ? 'as expected' : 'NOT AS EXPECTED'} · ${d.kind} · ${d.name}`)).join('\n')}
<h2>Compose against Exact</h2>${r.comparisons.map((c) => row(c)).join('\n')}`;
}

main();
