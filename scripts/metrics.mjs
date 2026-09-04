#!/usr/bin/env node
/**
 * metrics — the startup and speed numbers that matter, in one run under 30 s
 * on a warm cache. Diagnostic, never blocking (rules/RULES.md §Loop shape:
 * run everything, block on almost nothing).
 *
 *   node scripts/metrics.mjs            table
 *   node scripts/metrics.mjs --json     one JSON object
 *   node scripts/metrics.mjs --rebuild  also time an app edit → wasm rebuild (the cold path)
 *   node scripts/metrics.mjs --long     also the macOS host: a warm build, a touch-one-line
 *                                       rebuild, and the app's boot phases (minutes, not seconds)
 *
 * Budgets are read from rules/RULES.md so they cannot drift from the prose.
 */
import { spawnSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { gzipSync } from 'node:zlib';
import { dirname, resolve } from 'node:path';
import { staticFile, webContentType } from '../host/web/serve.mjs';
import { macBinary, macHostBinary } from '../host/apple/build.mjs';

const t0 = Date.now();
const ROOT = resolve(new URL('..', import.meta.url).pathname);
const json = process.argv.includes('--json');
const rebuild = process.argv.includes('--rebuild');
const long = process.argv.includes('--long');
const rules = readFileSync(resolve(ROOT, 'rules/RULES.md'), 'utf8');
const budget = (label) => rules.match(new RegExp(`\\|\\s*${label}[^|]*\\|\\s*([^|\\n]+)`, 'i'))?.[1].trim() ?? '?';
const out = {};
// The live dev-loop session reuses one Chrome profile (a fresh profile's
// first launch can stall for seconds); the --dump-dom render below gets a
// fresh one each time (with a reused profile it waits out its whole
// timeout). Every server here sends no-store, so nothing is cached.
const profile = resolve(ROOT, 'target/exact-chrome-profile');
mkdirSync(profile, { recursive: true });
const step = (name, f) => { const t = Date.now(); const v = f(); out[`_${name}_s`] = (Date.now() - t) / 1000; return v; };

// 1. Native pipeline numbers (a release bin; warm cache builds in ~1 s).
step('native', () => {
  const r = spawnSync('cargo', ['run', '-q', '--release', '-p', 'caltrain-web', '--bin', 'metrics'], { cwd: ROOT, encoding: 'utf8' });
  if (r.status !== 0) { console.error(r.stderr); process.exit(1); }
  Object.assign(out, JSON.parse(r.stdout.trim().split('\n').pop()));
});

// 2. The wasm, raw and gzipped — always rebuilt (a warm build is ~1.5 s),
// so every number below is for the code as it is now.
step('wasm', () => {
  const dist = resolve(ROOT, 'host/web/dist');
  const b = spawnSync('node', [resolve(ROOT, 'host/web/build.mjs')], { cwd: ROOT, stdio: ['ignore', 'ignore', 'inherit'] });
  if (b.status !== 0) process.exit(b.status ?? 1);
  const wasm = readFileSync(resolve(dist, 'app.wasm'));
  out.wasm_bytes = wasm.length;
  out.wasm_gzip_bytes = gzipSync(wasm, { level: 9 }).length;
  const glue = readFileSync(resolve(dist, 'glue.js'));
  out.glue_bytes = glue.length;
  if (existsSync(resolve(dist, 'gpu_bg.wasm'))) {
    const g = readFileSync(resolve(dist, 'gpu_bg.wasm'));
    out.gpu_wasm_bytes = g.length;
    out.gpu_wasm_gzip_bytes = gzipSync(g, { level: 9 }).length;
    out.gpu_glue_bytes = readFileSync(resolve(dist, 'gpu.js')).length;
  }
});

// 3. Boot modules (the fifth check's count).
step('boot', () => {
  const r = spawnSync('node', [resolve(ROOT, 'scripts/boot.mjs')], { cwd: ROOT, encoding: 'utf8' });
  out.boot_modules = Number(/before first pixel: (\d+)/.exec(r.stdout)?.[1] ?? NaN);
  out.boot_ok = r.status === 0;
});

// 4. A real browser: script start → first frame in the DOM (and painted, when a compositor exists).
{
  const t = Date.now();
  const dist = resolve(ROOT, 'host/web/dist');
  const server = createServer((req, res) => {
    const found = staticFile(dist, req.url.split('?')[0]);
    if (!found) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { 'content-type': webContentType(found.route), 'cache-control': 'no-store' });
    res.end(readFileSync(found.path));
  });
  await new Promise((ok) => server.listen(0, '127.0.0.1', ok));
  const port = server.address().port;
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const render = () => new Promise((done) => {
    const fresh = mkdtempSync(resolve(tmpdir(), 'exact-metrics-'));
    const child = spawn(chrome, ['--headless=new', '--disable-gpu', `--user-data-dir=${fresh}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run', '--no-default-browser-check', '--timeout=4000', '--dump-dom', `http://127.0.0.1:${port}/`], { detached: true, stdio: ['ignore', 'pipe', 'ignore'] });
    let dom = '';
    child.stdout.on('data', (d) => { dom += d; });
    const timer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, 20000);
    child.on('exit', () => { clearTimeout(timer); try { process.kill(-child.pid, 'SIGKILL'); } catch {} rmSync(fresh, { recursive: true, force: true }); done(dom); });
  });
  if (existsSync(chrome)) {
    let dom = await render();
    if (!/data-boot-ms=/.test(dom)) dom = await render();
    out.browser_dom_ms = Number(/data-boot-ms="([\d.]+)"/.exec(dom)?.[1] ?? NaN);
    out.browser_paint_ms = Number(/data-paint-ms="([\d.]+)"/.exec(dom)?.[1] ?? NaN);
    out.browser_dom_bytes = dom.length;
  } else {
    out.browser_dom_ms = NaN;
    out.browser_note = 'no Chrome at $CHROME';
  }
  server.close();
  out._browser_s = (Date.now() - t) / 1000;
}

// 5. The dev loop: edit app.contract → the page shows it, through the
// resident driver (host/web/dev.mjs; no cargo build in the loop). The budget
// row "Dev restart, request to present" measured end to end: file saved →
// first frame of the new plan in the DOM.
{
  const t = Date.now();
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const port = 20000 + Math.floor(Math.random() * 20000);
  const dev = spawn('node', [resolve(ROOT, 'host/web/dev.mjs'), '--port', String(port)], { cwd: ROOT, stdio: ['ignore', 'pipe', 'ignore'], detached: true });
  let compilerPid = null;
  const lines = [];
  let waiters = [];
  let buf = '';
  dev.stdout.on('data', (d) => { buf += d; const parts = buf.split('\n'); buf = parts.pop(); for (const l of parts) { lines.push(l); compilerPid ??= /^compiler pid (\d+)/.exec(l)?.[1]; waiters = waiters.filter((w) => !w(l)); } });
  const until = (re, ms) => new Promise((ok) => { const timer = setTimeout(() => ok(null), ms); const w = (l) => { const m = re.exec(l); if (m) { clearTimeout(timer); ok(m); return true; } return false; }; waiters.push(w); });
  const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));
  const app = resolve(ROOT, 'apps/caltrain/app.contract');
  const original = readFileSync(app, 'utf8');
  let page = null;
  try {
    const ready = await until(/^plan ready/, 120000); // a cold build of the dev bin can take a while; warm is ~1 s
    if (ready && existsSync(chrome)) {
      // A plain headless session (not --dump-dom, which freezes the page
      // after load): it lives until killed, so the event stream and the
      // reload run as they would in a real tab.
      page = spawn(chrome, ['--headless=new', '--disable-gpu', `--user-data-dir=${profile}`, '--no-sandbox', '--disable-extensions', '--disable-background-networking', '--no-first-run', '--no-default-browser-check', `http://127.0.0.1:${port}/`], { detached: true, stdio: 'ignore' });
      const connected = await until(/^page connected/, 20000);
      if (!connected) out.reload_note = 'the page never subscribed';
      const planReady = until(/^edit → plan ready (\d+) ms \(compile ([\d.]+) ms, bake ([\d.]+) ms/, 6000);
      const reloaded = until(/^reloaded seq=\d+ total_ms=(\d+)/, 6000);
      writeFileSync(app, original.replace('text "Caltrain"', 'text "Caltrain ·"'));
      const [p, r] = await Promise.all([planReady, reloaded]);
      out.reload_plan_ms = p ? Number(p[1]) : NaN;
      out.reload_ms = r ? Number(r[1]) : NaN;
      if (!r) out.reload_note = lines.slice(-3).join(' | ');
    } else {
      out.reload_ms = NaN;
      out.reload_note = ready ? 'no Chrome at $CHROME' : 'dev driver did not come up: ' + lines.slice(-2).join(' | ');
    }
  } finally {
    writeFileSync(app, original);
    await sleep(300);
    if (page) { try { process.kill(-page.pid, 'SIGKILL'); } catch {} }
    // Both groups, explicitly: the driver's and its resident compiler's.
    try { process.kill(-dev.pid, 'SIGTERM'); } catch {}
    await sleep(200);
    try { process.kill(-dev.pid, 'SIGKILL'); } catch {}
    if (compilerPid) { try { process.kill(-Number(compilerPid), 'SIGKILL'); } catch {} }
  }
  out._reload_s = (Date.now() - t) / 1000;
}

