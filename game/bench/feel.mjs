#!/usr/bin/env bun
// Live, headed Beacons diagnostic. stdout = JSONL; stderr = progress + table.
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { loadavg, platform } from 'node:os';
import { extname, join, resolve, relative } from 'node:path';
import { gzipSync } from 'node:zlib';

const here = import.meta.dir, resultsDir = join(here, 'results');
const GODOT = process.env.GODOT ?? resolve(process.env.HOME, 'Library/Caches/exact2-game/godot/Godot.app/Contents/MacOS/Godot');
const CHROME = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
// A future home probe needs only a registry entry (local root/page or an already
// served URL) and the window.feel protocol; scheduling/analysis stay unchanged.
const adapters = {
  three: { transport: 'web', variants: ['shipped'], root: resolve(here, '../twins/three'),
    page: '/beacons/index.html?feel=1', ready: '!!window.beacons && !!window.feel',
    started: 'beacons.state().mode === "playing"' },
  godot: { transport: 'godot', variants: ['shipped', 'interpolation'] },
};
let interrupted = false;
export function cancel() { interrupted = true; }
const delay = ms => new Promise(resolve => setTimeout(resolve, Math.max(0, ms)));
function check() { if (interrupted) throw new Error('Interrupted; cleaning up owned processes'); }
async function until(predicate, ms, label) {
  const end = performance.now() + ms;
  while (performance.now() < end) { check(); const value = await predicate(); if (value) return value; await delay(50); }
  throw new Error(`Timeout: ${label}`);
}

