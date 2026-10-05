#!/usr/bin/env bun
// LLP 1041 §8: opt-in sidecar diagnostic, never a new CI gate or FPS claim.
// Build the selected app first. Examples:
//   bun host/apple/build.mjs messages-stress-apple
//   bun scripts/native-resize-metrics.mjs macos --app messages-stress
//   EXACT_PAINTER=cpu bun scripts/native-resize-metrics.mjs linux --app messages-stress
//   bun apps/completion-storm/fixture.mjs --api-only   # separate process
//   bun scripts/native-resize-metrics.mjs macos --app completion-storm
//   bun scripts/native-resize-metrics.mjs macos --app markdown-stress --profile page16k --tap size-4194304
//
// Wire input: {op:"tap",resize:[width,height]}, no id or other input fields.
// Whole logical points, 64..4096 per dimension, <=8388608 area. Invalid sizes
// are refused before resize. AppKit calls NSWindow.setContentSize, allowing
// its normal content, toolbar and safe-area geometry to follow. Linux calls
// Presenter.resize and paints; this does not resize a desktop/DRM display.
// iOS/web do not implement this optional native input. Eight verbs remain.
//
// Each bounded cohort queues clock (when enabled), resize, type and wheel
// without awaiting the preceding command. The next cohort waits for all ACKs
// and verification. This is backpressured programmatic resizing, not an OS
// live-resize drag, a fixed-rate producer, nor proof of physical presentation.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { arch, cpus, platform, release, totalmem } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { open } from './agent.mjs';
import { linuxBinary, resolveApp } from './app.mjs';
import { appleArtifacts } from '../host/apple/build.mjs';
import { summarize } from './stress-metrics.mjs';

const profiles = {
  'messages-stress': {
    idle100: { taps: [], tick: 0, scroll: 'transcript', rows: 100 },
    eager1000: { taps: ['history-1000', 'toggle-eager', 'batch-32', 'start'], tick: 250, scroll: 'transcript', rows: 1000 },
  },
  'completion-storm': { storm128: { taps: ['count-128', 'errors-0'], tick: 0, scroll: 'completion-storm' } },
  'markdown-stress': {
    page16k: { taps: ['width-wide'], tick: 0, scroll: 'document' },
    eager256k: { taps: ['width-wide', 'size-262144', 'toggle-eager'], tick: 0, scroll: 'document' },
  },
};
const usage = 'bun scripts/native-resize-metrics.mjs <macos|linux> [--app messages-stress|completion-storm|markdown-stress] [--profile name] [--samples 1..120] [--repeats 1..10] [--from 800x640] [--to 1120x860] [--tap testId ...] [--tick-ms 0..1000] [--out directory]';
export function options(args) {
  const result = { host: args[0], app: 'messages-stress', samples: 12, repeats: 3, from: [800, 640], to: [1120, 860], taps: [] };
  if (!['macos', 'linux'].includes(result.host)) throw new Error(usage);
  for (let i = 1; i < args.length; i += 2) {
    const key = args[i], value = args[i + 1];
    if (!['--app', '--profile', '--samples', '--repeats', '--from', '--to', '--tap', '--tick-ms', '--out'].includes(key)
      || !value || value.startsWith('--')) throw new Error(usage);
    if (key === '--tap') result.taps.push(value);
    else if (['--samples', '--repeats', '--tick-ms'].includes(key)) result[key.slice(2)] = Number(value);
    else if (key === '--from' || key === '--to') {
      if (!/^\d+x\d+$/.test(value)) throw new Error('sizes must be WIDTHxHEIGHT');
      result[key.slice(2)] = value.split('x').map(Number);
    } else result[key.slice(2)] = value;
  }
  if (!profiles[result.app]) throw new Error('unknown stress app');
  if (result.profile && !profiles[result.app][result.profile]) throw new Error('unknown profile for app');
  for (const [key, min, max] of [['samples', 1, 120], ['repeats', 1, 10], ['tick-ms', 0, 1000]]) {
    if (result[key] !== undefined && (!Number.isInteger(result[key]) || result[key] < min || result[key] > max)) throw new Error(`${key}: expected integer ${min}..${max}`);
  }
  for (const pair of [result.from, result.to]) {
    if (pair.some(v => !Number.isInteger(v) || v < 64 || v > 4096) || pair[0] * pair[1] > 8_388_608) throw new Error('size outside native resize bounds');
  }
  if (result.taps.length > 32) throw new Error('at most 32 startup taps');
  result.out = resolve(result.out ?? `target/native-resize/${result.app}/${result.host}-${Date.now()}`);
  return result;
}
const within = (actual, expected, label) => {
  assert.equal(actual?.length, 2, `${label}: missing dimensions`);
  actual.forEach((v, i) => assert.ok(Number.isFinite(v) && Math.abs(v - expected[i]) <= 1, `${label}: ${actual} != ${expected}`));
};
const deadline = async (promise, label) => {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} exceeded 60s`)), 60_000);
    })]);
  } finally { clearTimeout(timer); }
};
const node = (tree, testId) => {
  const value = tree.nodes.find(n => n.props.testId === testId);
  assert.ok(value, `missing testId ${testId}`);
  return value;
};
const stats = tree => Object.fromEntries(['stats', 'load-stats', 'mode', 'pending-count', 'valid-count', 'failed-count', 'lifetime-counts']
  .map(key => [key, tree.nodes.find(n => n.props.testId === key)?.props.text]).filter(([, v]) => v !== undefined));
const fixture = async (path, method = 'GET') => {
  const response = await fetch(`http://127.0.0.1:4320${path}`, { method, signal: AbortSignal.timeout(5000) });
  assert.equal(response.status, 200, `fixture ${path}: HTTP ${response.status}`);
  return response.json();
};

