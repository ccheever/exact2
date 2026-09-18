#!/usr/bin/env bun
// Opt-in queued-work/resize acknowledgement samples; never physical FPS.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { arch, cpus, platform, release } from 'node:os';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';
import { resolveApp } from '../../scripts/app.mjs';
import { appleArtifacts } from '../../host/apple/build.mjs';

const args = process.argv.slice(2);
const host = args.shift();
assert.ok(['macos', 'linux'].includes(host), 'usage: native-sample.mjs <macos|linux> [--samples 1..40] [--out directory]');
const options = new Map();
for (let i = 0; i < args.length; i += 2) {
  assert.ok(['--samples', '--out'].includes(args[i]) && args[i + 1], `invalid option ${args[i]}`);
  options.set(args[i], args[i + 1]);
}
const samples = Number(options.get('--samples') ?? 12);
assert.ok(Number.isInteger(samples) && samples >= 1 && samples <= 40, 'samples must be an integer in 1..40');
const output = resolve(options.get('--out') ?? `apps/markdown-stress/target/${host}-samples-${platform()}`);
mkdirSync(output, { recursive: true });
const appInfo = resolveApp('markdown-stress');
const executable = host === 'macos'
  ? process.env.EXACT_MAC_BIN ?? appleArtifacts(appInfo).binary
  : process.env.EXACT_LINUX_BIN ?? resolve(appInfo.target, 'release/markdown-stress-linux');
const percentile = values => {
  const v = values.toSorted((a, b) => a - b);
  const q = p => v[Math.ceil(v.length * p) - 1];
  return { count: v.length, p50: q(.5), p95: q(.95), max: v.at(-1) };
};
const git = (...argv) => spawnSync('git', argv, { cwd: appInfo.workspace, encoding: 'utf8' }).stdout.trim();
const report = {
  host, os: platform(), os_release: release(), arch: arch(), cpu: cpus()[0]?.model,
  clock: 'agent seekable', executable, sha256: createHash('sha256').update(readFileSync(executable)).digest('hex'),
  revision: git('rev-parse', 'HEAD'), source_status: git('status', '--short'),
  recorded_at: new Date().toISOString(), profiles: [],
  note: 'Raw command send to native acknowledgment; includes queued work and IPC. Tree echo verified separately. Not OS input or physical presentation timing.',
  surface: host === 'macos' ? 'AppKit window on macOS' : `Linux host CPU raster on ${platform()}; no desktop compositor`,
};
async function deadline(work) {
  let timer;
  try { return await Promise.race([work, new Promise((_, reject) => { timer = setTimeout(() => reject(Error('command exceeded 30 seconds')), 30000); })]); }
  finally { clearTimeout(timer); }
}
for (const profile of ['idle40', 'reparse1m40', 'resize419']) {
  let app;
  const result = { profile, samples: [] };
  report.profiles.push(result);
  try {
    app = await open({ host, app: 'markdown-stress', size: [980, 820], env: host === 'linux' ? { EXACT_PAINTER: 'cpu' } : {} });
    await deadline(app.tap('profile-blocks'));
    if (profile === 'reparse1m40') await deadline(app.tap('size-1048576'));
    if (profile === 'resize419') await deadline(app.tap('toggle-eager'));
    const initial = await deadline(app.state());
    result.document = { ...initial.resources.doc };
    delete result.document.blocks;
    const tree = await deadline(app.tree());
    const id = name => tree.nodes.find(n => n.props.testId === name).id;
    const input = id('stress-input'), echo = id('stress-echo'), reparse = id('reparse');
    for (let i = 0; i < samples; i++) {
      const value = `${profile} sample ${i + 1} 🦀`;
      const start = performance.now();
      const work = profile === 'reparse1m40' ? app.op({ op: 'tap', id: reparse })
        : profile === 'resize419' ? app.op({ op: 'tap', resize: [i % 2 ? 980 : 640, 820] }) : Promise.resolve(null);
      const typed = app.op({ op: 'type', id: input, text: value }).then(r => ({ r, ms: performance.now() - start }));
      const [w, t] = await deadline(Promise.all([work, typed]));
      const after = await deadline(app.tree());
      assert.equal(after.nodes.find(n => n.id === echo).props.text, value);
      if (profile === 'resize419') assert.deepEqual(w.viewport, [i % 2 ? 980 : 640, 820]);
      result.samples.push({ index: i, input_ack_ms: t.ms, echo_tree_ack_ms: performance.now() - start, work: w });
    }
    assert.equal((await deadline(app.state())).resources.doc.revision, profile === 'reparse1m40' ? samples : 0);
    result.input_ack_ms = percentile(result.samples.map(s => s.input_ack_ms));
    result.passed = true;
  } catch (error) {
    result.passed = false;
    result.error = error.stack;
    process.exitCode = 1;
  } finally {
    await app?.close();
    writeFileSync(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
    console.log(JSON.stringify({ profile, ...result.input_ack_ms, passed: result.passed, error: result.error }));
  }
}
