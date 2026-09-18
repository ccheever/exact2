#!/usr/bin/env bun
// Live, headed Beacons diagnostic. stdout = JSONL; stderr = progress + table.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { loadavg, platform } from 'node:os';
import { extname, join, resolve, relative } from 'node:path';
import { gzipSync } from 'node:zlib';
import { pathToFileURL } from 'node:url';
import { edge, keys } from './probes/input.mjs';

const here = import.meta.dir, resultsDir = join(here, 'results');
const GODOT = process.env.GODOT ?? resolve(process.env.HOME, 'Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot');
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
// Adapters provide a page and the window.feel protocol; scheduling/analysis are shared.
export const adapters = {
  exact: { transport: 'web', variants: ['60hz'], hz: 60,
    root: resolve(here, '../games/beacons/dist'),
    page: '/index.html?feel=1', probe: resolve(here, 'probes/exact.mjs'),
    started: '!!document.querySelector("[data-gpu-input]")' },
  three: { transport: 'web', variants: ['shipped'], root: resolve(here, '../twins/three'),
    page: '/beacons/index.html?feel=1', ready: '!!window.beacons && !!window.feel',
    started: 'beacons.state().mode === "playing"' },
  godot: { transport: 'godot', variants: ['shipped', 'interpolation'] },
};
let interrupted = false, machineError;
export function cancel() { interrupted = true; }
const delay = ms => new Promise(resolve => setTimeout(resolve, Math.max(0, ms)));
function check() { if (machineError) throw machineError; if (interrupted) throw new Error('Interrupted; cleaning up owned processes'); }
async function until(predicate, ms, label) {
  const end = performance.now() + ms;
  while (performance.now() < end) { check(); const value = await predicate(); if (value) return value; await delay(50); }
  throw new Error(`Timeout: ${label}`);
}

export function script(seconds = 12) {
  const schedule = [];
  const key = (at_ms, code, down, trial = -1) => schedule.push({ at_ms, ...edge(code, down, trial) });
  key(0, 13, true); key(20, 13, false); // Play through the focused real button.
  key(500, 87, true); key(3000, 87, false);
  key(3000, 68, true); key(4500, 68, false);
  key(4500, 32, true); key(4520, 32, false);
  key(4520, 83, true); key(6020, 83, false);
  // One second idle completes the script at 7020 ms. Padding also lets the
  // exponential three.js deceleration reach a genuinely unchanged position.
  const start = Math.max(seconds * 1000, 10020);
  for (let trial = 0; trial < 20; trial++) {
    const at = start + trial * 3600, code = trial % 2 ? 83 : 87;
    key(at, code, true, trial); key(at + 100, code, false);
  }
  return { schedule, codes: Object.values(keys).map(([code]) => edge(code, false).code),
    duration_ms: start + 19 * 3600 + 500 };
}