export function displayObservation(host) {
  if (host !== 'macos') return { source: 'headless CPU presenter', physical_presentation: 'unmeasured; no display participates' };
  const result = spawnSync('/usr/sbin/system_profiler', ['SPDisplaysDataType', '-json'], { encoding: 'utf8', timeout: 5000, maxBuffer: 1024 * 1024 });
  try {
    if (result.status !== 0) throw new Error(result.error?.message ?? `exit ${result.status}`);
    const gpus = JSON.parse(result.stdout).SPDisplaysDataType;
    assert.ok(Array.isArray(gpus), 'missing display data');
    return { source: 'system_profiler SPDisplaysDataType -json', observed_at: new Date().toISOString(),
      physical_presentation: 'unmeasured; configured display mode is not presentation timing',
      gpus: gpus.map(gpu => ({ model: gpu.sppci_model ?? gpu._name, cores: gpu.sppci_cores ?? null,
        displays: (gpu.spdisplays_ndrvs ?? []).map(display => {
          const mode = display._spdisplays_resolution ?? display.spdisplays_resolution ?? null;
          const hz = mode?.match(/@\s*([\d.]+)\s*Hz/i)?.[1];
          return { name: display._name, mode, configured_hz: hz ? Number(hz) : null };
        }) })) };
  } catch (error) { return { source: 'system_profiler', unavailable: error.message }; }
}

async function verifyInvalid(session) {
  const before = (await session.op({ op: 'layout' })).viewport;
  const checks = [];
  // JSON cannot represent NaN/Infinity: JSON.stringify sends null, which must
  // also be rejected. Raw invalid JSON rejection is covered by the Rust test.
  for (const value of [[0, 600], [-1, 600], [NaN, 600], [Infinity, 600], [1e300, 600], [4096, 4096],
    [true, 600], ['800', 600], [800.5, 600], [800], [800, 600, 1], null]) {
    await assert.rejects(session.op({ op: 'tap', resize: value }), /resize/);
    assert.deepEqual((await session.op({ op: 'layout' })).viewport, before, 'invalid resize changed geometry');
    checks.push({ value, refused: true });
  }
  await assert.rejects(session.op({ op: 'tap', resize: [800, 600], wheel: [0, 10] }), /resize/);
  assert.deepEqual((await session.op({ op: 'layout' })).viewport, before);
  return checks;
}

