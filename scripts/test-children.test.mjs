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
const alive = pid => { try { process.kill(pid, 0); return !/^Z/.test(spawnSync('ps', ['-o', 'stat=', '-p', String(pid)], { encoding: 'utf8' }).stdout.trim() || 'Z'); } catch { return false; } };
/** Until `done()` or `ms` pass: true if it came. */
const until = async (done, ms = 10_000) => { const end = Date.now() + ms; while (!done() && Date.now() < end) await new Promise(ok => setTimeout(ok, 50)); return done(); };
// A child that starts a grandchild leading its own group (as a dev loop's tools
// do), says its pid, and with `stubborn` ignores SIGTERM in both.
const parent = stubborn => `const { spawn } = require('node:child_process');
${stubborn ? "process.on('SIGTERM', () => {});" : ''}
const g = spawn(process.execPath, ['-e', ${JSON.stringify(`${stubborn ? "process.on('SIGTERM', () => {}); " : ''}setInterval(() => {}, 1000)`)}], { detached: true, stdio: 'ignore' });
console.log(g.pid); setInterval(() => {}, 1000);`;
const grandchild = child => new Promise(ok => child.stdout.once('data', d => ok(Number(String(d).trim()))));

test.skipIf(!posix)('stopAll ends a child and the grandchild it detached, a SIGTERM-deaf pair after the grace', async () => {
  for (const stubborn of [false, true]) {
    const child = spawnOwned(process.execPath, ['-e', parent(stubborn)], { stdio: ['ignore', 'pipe', 'ignore'] });
    const pid = await grandchild(child);
    expect(await until(() => alive(pid))).toBe(true);
    await new Promise(ok => setTimeout(ok, 400)); // a look at what it started
    await stopAll(stubborn ? 500 : 5_000);
    expect(await until(() => !alive(child.pid) && !alive(pid))).toBe(true);
  }
});

test.skipIf(!posix)('a test that times out still ends what it spawned, SIGTERM-deaf or not', () => {
  const dir = mkdtempSync(join(tmpdir(), 'exact-test-children-'));
  try {
    const file = join(dir, 'timeout.test.mjs'), out = join(dir, 'pids');
    writeFileSync(file, `import { afterEach, test } from 'bun:test';
import { writeFileSync } from 'node:fs';
import { spawnOwned, stopAll } from ${JSON.stringify(resolve(import.meta.dir, 'test-children.mjs'))};
afterEach(() => stopAll(1_000));
test('hangs', async () => {
  const child = spawnOwned(process.execPath, ['-e', ${JSON.stringify(parent(true))}], { stdio: ['ignore', 'pipe', 'ignore'] });
  const pid = await new Promise(ok => child.stdout.once('data', d => ok(Number(String(d).trim()))));
  await new Promise(ok => setTimeout(ok, 400));
  writeFileSync(${JSON.stringify(out)}, child.pid + ' ' + pid);
  await new Promise(() => {});
}, 3000);
`);
    const run = spawnSync(process.execPath, ['test', file], { encoding: 'utf8', timeout: 60_000 });
    expect(run.error).toBeUndefined();
    expect(`${run.stdout}${run.stderr}`).toMatch(/timed out/i);
    const [child, pid] = readFileSync(out, 'utf8').split(' ').map(Number);
    const end = Date.now() + 10_000;
    while ((alive(child) || alive(pid)) && Date.now() < end) spawnSync('sleep', ['0.05']);
    expect(alive(child)).toBe(false);
    expect(alive(pid)).toBe(false);
  } finally { rmSync(dir, { recursive: true, force: true }); }
}, 90_000);
