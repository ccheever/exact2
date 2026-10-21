// Harmony's `seed --post` refuses to empty a server that holds data unless told to (the lead's 2026-10-21 ask): a
// dev server on this machine is the live demo people use, and an agent following the README would erase it. Run
// against a Snapback server of the test's own, in memory, never the origin the app names.
import { expect, test } from 'bun:test';
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

const ROOT = resolve(import.meta.dir, '..');
const BACKEND = resolve(ROOT, 'apps/harmony');

test('seed --post seeds an empty server, refuses one that holds data, and erases it with --force', async () => {
  if (!existsSync(resolve(BACKEND, 'node_modules/snapback4/package.json'))) {
    expect(spawnSync(process.execPath, ['install', '--frozen-lockfile'], { cwd: BACKEND }).status).toBe(0);
  }
  const { resolveBinary } = await import(pathToFileURL(resolve(BACKEND, 'node_modules/snapback4/dist/bin-path.js')).href);
  const server = spawn(resolveBinary(), ['dev', '--port', '0', '--memory', '--no-watch'],
    { cwd: BACKEND, stdio: ['ignore', 'pipe', 'ignore'], env: { ...process.env, SNAPBACK4_SUPERVISOR_PID: String(process.pid) } });
  const dir = mkdtempSync(join(tmpdir(), 'harmony-seed-'));
  try {
    const origin = new URL(await new Promise((ok) => createInterface({ input: server.stdout }).once('line', ok))).origin;
    server.stdout.resume();
    const status = await (await fetch(`${origin}/dev/status`)).json();
    expect(status.pid).toBe(server.pid);
    expect(status.temporary).toBe(true);
    const seed = (...flags) => spawnSync(process.execPath, ['seed.mjs', '--json', join(dir, 'seed.json'), '--post', origin, ...flags], { cwd: BACKEND, encoding: 'utf8' });
    const people = async () => (await (await fetch(`${origin}/q/people`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-snapback-persona': 'ada' }, body: '{"args":{}}' })).json()).data;
    // Empty: seeded.
    expect(seed().status).toBe(0);
    expect((await people()).length).toBe(6);
    // Holding data (a change someone made since): refused, nothing erased.
    const sent = await (await fetch(`${origin}/m/setProfile`, { method: 'POST', headers: { 'content-type': 'application/json', 'x-snapback-persona': 'ada' },
      body: JSON.stringify({ id: '01HARMNYTEST00000000000001', args: { name: 'Ada Quill', handle: 'ada', status: 'not seed data', hue: 1 }, newIds: [] }) })).json();
    expect(sent.state).toBe('sent');
    const refused = seed();
    expect(refused.status).toBe(1);
    expect(refused.stderr).toContain("ERASE EVERYONE'S MESSAGES");
    expect(refused.stderr).toContain('--force');
    expect((await people()).find((p) => p.handle === 'ada').status).toBe('not seed data');
    expect((await people()).length).toBe(6);
    // --force (or --reset): erased and seeded again.
    expect(seed('--force').status).toBe(0);
    expect(seed('--reset').status).toBe(0);
    expect((await people()).find((p) => p.handle === 'ada').status).toBe('Shipping Harmony 🚀');
  } finally {
    server.kill('SIGTERM');
    if (server.exitCode === null && server.signalCode === null) await new Promise((ok) => server.once('exit', ok));
    rmSync(dir, { recursive: true, force: true });
  }
}, 120_000);