// 6. Optional: the dev loop without the resident driver — touch app.contract, rebuild the wasm.
if (rebuild) {
  step('rebuild', () => {
    const app = resolve(ROOT, 'apps/caltrain/app.contract');
    const now = new Date();
    utimesSync(app, now, now);
    const t = Date.now();
    const r = spawnSync('node', [resolve(ROOT, 'host/web/build.mjs')], { cwd: ROOT, stdio: 'ignore' });
    out.rebuild_ms = r.status === 0 ? Date.now() - t : NaN;
  });
}

// 6. The macOS app's startup, when it has been built (`node host/apple/build.mjs`;
// --long builds it): exec → main (dyld), NSApplication, the window, the runner
// with layout and text measurement, the batch applied, the first paint.
const macBin = macBinary;
const macRun = () => spawnSync(macBin, [], { cwd: ROOT, encoding: 'utf8', env: { ...process.env, EXACT_SMOKE: '1' }, timeout: 20000 });
const macParse = (o) => {
  out.macos_boot_ms = Number(/^boot ([\d.]+) ms/m.exec(o)?.[1] ?? NaN);
  const s = /startup: exec→main ([\d.?]+) ms; main→NSApplication ([\d.]+) ms; →window ([\d.]+) ms/.exec(o);
  if (s) { out.macos_exec_ms = Number(s[1]); out.macos_nsapp_ms = Number(s[2]); out.macos_window_ms = Number(s[3]); }
  const p = /runner\+layout ([\d.]+) ms of which (\d+) text measurements \((\d+) cached\) ([\d.]+) ms in CoreText; apply ([\d.]+) ms/.exec(o);
  if (p) { out.macos_runner_ms = Number(p[1]); out.macos_measurements = Number(p[2]); out.macos_measure_hits = Number(p[3]); out.macos_measure_ms = Number(p[4]); out.macos_apply_ms = Number(p[5]); }
  out.macos_paint_ms = Number(/^painted ([\d.]+) ms/m.exec(o)?.[1] ?? NaN);
  out.macos_gpu_ms = Number(/^gpu: module loaded in ([\d.]+) ms/m.exec(o)?.[1] ?? NaN);
  out.macos_web_loaded = /^web: module loaded/m.test(o);
  out.macos_views = Number(/; (\d+) views/.exec(o)?.[1] ?? NaN);
  const stampOf = (line, label) => Number(new RegExp(`${label.replace(/[.()]/g, '\\$&')} ([\\d.]+)`).exec(line)?.[1] ?? NaN);
  const st = /^stamps: (.*)$/m.exec(o)?.[1] ?? '';
  out.macos_finish_launching_ms = stampOf(st, 'didFinishLaunching');
  out.macos_first_frame_ms = stampOf(st, 'first frame applied');
};
// The platform floor: an empty AppKit app with the same stamps, built once.
const floorBin = resolve(ROOT, 'target/exact-floor');
const floorRun = () => {
  if (!existsSync(floorBin)) spawnSync('swiftc', ['-O', '-o', floorBin, resolve(ROOT, 'host/apple/macos/floor.swift')], { stdio: 'ignore' });
  if (!existsSync(floorBin)) return;
  spawnSync(floorBin, [], { encoding: 'utf8', timeout: 10000 }); // warm up
  const line = /^floor: (.*)$/m.exec(spawnSync(floorBin, [], { encoding: 'utf8', timeout: 10000 }).stdout ?? '')?.[1] ?? '';
  const stampOf = (label) => Number(new RegExp(`${label.replace(/[.()]/g, '\\$&')} ([\\d.]+)`).exec(line)?.[1] ?? NaN);
  out.floor_nsapp_ms = stampOf('NSApplication.shared');
  out.floor_window_ms = stampOf('NSWindow') - stampOf('NSScrollView');
  out.floor_finish_launching_ms = stampOf('didFinishLaunching') - stampOf('activate');
  out.floor_draw_ms = stampOf('first draw');
};
step('macos-boot', () => {
  if (!existsSync(macBin)) { out.macos_boot_ms = NaN; out.macos_note = 'not built (node host/apple/build.mjs)'; return; }
  macRun(); // the first launch of a fresh binary is a cold outlier: warm up, report the second
  macParse(macRun().stdout ?? '');
  floorRun();
});