export async function run(config) {
  assert.equal(platform(), config.host === 'macos' ? 'darwin' : 'linux', 'run on the named OS; a cross-compiled host is not that OS');
  const app = resolveApp(config.app);
  const executable = config.host === 'macos'
    ? process.env.EXACT_MAC_BIN ?? appleArtifacts(app).binary
    : process.env.EXACT_LINUX_BIN ?? linuxBinary(app);
  const git = (...args) => spawnSync('git', args, { cwd: app.workspace, encoding: 'utf8' }).stdout?.trim() ?? null;
  const report = {
    app: config.app, host: config.host, config, at: new Date().toISOString(),
    os: { platform: platform(), release: release(), arch: arch(), cpu: cpus()[0]?.model, cpus: cpus().length, total_memory: totalmem() },
    display: displayObservation(config.host),
    bun: Bun.version, source: { head: git('rev-parse', 'HEAD'), status: git('status', '--short'),
      note: 'Checkout observation, not attestation of the executable build inputs.' },
    build_note: process.env.EXACT_RESIZE_BUILD_NOTE ?? null,
    executable, executable_sha256: createHash('sha256').update(readFileSync(executable)).digest('hex'),
    measurement: 'Raw sidecar command-send to ACK latency includes IPC and preceding queued commands. Echo/tree/layout verification is separate. No OS input-to-display or physical FPS measurement.',
    resize: config.host === 'macos' ? 'NSWindow.setContentSize + layoutSubtreeIfNeeded + displayIfNeeded; AppKit owns toolbar geometry; no live-resize tracking loop'
      : 'Presenter.resize + frame; headless CPU raster on actual Linux; no desktop compositor or DRM mode switch',
    pacing: 'One bounded cohort (at most four commands) then verification. Clock increments are virtual, not a wall-clock producer. No fixed-rate throughput claim.',
    frame_interval_target_ms: 1000 / 120, presentation: 'unmeasured', runs: [],
    target_basis: '8.33 ms processing target; configured display refresh and physical presentation are separate observations.',
    pass_criteria: 'Geometry, typing echo, scroll movement and recovery correctness only. Latency is reported, never used to claim a frame-rate pass.',
  };
  mkdirSync(config.out, { recursive: true });
  const save = () => writeFileSync(resolve(config.out, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  const selected = config.profile ? [[config.profile, profiles[config.app][config.profile]]] : Object.entries(profiles[config.app]);
  for (const [profile, controls] of selected) for (let repeat = 1; repeat <= config.repeats; repeat++) {
    const row = { profile, repeat, status: 'running', controls: [...controls.taps, ...config.taps], samples: [] };
    report.runs.push(row); save();
    let session, wave;
    try {
      session = await open({ host: config.host, app: config.app, size: config.from,
        env: config.host === 'linux' ? { EXACT_PAINTER: 'cpu' } : {} });
      row.boot_ms = session.boot;
      row.invalid_sizes = await deadline(verifyInvalid(session), 'invalid input checks');
      for (const control of row.controls) await deadline(session.tap(control), `setup ${control}`);
      if (config.app === 'completion-storm') {
        await fixture('/api/stats');
        await deadline(session.tap('start-wave'), 'start wave');
        const limit = performance.now() + 5000;
        while (performance.now() < limit) {
          const tree = await session.tree();
          wave = Number(node(tree, 'wave-status').props.text?.match(/^Wave (\d+):/)?.[1]);
          if (wave) break;
          await Bun.sleep(10);
        }
        assert.ok(wave, 'wave creation must reach UI before diagnostic');
        row.fixture_before = await fixture('/api/stats');
      }
      const initial = await session.tree();
      const input = node(initial, 'stress-input').id, echo = node(initial, 'stress-echo').id;
      const scroll = node(initial, controls.scroll).id;
      row.initial = { nodes: initial.nodes.length, stats: stats(initial) };
      if (controls.rows && !config.taps.length) {
        assert.equal(initial.nodes.filter(n => n.props.testId?.startsWith('message-')).length, controls.rows);
      }
      const tick = config['tick-ms'] ?? controls.tick;
      let scrolled = false;
      for (let index = 0; index < config.samples; index++) {
        const phase = index % 12, fraction = phase <= 6 ? phase / 6 : (12 - phase) / 6;
        const size = config.from.map((v, i) => Math.round(v + fraction * (config.to[i] - v)));
        const value = `${profile}/${repeat}/${index} resize draft 🦀`;
        const sample = { index, requested: size, commands: [] };
        row.samples.push(sample);
        const started = performance.now();
        const command = request => {
          const sent = performance.now();
          return session.op(request).then(reply => {
            const ack = performance.now();
            const entry = { request, sent_ms: sent - started, ack_ms: ack - started, latency_ms: ack - sent, reply };
            sample.commands.push(entry); return entry;
          });
        };
        const before = (await session.op({ op: 'layout' })).nodes.find(n => n.id === scroll);
        // Release this run's wave only. A sidecar release can progress even
        // when the old native FIFO is blocked by a held HTTP response.
        const releaseWave = wave && index === Math.min(2, config.samples - 1)
          ? fixture(`/api/release?wave=${wave}`, 'POST').then(reply => { sample.fixture_release = reply; }) : Promise.resolve();
        const jobs = [];
        if (tick) jobs.push(command({ op: 'clock', to: session.now + tick }).then(entry => { session.now = entry.reply.clock; }));
        const resized = command({ op: 'tap', resize: size }); jobs.push(resized);
        const typed = command({ op: 'type', id: input, text: value }); jobs.push(typed);
        const wheeled = command({ op: 'tap', id: scroll, wheel: [0, index % 8 < 4 ? 120 : -120] }); jobs.push(wheeled);
        await deadline(Promise.all([...jobs, releaseWave]), 'resize/type/scroll cohort');
        const resizeReply = (await resized).reply;
        within(resizeReply.resized, size, 'native content size');
        const verifyAt = performance.now();
        const [tree, layout] = await deadline(Promise.all([session.tree(), session.op({ op: 'layout' })]), 'echo/geometry verification');
        assert.equal(tree.nodes.find(n => n.id === echo)?.props.text, value, 'typing must survive resizing');
        within(resizeReply.viewport, [layout.viewport.w, layout.viewport.h], 'ACK versus observed viewport');
        const after = layout.nodes.find(n => n.id === scroll);
        assert.ok(after, 'scroll container must survive resize');
        scrolled ||= Math.abs((after.sy ?? 0) - (before?.sy ?? 0)) > 0;
        sample.scroll = { before: before?.sy ?? 0, after: after.sy ?? 0 };
        sample.observed_viewport = layout.viewport;
        sample.verification_ms = performance.now() - verifyAt;
        sample.cohort_and_verification_ms = performance.now() - started;
        sample.stats = stats(tree);
      }
      assert.ok(scrolled, 'at least one interleaved wheel must move content');
      if (wave) {
        await fixture(`/api/release?wave=${wave}`, 'POST');
        const settled = await deadline(session.clock('settle'), 'completion recovery');
        assert.equal(settled.settled, true, 'completion wave must settle');
        // The app's lifetime counters refresh on a 500 ms task. Agent time
        // is frozen during the resize cohort; advance it before reconciling.
        await session.clock('+500');
        const tree = await session.tree();
        assert.match(node(tree, 'pending-count').props.text, /^0 pending/);
        assert.match(node(tree, 'valid-count').props.text, /^128 valid/);
        assert.match(node(tree, 'lifetime-counts').props.text, /128 requests emitted · 128 current-ticket replies parsed/);
        row.fixture_after = await fixture('/api/stats');
      }
      if (tick && config.app !== 'completion-storm') await session.tap('pause');
      const recoveryAt = performance.now();
      await session.op({ op: 'tap', resize: config.from });
      await session.op({ op: 'type', id: input, text: 'recovered after resize load' });
      assert.equal(node(await session.tree(), 'stress-echo').props.text, 'recovered after resize load');
      row.recovery_command_and_echo_ms = performance.now() - recoveryAt;
      row.final = stats(await session.tree());
      const path = resolve(config.out, `${profile}-${repeat}.png`);
      await deadline(session.screenshot(path), 'screenshot'); row.screenshot = path;
      const png = readFileSync(path);
      row.screenshot_pixels = [png.readUInt32BE(16), png.readUInt32BE(20)];
      if (config.host === 'linux') within(row.screenshot_pixels, config.from, 'Linux painted PNG size');
      row.logs = await session.logs();
      for (const operation of ['resize', 'type', 'wheel', 'clock']) {
        row[`${operation}_ack_ms`] = summarize(row.samples.flatMap(s => s.commands.filter(c => operation === 'resize' ? c.request.resize
          : operation === 'wheel' ? c.request.wheel : c.request.op === operation).map(c => c.latency_ms)), report.frame_interval_target_ms);
      }
      row.status = 'passed';
    } catch (error) {
      row.status = 'failed'; row.error = error.stack ?? String(error);
    } finally {
      // Each cleanup is bounded, and never releases another driver's wave.
      if (wave) try { await fixture(`/api/release?wave=${wave}`, 'POST'); } catch {}
      await session?.close(); save();
    }
    console.log(JSON.stringify({ profile, repeat, status: row.status, resize_ack_ms: row.resize_ack_ms, type_ack_ms: row.type_ack_ms, error: row.error }));
  }
  console.log(`Report: ${resolve(config.out, 'report.json')}`);
  return report.runs.every(r => r.status === 'passed') ? 0 : 1;
}

if (import.meta.main) {
  if (process.argv.includes('--help')) console.log(usage);
  else run(options(process.argv.slice(2))).then(code => { process.exitCode = code; }, error => { console.error(error.stack); process.exitCode = 1; });
}
