// Harmony's `seed --post` seeds a store nothing was written to unasked and refuses any other unless told to erase it
// (the lead's 2026-10-21 ask): the demo's dev server on this machine holds what people wrote, and an agent following
// the README would erase it. Run against servers of the test's own: one in memory, one keeping its data in a directory
// of the test's, and stand-ins answering as a dev server would; never the origin the app names.
import { afterEach, expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, readdirSync, rmSync } from 'node:fs';
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
  const r = spawnSync(process.execPath, ['seed.mjs', '--json', join(json, 'seed.json'), ...args], { cwd: BACKEND, encoding: 'utf8', timeout: 60_000 });
  return { ...r, wrote: readdirSync(json).length > 0 };
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

test('a store nothing was written to is seeded unasked; one seeded before is refused, nothing erased, unless --force', async () => {
  const origin = await server(BACKEND, true);
  expect(seed('--post', origin).status).toBe(0);
  expect((await people(origin)).length).toBe(6);
  // In memory or not, a store written to may hold what people wrote.
  const written = await sentinel(origin);
  const refused = seed('--post', origin);
  expect(refused.status).toBe(1);
  expect(refused.stderr).toContain("ERASE EVERYONE'S MESSAGES");
  expect(refused.stderr).toContain('Nothing was erased');
  expect(refused.stderr).toContain('--force');
  expect(await adaStatus(origin)).toBe(written);
  expect(seed('--post', '--force', origin).status).toBe(0);
  expect(await adaStatus(origin)).toBe('Shipping Harmony 🚀');
}, 120_000);

test('a store that keeps its data and was written to is refused, and erased by --force or --reset, before or after the origin', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'harmony-seed-store-'));
  dirs.push(dir);
  cpSync(resolve(BACKEND, 'snapback'), join(dir, 'snapback'), { recursive: true });
  const origin = await server(dir, false);
  // `bun exact.mjs seed --force <origin>` arrives as `--post --force <origin>`.
  for (const flags of [['--post', '--force', origin], ['--post', origin, '--force'], ['--post', '--reset', origin], ['--post', origin, '--reset']]) {
    const written = await sentinel(origin);
    const refused = seed('--post', origin);
    expect(refused.status).toBe(1);
    expect(refused.stderr).toContain('kept on disk');
    expect(await adaStatus(origin)).toBe(written);
    expect(seed(...flags).status).toBe(0);
    expect(await adaStatus(origin)).toBe('Shipping Harmony 🚀');
  }
}, 120_000);

test('an argument seed does not take, two origins, or an origin without --post is refused before anything is written', () => {
  for (const args of [['--post', '--origin', 'http://127.0.0.1:1'], ['--post', 'http://127.0.0.1:1', 'http://127.0.0.1:2'], ['http://127.0.0.1:1']]) {
    const r = seed(...args);
    expect(r.status).toBe(2);
    expect(r.wrote).toBe(false);
  }
});

/** A stand-in answering `/dev/status` with each of `statuses` in turn and a head of 0, keeping the instance each seed and reset names. */
function standIn(statuses) {
  const seen = { seeds: [], resets: [] };
  let n = 0;
  const s = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/dev/status') return Response.json(statuses[Math.min(n++, statuses.length - 1)]);
    if (path === '/changes') return Response.json({ seq: 0, store_id: 's1' });
    if (path === '/seed') { seen.seeds.push(req.headers.get('x-snapback-dev-instance')); return Response.json({ seeded: 1 }); }
    if (path === '/dev/reset') { seen.resets.push(req.headers.get('x-snapback-dev-instance')); return Response.json({ reset: true }); }
    return new Response('', { status: 404 });
  } });
  stands.push(s);
  return { origin: `http://127.0.0.1:${s.port}`, seen };
}
const stands = [];
/** `seed` run without blocking this process, which answers as the stand-in. */
async function seedAsync(...args) {
  const json = mkdtempSync(join(tmpdir(), 'harmony-seed-json-'));
  dirs.push(json);
  const p = Bun.spawn([process.execPath, 'seed.mjs', '--json', join(json, 'seed.json'), ...args], { cwd: BACKEND, stdout: 'pipe', stderr: 'pipe' });
  const [status, stderr] = await Promise.all([p.exited, new Response(p.stderr).text()]);
  return { status, stderr };
}
afterEach(() => { for (const s of stands.splice(0)) s.stop(true); });
const ok = { storeId: 's1', instance: 'i1', pid: 4242, personas: ['ada'], temporary: true };

test('unasked, a store nothing was written to is seeded by /seed (an upsert), never reset, naming the instance asked', async () => {
  const unasked = standIn([ok]);
  expect((await seedAsync('--post', unasked.origin)).status).toBe(0);
  expect(unasked.seen).toEqual({ seeds: ['i1'], resets: [] });
}, 60_000);

test('a malformed status, or a server replaced between its two readings, is never reset; the reset names the instance asked', async () => {
  for (const bad of [{ storeId: '', temporary: true }, { ...ok, instance: '' }, { ...ok, pid: undefined }, { ...ok, personas: [] }]) {
    const { origin, seen } = standIn([bad]);
    expect((await seedAsync('--post', '--force', origin)).status).toBe(1);
    expect(seen).toEqual({ seeds: [], resets: [] });
  }
  const replaced = standIn([ok, { ...ok, instance: 'i2' }]);
  const r = await seedAsync('--post', '--force', replaced.origin);
  expect(r.status).toBe(1);
  expect(r.stderr).toContain('changed while it was asked');
  expect(replaced.seen).toEqual({ seeds: [], resets: [] });
  const same = standIn([ok]);
  expect((await seedAsync('--post', '--force', same.origin)).status).toBe(0);
  expect(same.seen).toEqual({ seeds: [], resets: ['i1'] });
}, 60_000);
