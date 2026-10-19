// The Material oracle (LLP 1115.001.001 D5): each specimen drawn by Compose Material 3 (the reference, `:reference`)
// and by Exact (the specimen door, `apps/material-specimens`), compared in the layers that need no phone: geometry,
// colour at probes, text's appearance, and pixels. Then the detection test (§3 O1): each specimen against itself
// passes, and each planted defect fails.
//
//   bun host/android/reference/oracle.mjs [--out <dir>] [--compare] [--phone <serial>]
//
// Without --compare it draws both sides first: the reference through Gradle (JAVA_HOME a JDK 21, ANDROID_HOME the
// SDK) and Exact through Cargo, given the reference's own fonts (Robolectric's, from Gradle's cache). It writes
// report.json and report.html into the out directory (default: host/android/reference/build/oracle) and exits 1
// when the detection test fails. The comparison of the two sides reports and does not fail: no component is matched
// yet (D7). With --phone it then calibrates on the adb device of that serial (D4; `phone` below).
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
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

/**
 * The thresholds, from O2's calibration on a Pixel (LLP 1115.001.001 §3): each above the largest difference between
 * two renders of one side (Compose on the JVM and on the phone; Exact headless and on the reader), and below the
 * smallest planted defect.
 */
const LIMIT = {
  geometry: 0.5, // dp, any box edge or corner radius (floors 0.16 and 0.41; defects from 1.6)
  probe: 2, // ΔE2000 (floors 1.25 and 1.06; defects from 15)
  ink: 2, // ΔE2000, a text's ink (floors 0.50 and 0.52; defects from 16)
  coverage: 0.15, // a text's ink, as a share of the larger (floors 0.013 and 0.068, the latter two Robotos)
  pixels: 0.02, // share of a crop's pixels past ΔE2000 5, text masked (floors 0.005 and 0.010)
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
  console.log(`Compose against Exact: ${failing.length} of ${report.comparisons.length} specimens differ`);
  for (const c of failing) console.log(`  ${c.name}: ${summary(c)}`);
  console.log(`report: ${relative(process.cwd(), join(out, 'report.html'))}`);
  if (option('--phone')) phone(option('--phone'));
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

/** `in_` is where the two sides' pictures are and the colours they were drawn in: the build box's, or a phone pair's. */
function compare(name, specimen, scheme, [refSide, candSide], candName = name, in_ = { dir: out, schemes }) {
  const ref = load(in_.dir, refSide, name);
  const cand = load(in_.dir, candSide, candName);
  const density = manifest.density;
  const ground = argb(in_.schemes[scheme].surface);
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
  const px = pixels(ref, cand, texts, density, `${refSide}-${candSide}.${candName}`, in_.dir);
  layers.pixels = { ok: px.share <= LIMIT.pixels, share: round(px.share, 4), heat: px.heat, crops: px.crops };
  return { name: candName === name ? name : `${name} ← ${candName}`, sides: [refSide, candSide], layers };
}

function failed(c) {
  return Object.values(c.layers).some((l) => !l.ok);
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

function load(dir, side, name) {
  const png = decodePng(readFileSync(join(dir, side, `${name}.png`)));
  const facts = JSON.parse(readFileSync(join(dir, side, `${name}.json`), 'utf8'));
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

function pixels(ref, cand, texts, density, file, dir) {
  // Each side's crop starts at its own layout box less the margin, and both keep only what is inside both pictures,
  // so that a box near one picture's edge (a top bar under a status bar on one side only) is cut alike on both.
  const start = (side) => [Math.round((side.facts.layout.x - MARGIN) * density), Math.round((side.facts.layout.y - MARGIN) * density)];
  const size = (side) => [Math.round((side.facts.layout.w + 2 * MARGIN) * density), Math.round((side.facts.layout.h + 2 * MARGIN) * density)];
  const [[ax0, ay0], [bx0, by0]] = [start(ref), start(cand)];
  const [[aw, ah], [bw, bh]] = [size(ref), size(cand)];
  const x0 = Math.max(0, -ax0, -bx0), y0 = Math.max(0, -ay0, -by0);
  const x1 = Math.min(aw, bw, ref.png.width - ax0, cand.png.width - bx0);
  const y1 = Math.min(ah, bh, ref.png.height - ay0, cand.png.height - by0);
  const [ax, ay, bx, by, w, h] = [ax0 + x0, ay0 + y0, bx0 + x0, by0 + y0, Math.max(0, x1 - x0), Math.max(0, y1 - y0)];
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
  mkdirSync(join(dir, 'crops'), { recursive: true });
  const names = ['a', 'b', 'heat'].map((k) => `crops/${file}.${k}.png`);
  writeFileSync(join(dir, names[0]), encodePng(a));
  writeFileSync(join(dir, names[1]), encodePng(b));
  writeFileSync(join(dir, names[2]), encodePng({ width: w, height: h, data: heat }));
  return { share: counted ? over / counted : 0, heat: names[2], crops: names.slice(0, 2) };
}

// ------------------------------------------------------------------------------------------------------------ phone

/**
 * D4's calibration on a phone, after the build box's run: each specimen drawn by the catalogue (`:reference`'s debug
 * APK) in Material's baseline scheme, twice, and in the phone's dynamic one; and by the specimen app on the reader
 * (`agent-android.mjs --reader`, the specimen's plan pushed), twice, in the phone's colours, which the door is then
 * told for a headless drawing of its own. Light and dark, by the phone's night mode, which is put back. Five pairs
 * are compared, each layer's largest difference over them written to phone/floors.json and printed: Compose on the
 * JVM against the phone, and its repeat; Exact headless against the reader, and its repeat; and Compose against
 * Exact, both on the phone. The phone renders name no text boxes: each is compared at its side's headless ones,
 * placed where the phone put the specimen (the catalogue's whole screen; the reader's page, below the status bar the
 * phone reports, its bars where its state has them).
 */
function phone(serial) {
  const dir = join(out, 'phone');
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(join(dir, 'plans'), { recursive: true });
  const env = { ...process.env, ANDROID_SERIAL: serial };
  const sdk = env.ANDROID_HOME ?? join(homedir(), 'Android/Sdk');
  const tool = existsSync(join(sdk, 'platform-tools/adb')) ? join(sdk, 'platform-tools/adb') : 'adb';
  const adb = (...a) => {
    const r = spawnSync(tool, ['-s', serial, ...a], { maxBuffer: 1 << 28 });
    if (r.status !== 0) throw new Error(`adb ${a.join(' ')}: ${r.stderr}`);
    return r.stdout;
  };
  const text = (...a) => adb(...a).toString('utf8');
  run('./gradlew', [':reference:assembleDebug', '-q'], join(root, 'host/android'));
  adb('install', '-r', join(root, 'host/android/reference/build/outputs/apk/debug/reference-debug.apk'));
  run('bun', ['scripts/agent-android.mjs', 'build', 'material-specimens', '--apk'], root, env);
  const specimens = join(root, 'apps/material-specimens/specimens');
  for (const s of manifest.specimens) {
    run('cargo', ['run', '-q', '-p', 'contract', '--', 'build', join(specimens, `${s.id}.contract`), '-o', join(dir, 'plans', `${s.id}.plan`)], root);
  }
  // Where the reader's page is: below the status bar and above the navigation bar, as the phone reports them.
  const windows = text('shell', 'dumpsys', 'window');
  const bar = (type) => windows.match(new RegExp(String.raw`type=${type} frame=\[(\d+),(\d+)\]\[(\d+),(\d+)\]`))?.slice(1).map(Number);
  const [status, navigation] = [bar('statusBars'), bar('navigationBars')];
  if (!status || !navigation) throw new Error('dumpsys window names no status or navigation bar');
  const night = text('shell', 'cmd', 'uimode', 'night').match(/Night mode: (\w+)/)?.[1] ?? 'no';
  const dynamic = {};
  const wait = (done, what) => {
    for (const end = Date.now() + 30000; Date.now() < end; Bun.sleepSync(100)) if (done()) return;
    throw new Error(`${what}: not in 30 s`);
  };
  // The catalogue: one specimen, `variant` the scheme it is told, a picture once it says it has settled.
  const catalogue = (id, variant, file) => {
    const nonce = `${id}.${variant}.${Date.now()}`;
    adb('logcat', '-c');
    adb('shell', 'am', 'force-stop', 'dev.exact.reference');
    adb('shell', 'am', 'start', '-W', '-n', 'dev.exact.reference/.Catalogue', '--es', 'id', id, '--es', 'scheme', variant, '--es', 'nonce', nonce);
    let log = '';
    wait(() => (log = text('logcat', '-d', '-s', 'reference:I')).includes(`drawn ${nonce}`), `the catalogue's ${id}`);
    writeFileSync(file, adb('exec-out', 'screencap', '-p'));
    // The phone's dynamic schemes, which each launch logs, for the door.
    for (const [, which, roles] of log.matchAll(/dynamic (light|dark) (\S+)/g)) {
      dynamic[which] = Object.fromEntries(roles.split(',').map((kv) => kv.split('=')).map(([k, v]) => [k, Number(v)]));
    }
  };
  // The reader: the specimen's plan, pushed it if pushes, focused if focused, settled, its boxes and a picture.
  const reader = (s, file) => {
    const source = readFileSync(join(specimens, `${s.id}.contract`), 'utf8');
    const ops = [...(source.includes('testId="push"') ? ['tap push'] : []), ...(s.id.endsWith('-focused') ? ['tap specimen'] : []),
      'clock settle', 'tree', 'layout', 'state', `screenshot ${file}`];
    // A drive whose app never answered is tried once more: the reader is sometimes slow to open its socket.
    let r, replies;
    for (let tries = 0; tries < 2; tries++) {
      r = spawnSync('bun', ['scripts/agent.mjs', 'android', '--reader', '--app', 'material-specimens', '--plan',
        join(dir, 'plans', `${s.id}.plan`), '--json', ...ops], { cwd: root, env, encoding: 'utf8', maxBuffer: 1 << 28 });
      replies = r.stdout.split('\n').filter((l) => l.startsWith('{')).map((l) => JSON.parse(l));
      if (r.status === 0 && !replies.some((x) => x.error)) break;
    }
    if (r.status !== 0 || replies.some((x) => x.error)) throw new Error(`the reader's ${s.id}: ${r.stderr.slice(-800)}`);
    const tree = replies.find((x) => x.nodes && x.roots), layout = replies.find((x) => x.viewport), state = replies.find((x) => x.bars);
    const node = tree.nodes.find((n) => n.props?.testId === 'specimen');
    return { box: node && layout.nodes.find((n) => n.id === node.id), bars: state.bars };
  };
  const captured = {};
  try {
    adb('logcat', '-c');
    for (const scheme of manifest.schemes) {
      adb('shell', 'cmd', 'uimode', 'night', scheme === 'dark' ? 'yes' : 'no');
      adb('shell', 'input', 'keyevent', 'KEYCODE_WAKEUP');
      adb('shell', 'wm', 'dismiss-keyguard');
      for (const s of manifest.specimens) {
        const name = `${s.id}.${scheme}`;
        console.log(`phone: ${name}`);
        for (const [side, variant] of [['compose', scheme], ['compose-again', scheme], ['compose-dynamic', `dynamic-${scheme}`]]) {
          mkdirSync(join(dir, side), { recursive: true });
          catalogue(s.id, variant, join(dir, side, `${name}.png`));
        }
        for (const side of ['reader', 'reader-again']) {
          mkdirSync(join(dir, side), { recursive: true });
          captured[`${side}/${name}`] = reader(s, join(dir, side, `${name}.png`));
        }
      }
    }
  } finally {
    adb('shell', 'cmd', 'uimode', 'night', night);
    adb('shell', 'am', 'force-stop', 'dev.exact.reference');
  }
  // What the reader reported of each capture: the specimen's box in its page, and its bars.
  writeFileSync(join(dir, 'captured.json'), JSON.stringify(captured, null, 1));
  // Exact headless in the phone's dynamic schemes, at the phone's window.
  const headless = join(dir, 'headless');
  mkdirSync(join(headless, 'compose'), { recursive: true });
  writeFileSync(join(headless, 'schemes.json'), JSON.stringify(dynamic, null, 2));
  for (const f of readdirSync(join(dir, 'reader'))) cpSync(join(dir, 'reader', f), join(headless, 'compose', f));
  run('cargo', ['run', '-q', '-p', 'material-specimens-linux', '--', join(here, 'specimens.json'), specimens, headless,
    join(headless, 'exact')], root, { EXACT_FONTS: join(out, 'fonts'), EXACT_FONT: 'Roboto' });
  // Each side's facts on the phone. The catalogue's are the JVM's, a bar at the window's foot moved by what the JVM's
  // window lacks of the phone's; the reader's are its headless ones, moved to where it drew the specimen.
  const density = manifest.density;
  const facts = (side, name) => JSON.parse(readFileSync(join(side, `${name}.json`), 'utf8'));
  const moved = (f, dx, dy) => {
    const g = structuredClone(f);
    g.layout.x += dx; g.layout.y += dy;
    for (const t of g.texts ?? []) { t.box.x += dx; t.box.y += dy; for (const l of t.lines ?? []) { l.top += dy; l.bottom += dy; l.baseline += dy; } }
    return g;
  };
  const height = (file) => decodePng(readFileSync(file)).height / density;
  const composeFacts = (name) => {
    const f = facts(join(out, 'compose'), name);
    const jvm = height(join(out, 'compose', `${name}.png`)), screen = height(join(dir, 'compose', `${name}.png`));
    return Math.abs(f.layout.y + f.layout.h - jvm) < 1 ? moved(f, 0, screen - jvm) : f;
  };
  const readerFacts = (side, s, scheme) => {
    const name = `${s.id}.${scheme}`, f = facts(join(headless, 'exact'), name), at = captured[`${side}/${name}`];
    if (s.id.startsWith('top-app-bar')) return moved(f, 0, status[3] / density);
    if (s.id === 'navigation-bar') return moved(f, 0, at.bars.bottom.frame.y - f.layout.y);
    // Its box as the phone laid it out: where, and its size.
    const g = moved(f, at.box.x - f.layout.x, at.box.y + status[3] / density - f.layout.y);
    return { ...g, layout: { x: at.box.x, y: at.box.y + status[3] / density, w: at.box.w, h: at.box.h } };
  };
  const pairs = {
    'compose-jvm-phone': { schemes: JSON.parse(readFileSync(join(out, 'schemes.json'), 'utf8')),
      ref: (n) => [join(out, 'compose', `${n}.png`), facts(join(out, 'compose'), n)], cand: (n) => [join(dir, 'compose', `${n}.png`), composeFacts(n)] },
    'compose-phone-phone': { schemes: JSON.parse(readFileSync(join(out, 'schemes.json'), 'utf8')),
      ref: (n) => [join(dir, 'compose', `${n}.png`), composeFacts(n)], cand: (n) => [join(dir, 'compose-again', `${n}.png`), composeFacts(n)] },
    'exact-headless-reader': { schemes: dynamic,
      ref: (n) => [join(headless, 'exact', `${n}.png`), facts(join(headless, 'exact'), n)], cand: (n, s, sc) => [join(dir, 'reader', `${n}.png`), readerFacts('reader', s, sc)] },
    'exact-reader-reader': { schemes: dynamic,
      ref: (n, s, sc) => [join(dir, 'reader', `${n}.png`), readerFacts('reader', s, sc)], cand: (n, s, sc) => [join(dir, 'reader-again', `${n}.png`), readerFacts('reader-again', s, sc)] },
    'compose-exact-phone': { schemes: dynamic,
      ref: (n) => [join(dir, 'compose-dynamic', `${n}.png`), composeFacts(n)], cand: (n, s, sc) => [join(dir, 'reader', `${n}.png`), readerFacts('reader', s, sc)] },
  };
  const floors = {}, differ = {};
  for (const [pair, p] of Object.entries(pairs)) {
    const at = join(dir, 'pairs', pair);
    for (const side of ['compose', 'exact']) mkdirSync(join(at, side), { recursive: true });
    const comparisons = [];
    for (const s of manifest.specimens) for (const scheme of manifest.schemes) {
      const name = `${s.id}.${scheme}`;
      const [[refPng, refFacts], [candPng, candFacts]] = [p.ref(name, s, scheme), p.cand(name, s, scheme)];
      // Where each side's window put the component is no tell (a page under a status bar, or the whole screen): the
      // candidate is moved so that its layout box starts where the reference's does, and the rest is compared.
      const [dx, dy] = ['x', 'y'].map((k) => Math.round((refFacts.layout[k] - candFacts.layout[k]) * density));
      cpSync(refPng, join(at, 'compose', `${name}.png`));
      writeFileSync(join(at, 'compose', `${name}.json`), JSON.stringify(refFacts, null, 2));
      writeFileSync(join(at, 'exact', `${name}.png`), encodePng(shifted(decodePng(readFileSync(candPng)), dx, dy)));
      writeFileSync(join(at, 'exact', `${name}.json`), JSON.stringify(moved(candFacts, dx / density, dy / density), null, 2));
      comparisons.push(compare(name, s, scheme, ['compose', 'exact'], name, { dir: at, schemes: p.schemes }));
    }
    floors[pair] = largest(comparisons);
    differ[pair] = comparisons.filter(failed).map((c) => `${c.name}: ${summary(c)}`);
  }
  writeFileSync(join(dir, 'floors.json'), JSON.stringify({ serial, limits: LIMIT, floors, differ }, null, 2));
  const layers = [...new Set(Object.values(floors).flatMap(Object.keys))].sort();
  console.log(`\nphone ${serial}: each layer's largest difference (phone/floors.json)`);
  console.log(['layer'.padEnd(22), ...Object.keys(pairs).map((k) => k.padStart(22))].join(''));
  for (const l of layers) console.log([l.padEnd(22), ...Object.keys(pairs).map((k) => String(floors[k][l]?.value ?? '').padStart(22))].join(''));
  for (const [pair, list] of Object.entries(differ)) {
    console.log(`${pair}: ${list.length} of ${manifest.specimens.length * manifest.schemes.length} differ`);
    for (const d of list) console.log(`  ${d}`);
  }
}

/** `png` moved by whole pixels, what comes in from its edge its edge's colour. */
function shifted(png, dx, dy) {
  const { width: w, height: h } = png, data = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    const sy = Math.max(0, Math.min(h - 1, y - dy));
    for (let x = 0; x < w; x++) {
      const sx = Math.max(0, Math.min(w - 1, x - dx));
      data.set(png.data.subarray((sy * w + sx) * 4, (sy * w + sx) * 4 + 4), (y * w + x) * 4);
    }
  }
  return { width: w, height: h, data };
}

/** Each layer's largest difference over `comparisons`, and where: D4's floor of a pair of renders. */
function largest(comparisons) {
  const kind = (what) => (/^(layout|visual|corner)/.test(what) ? what.split(' ')[0] : / baseline /.test(what) ? 'baseline'
    : / lines$/.test(what) ? 'lines' : what.startsWith('text') ? 'text box' : what);
  const out = {};
  const put = (k, v, where) => { if (v != null && !Number.isNaN(v) && !(v <= out[k]?.value)) out[k] = { value: round(v, 3), where }; };
  for (const c of comparisons) {
    for (const g of c.layers.geometry.items) {
      if (typeof g.reference === 'number' && typeof g.candidate === 'number') put(`geometry ${kind(g.what)}`, Math.abs(g.reference - g.candidate), `${c.name} ${g.what}`);
      else if (g.note) put('missing', 1, `${c.name} ${g.what}`);
    }
    for (const p of c.layers.probes.items) put('probe ΔE', p.deltaE, `${c.name} ${p.what}`);
    for (const t of c.layers.text.items) {
      put('ink ΔE', t.deltaE, `${c.name} ${t.what}`);
      put('coverage', Math.abs(t.coverageReference - t.coverageCandidate) / Math.max(t.coverageReference, t.coverageCandidate, 1e-6), `${c.name} ${t.what}`);
    }
    put('pixels', c.layers.pixels.share, c.name);
  }
  return out;
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
<h1>Material oracle</h1><p>Limits: ${esc(JSON.stringify(r.limits))}, from O2's calibration (LLP 1115.001.001 §3). Compose against Exact reports and does not fail.</p>
<h2>Detection test</h2>${r.detection.map((d) => row(d.comparison, `${d.ok ? 'as expected' : 'NOT AS EXPECTED'} · ${d.kind} · ${d.name}`)).join('\n')}
<h2>Compose against Exact</h2>${r.comparisons.map((c) => row(c)).join('\n')}`;
}

main();