export function script(seconds = 12) {
  const schedule = [];
  const key = (at_ms, code, down, trial = -1) => schedule.push({ at_ms, code, down, trial });
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
  return { schedule, duration_ms: start + 19 * 3600 + 500 };
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
  if (raw.schema !== 1 || raw.stride !== 8 || raw.overflow) throw new Error('Invalid/overflowed probe buffer');
  const events = unpack(raw.events, 4), allFrames = unpack(raw.frames, 8);
  if (events.length !== plan.schedule.length) throw new Error(`Expected ${plan.schedule.length} delivered events, got ${events.length}`);
  events.forEach((e, i) => {
    const wanted = plan.schedule[i];
    if (e[1] !== wanted.code || e[2] !== +wanted.down || e[3] !== wanted.trial) throw new Error(`Event ${i} differs from schedule: ${e}`);
  });
  const frames = allFrames.filter(f => f[1] >= events[0][0]);
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
  return { frames: frames.length, duration_ms: frames.at(-1)[1] - frames[0][1],
    refresh_interval_ms: median, observed_refresh_hz: 1000 / median,
    frame_ms: pacing, hitches, hitch_percent: hitches / frames.length * 100,
    player, camera, latency: { trials: latency, valid_trials: valid.length,
      median_ms: quantile(valid, .5), p95_ms: quantile(valid, .95),
      median_intervals: valid.length ? quantile(valid, .5) / median : null,
      p95_intervals: valid.length ? quantile(valid, .95) / median : null },
    input_schedule_error_ms: events.map((e, i) => e[0] - events[0][0] - plan.schedule[i].at_ms),
    w_hold_ms: events[3][0] - events[2][0] };
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
    const path = resolve(root, '.' + decodeURIComponent(new URL(req.url).pathname));
    if (!path.startsWith(root + '/')) return new Response('Forbidden', { status: 403 });
    const file = Bun.file(path);
    return await file.exists() ? new Response(file, { headers: { 'content-type': {
      '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json',
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
    await until(() => cdp.evaluate(adapter.ready ?? '!!window.feel'), 20000, 'web probe load');
    await cdp.send('Page.bringToFront'); desktop(owned.pid, true);
    await until(async () => desktop(owned.pid).frontmost && await cdp.evaluate('document.hasFocus() && document.visibilityState === "visible"'), 5000, 'Chrome foreground');
    // Match the shipped Godot content size, without device or virtual-time emulation.
    const bounds = await cdp.send('Browser.getWindowForTarget');
    const size = await cdp.evaluate('({w:outerWidth-innerWidth+1100,h:outerHeight-innerHeight+760})');
    await cdp.send('Browser.setWindowBounds', { windowId: bounds.windowId, bounds: { width: size.w, height: size.h } });
    await delay(1500); // shader/startup warmup outside measurement
    const display = desktop(owned.pid), version = await cdp.send('Browser.getVersion');
    await cdp.evaluate(`feel.begin(${plan.duration_ms})`);
    const epoch = performance.now();
    const codes = { 13: ['Enter', 'Enter'], 87: ['KeyW', 'w'], 68: ['KeyD', 'd'], 83: ['KeyS', 's'], 32: ['Space', ' '] };
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
  const cells = row => [row.variant, row.label ?? `${row.run}${row.attempt > 1 ? `.${row.attempt}` : ''}`,
    `${row.valid === false ? 'INVALID ' : ''}${row.provisional ? 'PROVISIONAL' : 'quiet'}`,
    fmt(row.load1, 1), fmt(row.observed_refresh_hz, 1), fmt(row.frame_ms.p50), fmt(row.frame_ms.p95),
    fmt(row.frame_ms.p99), fmt(row.frame_ms.max), `${row.hitches} (${fmt(row.hitch_percent)}%)`,
    judderText(row.player.judder), fmt(row.player.repeated_fraction * 100, 1), judderText(row.camera.judder),
    fmt(row.camera.repeated_fraction * 100, 1), fmt(row.latency.median_ms), fmt(row.latency.p95_ms),
    `${fmt(row.latency.median_intervals)}/${fmt(row.latency.p95_intervals)}`,
    row.frontmost_visible_confirmed ? 'yes' : 'NO'];
  const lines = [['variant', 'run', 'status', 'load1', 'Hz seen', 'p50 ms', 'p95', 'p99', 'max',
    'hitches', 'player J', 'zero %', 'camera J', 'zero %', 'input p50', 'p95', 'input intervals', 'front/visible']];
  for (const variant of [...new Set(rows.map(r => r.variant))]) {
    const all = rows.filter(r => r.variant === variant);
    all.forEach(r => lines.push(cells(r)));
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
    middle.provisional = group.some(r => r.provisional);
    middle.frontmost_visible_confirmed = group.every(r => r.frontmost_visible_confirmed);
    lines.push(cells({ ...middle, label: `median metrics (n=${group.length})` }));
  }
  return lines.map((line, i) => `| ${line.join(' | ')} |${i === 0 ? '\n|' + line.map(() => '---').join('|') + '|' : ''}`).join('\n');
}

export async function measure(engine, variant, run, seconds = 12, options = {}) {
  check();
  mkdirSync(resultsDir, { recursive: true });
  const day = new Date().toISOString().slice(0, 10), batch = options.batch ?? new Date().toISOString().replace(/[:.]/g, '-');
  const attempt = options.attempt ?? 1, jsonl = join(resultsDir, `feel-${day}.jsonl`), plan = script(seconds);
  const temp = mkdtempSync(join(here, '.feel-')), loads = [loadavg()[0]];
  const timer = setInterval(() => loads.push(loadavg()[0]), 1000);
  console.error(`${engine} ${variant} ${run}/3 attempt ${attempt}: ${(plan.duration_ms / 1000).toFixed(1)} s live script + warmup; load1=${fmt(loads[0], 1)}`);
  try {
    const capture = adapters[engine].transport === 'web' ? await web(plan, temp, adapters[engine]) : await godot(plan, variant, temp);
    loads.push(loadavg()[0]); clearInterval(timer);
    const raw = capture.raw;
    const trace = `feel-${batch}-${engine}-${variant}-${run}-attempt${attempt}.json.gz`;
    writeFileSync(join(resultsDir, trace), gzipSync(JSON.stringify({ ...raw, schedule: plan.schedule })));
    const metrics = analyze(raw, plan);
    const row = { schema: 1, batch, date: new Date().toISOString(), engine, variant, run, attempt, seconds,
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
  } finally { clearInterval(timer); rmSync(temp, { recursive: true, force: true }); }
}

async function main() {
  const [engine, ...args] = process.argv.slice(2);
  const seconds = args.length ? Number(args[1]) : 12;
  if (!Object.hasOwn(adapters, engine) || (args.length && (args.length !== 2 || args[0] !== '--seconds')) || !Number.isFinite(seconds) || seconds < 7.02 || seconds > 300) {
    throw new Error('usage: bun game/bench/feel.mjs <three|godot> [--seconds 12] (7.02–300; minimum main-script window, plus 20 latency trials)');
  }
  process.once('SIGINT', cancel); process.once('SIGTERM', cancel);
  const batch = new Date().toISOString().replace(/[:.]/g, '-'), rows = [];
  // Alternate variants; a focus loss gets a bounded retake, never a hidden discard.
  for (let run = 1; run <= 3; run++) for (const variant of adapters[engine].variants) {
    for (let attempt = 1; attempt <= 3; attempt++) {
      const row = await measure(engine, variant, run, seconds, { batch, attempt }); rows.push(row);
      if (row.valid) break;
      console.error(`Invalid run retained; ${attempt < 3 ? 'retaking this slot' : 'three attempts exhausted'}`);
    }
  }
  console.error(table(rows)); console.error(`Saved JSONL and raw traces under ${resultsDir}`);
  if (rows.filter(r => r.valid).length !== 3 * adapters[engine].variants.length) process.exitCode = 1;
}
// Bun can retain signal/CDP handles after the last browser has exited. Cleanup
// above has already awaited every owned process before either completion path.
if (import.meta.main) main().then(() => process.exit(process.exitCode ?? 0), error => {
  console.error(error.stack); process.exit(1);
});