export function quantile(values, q) {
  if (!values.length) return null;
  const a = [...values].sort((a, b) => a - b), p = (a.length - 1) * q, lo = Math.floor(p);
  return a[lo] + (a[Math.ceil(p)] - a[lo]) * (p - lo);
}
function distribution(a) {
  return { p50: quantile(a, .5), p95: quantile(a, .95), p99: quantile(a, .99), max: Math.max(...a) };
}
function motion(frames, offset) {
  const ds = [];
  for (let i = 1; i < frames.length; i++) ds.push(Math.hypot(...[0, 1, 2].map(k => frames[i][offset + k] - frames[i - 1][offset + k])));
  const mean = ds.reduce((a, b) => a + b, 0) / ds.length;
  const stddev = Math.sqrt(ds.reduce((a, d) => a + (d - mean) ** 2, 0) / ds.length);
  return { samples: ds.length, mean_displacement_m: mean, stddev_displacement_m: stddev,
    judder: mean > 0 ? stddev / mean : null, repeated_fraction: ds.filter(d => d === 0).length / ds.length };
}
function unpack(array, stride) {
  return Array.from({ length: array.length / stride }, (_, i) => array.slice(i * stride, (i + 1) * stride));
}
export function analyze(raw, plan) {
  if (raw.schema !== 1 || raw.stride !== 8 || raw.overflow || raw.events.length % 4 || raw.frames.length % 8
      || !raw.events.every(Number.isFinite)) throw new Error('Invalid/overflowed probe buffer');
  const events = unpack(raw.events, 4), allFrames = unpack(raw.frames, 8);
  if (events.length !== plan.schedule.length) throw new Error(`Expected ${plan.schedule.length} delivered events, got ${events.length}; extra/missing input invalidates this attempt (no deduplication)`);
  events.forEach((e, i) => {
    const wanted = edge(plan.schedule[i].code, plan.schedule[i].down, plan.schedule[i].trial);
    if (e[1] !== wanted.code || e[2] !== +wanted.down || e[3] !== wanted.trial) throw new Error(`Event ${i} differs from schedule: ${e}`);
  });
  // A redraw at an unchanged frame time (a resize, a swap) is the same presented
  // frame drawn again, not a frame interval: keep the first row of each time.
  const frames = allFrames.filter(f => f[1] >= events[0][0]).filter((f, i, all) => i === 0 || f[0] !== all[i - 1][0]);
  if (frames.length < 100 || frames.some(f => !f.every(Number.isFinite))) throw new Error('Missing/nonfinite frame samples');
  const intervals = frames.slice(1).map((f, i) => f[0] - frames[i][0]);
  if (intervals.some(x => x <= 0)) throw new Error('Nonmonotonic frame timestamps');
  const pacing = distribution(intervals), median = pacing.p50;
  const steady = frames.filter(f => f[1] >= events[2][0] + 1000 && f[1] < events[3][0]);
  const player = motion(steady, 2), camera = motion(steady, 5);
  if (player.samples < 10 || player.judder === null) throw new Error('No constant-velocity W motion measured');
  const latency = [];
  const same = (a, b) => a[2] === b[2] && a[3] === b[3] && a[4] === b[4];
  for (const e of events.filter(e => e[3] >= 0)) {
    const before = frames.filter(f => f[1] < e[0]), baseline = before.at(-1);
    const rest = before.filter(f => f[1] >= e[0] - 100);
    // Exact equality: a coasting player cannot generate a spurious fast response.
    const stationary = rest.length >= 3 && rest.every(f => same(f, baseline));
    const up = events.find(x => x[0] > e[0] && x[1] === e[1] && !x[2]);
    const first = frames.find(f => f[1] >= e[0] && f[1] < up[0] + 300 && !same(f, baseline));
    latency.push({ trial: e[3], delivered_ms: e[0], first_changed_frame_ms: first?.[1] ?? null,
      latency_ms: stationary && first ? first[1] - e[0] : null, stationary_before: stationary });
  }
  const valid = latency.filter(x => x.latency_ms !== null).map(x => x.latency_ms);
  const hitches = intervals.filter(x => x > median * 1.5).length;
  return { delivered_events: events.length, expected_events: plan.schedule.length, frames: frames.length, duration_ms: frames.at(-1)[1] - frames[0][1],
    refresh_interval_ms: median, observed_refresh_hz: 1000 / median,
    frame_ms: pacing, hitches, hitch_percent: hitches / frames.length * 100,
    player, camera, latency: { trials: latency, valid_trials: valid.length,
      median_ms: quantile(valid, .5), p95_ms: quantile(valid, .95),
      median_intervals: valid.length ? quantile(valid, .5) / median : null,
      p95_intervals: valid.length ? quantile(valid, .95) / median : null },
    input_schedule_error_ms: events.map((e, i) => e[0] - events[0][0] - plan.schedule[i].at_ms),
    w_hold_ms: events[3][0] - events[2][0] };
}

export function consoleState(root, hid, idleSeconds = 30) {
  const locked = root.match(/"IOConsoleLocked"\s*=\s*(Yes|No)/)?.[1];
  const ns = hid.match(/"HIDIdleTime"\s*=\s*(\d+)/)?.[1];
  if (!locked || !ns) throw new Error('Feel preflight: cannot read IOConsoleLocked / HIDIdleTime; refusing measurement');
  const idle = Number(ns) / 1e9;
  if (locked !== 'No') throw new Error('Feel preflight: display is locked; unlock it, then leave the console idle for 30 seconds');
  if (idle < idleSeconds) throw new Error(`Feel preflight: console active (HID idle ${idle.toFixed(1)} s; need ${idleSeconds} s). Leave keyboard/mouse untouched, then rerun`);
  return { locked: false, idle_seconds: idle };
}
export function preflight() {
  if (platform() !== 'darwin') throw new Error('Feel preflight requires macOS IOConsoleLocked / HIDIdleTime');
  const read = args => {
    const r = spawnSync('ioreg', args, { encoding: 'utf8', timeout: 5000, maxBuffer: 4 * 1024 * 1024 });
    if (r.status !== 0) throw new Error(`Feel preflight: ioreg failed: ${r.error?.message ?? r.stderr}`);
    return r.stdout;
  };
  return consoleState(read(['-n', 'Root', '-d1']), read(['-r', '-c', 'IOHIDSystem']));
}

