// scripts/test-children.mjs: a test's children, and what they start, end when
// the test ends, its body's own way or by bun:test's timeout. POSIX only.
import { afterEach, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnOwned, stopAll } from './test-children.mjs';

afterEach(() => stopAll());
const posix = process.platform !== 'win32';
const alive = pid => { try { process.kill(pid, 0); } catch { return false; } return !/^Z/.test(spawnSync('ps', ['-o', 'stat=', '-p', String(pid)], { encoding: 'utf8' }).stdout.trim() || 'Z'); };
const until = async (done, ms = 10_000) => { const end = Date.now() + ms; while (!done() && Date.now() < end) await new Promise(ok => setTimeout(ok, 50)); return done(); };
// A child that starts a grandchild leading its own group (as a dev loop's tools
// do), with `stubborn` both ignoring SIGTERM; it prints the grandchild's pid
// once the grandchild says its handler is in. Both end on their own after 60 s.
const parent = stubborn => {
  const deaf = stubborn ? "process.on('SIGTERM', () => {}); " : '';
  const grand = `${deaf}setTimeout(() => process.exit(0), 60000); console.log('ready'); setInterval(() => {}, 1000);`;
  return `${deaf}setTimeout(() => process.exit(0), 60000);
const g = require('node:child_process').spawn(process.execPath, ['-e', ${JSON.stringify(grand)}], { detached: true, stdio: ['ignore', 'pipe', 'ignore'] });
g.stdout.once('data', () => console.log(g.pid)); setInterval(() => {}, 1000);`;
};
const grandchild = child => new Promise(ok => child.stdout.once('data', d => ok(Number(String(d).trim()))));

test.skipIf(!posix)('stopAll ends a child and the grandchild it detached, a SIGTERM-deaf pair after the grace', async () => {
  for (const stubborn of [false, true]) {
    const child = spawnOwned(process.execPath, ['-e', parent(stubborn)], { stdio: ['ignore', 'pipe', 'ignore'] });
    const pid = await grandchild(child);
    expect(await until(() => child.known.has(pid))).toBe(true); // noted while its parent lives
    await stopAll(stubborn ? 500 : 5_000);
    expect(await until(() => !alive(child.pid) && !alive(pid))).toBe(true);
  }
});

test.skipIf(!posix)('stop() ends them asynchronously, keeping the event loop free', async () => {
  const child = spawnOwned(process.execPath, ['-e', parent(true)], { stdio: ['ignore', 'pipe', 'ignore'] });
  const pid = await grandchild(child);
  expect(await until(() => child.known.has(pid))).toBe(true);
  let ticks = 0; const ticker = setInterval(() => ticks++, 20);
  await child.stop(500);
  clearInterval(ticker);
  expect(ticks).toBeGreaterThan(5);
  expect(await until(() => !alive(child.pid) && !alive(pid))).toBe(true);
});

test.skipIf(!posix)('a test that times out still ends what it spawned, SIGTERM-deaf', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-test-children-'));
  let pids = [];
  try {
    const file = join(dir, 'timeout.test.mjs'), out = join(dir, 'pids');
    writeFileSync(file, `import { afterEach, test } from 'bun:test';
import { writeFileSync } from 'node:fs';
import { spawnOwned, stopAll } from ${JSON.stringify(resolve(import.meta.dir, 'test-children.mjs'))};
afterEach(() => stopAll(1_000));
test('hangs', async () => {
  const child = spawnOwned(process.execPath, ['-e', ${JSON.stringify(parent(true))}], { stdio: ['ignore', 'pipe', 'ignore'] });
  const pid = await new Promise(ok => child.stdout.once('data', d => ok(Number(String(d).trim()))));
  while (!child.known.has(pid)) await new Promise(ok => setTimeout(ok, 50));
  writeFileSync(${JSON.stringify(out)}, child.pid + ' ' + pid);
  await new Promise(() => {});
}, 5000);
`);
    const run = spawnSync(process.execPath, ['test', file], { encoding: 'utf8', timeout: 60_000 });
    expect(run.error).toBeUndefined();
    expect(`${run.stdout}${run.stderr}`).toMatch(/timed out/i);
    pids = readFileSync(out, 'utf8').split(' ').map(Number);
    const end = Date.now() + 10_000;
    while (pids.some(alive) && Date.now() < end) spawnSync('sleep', ['0.05']);
    expect(pids.filter(alive)).toEqual([]);
  } finally {
    for (const pid of pids) if (alive(pid)) { try { process.kill(pid, 'SIGKILL'); } catch {} }
    rmSync(dir, { recursive: true, force: true });
  }
}, 90_000);
