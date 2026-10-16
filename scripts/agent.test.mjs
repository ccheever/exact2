// The driver's own words (scripts/agent.mjs): its grammar, how it renders a tree, and what it says when a native app hangs up.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

test('the driver grammar names an unquoted mediasession action beside pinch', () => {
  const source = readFileSync(new URL('./agent.mjs', import.meta.url), 'utf8');
  const header = source.split('\n').slice(0, 16).join('\n');
  const form = 'pinch <scale> [at <x> <y>] | mediasession <action> [seconds]';
  assert.ok(header.includes(form), header);
  const usage = spawnSync(process.execPath, [resolve(import.meta.dir, 'agent.mjs')], { encoding: 'utf8' });
  assert.equal(usage.status, 2, usage.stderr);
  assert.ok(usage.stderr.includes(form), usage.stderr);
}, 60_000);

test('rendered tree keeps focus; an accessible name is tree --ax\'s (LLP 1080.002)', async () => {
  const { render } = await import('./agent.mjs');
  const line = render('tree', {nodes:[{id:1, depth:0, type:'View', props:{testId:'play'}, focused:true}]});
  assert.match(line, /\[focused\]/);
  assert.doesNotMatch(line, /name=/);
});

test('a native hangup names how the app ended, its crash reports, and only its last 20 lines', async () => {
  const { hangup } = await import('./agent.mjs');
  const hostLines = Array.from({ length: 30 }, (_, i) => `app: line ${i}`);
  const hung = hangup({ what: 'the app hung up', pid: 999999, exit: { code: null, signal: 'SIGKILL' }, reports: ['/r/Caltrain-1.ips'], hostLines });
  assert.match(hung, /^the app hung up \(killed by SIGKILL\)\ncrash report: \/r\/Caltrain-1\.ips\napp: line 10\n/);
  assert.doesNotMatch(hung, /line 9\n/);
  assert.match(hangup({ what: 'clock did not answer', pid: process.pid }), /^clock did not answer \(pid \d+ still running\)$/);
  assert.match(hangup({ what: 'the app hung up', pid: 999999 }), /\(pid 999999 gone\)$/);
  assert.match(hangup({ what: 'the app exited', exit: { code: 3, signal: null } }), /\(exit code 3\)$/);
});
