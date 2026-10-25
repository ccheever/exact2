// The JS build's GPU cache (host/web-js/gpu-cache.mjs), shared by every loop of one app on one target: two loops at
// once, one held between its build and its rename, each copy a complete cache; loops racing unheld do too; a warm
// rebuild skips the build; a failed build leaves the cache as it was, as does one killed between its renames; waiters
// on a lock a killed build left take it one at a time (withLock, host/web/serve.mjs). Each loop is a process running
// copyGpuCache with a build of its own (three files written a beat apart, each naming the loop), so a copy of a tree
// another loop is removing or rewriting shows as a missing file or a mix of names.
import { afterEach, expect, test } from 'bun:test';
import { spawnOwned, stopAll } from '../../scripts/test-children.mjs';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const MODULE = resolve(new URL('./gpu-cache.mjs', import.meta.url).pathname);
const SERVE = resolve(new URL('../web/serve.mjs', import.meta.url).pathname);
const FILES = ['gpu.js', 'gpu_bg.wasm', 'shaders/main.wgsl'];
afterEach(() => stopAll());

const LOOP = `
import { copyGpuCache } from ${JSON.stringify(MODULE)};
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
const [cache, out, name, mode, hold] = process.argv.slice(2);
const nap = ms => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
const built = copyGpuCache(cache, out, {
  // Fresh when it holds this loop's mode's mark: 'warm' takes any complete cache, 'stale' never does.
  fresh: dir => mode === 'warm' && existsSync(resolve(dir, 'gpu_bg.wasm')),
  build: stage => {
    if (mode === 'fail') throw new Error('no GPU crate');
    for (const file of ${JSON.stringify(FILES)}) {
      mkdirSync(dirname(resolve(stage, file)), { recursive: true });
      writeFileSync(resolve(stage, file), name.repeat(4096));
      nap(40);
    }
  },
  beforeRename: () => { if (!hold) return; writeFileSync(hold + '.held', ''); while (existsSync(hold)) nap(10); },
});
console.log(JSON.stringify({ built }));
`;

let dir;
afterEach(() => { if (dir) rmSync(dir, { recursive: true, force: true }); dir = undefined; });
function setup() {
  dir = mkdtempSync(join(tmpdir(), 'exact-gpu-cache-'));
  writeFileSync(join(dir, 'loop.mjs'), LOOP);
  return { cache: join(dir, 'target/web-js-gpu/app') };
}
/** A loop run to its end: what it printed and its exit code. */
function loop(cache, name, mode = 'stale', hold = '') {
  const out = join(dir, `out-${name}`);
  const child = spawnOwned(process.execPath, [join(dir, 'loop.mjs'), cache, out, name, mode, hold], { stdio: ['ignore', 'pipe', 'pipe'] });
  let said = '';
  child.stdout.on('data', data => { said += data; }); child.stderr.on('data', data => { said += data; });
  return { out, done: new Promise(ok => child.once('exit', code => ok({ code, said }))) };
}
/** The loop name every file of the copy at `out` holds, or what is wrong with it. */
function whose(out) {
  const names = FILES.map(file => existsSync(join(out, file)) ? readFileSync(join(out, file), 'utf8') : '');
  if (names.some(text => text.length !== 4096 || text !== names[0])) return `incomplete: ${names.map(text => `${text[0] ?? '-'}×${text.length}`).join(' ')}`;
  return names[0][0];
}
const waitFor = async path => { for (let i = 0; !existsSync(path); i++) { if (i > 3000) throw new Error(`${path} never came`); await Bun.sleep(10); } };
const leftovers = cache => readdirSync(join(cache, '..')).filter(name => name !== 'app').sort();

test('two loops at once, one held before its rename, each copy a complete cache', async () => {
  const { cache } = setup(), hold = join(dir, 'hold');
  writeFileSync(hold, '');
  const a = loop(cache, 'a', 'stale', hold);
  await waitFor(`${hold}.held`);
  // While a's stage waits, b builds, renames and copies its own.
  const b = loop(cache, 'b');
  expect((await b.done).code).toBe(0);
  expect(whose(b.out)).toBe('b');
  rmSync(hold);
  const ran = await a.done;
  expect(ran.code).toBe(0);
  expect(JSON.parse(ran.said)).toEqual({ built: true });
  expect(whose(a.out)).toBe('a');
  expect(whose(cache)).toBe('a');
  expect(leftovers(cache)).toEqual([]);
}, 60_000);

test('loops racing unheld each copy a complete cache', async () => {
  const { cache } = setup();
  const loops = ['c', 'd', 'e', 'f'].map(name => ({ name, ...loop(cache, name) }));
  for (const { name, out, done } of loops) {
    expect((await done).code).toBe(0);
    expect(whose(out)).toBe(name);
  }
  expect(['c', 'd', 'e', 'f']).toContain(whose(cache));
  expect(leftovers(cache)).toEqual([]);
}, 60_000);

test('a warm rebuild copies the cache without building', async () => {
  const { cache } = setup();
  expect(JSON.parse((await loop(cache, 'g').done).said)).toEqual({ built: true });
  const warm = loop(cache, 'h', 'warm'), ran = await warm.done;
  expect(JSON.parse(ran.said)).toEqual({ built: false });
  expect(whose(warm.out)).toBe('g');
}, 60_000);