// AppKit activation needs no Accessibility permission and targets the recorded PID.
function desktop(pid, activate = false) {
  if (platform() !== 'darwin') return { frontmost: null, display_refresh_hz: null };
  const code = `ObjC.import('AppKit'); ObjC.import('CoreGraphics');
    ${activate ? `$.NSRunningApplication.runningApplicationWithProcessIdentifier(${pid}).activateWithOptions(3);` : ''}
    var s=$.NSScreen.mainScreen, id=s.deviceDescription.objectForKey('NSScreenNumber').unsignedIntValue;
    JSON.stringify({frontmost_pid:$.NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier,
      display_refresh_hz:$.CGDisplayModeGetRefreshRate($.CGDisplayCopyDisplayMode(id)),
      display_pixels:[s.frame.size.width*s.backingScaleFactor,s.frame.size.height*s.backingScaleFactor]});`;
  const r = spawnSync('osascript', ['-l', 'JavaScript', '-e', code], { encoding: 'utf8', timeout: 5000 });
  if (r.status !== 0) throw new Error(`Cannot confirm macOS foreground: ${r.stderr}`);
  const result = JSON.parse(r.stdout); return { ...result, frontmost: result.frontmost_pid === pid };
}
function launch(executable, args) {
  const child = spawn(executable, args, { stdio: ['ignore', 'pipe', 'pipe'] });
  const owned = { child, pid: child.pid, log: '', done: false };
  child.stdout.on('data', d => { owned.log += d; }); child.stderr.on('data', d => { owned.log += d; });
  owned.exit = new Promise(resolve => {
    child.once('exit', (code, signal) => { owned.done = true; resolve({ code, signal }); });
    child.once('error', error => { owned.done = true; owned.log += error.stack; resolve({ error: error.message }); });
  });
  console.error(`Launched ${executable.includes('Chrome') ? 'Chrome' : 'Godot'} PID ${owned.pid}`);
  return owned;
}
async function stop(owned) {
  if (!owned) return;
  if (!owned.done) owned.child.kill('SIGKILL'); // Only our recorded process; always wait.
  await owned.exit;
}
function processAudit(pid, profile) {
  const ps = spawnSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' });
  if (ps.status !== 0) throw new Error('ps cleanup audit failed');
  const remaining = ps.stdout.split('\n').filter(line => {
    const match = line.trim().match(/^(\d+)\s+(.*)$/);
    return match && (+match[1] === pid || (profile && match[2].includes(profile)));
  });
  if (remaining.length) throw new Error(`Owned processes remain after exit:\n${remaining.join('\n')}`);
  console.error(`Exit confirmed; ps clear for PID ${pid}${profile ? ` and ${profile}` : ''}`);
}
async function connect(port) {
  const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const ws = new WebSocket(tabs.find(t => t.type === 'page').webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
  let seq = 0;
  const pending = new Map(), errors = [];
  ws.onmessage = event => {
    const m = JSON.parse(event.data), p = pending.get(m.id);
    if (p) { pending.delete(m.id); clearTimeout(p.timer); m.error ? p.reject(new Error(JSON.stringify(m.error))) : p.resolve(m.result); }
    if (m.method === 'Runtime.exceptionThrown') errors.push(m.params.exceptionDetails);
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++seq, timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 10000);
    pending.set(id, { resolve, reject, timer }); ws.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const r = await send('Runtime.evaluate', { expression, returnByValue: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  return { send, evaluate, errors, close() { ws.close(); for (const p of pending.values()) clearTimeout(p.timer); } };
}

export async function web(plan, temp, adapter) {
  const root = adapter.root, profile = join(temp, 'exact2-feel-chrome');
  mkdirSync(profile);
  const server = adapter.url ? null : Bun.serve({ port: 0, hostname: '127.0.0.1', async fetch(req) {
    const url = new URL(req.url);
    if (url.pathname === '/bench/probes/input.mjs' || url.pathname === '/__feel/input.mjs') return new Response(Bun.file(join(here, 'probes/input.mjs')), { headers: { 'content-type': 'text/javascript' } });
    if (adapter.probe && url.pathname === '/__feel/exact.mjs') return new Response(Bun.file(adapter.probe), { headers: { 'content-type': 'text/javascript' } });
    const path = resolve(root, '.' + decodeURIComponent(url.pathname));
    if (adapter.probe && url.pathname === '/index.html') return new Response(
      await Bun.file(path).text() + `<script type="module">import { install } from '/__feel/exact.mjs'; await install(${JSON.stringify(adapter.options ?? {}).replaceAll('<', '\\u003c')});</script>`,
      { headers: { 'content-type': 'text/html' } });
    if (!path.startsWith(root + '/')) return new Response('Forbidden', { status: 403 });
    const file = Bun.file(path);
    return await file.exists() ? new Response(file, { headers: { 'content-type': {
      '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json',
    }[extname(path)] ?? 'application/octet-stream' } }) : new Response('Missing', { status: 404 });
  } });
  let owned, cdp;
  try {
    owned = launch(CHROME, [`--user-data-dir=${profile}`, '--remote-debugging-port=0', '--no-first-run',
      '--no-default-browser-check', '--disable-background-networking', '--disable-component-update',
      '--window-size=1100,788', '--window-position=40,40', '--app=about:blank']);
    const port = await until(() => {
      if (owned.done) throw new Error(owned.log);
      try { return readFileSync(join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]; } catch { return false; }
    }, 20000, 'Chrome CDP port');
    cdp = await connect(port);
    await cdp.send('Runtime.enable'); await cdp.send('Page.enable');
    await cdp.send('Page.navigate', { url: adapter.url ?? `http://127.0.0.1:${server.port}${adapter.page}` });
    await until(async () => {
      if (cdp.errors.length) throw new Error(JSON.stringify(cdp.errors));
      return cdp.evaluate(adapter.ready ?? '!!window.feel');
    }, 20000, 'web probe load');
    await cdp.send('Page.bringToFront'); desktop(owned.pid, true);
    try {
      await until(async () => desktop(owned.pid).frontmost && await cdp.evaluate('document.hasFocus() && document.visibilityState === "visible"'), 5000, 'Chrome foreground');
    } catch (error) {
      throw new Error(`${error.message}: ${JSON.stringify(desktop(owned.pid))}; page=${JSON.stringify(await cdp.evaluate('({focus:document.hasFocus(),visibility:document.visibilityState})'))}`);
    }
    // Match the shipped Godot content size, without device or virtual-time emulation.
    const bounds = await cdp.send('Browser.getWindowForTarget');
    const size = await cdp.evaluate('({w:outerWidth-innerWidth+1100,h:outerHeight-innerHeight+760})');
    await cdp.send('Browser.setWindowBounds', { windowId: bounds.windowId, bounds: { width: size.w, height: size.h } });
    await delay(1500); // shader/startup warmup outside measurement
    const display = desktop(owned.pid), version = await cdp.send('Browser.getVersion');
    await cdp.evaluate(`feel.begin(${plan.duration_ms})`);
    const epoch = performance.now();
    const codes = Object.fromEntries(Object.entries(keys).map(([code, [vk, key]]) => [vk, [code, key]]));
    for (const item of plan.schedule) {
      await delay(epoch + item.at_ms - performance.now()); check();
      if (cdp.errors.length) throw new Error(JSON.stringify(cdp.errors));
      if (item.code === 87 && item.trial === -1 && adapter.started && !await cdp.evaluate(adapter.started)) throw new Error('Play did not activate');
      if (item.trial >= 0) await cdp.evaluate(`feel.arm(${item.trial})`);
      await cdp.send('Input.dispatchKeyEvent', { type: item.down ? 'keyDown' : 'keyUp',
        code: codes[item.code][0], key: codes[item.code][1], windowsVirtualKeyCode: item.code,
        ...(item.code === 13 && item.down ? { text: '\r', unmodifiedText: '\r' } : {}) });
    }
    await delay(epoch + plan.duration_ms - performance.now()); check();
    const raw = await cdp.evaluate('feel.end()'); // one bulk read, after sampling stops
    const endDisplay = desktop(owned.pid);
    if (cdp.errors.length) throw new Error(JSON.stringify(cdp.errors));
    return { raw, display, end_frontmost: endDisplay.frontmost, browser_version: version.product, pid: owned.pid };
  } finally {
    cdp?.close(); await stop(owned); server?.stop(true);
    if (owned) { await delay(250); processAudit(owned.pid, profile); }
  }
}

export async function godot(plan, variant, temp) {
  const output = join(temp, 'samples.json'), config = join(temp, 'schedule.json');
  writeFileSync(config, JSON.stringify({ ...plan, output }));
  const owned = launch(GODOT, ['--path', resolve(here, '../twins/godot/beacons'), '--position', '40,40', '--resolution', '2200x1520',
    '--log-file', join(temp, 'godot.log'), '--', '--feel', `--feel-config=${config}`,
    ...(variant === 'interpolation' ? ['--feel-interpolation'] : [])]);
  try {
    await until(() => { if (owned.done) throw new Error(owned.log); return existsSync(output + '.ready'); }, 20000, 'Godot probe ready');
    desktop(owned.pid, true);
    await until(() => desktop(owned.pid).frontmost, 2000, 'Godot foreground');
    const display = desktop(owned.pid);
    await until(() => owned.done, plan.duration_ms + 15000, 'Godot live run');
    const exit = await owned.exit;
    if (exit.code !== 0 || /SCRIPT ERROR/.test(owned.log)) throw new Error(JSON.stringify(exit) + '\n' + owned.log);
    const raw = JSON.parse(readFileSync(output, 'utf8'));
    return { raw, display, end_frontmost: raw.unfocused_frames === 0, pid: owned.pid };
  } finally { await stop(owned); processAudit(owned.pid); }
}

const fmt = (n, digits = 2) => n === null ? 'INVALID' : n.toFixed(digits);
const judderText = n => n !== null && n > 0 && n < .001 ? n.toExponential(2) : fmt(n, 3);
export function table(rows) {
  const cells = row => [`${row.engine ?? ''}/${row.variant}`, row.label ?? `${row.run}${row.attempt > 1 ? `.${row.attempt}` : ''}`,
    `${row.valid === false ? 'INVALID ' : ''}${row.provisional ? 'PROVISIONAL' : 'quiet'}`,
    row.load1 == null ? '—' : fmt(row.load1, 1), fmt(row.observed_refresh_hz, 1), fmt(row.frame_ms.p50), fmt(row.frame_ms.p95),
    fmt(row.frame_ms.p99), fmt(row.frame_ms.max), `${row.hitches} (${fmt(row.hitch_percent)}%)`,
    judderText(row.player.judder), fmt(row.player.repeated_fraction * 100, 1), judderText(row.camera.judder),
    fmt(row.camera.repeated_fraction * 100, 1), fmt(row.latency.median_ms), fmt(row.latency.p95_ms), row.tick_phase == null ? '—' : fmt(row.tick_phase, 4),
    `${fmt(row.latency.median_intervals)}/${fmt(row.latency.p95_intervals)}`,
    row.frontmost_visible_confirmed ? 'yes' : 'NO'];
  const lines = [['variant', 'run', 'status', 'load1', 'Hz seen', 'p50 ms', 'p95', 'p99', 'max',
    'hitches', 'player J', 'zero %', 'camera J', 'zero %', 'input p50', 'p95', 'tick_phase', 'input intervals', 'front/visible']];
  for (const variant of [...new Set(rows.map(r => `${r.engine}/${r.variant}`))]) {
    const all = rows.filter(r => `${r.engine}/${r.variant}` === variant);
    all.forEach(r => lines.push(r.error
      ? [variant, `${r.run}.${r.attempt}`, `INVALID ${r.provisional ? 'PROVISIONAL ' : ''}${r.error.replace(/[|\r\n]/g, '/')}`, fmt(r.load1, 1), ...Array(15).fill('—')]
      : cells(r)));
    const group = all.filter(r => r.valid);
    if (!group.length) continue;
    const best = [...group].sort((a, b) => a.hitch_percent - b.hitch_percent || a.frame_ms.p99 - b.frame_ms.p99)[0];
    lines.push(cells({ ...best, label: `best (#${best.run}${best.attempt > 1 ? `.${best.attempt}` : ''})` }));
    const middle = structuredClone(group[0]);
    for (const key of ['load1', 'observed_refresh_hz', 'hitches', 'hitch_percent']) middle[key] = quantile(group.map(r => r[key]), .5);
    for (const [key, fields] of Object.entries({ frame_ms: ['p50', 'p95', 'p99', 'max'],
      player: ['judder', 'repeated_fraction'], camera: ['judder', 'repeated_fraction'],
      latency: ['median_ms', 'p95_ms', 'median_intervals', 'p95_intervals'] })) {
      for (const field of fields) middle[key][field] = group.some(r => r[key][field] === null) ? null : quantile(group.map(r => r[key][field]), .5);
    }
    middle.tick_phase = group.every(r => Number.isFinite(r.tick_phase)) ? quantile(group.map(r => r.tick_phase), .5) : null;
    middle.provisional = group.some(r => r.provisional);
    middle.frontmost_visible_confirmed = group.every(r => r.frontmost_visible_confirmed);
    lines.push(cells({ ...middle, label: `median metrics (n=${group.length})` }));
  }
  return lines.map((line, i) => `| ${line.join(' | ')} |${i === 0 ? '\n|' + line.map(() => '---').join('|') + '|' : ''}`).join('\n');
}

export async function measure(engine, variant, run, seconds = 12, options = {}) {
  check();
  const consoleStart = preflight();
  mkdirSync(resultsDir, { recursive: true });
  const day = new Date().toISOString().slice(0, 10), batch = options.batch ?? new Date().toISOString().replace(/[:.]/g, '-');
  const attempt = options.attempt ?? 1, jsonl = join(resultsDir, `feel-${day}.jsonl`), plan = script(seconds);
  const temp = mkdtempSync(join(here, '.feel-')), loads = [loadavg()[0]];
  let polls = 0;
  const timer = setInterval(() => {
    loads.push(loadavg()[0]);
    if (++polls % 5 === 0) try { preflight(); } catch (error) { machineError = error; }
  }, 1000);
  console.error(`${engine} ${variant} ${run}/${options.attempts ?? 3} attempt ${attempt}: ${(plan.duration_ms / 1000).toFixed(1)} s live script + warmup; load1=${fmt(loads[0], 1)}`);
  let trace;
  const adapter = options.adapter ?? adapters[engine];
  try {
    const capture = adapter.transport === 'web' ? await web(plan, temp, adapter) : await godot(plan, variant, temp);
    loads.push(loadavg()[0]); clearInterval(timer);
    const raw = capture.raw;
    trace = `feel-${batch}-${engine}-${variant}-${run}-attempt${attempt}.json.gz`;
    writeFileSync(join(resultsDir, trace), gzipSync(JSON.stringify({ ...raw, schedule: plan.schedule })));
    if (engine === 'exact' && raw.tick_hz !== adapter.hz) throw new Error(`Exact build is ${raw.tick_hz} Hz, expected ${adapter.hz} Hz; refusing a mislabeled row`);
    check();
    let consoleEnd;
    try { consoleEnd = preflight(); } catch (error) { machineError = error; throw error; }
    const metrics = analyze(raw, plan);
    const row = { schema: 1, batch, date: new Date().toISOString(), engine, variant, run, attempt, seconds,
      game: options.game ?? 'beacons', console_start: consoleStart, console_end: consoleEnd,
      build: adapter.build ?? null, tick_hz: raw.tick_hz ?? null, tick_phase: raw.tick_phase ?? null,
      engine_version: raw.engine_version, browser_version: capture.browser_version ?? null,
      display_refresh_hz: capture.display.display_refresh_hz || raw.display_refresh_hz || null,
      display_pixels: capture.display.display_pixels, window_pixels: raw.window_pixels,
      viewport: raw.viewport_css ?? raw.viewport_pixels,
      frontmost_visible_confirmed: capture.display.frontmost === true && capture.end_frontmost && raw.hidden_frames === 0 && raw.unfocused_frames === 0,
      hidden_frames: raw.hidden_frames, unfocused_frames: raw.unfocused_frames,
      load1: Math.max(...loads), load1_start: loads[0], load1_end: loads.at(-1), provisional: loads.some(x => x > 8),
      interpolation: raw.interpolation, timestamp_source: raw.timestamp_source,
      ...metrics, trace: relative(here, join(resultsDir, trace)), launched_pid: capture.pid, process_exit_confirmed: true };
    row.valid = row.latency.valid_trials === 20 && row.frontmost_visible_confirmed;
    const line = JSON.stringify(row); appendFileSync(jsonl, line + '\n'); console.log(line);
    console.error(`${engine}/${variant} #${run}: J=${fmt(row.player.judder, 3)}, input=${fmt(row.latency.median_ms)} ms, valid=${row.valid}`);
    return row;
  } catch (error) {
    const row = { schema: 1, batch, date: new Date().toISOString(), engine, variant, run, attempt,
      game: options.game ?? 'beacons', valid: false, error: error.message,
      load1: Math.max(...loads), provisional: loads.some(x => x > 8), trace: trace ? `results/${trace}` : null };
    appendFileSync(jsonl, JSON.stringify(row) + '\n'); console.log(JSON.stringify(row));
    if (machineError || interrupted) { error.row = row; throw error; }
    return row;
  } finally { clearInterval(timer); rmSync(temp, { recursive: true, force: true }); }
}

export function cli(args) {
  const [engine, ...rest] = args;
  const options = { engine, seconds: 12, game: 'beacons', entity: 'player', play: '[data-testid="play"]', noBuild: false, attempts: 3, hz: Number(process.env.FEEL_EXACT_HZ ?? 60) };
  const known = { '--seconds': 'seconds', '--game': 'game', '--entity': 'entity', '--play': 'play', '--hz': 'hz', '--attempts': 'attempts' };
  for (let i = 0; i < rest.length; i++) {
    if (rest[i] === '--no-build') { options.noBuild = true; continue; }
    const flag = rest[i], key = known[flag], value = rest[++i];
    if (!key || !value) throw new Error(`Unknown/incomplete feel option: ${flag}`);
    options[key] = ['seconds', 'hz', 'attempts'].includes(key) ? Number(value) : value;
  }
  if ((!Object.hasOwn(adapters, engine) && engine !== 'compare') || !Number.isFinite(options.seconds)
      || options.seconds < 7.02 || options.seconds > 300 || ![60, 120].includes(options.hz)
      || ![1, 2, 3].includes(options.attempts) || !/^[a-z][a-z0-9_-]*$/.test(options.game)) {
    throw new Error('usage: bun game/bench/feel.mjs <exact|three|godot|compare> [--game beacons] [--entity player] [--play selector] [--hz 60|120] [--no-build] [--attempts 1|2|3] [--seconds 12]');
  }
  if (engine !== 'exact' && options.game !== 'beacons') throw new Error('The twins/compare currently implement Beacons only');
  return options;
}

export function exactAdapter(options) {
  const app = resolve(here, '../games', options.game);
  return { ...adapters.exact, hz: options.hz, variants: [`${options.hz}hz`],
    root: resolve(process.env.FEEL_EXACT_DIST ?? join(app, options.hz === 120 ? 'target/feel120' : 'dist')),
    options: { entity: options.entity, play: options.play } };
}

// Ordinary builds reuse the proof's content/artifact-digest gate. --no-build
// deliberately trusts the supplied bake, without calling its staleness/build path.
export async function prepareExact(options) {
  const app = resolve(here, '../games', options.game), adapter = exactAdapter(options);
  if (options.noBuild || options.hz === 120 || process.env.FEEL_EXACT_DIST) {
    for (const name of ['index.html', 'exact.json', 'gpu_bg.wasm']) {
      if (!existsSync(join(adapter.root, name))) throw new Error(`Missing baked ${options.hz} Hz artifact: ${join(adapter.root, name)}`);
    }
    const { artifactDigest } = await import('../proof.mjs');
    const digest = artifactDigest('web', adapter.root);
    if (!digest) throw new Error(`Cannot fingerprint bake: ${adapter.root}`);
    adapter.build = { source: options.noBuild ? '--no-build' : 'prebuilt experiment',
      freshness_checked: false, dist: adapter.root, artifact_sha256: digest };
    console.error(`Reusing ${options.hz} Hz bake ${adapter.root}; freshness skipped, actual Hz checked in every trace`);
    return adapter;
  }
  const code = `import { proof } from ${JSON.stringify(resolve(here, '../proof.mjs'))}; process.argv[2] = 'web'; await proof({url:${JSON.stringify(pathToFileURL(join(app, 'proof.mjs')).href)}}, async () => {});`;
  check();
  const child = spawn('bun', ['-e', code], { cwd: resolve(here, '../..'),
    env: { ...process.env, EXACT_APP_DIR: app, CARGO_TARGET_DIR: join(app, 'target'),
      EXACT_UPDATE_TRUST: 'development', DEVELOPER_DIR: process.env.DEVELOPER_DIR ?? '/Library/Developer/CommandLineTools' },
    stdio: ['ignore', 'pipe', 'pipe'] });
  console.error(`Build PID ${child.pid} (proof.mjs cache/build path)`);
  let lastOutput = performance.now(), stalled = false;
  const recorded = new Set([child.pid]);
  const output = data => { lastOutput = performance.now(); process.stderr.write(data); };
  child.stdout.on('data', output); child.stderr.on('data', output);
  const monitor = setInterval(() => {
    const ps = spawnSync('ps', ['-axo', 'pid=,ppid='], { encoding: 'utf8' });
    const rows = ps.stdout.trim().split('\n').map(line => line.trim().split(/\s+/).map(Number));
    for (let more = true; more;) {
      more = false;
      for (const [pid, parent] of rows) if (recorded.has(parent) && !recorded.has(pid)) { recorded.add(pid); more = true; }
    }
    if (performance.now() - lastOutput > 60000 || interrupted) {
      stalled = !interrupted;
      for (const pid of [...recorded].reverse()) try { process.kill(pid, 'SIGKILL'); } catch {}
    }
  }, 1000);
  let status;
  try { status = await new Promise((ok, reject) => { child.once('exit', ok); child.once('error', reject); }); }
  finally { clearInterval(monitor); }
  if (stalled) throw new Error('Build silent for 60 seconds; stopped recorded build processes. Use the provisioned Linux builder for this build step');
  check();
  if (status === 0) { adapter.build = { source: 'proof.mjs', receipt: JSON.parse(readFileSync(join(app, 'artifacts/build-web.sha256'), 'utf8')) }; return adapter; }
  throw new Error('Build failed in the shared proof path; no measurement taken');
}

function saveTable(batch, rows) {
  const report = table(rows);
  writeFileSync(join(resultsDir, `feel-${batch}.md`), report + '\n');
  console.error(report); console.error(`Saved JSONL, table and raw traces under ${resultsDir}`);
}

export async function runFeel(options, { ready = preflight, prepare = prepareExact, take = measure, save = saveTable } = {}) {
  console.error(`Feel preflight: ${JSON.stringify(ready())}`);
  const batch = new Date().toISOString().replace(/[:.]/g, '-'), rows = [], jobs = [];
  if (['exact', 'compare'].includes(options.engine)) {
    for (const hz of options.engine === 'compare' ? [60, 120] : [options.hz]) {
      jobs.push({ engine: 'exact', variant: `${hz}hz`, adapter: await prepare({ ...options, hz }) });
    }
  }
  for (const engine of options.engine === 'compare' ? ['three', 'godot'] : ['three', 'godot'].filter(e => e === options.engine)) {
    for (const variant of adapters[engine].variants) jobs.push({ engine, variant, adapter: adapters[engine] });
  }
  ready(); // A long build must not turn an earlier idle check into permission.
  try {
    for (let run = 1; run <= options.attempts; run++) for (const job of jobs) {
      const row = await take(job.engine, job.variant, run, options.seconds, { ...options, batch, attempt: 1, adapter: job.adapter });
      rows.push(row); // Exactly three attempts by default, including invalid rows; no hidden retakes.
    }
  } catch (error) {
    if (error.row) rows.push(error.row);
    throw error;
  } finally { if (rows.length) save(batch, rows); }
  return rows;
}

// `reanalyze <trace.json.gz>…` scores saved raw traces with the current analyzer and
// prints one table; the rows say which trace they came from. A row scored this way
// is labelled "reanalyzed" and is never appended to the sitting's JSONL.
export function reanalyze(paths) {
  const { gunzipSync } = require('node:zlib');
  return paths.map(path => {
    const raw = JSON.parse(gunzipSync(readFileSync(path)).toString());
    const name = path.replace(/^.*feel-/, '').replace(/\.json\.gz$/, '');
    const [, engine, variant] = name.match(/Z-([a-z]+)-([a-z0-9]+)-\d+-attempt\d+$/) ?? [, 'trace', name];
    try {
      const m = analyze(raw, script());
      return { engine, variant, run: name.slice(0, 24), attempt: 1, label: 'reanalyzed ' + name.slice(-22), valid: true, provisional: true, load1: raw.load1 ?? null,
        tick_phase: m.tick_phase ?? null, ...m, frontmost_visible_confirmed: raw.unfocused_frames === 0 && raw.hidden_frames === 0 };
    } catch (error) { return { engine, variant, run: name.slice(0, 24), attempt: 1, valid: false, provisional: true, load1: null, error: error.message }; }
  });
}

async function main() {
  if (process.argv[2] === 'reanalyze') { console.log(table(reanalyze(process.argv.slice(3)))); return; }
  const options = cli(process.argv.slice(2));
  process.once('SIGINT', cancel); process.once('SIGTERM', cancel);
  const rows = await runFeel(options);
  if (rows.some(r => !r.valid)) process.exitCode = 1;
}
// Bun can retain signal/CDP handles after the last browser has exited. Cleanup
// above has already awaited every owned process before either completion path.
if (import.meta.main) main().then(() => process.exit(process.exitCode ?? 0), error => {
  console.error(error.message); process.exit(1);
});
