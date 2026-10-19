// scripts/test-children.mjs: a test's children, and what they start, end when
// the test ends, its body's own way or by bun:test's timeout.
import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from './test-children.mjs';

const alive = pid => { try { process.kill(pid, 0); return true; } catch { return false; } };
// A child that starts a grandchild leading its own group (as a dev loop's tools do) and says its pid.
const parent = `const { spawn } = require('node:child_process');
const g = spawn(process.execPath, ['-e', 'setTimeout(() => {}, 300000)'], { detached: true, stdio: 'ignore' });
console.log(g.pid); setTimeout(() => {}, 300000);`;
const grandchild = child => new Promise(ok => child.stdout.once('data', d => ok(Number(String(d).trim()))));

test('stopAll ends a child and the grandchild it detached', async () => {
  const child = spawnOwned(process.execPath, ['-e', parent], { stdio: ['ignore', 'pipe', 'ignore'] });
  const pid = await grandchild(child);
  expect(alive(pid)).toBe(true);
  await stopAll(2_000);
  await new Promise(ok => setTimeout(ok, 200));
  expect(alive(child.pid)).toBe(false);
  expect(alive(pid)).toBe(false);
});

test('a test that times out still ends what it spawned', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-test-children-'));
  try {
    const file = join(dir, 'timeout.test.mjs'), out = join(dir, 'pids');
    writeFileSync(file, `import { test } from 'bun:test';
import { writeFileSync } from 'node:fs';
import { spawnOwned } from ${JSON.stringify(resolve(import.meta.dir, 'test-children.mjs'))};
test('hangs', async () => {
  const child = spawnOwned(process.execPath, ['-e', ${JSON.stringify(parent)}], { stdio: ['ignore', 'pipe', 'ignore'] });
  const pid = await new Promise(ok => child.stdout.once('data', d => ok(Number(String(d).trim()))));
  writeFileSync(${JSON.stringify(out)}, child.pid + ' ' + pid);
  await new Promise(() => {});
}, 3000);
`);
    const run = spawnSync(process.execPath, ['test', file], { encoding: 'utf8', timeout: 60_000 });
    expect(run.status).not.toBe(0); // the test timed out
    const [child, pid] = readFileSync(out, 'utf8').split(' ').map(Number);
    spawnSync('sleep', ['0.3']);
    expect(alive(child)).toBe(false);
    expect(alive(pid)).toBe(false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 90_000);