// 7. Long: the macOS host's builds — a warm build (cargo release staticlib +
// swift), then the budget row "touch one line, rebuild that crate" for the
// host crate — and the startup again on the fresh binary.
if (long) {
  step('macos', () => {
    const build = () => {
      const t = Date.now();
      const r = spawnSync('node', [resolve(ROOT, 'host/apple/build.mjs')], { cwd: ROOT, encoding: 'utf8' });
      const m = /cargo ([\d.]+) s, swift ([\d.]+) s/.exec(r.stdout ?? '');
      return { ok: r.status === 0, total_s: (Date.now() - t) / 1000, cargo_s: m ? Number(m[1]) : NaN, swift_s: m ? Number(m[2]) : NaN };
    };
    const warm = build();
    out.macos_build_s = warm.ok ? warm.total_s : NaN;
    out.macos_build_cargo_s = warm.cargo_s;
    out.macos_build_swift_s = warm.swift_s;
    const src = resolve(ROOT, 'host/apple/src/host.rs');
    const now = new Date();
    utimesSync(src, now, now);
    const touched = build();
    out.macos_touch_s = touched.ok ? touched.total_s : NaN;
    macRun();
    macParse(macRun().stdout ?? '');
  });
  // The linked delta (LLP 1031 D7): the sample host — a native app linking
  // the archive and ExactKit and nothing else — against the empty AppKit app
  // `floor.swift`, installed bytes and gzip apart; each optional artifact
  // (the GPU module, the web arm) reported beside it, never folded in.
  step('macos-link-delta', () => {
    const r = spawnSync('node', [resolve(ROOT, 'host/apple/build.mjs'), '--host'], { cwd: ROOT, encoding: 'utf8' });
    if (r.status !== 0 || !existsSync(macHostBinary) || !existsSync(floorBin)) { out.link_delta_bytes = NaN; return; }
    const size = (f) => statSync(f).size;
    const gz = (f) => gzipSync(readFileSync(f), { level: 9 }).length;
    const binDir = dirname(macHostBinary);
    out.floor_bytes = size(floorBin); out.floor_gzip_bytes = gz(floorBin);
    out.host_bytes = size(macHostBinary); out.host_gzip_bytes = gz(macHostBinary);
    out.macos_bytes = size(macBin); out.macos_gzip_bytes = gz(macBin);
    out.link_delta_bytes = out.host_bytes - out.floor_bytes;
    out.link_delta_gzip_bytes = out.host_gzip_bytes - out.floor_gzip_bytes;
    for (const [key, name] of [['gpu_module', 'libexact_gpu.dylib'], ['web_module', 'libexact_web.dylib']]) {
      const f = resolve(binDir, name);
      out[`${key}_bytes`] = existsSync(f) ? size(f) : NaN;
      out[`${key}_gzip_bytes`] = existsSync(f) ? gz(f) : NaN;
    }
  });
}

