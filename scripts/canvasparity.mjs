// The Canvas 2D parity smoke (LLP 1056 §4; Charlie approved the apparatus,
// §0.1): `apps/canvas-gallery` on every host against Chrome, through both
// oracles.
//
// 1. The API oracle: the recorder's rules (canvas/tests/cases.txt) run in
//    headless Chrome and must be Chrome's answers; and direct.html draws the
//    fixtures on Chrome's own context, with no recorder and no glue, which
//    the web's recorded path must equal.
// 2. The replay oracle: each native host's `fx-*` canvases against the web's,
//    at one pixel per point, registered within ±1 px, on white and on black
//    (the backdrop shows the bitmap's alpha), on page 3 at every step of the
//    sequences; Linux under both painters; and at native resolution against
//    direct.html drawn at the host's scale.
// 3. Caltrain's line map (LLP 1056 §8, the take): with the sky off, the
//    map's own pixels — the part of its canvas no child covers: background
//    and the train — on each native host against Chrome's.
// 4. Probes: every canvas drew and none is pending; the throwing fixture
//    reports its error at odd steps and kept what it drew; the resizing one's
//    generation is its step; the explicit bitmap is 40 × 25, stretched.
//
// Bands (provisional, §4): mean |Δ| ≤ 4/255 and ≤ 6% of pixels off by more
// than 32. Failing crops are written out in pairs.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { decodePng, encodePng } from './png.mjs';
import { boxes, register, shot } from './parity.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const MEAN = 4, OFF = 0.06, BAND = 32, SLOP = 1, STEPS = 4;
const BACKDROPS = { light: [255, 255, 255], dark: [0, 0, 0] };

/** The page sequence, captured the same way on every host. */
async function capture(open, host, dir, check, env) {
  const s = await open({ host, app: 'canvas-gallery', env });
  const label = env?.EXACT_PAINTER ? `${host}-${env.EXACT_PAINTER}` : host;
  const out = {}, natives = {};
  let dark = false;
  const take = async (key) => {
    await s.clock('settle');
    const view = await shot(s, host, dir, `${label}-${key}`);
    out[key] = boxes(view, { slop: SLOP });
    natives[key] = { k: view.k, crops: boxes(view, { slop: 0, native: true }) };
    return view;
  };
  const both = async (key) => {
    await take(`${key}-${dark ? 'dark' : 'light'}`);
    await s.tap('backdrop'); dark = !dark;
    await take(`${key}-${dark ? 'dark' : 'light'}`);
  };
  try {
    // Sequences first, so their first draw is step 0's.
    await s.tap('page-3');
    for (let step = 0; step < STEPS; step++) {
      if (step) await s.tap('step');
      await both(`page-3@${step}`);
      await probe(s, host, step, check);
    }
    for (const page of ['page-1', 'page-2']) {
      await s.tap(page);
      await both(page);
      await probe(s, host, null, check);
    }
    return { out, natives };
  } finally {
    await s.close?.();
  }
}

/** `state` and `tree` against what the fixtures must have done (§4's probes). */
async function probe(s, host, step, check) {
  const [state, tree] = [await s.state(), await s.tree()];
  const ids = new Map(tree.nodes.filter((n) => n.props?.testId?.startsWith('fx-')).map((n) => [n.id, n.props.testId]));
  const records = (state.canvas ?? []).filter((c) => ids.has(c.view));
  check(records.length === ids.size, `${host}: ${ids.size} fx canvases, ${records.length} in state.canvas`);
  for (const c of records) {
    const id = ids.get(c.view);
    check(c.draws >= 1 && !c.pending && !c.refused, `${host} ${id}: draws ${c.draws}, pending ${c.pending}, refused ${c.refused}`);
    if (id === 'fx-throws') check(Boolean(c.error) === (step % 2 === 1), `${host} fx-throws at step ${step}: error ${JSON.stringify(c.error)}`);
    else check(!c.error, `${host} ${id}: ${c.error}`);
    if (id === 'fx-resize') check(c.generation === step, `${host} fx-resize at step ${step}: generation ${c.generation}`);
    if (id === 'fx-bitmap') check(c.width === 40 && c.height === 25 && c.stretch && c.scale === 1, `${host} fx-bitmap: ${c.width} × ${c.height} at ${c.scale}, stretch ${c.stretch}`);
  }
}

