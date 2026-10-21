// Harmony's `seed --post` seeds a temporary (in-memory) store unasked and refuses one that keeps its data unless told
// to erase it (the lead's 2026-10-21 ask): the demo's dev server on this machine holds what people wrote, and an
// agent following the README would erase it. Run against servers of the test's own: one in memory, one keeping its
// data in a directory of the test's, never the origin the app names.
import { afterEach, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';
import { spawnOwned, stopAll } from './test-children.mjs';

const ROOT = resolve(import.meta.dir, '..');
const BACKEND = resolve(ROOT, 'apps/harmony');
const dirs = [];
afterEach(async () => { await stopAll(); for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true }); });

/** A snapback4 dev server of this test's in `cwd` (`--memory`, or keeping its data there): its origin, proven its own. */
async function server(cwd, memory) {
  if (!existsSync(resolve(BACKEND, 'node_modules/snapback4/package.json'))) {
    expect(spawnSync(process.execPath, ['install', '--frozen-lockfile'], { cwd: BACKEND, timeout: 120_000 }).status).toBe(0);
  }
  const { resolveBinary } = await import(pathToFileURL(resolve(BACKEND, 'node_modules/snapback4/dist/bin-path.js')).href);
  const child = spawnOwned(resolveBinary(), ['dev', '--port', '0', '--no-watch', ...(memory ? ['--memory'] : [])],
    { cwd, stdio: ['ignore', 'pipe', 'ignore'], env: { ...process.env, SNAPBACK4_SUPERVISOR_PID: String(process.pid) } });
  let timer;
  const url = await Promise.race([
    new Promise((ok) => createInterface({ input: child.stdout }).once('line', (l) => ok(l.trim()))),
    new Promise((_, fail) => child.once('exit', (c) => fail(new Error(`snapback4 dev exited ${c}`)))),
    new Promise((_, fail) => { timer = setTimeout(() => fail(new Error('snapback4 dev printed no URL in 60 s')), 60_000); }),
  ]).finally(() => clearTimeout(timer));
  child.stdout.resume();
  const origin = new URL(url).origin;
  expect(origin).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);
  const status = await (await fetch(`${origin}/dev/status`)).json();
  expect(status.pid).toBe(child.pid);
  expect(status.temporary).toBe(memory);
  return origin;
}

const seed = (...args) => {
  const json = mkdtempSync(join(tmpdir(), 'harmony-seed-json-'));
  dirs.push(json);
  return spawnSync(process.execPath, ['seed.mjs', '--json', join(json, 'seed.json'), ...args], { cwd: BACKEND, encoding: 'utf8', timeout: 60_000 });
};
const people = async (origin) => (await (await fetch(`${origin}/q/people`, { method: 'POST',
  headers: { 'content-type': 'application/json', 'x-snapback-persona': 'ada' }, body: '{"args":{}}' })).json()).data;
let mark = 0;
/** Ada's status set to a sentinel no seed writes. */
async function sentinel(origin) {
  const status = `not seed data ${++mark}`;
  const r = await (await fetch(`${origin}/m/setProfile`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-snapback-persona': 'ada' },
    body: JSON.stringify({ id: `01HARMNYTEST${String(mark).padStart(14, '0')}`, args: { name: 'Ada Quill', handle: 'ada', status, hue: 1 }, newIds: [] }) })).json();
  expect(r.state).toBe('sent');
  return status;
}
const adaStatus = async (origin) => (await people(origin)).find((p) => p.handle === 'ada')?.status;

test('a temporary store is seeded unasked', async () => {
  const origin = await server(BACKEND, true);
  expect(seed('--post', origin).status).toBe(0);
  expect((await people(origin)).length).toBe(6);
  // and seeded again, whatever it holds: its data goes when it stops anyway.
  await sentinel(origin);
  expect(seed('--post', origin).status).toBe(0);
  expect(await adaStatus(origin)).toBe('Shipping Harmony 🚀');
}, 120_000);

test('a store that keeps its data is refused, nothing erased, and erased with --force or --reset in either order', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'harmony-seed-store-'));
  dirs.push(dir);
  cpSync(resolve(BACKEND, 'snapback'), join(dir, 'snapback'), { recursive: true });
  const origin = await server(dir, false);
  const refused = seed('--post', origin);
  expect(refused.status).toBe(1);
  expect(refused.stderr).toContain("ERASE EVERYONE'S MESSAGES");
  expect(refused.stderr).toContain('Nothing was erased');
  expect(refused.stderr).toContain('--force');
  expect(await people(origin)).toEqual([]);
  // Each flag erases on its own, the origin before or after it (`bun exact.mjs seed --force <origin>` arrives as
  // `--post --force <origin>`), and the next refusal leaves what was written.
  for (const flags of [['--post', '--force', origin], ['--post', origin, '--reset']]) {
    const written = await sentinel(origin);
    expect(seed('--post', origin).status).toBe(1);
    expect(await adaStatus(origin)).toBe(written);
    const r = seed(...flags);
    expect(r.status).toBe(0);
    expect(await adaStatus(origin)).toBe('Shipping Harmony 🚀');
  }
}, 120_000);

test('an argument seed does not take, two origins, or an origin without --post is refused before anything is written', () => {
  for (const args of [['--post', '--origin', 'http://127.0.0.1:1'], ['--post', 'http://127.0.0.1:1', 'http://127.0.0.1:2'], ['http://127.0.0.1:1']]) {
    const r = seed(...args);
    expect(r.status).toBe(2);
  }
});
