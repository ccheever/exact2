#!/usr/bin/env bun
// Real host correctness drive. Timings are command acknowledgements, never FPS.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { platform, release, arch } from 'node:os';
import { resolve } from 'node:path';
import { open } from '../../scripts/agent.mjs';

const args = process.argv.slice(2);
const host = args.shift() ?? 'web';
assert.ok(['web', 'macos', 'linux'].includes(host), 'host must be web, macos or linux');
const options = new Map();
for (let i = 0; i < args.length; i += 2) {
  assert.ok(['--out', '--url', '--size', '--bytes', '--profile'].includes(args[i]) && args[i + 1], `invalid option ${args[i]}`);
  options.set(args[i], args[i + 1]);
}
const output = resolve(options.get('--out') ?? `apps/markdown-stress/target/${host}-${platform()}`);
const size = (options.get('--size') ?? '980x820').split('x').map(Number);
assert.ok(size.length === 2 && size.every(n => Number.isInteger(n) && n >= 390 && n <= 4000), '--size must be WIDTHxHEIGHT, each 390..4000');
const bytes = Number(options.get('--bytes') ?? 16384);
assert.ok([16384, 262144, 1048576, 4194304].includes(bytes), 'unsupported --bytes');
const profiles = options.has('--profile') ? [options.get('--profile')] : ['mixed', 'paragraph', 'code', 'table', 'blocks'];
assert.ok(profiles.every(p => ['mixed', 'paragraph', 'code', 'table', 'blocks'].includes(p)), 'unknown --profile');
mkdirSync(output, { recursive: true });
const report = {
  host, os: platform(), os_release: release(), arch: arch(), viewport_requested: size,
  budget_bytes: bytes, results: [],
  measurement: 'Agent command through state/tree acknowledgement, including inspection and IPC; no display FPS or OS input latency measurement.',
  surface: host === 'linux' ? `Linux host CPU raster on ${platform()}; no desktop compositor` : host,
  resizing: 'Column-width controls reflow content inside one window. This driver does not resize the OS window.',
};
let app;
async function command(work) {
  let timer;
  try { return await Promise.race([work, new Promise((_, reject) => { timer = setTimeout(() => reject(Error('command exceeded 30 seconds')), 30000); })]); }
  finally { clearTimeout(timer); }
}
const tap = name => command(app.tap(name));
const echo = async value => {
  await command(app.type('stress-input', value));
  assert.equal((await command(app.find('stress-echo'))).props.text, value);
};
try {
  app = await open({ host, app: 'markdown-stress', size,
    ...(host === 'web' ? { url: options.get('--url') ?? 'http://127.0.0.1:4321' } : {}),
    ...(host === 'linux' ? { env: { EXACT_PAINTER: 'cpu' } } : {}),
  });
  for (const profile of profiles) {
    await tap('reset');
    await tap(`profile-${profile}`);
    const before = performance.now();
    await tap(`size-${bytes}`);
    await echo(`Reading ${profile}: café 🦀`);
    await tap('width-wide');
    await tap('reparse');
    const state = await command(app.state());
    const doc = state.resources.doc;
    assert.equal(state.slots.draft, `Reading ${profile}: café 🦀`);
    assert.equal(doc.revision, 1);
    assert.ok(doc.sourceBytes <= bytes && doc.sourceBytes > bytes * 0.9);
    assert.ok(doc.materialized <= 40);
    const layoutBefore = (await command(app.layout())).nodes.find(n => n.testId === 'document');
    await command(app.tap('document', { wheel: [0, 400] }));
    const layoutAfter = (await command(app.layout())).nodes.find(n => n.testId === 'document');
    assert.ok(layoutAfter.sy > layoutBefore.sy, `${profile}: document must scroll`);
    await app.screenshot(resolve(output, `${profile}.png`));
    report.results.push({ profile, status: 'passed', sourceBytes: doc.sourceBytes, totalBlocks: doc.totalBlocks,
      materialized: doc.materialized, largestBlock: doc.largestBlock, command_sequence_ack_ms: performance.now() - before,
      scroll_before: layoutBefore.sy, scroll_after: layoutAfter.sy });
  }
  // Keep eager correctness bounded regardless of the opt-in giant input above.
  await tap('reset');
  await tap('profile-blocks');
  await tap('next-page');
  assert.equal((await command(app.state())).resources.doc.first, 40);
  await tap('toggle-eager');
  const eager = (await command(app.state())).resources.doc;
  assert.equal(eager.materialized, eager.totalBlocks);
  assert.ok(eager.totalBlocks > 100);
  await tap('start');
  await command(app.clock('+1000'));
  assert.equal((await command(app.state())).resources.doc.revision, 1);
  await tap('pause');
  await command(app.clock('+1000'));
  assert.equal((await command(app.state())).resources.doc.revision, 1);
  await tap('reset');
  assert.equal((await command(app.find('stress-echo'))).props.text, '');
  report.passed = true;
  report.logs = await command(app.logs());
} catch (error) {
  report.passed = false;
  report.error = error.stack ?? String(error);
  process.exitCode = 1;
} finally {
  await app?.close();
  writeFileSync(resolve(output, 'report.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
}