/** direct.html's fixtures at `scale`: `{ 'fx-x' or 'fx-x@step': RGBA }`. */
function direct(dir, scale, check) {
  const page = resolve(dir, `direct-${scale}`);
  mkdirSync(page, { recursive: true });
  copyFileSync(resolve(ROOT, 'apps/canvas-gallery/direct.html'), resolve(page, 'direct.html'));
  // fixtures.ts imports its types only; Bun strips them.
  const b = spawnSync('bun', ['build', resolve(ROOT, 'apps/canvas-gallery/fixtures.ts'), '--target', 'browser', '--format', 'esm', '--outfile', resolve(page, 'fixtures.js')], { encoding: 'utf8' });
  if (b.status !== 0) throw new Error(`bun build fixtures.ts: ${b.stderr}`);
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const profile = resolve(page, 'profile');
  const r = spawnSync(chrome, ['--headless=new', '--disable-gpu', '--allow-file-access-from-files', `--user-data-dir=${profile}`, '--virtual-time-budget=5000', '--dump-dom',
    `file://${resolve(page, 'direct.html')}?scale=${scale}&steps=${STEPS}`], { encoding: 'utf8', timeout: 120000, maxBuffer: 256 << 20 });
  rmSync(profile, { recursive: true, force: true });
  const text = r.stdout?.match(/<pre id="out">(.*)<\/pre>/s)?.[1];
  if (!text) throw new Error(`direct.html gave nothing: ${r.stderr?.slice(0, 400)}`);
  const json = JSON.parse(text.replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&'));
  // The throwing fixture throws at odd steps, on Chrome too.
  const expected = [...Array(STEPS).keys()].filter((i) => i % 2).map((i) => `throws step ${i}: IndexSizeError`);
  check(json.errors.every((e) => expected.some((x) => e.startsWith(x))) && json.errors.length === expected.length, `direct.html errors: ${JSON.stringify(json.errors)}`);
  const images = {};
  for (const [key, url] of Object.entries(json.images)) images[key] = decodePng(Buffer.from(url.split(',')[1], 'base64'));
  return images;
}

/** `img` (straight-alpha RGBA) over a solid backdrop, `pad` px of it around. */
function over(img, rgb, pad) {
  const width = img.width + 2 * pad, height = img.height + 2 * pad, data = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const o = (y * width + x) * 4, sx = x - pad, sy = y - pad;
    const inside = sx >= 0 && sy >= 0 && sx < img.width && sy < img.height, i = (sy * img.width + sx) * 4;
    const a = inside ? img.data[i + 3] / 255 : 0;
    for (let c = 0; c < 3; c++) data[o + c] = Math.round((inside ? img.data[i + c] : 0) * a + rgb[c] * (1 - a));
    data[o + 3] = 255;
  }
  return { width, height, data };
}

/** Caltrain's line map with the sky off: `{ crop, record }` of its canvas's
 * left part, where no child is drawn. The web's is served from its own
 * dist (`target/web-dist/caltrain`), built here if missing. */
async function caltrainMap(open, host, dir, check, env) {
  let webDist;
  if (host === 'web') {
    webDist = resolve(ROOT, 'target/web-dist/caltrain');
    if (!existsSync(resolve(webDist, '.exact-build.json'))) {
      mkdirSync(dirname(webDist), { recursive: true });
      const b = spawnSync('bun', [resolve(ROOT, 'host/web/build.mjs'), 'caltrain-web'], { cwd: ROOT, env: { ...process.env, EXACT_WEB_DIST: webDist }, encoding: 'utf8', stdio: 'inherit' });
      if (b.status !== 0) throw new Error('the Caltrain web build failed');
    }
  }
  const s = await open({ host, app: 'caltrain', env, ...(webDist ? { webDist } : {}) });
  try {
    await s.tap('sky-toggle');
    await s.clock('settle');
    const view = await shot(s, host, dir, `${env?.EXACT_PAINTER ? `${host}-${env.EXACT_PAINTER}` : host}-caltrain`);
    const map = boxes(view, { prefix: 'line-map', slop: SLOP })['line-map'];
    const state = await s.state(), tree = await s.tree();
    const id = tree.nodes.find((n) => n.props?.testId === 'line-map')?.id;
    const record = (state.canvas ?? []).find((c) => c.view === id);
    check(record && record.draws >= 1 && !record.error && !record.pending, `${host} Caltrain line-map: ${JSON.stringify(record)}`);
    // The station dots and names start at the centre column; left of it is
    // the map's background and the train alone. The map is as wide as the
    // screen, so the strip is fixed to the centre, where the train is drawn.
    return map && strip(map, Math.round(map.width / 2) - 45, 36);
  } finally {
    await s.close?.();
  }
}

function strip(img, x, w) {
  const data = new Uint8Array(w * img.height * 4);
  for (let y = 0; y < img.height; y++) data.set(img.data.subarray((y * img.width + x) * 4, (y * img.width + x + w) * 4), y * w * 4);
  return { width: w, height: img.height, data };
}