test('a warm build sweeps a killed build\'s stage and a .previous left beside the cache', async () => {
  const { cache } = setup();
  await loop(cache, 'm').done;
  tree(`${cache}.stage-${deadPid()}-1`, 'n');
  tree(`${cache}.previous`, 'o');
  const warm = loop(cache, 'r', 'warm');
  expect(JSON.parse((await warm.done).said)).toEqual({ built: false });
  expect(whose(warm.out)).toBe('m');
  expect(leftovers(cache)).toEqual([]);
}, 60_000);

test('a failed build leaves the cache as it was and no stage', async () => {
  const { cache } = setup();
  await loop(cache, 'i').done;
  const failed = await loop(cache, 'j', 'fail').done;
  expect(failed.code).not.toBe(0);
  expect(failed.said).toContain('no GPU crate');
  expect(whose(cache)).toBe('i');
  expect(leftovers(cache)).toEqual([]);
}, 60_000);

/** A pid no process has now: a child's, once it has exited. */
const deadPid = () => spawnSync(process.execPath, ['-e', ''], { stdio: 'ignore' }).pid;
// The lock's waiters end on their own (a lock held 30 s throws): plain children, ended here if a test fails, not owned
// (an owned child is a look at the machine's process table each, eighty of them here).
const waiting = new Set();
afterEach(() => { for (const child of waiting) child.kill('SIGKILL'); waiting.clear(); });
/** A cache tree at `path` whose every file names `name`. */
function tree(path, name) {
  for (const file of FILES) { mkdirSync(join(path, file, '..'), { recursive: true }); writeFileSync(join(path, file), name.repeat(4096)); }
}

test('a build killed between its renames leaves the cache it replaced, put back by the next', async () => {
  const { cache } = setup(), dead = deadPid();
  // As that build left it: its lock, the cache it replaced at .previous, its finished stage, no cache.
  tree(`${cache}.previous`, 'p');
  tree(`${cache}.stage-${dead}-1`, 'q');
  writeFileSync(`${cache}.lock`, String(dead));
  const failed = await loop(cache, 'k', 'fail').done;
  expect(failed.code).not.toBe(0);
  expect(whose(cache)).toBe('p');
  expect(leftovers(cache)).toEqual([]);
  const warm = loop(cache, 'l', 'warm');
  expect(JSON.parse((await warm.done).said)).toEqual({ built: false });
  expect(whose(warm.out)).toBe('p');
}, 60_000);

test('waiters on a lock a killed build left take it one at a time', async () => {
  dir = mkdtempSync(join(tmpdir(), 'exact-gpu-cache-'));
  const lock = join(dir, 'app.lock'), inside = join(dir, 'inside');
  writeFileSync(join(dir, 'waiter.mjs'), `
import { withLock } from ${JSON.stringify(SERVE)};
import { existsSync, rmSync, writeFileSync } from 'node:fs';
const nap = ms => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
// All at once, so they find the dead owner together; a second holder at once finds the first's mark and fails.
writeFileSync(${JSON.stringify(dir)} + '/ready-' + process.pid, '');
while (!existsSync(${JSON.stringify(join(dir, 'go'))})) nap(1);
withLock(${JSON.stringify(lock)}, () => { writeFileSync(${JSON.stringify(inside)}, '', { flag: 'wx' }); nap(20); rmSync(${JSON.stringify(inside)}); });
`);
  for (let round = 0; round < 10; round++) {
    writeFileSync(lock, String(deadPid()));
    // Every other round a waiter was killed inside its steal too, its `.steal` left behind.
    if (round % 2) writeFileSync(`${lock}.steal`, String(deadPid()));
    rmSync(join(dir, 'go'), { force: true });
    for (const name of readdirSync(dir)) if (name.startsWith('ready-')) rmSync(join(dir, name));
    const waiters = Array.from({ length: 8 }, () => {
      const child = spawn(process.execPath, [join(dir, 'waiter.mjs')], { stdio: ['ignore', 'ignore', 'pipe'] });
      waiting.add(child);
      let said = '';
      child.stderr.on('data', data => { said += data; });
      return new Promise(ok => child.once('exit', code => ok({ code, said })));
    });
    while (readdirSync(dir).filter(name => name.startsWith('ready-')).length < 8) await Bun.sleep(5);
    writeFileSync(join(dir, 'go'), '');
    for (const ran of await Promise.all(waiters)) expect(ran).toEqual({ code: 0, said: '' });
    expect(readdirSync(dir).filter(name => name.startsWith('app.lock'))).toEqual([]);
  }
  // A live waiter's `.steal` (this process's pid) is honoured: the lock waits for it, then is taken.
  writeFileSync(lock, String(deadPid()));
  writeFileSync(`${lock}.steal`, String(process.pid));
  rmSync(join(dir, 'go'), { force: true });
  writeFileSync(join(dir, 'go'), '');
  const child = spawn(process.execPath, [join(dir, 'waiter.mjs')], { stdio: ['ignore', 'ignore', 'pipe'] });
  waiting.add(child);
  const exited = new Promise(ok => child.once('exit', ok));
  await Bun.sleep(1500);
  expect(child.exitCode).toBe(null);
  expect(existsSync(lock)).toBe(true);
  rmSync(`${lock}.steal`);
  expect(await exited).toBe(0);
  expect(readdirSync(dir).filter(name => name.startsWith('app.lock'))).toEqual([]);
}, 120_000);
