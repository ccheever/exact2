#!/usr/bin/env bun
// Opt-in LLP 1041 native correctness + command-ack diagnostic, no frame-rate claim.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { arch, cpus, platform, release } from 'node:os';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { open } from '../../scripts/agent.mjs';
import { resolveApp } from '../../scripts/app.mjs';
import { appleArtifacts } from '../../host/apple/build.mjs';

const args = process.argv.slice(2);
const host = args[0];
if (!['macos', 'linux'].includes(host)) throw new Error('usage: bun apps/messages-stress/native-smoke.mjs <macos|linux> [--samples 1..40] [--out directory]');
const option = (name, fallback) => {
  const index = args.indexOf(name);
  if (index < 0) return fallback;
  if (!args[index + 1] || args[index + 1].startsWith('--')) throw new Error(`${name} needs a value`);
  return args[index + 1];
};
for (let i = 1; i < args.length; i += 2) {
  if (!['--samples', '--out'].includes(args[i])) throw new Error(`unknown option ${args[i]}`);
}
const samples = Number(option('--samples', '12'));
if (!Number.isInteger(samples) || samples < 1 || samples > 40) throw new Error('--samples must be an integer in 1..40');
const output = resolve(option('--out', `target/messages-stress-native/${host}-${platform()}`));
mkdirSync(output, { recursive: true });
const app = resolveApp('messages-stress');
const executable = host === 'macos'
  ? (process.env.EXACT_MAC_BIN ?? appleArtifacts(app).binary)
  : (process.env.EXACT_LINUX_BIN ?? resolve(app.target, 'release/messages-stress-linux'));
const git = (...argv) => spawnSync('git', argv, { cwd: app.workspace, encoding: 'utf8' }).stdout?.trim() ?? null;
const report = {
  app: app.id, host, operating_system: platform(), os_release: release(), architecture: arch(),
  cpu: cpus()[0]?.model ?? null, bun: Bun.version, recorded_at: new Date().toISOString(),
  source_revision: git('rev-parse', 'HEAD'), source_status: git('status', '--short'),
  executable, executable_sha256: createHash('sha256').update(readFileSync(executable)).digest('hex'),
  viewport_requested: [980, 820], samples_per_profile: samples,
  surface: host === 'macos' ? 'AppKit on macOS' : `Linux host headless software renderer on ${platform()}; no desktop compositor`,
  timing: 'Seekable agent clock. Loaded samples queue one 250 ms clock advance, then a type command, without waiting for the advance. Not a wall-clock producer.',
  measurement: 'Command send to native agent type acknowledgement, then exact echo in runner tree acknowledgement; includes IPC, queued work and inspection. Not OS input latency, native presentation, FPS or a display deadline.',
  view_count: null, view_count_reason: 'Runner tree node counts are reported separately; native live view count is not instrumented.',
  profiles: [],
};
const save = () => writeFileSync(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
const summarize = values => {
  const sorted = [...values].sort((a, b) => a - b);
  const at = q => sorted[Math.max(0, Math.ceil(q * sorted.length) - 1)] ?? null;
  return { count: values.length, p50: at(0.5), p95: at(0.95), p99: at(0.99), max: sorted.at(-1) ?? null };
};
const deadline = async promise => {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('native command exceeded 30 s')), 30_000);
    })]);
  } finally { clearTimeout(timer); }
};

for (const profile of ['idle100', 'eager1000']) {
  const row = { profile, status: 'running', samples: [] };
  report.profiles.push(row);
  save();
  let session;
  try {
    session = await open({ host, app: 'messages-stress', size: report.viewport_requested,
      env: host === 'linux' ? { EXACT_PAINTER: 'cpu' } : {} });
    row.agent_ready_ms = session.boot;
    if (profile === 'eager1000') {
      for (const control of ['history-1000', 'toggle-eager', 'batch-32', 'start']) {
        await deadline(session.tap(control));
      }
    }
    const tree = await deadline(session.tree());
    row.runner_nodes = tree.nodes.length;
    row.message_rows = tree.nodes.filter(n => n.props.testId?.startsWith('message-')).length;
    assert.equal(row.message_rows, profile === 'idle100' ? 100 : 1000);
    const input = tree.nodes.find(n => n.props.testId === 'stress-input').id;
    const echo = tree.nodes.find(n => n.props.testId === 'stress-echo').id;
    for (let i = 0; i < samples; i++) {
      const value = `${profile} draft ${i + 1} — native echo 🦀`;
      const started = performance.now();
      const tick = profile === 'eager1000'
        ? session.clock('+250').then(() => performance.now() - started)
        : Promise.resolve(null);
      // Numeric id avoids a full tree lookup before every input command.
      const typed = session.op({ op: 'type', id: input, text: value })
        .then(reply => ({ reply, ms: performance.now() - started }));
      const [updateAck, inputAck] = await deadline(Promise.all([tick, typed]));
      const after = await deadline(session.tree());
      assert.equal(after.nodes.find(n => n.id === echo)?.props.text, value);
      const stats = after.nodes.find(n => n.props.testId === 'stats').props.text;
      assert.match(stats, new RegExp(`revision ${profile === 'eager1000' ? i + 1 : 0}/120`));
      row.samples.push({ index: i + 1, update_ack_ms: updateAck, input_ack_ms: inputAck.ms,
        echo_tree_ack_ms: performance.now() - started, clock: inputAck.reply.clock, stats });
    }
    if (profile === 'eager1000') await deadline(session.tap('pause'));
    const before = (await deadline(session.layout())).nodes.find(n => n.testId === 'transcript');
    await deadline(session.tap('transcript', { wheel: [0, 600] }));
    const after = (await deadline(session.layout())).nodes.find(n => n.testId === 'transcript');
    row.scroll = { before_y: before.sy, after_y: after.sy };
    assert.ok(after.sy > before.sy, 'transcript must scroll through the native host');
    row.screenshot = resolve(output, `${profile}.png`);
    await deadline(session.screenshot(row.screenshot));
    await deadline(session.tap('send'));
    assert.equal((await deadline(session.find('stress-echo'))).props.text, '');
    assert.ok(await deadline(session.find('message-local-echo')));
    await deadline(session.tap('reset'));
    assert.match((await deadline(session.find('stats'))).props.text, /100 logical · 100 materialized rows · revision 0\/120/);
    row.input_ack_ms = summarize(row.samples.map(x => x.input_ack_ms));
    row.echo_tree_ack_ms = summarize(row.samples.map(x => x.echo_tree_ack_ms));
    row.update_ack_ms = summarize(row.samples.map(x => x.update_ack_ms).filter(x => x !== null));
    row.status = 'passed';
    row.logs = await deadline(session.logs());
  } catch (error) {
    row.status = 'failed';
    row.error = error.stack ?? String(error);
  } finally {
    await session?.close();
    save();
    console.log(JSON.stringify({ profile, status: row.status, input_ack_ms: row.input_ack_ms, error: row.error }));
  }
}
console.log(`Report: ${resolve(output, 'report.json')}`);
if (report.profiles.some(p => p.status !== 'passed')) process.exitCode = 1;