// Keep the wall-clock observation whole: it is what a user sees. AppKit's
// empty-window floor is a separate experiment, not a subtraction that can
// make a slow launch look fast. The portion Exact can trade against its cold
// start budget is the directly stamped runner/layout + presenter application.
out.macos_total_ms = out.macos_exec_ms + out.macos_paint_ms;
out.macos_framework_ms = out.macos_runner_ms + out.macos_apply_ms;
out.total_s = (Date.now() - t0) / 1000;

if (json) { console.log(JSON.stringify(out)); process.exit(0); }

const ms = (v) => (Number.isFinite(v) ? `${v.toFixed(v < 10 ? 2 : 1)} ms` : 'n/a');
const kib = (v) => `${(v / 1024).toFixed(0)} KiB`;
const grade = (v, label) => {
  const stated = budget(label);
  const ceiling = Number(/^([\d.]+)\s*ms\b/i.exec(stated)?.[1] ?? NaN);
  if (!Number.isFinite(v) || !Number.isFinite(ceiling)) return `budget ${stated}`;
  return `budget ${stated}; ${v <= ceiling ? 'within' : 'OVER'}`;
};
const rows = [
  ['compile app.contract → plan', ms(out.compile_ms), `${out.plan_bytes.toLocaleString()} B`],
  ['bake (one runner boot at build)', ms(out.bake_ms), `${out.baked_bytes.toLocaleString()} B baked`],
  ['decode + validate the plan', ms(out.decode_ms), ''],
  ['runner boot → first frame', ms(out.boot_ms), `${out.nodes} nodes, ${out.text_nodes} text`],
  ['layout 390×844 (Taffy)', ms(out.layout_ms), ''],
  ['update: screen swap (press)', ms(out.update_ms), `budget ${budget('Dev restart')}`],
  ['tick: advance 1 s', ms(out.tick_ms), ''],
  ['web host first batch', ms(out.web_boot_ms), `${kib(out.web_first_batch_bytes)} JSON`],
  ['web host update batch', ms(out.web_update_ms), `${kib(out.web_update_batch_bytes)} JSON`],
  ['wasm (web profile + wasm-opt)', kib(out.wasm_bytes), `${kib(out.wasm_gzip_bytes)} gzip; glue ${kib(out.glue_bytes)}`],
  ['GPU module (web, on demand)', Number.isFinite(out.gpu_wasm_bytes) ? kib(out.gpu_wasm_bytes) : 'n/a', Number.isFinite(out.gpu_wasm_bytes) ? `${kib(out.gpu_wasm_gzip_bytes)} gzip; glue ${kib(out.gpu_glue_bytes)}; after first paint` : ''],
  ['browser: script → DOM', ms(out.browser_dom_ms), `budget ${budget('Cold start')}`],
  ['browser: → painted', ms(out.browser_paint_ms), Number.isFinite(out.browser_paint_ms) ? '' : 'headless has no compositor frame'],
  ['boot modules before first pixel', `${out.boot_modules}`, `budget: ${budget('App JS executed')} app JS; ${out.boot_ok ? 'ok' : 'VIOLATION'}`],
  ['edit → present (resident dev loop)', ms(out.reload_ms), Number.isFinite(out.reload_ms) ? `plan ready ${ms(out.reload_plan_ms)} after save; budget ${budget('Dev restart')}` : out.reload_note ?? ''],
  ['macOS: exec → first paint (raw)', ms(out.macos_total_ms), Number.isFinite(out.macos_paint_ms) ? `${out.macos_views} views; empty AppKit main → draw ${ms(out.floor_draw_ms)}` : out.macos_note ?? ''],
];
if (Number.isFinite(out.macos_paint_ms)) rows.push(
  ['  Exact: runner → NSViews', ms(out.macos_framework_ms), grade(out.macos_framework_ms, 'Cold start')],
  ['  ours: runner + layout', ms(out.macos_runner_ms), `${out.macos_measurements} text measurements (${out.macos_measure_hits} cached), ${ms(out.macos_measure_ms)} in CoreText`],
  ['  ours: batch → NSViews', ms(out.macos_apply_ms), ''],
  ['  AppKit: exec → main', ms(out.macos_exec_ms), 'dyld, the Swift runtime'],
  ['  AppKit: NSApplication', ms(out.macos_nsapp_ms), `floor ${ms(out.floor_nsapp_ms)} (waits on the window server)`],
  ['  AppKit: NSWindow', ms(out.macos_window_ms), `floor ${ms(out.floor_window_ms)} (NSThemeFrame, a dlopen)`],
  ['  AppKit: run → didFinishLaunching', ms(out.macos_finish_launching_ms - out.macos_first_frame_ms), `floor ${ms(out.floor_finish_launching_ms)} (Dock registration ≈ 65 ms of it)`],
  ['  main → first paint', ms(out.macos_paint_ms), `floor ${ms(out.floor_draw_ms)}: an empty window on this machine`],
  ['  GPU module (dlopen + device)', ms(out.macos_gpu_ms), 'after first paint, first canvas'],
  ['  web arm (dlopen)', out.macos_web_loaded ? 'loaded' : 'not loaded', out.macos_web_loaded ? 'VIOLATION: first screen has no iframe' : 'first iframe commit only'],
);
if (rebuild) rows.push(['edit → wasm rebuilt (no driver)', ms(out.rebuild_ms), 'the cold path: cargo build of the app crate']);
if (long) {
  const s = (v) => (Number.isFinite(v) ? `${v.toFixed(1)} s` : 'n/a');
  rows.push(['macOS: warm build', s(out.macos_build_s), `cargo ${s(out.macos_build_cargo_s)} · swift ${s(out.macos_build_swift_s)}; budget ${budget('Full build')}`]);
  const mib = (v) => (Number.isFinite(v) ? `${(v / 1048576).toFixed(2)} MB` : 'n/a');
  rows.push(
    ['macOS: link delta (sample host − floor)', mib(out.link_delta_bytes), Number.isFinite(out.link_delta_bytes) ? `${mib(out.link_delta_gzip_bytes)} gzip; host ${mib(out.host_bytes)}, floor ${mib(out.floor_bytes)}; the archive + ExactKit, nothing optional (LLP 1031 D7)` : 'not measured (the sample host or the floor did not build)'],
    ['  optional: GPU module (dlopen)', mib(out.gpu_module_bytes), Number.isFinite(out.gpu_module_bytes) ? `${mib(out.gpu_module_gzip_bytes)} gzip; paid at the first canvas` : 'no GPU crate'],
    ['  optional: web arm (dlopen)', mib(out.web_module_bytes), Number.isFinite(out.web_module_bytes) ? `${mib(out.web_module_gzip_bytes)} gzip; paid at the first iframe` : 'n/a'],
  );
  rows.push(['macOS: touch one line, rebuild', s(out.macos_touch_s), `host/apple/src/host.rs; budget ${budget('Touch one line')}`]);
}
console.log(`exact2 metrics — ${new Date().toISOString().slice(0, 19)}Z, warm cache, p50 where repeated`);
for (const [k, v, note] of rows) console.log(`  ${k.padEnd(34)} ${v.padStart(11)}   ${note}`);
console.log(`  ${'total'.padEnd(34)} ${`${out.total_s.toFixed(1)} s`.padStart(11)}   native ${out._native_s.toFixed(1)} s · wasm ${out._wasm_s.toFixed(1)} s · browser ${out._browser_s.toFixed(1)} s · dev loop ${out._reload_s.toFixed(1)} s · macOS boot ${out['_macos-boot_s'].toFixed(1)} s${rebuild ? ` · rebuild ${out._rebuild_s.toFixed(1)} s` : ''}${long ? ` · macOS ${out._macos_s.toFixed(1)} s` : ''}`);