/** 1 and 2: the gallery through both oracles. */
async function gallery({ open, check, dir, record, runs }) {
  // 1. The API oracle: the rules, then the recorded path against the direct one.
  const cases = spawnSync('bun', [resolve(ROOT, 'canvas/tests/cases.mjs'), '--chrome'], { encoding: 'utf8' });
  check(cases.status === 0, `the recorder's rules disagree with Chrome: ${cases.stdout}${cases.stderr}`);
  console.log(`canvas parity: ${cases.stdout.trim()}`);
  const direct1 = direct(dir, 1, check);
  const web = await capture(open, 'web', dir, check);
  const keyed = (key) => {
    const page = key.replace(/-(light|dark)$/, '');
    return { backdrop: key.endsWith('dark') ? 'dark' : 'light', step: page.includes('@') ? page.split('@')[1] : null };
  };
  for (const [key, fixtures] of Object.entries(web.out)) {
    const { backdrop, step } = keyed(key);
    for (const [id, a] of Object.entries(fixtures)) {
      const d = direct1[step !== null ? `${id}@${step}` : id];
      if (d) record('direct', key, id, over(d, BACKDROPS[backdrop], SLOP), a);
    }
  }
  // 2. The replay oracle: every native host against the web, and at its
  // native resolution against direct.html drawn at its scale.
  const scales = new Map();
  for (const [host, env] of runs) {
    const label = env?.EXACT_PAINTER ? `${host}-${env.EXACT_PAINTER}` : host;
    let got;
    try { got = await capture(open, host, dir, check, env); } catch (error) { check(false, `${label}: ${error.message}`); continue; }
    for (const [key, fixtures] of Object.entries(web.out)) {
      for (const [id, a] of Object.entries(fixtures)) record(label, key, id, a, got.out[key]?.[id]);
    }
    const k = Object.values(got.natives)[0]?.k ?? 1;
    if (!scales.has(k)) scales.set(k, direct(dir, k, check));
    const directK = scales.get(k);
    for (const [key, { crops }] of Object.entries(got.natives)) {
      const { backdrop, step } = keyed(key);
      for (const [id, b] of Object.entries(crops)) {
        const d = directK[step !== null ? `${id}@${step}` : id];
        if (d && b) record(`${label}@${k}x`, key, id, over(d, BACKDROPS[backdrop], 0), b);
      }
    }
  }
}

/** Run the smoke; `check(ok, what)` records a failure. `only: 'caltrain'`
 * runs the Caltrain map alone. */
export async function canvasParity({ open, check, hosts, only }) {
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-canvas-parity-'));
  const rows = [];
  const record = (host, key, id, a, b, limits = { mean: MEAN, off: OFF }) => {
    if (!check(a && b && a.width === b.width && a.height === b.height, `${host} ${key} ${id}: no box to compare (off screen, missing or resized: ${a?.width}×${a?.height} vs ${b?.width}×${b?.height})`)) return;
    const best = register(a, b, { slop: SLOP, band: BAND });
    const ok = best.mean <= limits.mean && best.off <= limits.off;
    rows.push({ host, key, id, ...best, ok });
    if (!check(ok, `${host} ${key} ${id}: mean |Δ| ${best.mean.toFixed(2)}/255, ${(best.off * 100).toFixed(2)}% off by > ${BAND} (limits ${limits.mean}, ${limits.off * 100}%)`)) {
      const pair = resolve(dir, 'failed');
      mkdirSync(pair, { recursive: true });
      writeFileSync(resolve(pair, `${host}-${key}-${id}.png`), encodePng(b));
      writeFileSync(resolve(pair, `ref-${key}-${id}.png`), encodePng(a));
    }
  };
  const runs = hosts.flatMap((host) => (host === 'linux' ? [['linux', { EXACT_PAINTER: 'cpu' }], ['linux', { EXACT_PAINTER: 'gpu' }]] : [[host, undefined]]));
  if (only !== 'caltrain') await gallery({ open, check, dir, record, runs });
  // 3. Caltrain's line map against Chrome's.
  let chromeMap = null;
  try { chromeMap = await caltrainMap(open, 'web', dir, check); } catch (error) { check(false, `web Caltrain: ${error.message}`); }
  for (const [host, env] of runs) {
    const label = env?.EXACT_PAINTER ? `${host}-${env.EXACT_PAINTER}` : host;
    try { record(label, 'caltrain', 'line-map', chromeMap, await caltrainMap(open, host, dir, check, env)); } catch (error) { check(false, `${label} Caltrain: ${error.message}`); }
  }
  for (const r of rows.filter((r) => !r.ok || process.env.EXACT_PARITY_ALL || r.key === 'caltrain')) console.log(`canvas parity ${r.host.padEnd(12)} ${r.key.padEnd(18)} ${r.id.padEnd(16)} mean ${r.mean.toFixed(2).padStart(5)}  off ${(r.off * 100).toFixed(2).padStart(5)}%  shift ${r.dx},${r.dy}${r.ok ? '' : '  FAIL'}`);
  for (const host of [...new Set(rows.map((r) => r.host))]) {
    const mine = rows.filter((r) => r.host === host && r.key !== 'caltrain');
    if (!mine.length) continue;
    console.log(`canvas parity ${host}: ${mine.length} crops, mean |Δ| ${(mine.reduce((a, r) => a + r.mean, 0) / mine.length).toFixed(2)}/255, worst ${Math.max(...mine.map((r) => r.mean)).toFixed(2)}; pixels off by > ${BAND}: mean ${(100 * mine.reduce((a, r) => a + r.off, 0) / mine.length).toFixed(2)}%, worst ${(100 * Math.max(...mine.map((r) => r.off))).toFixed(2)}%`);
  }
  console.log(`canvas parity: crops in ${dir}`);
}
