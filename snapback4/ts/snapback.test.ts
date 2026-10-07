// The web path end to end: this driver and the wasm (built by web/build.mjs)
// against a real `snapback4 dev`, the partition persisted in SQLite as
// Exact's storage keeps it. `bun test snapback4/ts` from the checkout root.

import { afterAll, beforeAll, expect, test } from 'bun:test';
import { Database } from 'bun:sqlite';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { Snapback } from './snapback.ts';
import { commitChanges, readKept, type Change, type SqliteStorage } from './web.ts';

const SCHEMA = `
use identity
table messages:
  author: principal
  body: text <=50
  at: time
  by byTime: at, id
  public 'messages are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  sync public last 100 by byTime

query inbox():
  return messages last 50 by byTime

mutation send(body: text <=50):
  require body != '' else EMPTY
  row = insert messages { author: viewer, body, at: now }
  return { id: row.id }
`;

const scratch = mkdtempSync(join(tmpdir(), 'exact-snapback4-ts-'));
const wasm = join(scratch, 'snapback4.wasm');
let server: ChildProcess;
let origin = '';

beforeAll(async () => {
  const built = spawnSync('bun', [resolve(import.meta.dir, '../web/build.mjs'), wasm], { stdio: 'inherit' });
  if (built.status !== 0) throw new Error('web/build.mjs failed');
  const project = join(scratch, 'server');
  mkdirSync(join(project, 'snapback'), { recursive: true });
  writeFileSync(join(project, 'snapback/schema.q'), SCHEMA);
  const binary = process.env.SNAPBACK4_BIN ?? resolve(import.meta.dir, '../../node_modules/snapback4-darwin-arm64/snapback4');
  server = spawn(binary, ['dev', '--port', '0', '--memory', '--no-watch'], { cwd: project, stdio: ['ignore', 'pipe', 'ignore'] });
  origin = await new Promise<string>((done, fail) => {
    const lines = createInterface({ input: server.stdout! });
    lines.on('line', line => { if (/^http:\/\/127\.0\.0\.1:\d+$/.test(line.trim())) done(line.trim()); });
    server.on('exit', code => fail(new Error(`snapback4 dev exited ${code}`)));
  });
}, 600_000);

afterAll(() => {
  server?.kill();
  rmSync(scratch, { recursive: true, force: true });
});

/** Exact's `storage.sqlite` as the web host runs it, over bun:sqlite: one
 * ordered queue of operations, its bounds (10,000 commands a transaction;
 * 16 MiB of parameters; 100,000 rows or 16 MiB a result), and a connection
 * that closes when a commit's write fails. `faults` arms one failed commit
 * or slows commits down. */
type Faults = { failNext?: boolean; failCommits?: number; failOpens?: number; openDelay?: number; delay?: number; commits?: number; log?: string[]; handles?: number };
function storage(dir: string, faults: Faults = {}): SqliteStorage {
  mkdirSync(dir, { recursive: true });
  let queue: Promise<unknown> = Promise.resolve();
  const ordered = <T>(work: () => T | Promise<T>): Promise<T> => {
    const next = queue.then(work);
    queue = next.catch(() => undefined);
    return next;
  };
  const bytes = (values: unknown[]) => values.reduce<number>((n, v) => n + 24 + (typeof v === 'string' ? Buffer.byteLength(v) : 0), 0);
  return { sqlite: { async open(path: string) {
    if (faults.openDelay) await new Promise(r => setTimeout(r, faults.openDelay));
    if (faults.failOpens) { faults.failOpens--; throw new Error('storage unavailable'); }
    const db = new Database(join(dir, path.replace('app:/data/', '')));
    let open = true;
    faults.handles = (faults.handles ?? 0) + 1;
    const live = () => { if (!open) throw new Error('database is closed'); };
    return {
      execute: (sql: string) => ordered(() => { live(); db.run(sql); }),
      query: (sql: string, params: unknown[] = []) => ordered(() => {
        live();
        if (bytes(params) > 16 << 20) throw new Error('SQLite parameters exceed 16 MiB limit');
        const rows = db.query(sql).values(...(params as never[])) as unknown[][];
        if (rows.length > 100000) throw new Error('query result exceeds row limit');
        if (rows.reduce<number>((n, row) => n + bytes(row), 0) > 16 << 20) throw new Error('query result exceeds 16 MiB limit');
        return { rows };
      }),
      transaction: (commands: { sql: string; params?: unknown[] }[]) => ordered(async () => {
        live();
        if (commands.length > 10000) throw new Error('transaction exceeds 10000 commands or is not an array');
        if (faults.delay) await new Promise(r => setTimeout(r, faults.delay));
        for (const command of commands) if (bytes(command.params ?? []) > 16 << 20) throw new Error('SQLite parameters exceed 16 MiB limit');
        if (faults.failNext || faults.failCommits) {
          // Exact: the file write failed, so the file never holds this
          // commit, and every handle to it is closed.
          if (faults.failNext) faults.failNext = false; else faults.failCommits!--;
          open = false; db.close(); faults.handles!--;
          throw new Error('storage write failed');
        }
        db.transaction(() => { for (const { sql, params } of commands) db.query(sql).run(...((params ?? []) as never[])); })();
        faults.commits = (faults.commits ?? 0) + 1;
        faults.log?.push('commit');
      }),
      close: () => ordered(() => { if (open) { open = false; db.close(); faults.handles!--; } }),
    };
  } } };
}

const open = (persona: string, dir: string, at = origin, faults: Faults = {}) => Snapback.open({
  app: 'test.exact.snapback4', name: 'inbox', origin: at, viewer: `dev:${persona}`,
  headers: () => ({ 'x-snapback-persona': persona }), storage: storage(dir, faults), wasm: `file://${wasm}`,
});

const bodies = (rows: { body: string; pending?: boolean }[]) => rows.map(row => [row.body, row.pending === true]);

test('two web devices sync, send, keep offline writes across a reopen, refuse and poll', async () => {
  const aliceDir = join(scratch, 'alice'), bobDir = join(scratch, 'bob');
  const alice = await open('alice', aliceDir);
  expect(alice.isOpen).toBe(false);
  expect(await alice.sync()).toEqual({ ok: true });
  expect((await alice.read('inbox', {}, Date.now())).data).toEqual([]);

  const written = await alice.write('send', { body: 'hello' }, Date.now());
  expect(written.state).toBe('pending');
  expect(bodies((await alice.read<{ body: string; pending?: boolean }[]>('inbox', {}, Date.now())).data!)).toEqual([['hello', true]]);
  expect(await alice.sync()).toEqual({ ok: true });
  const outcome = await alice.outcome(written.id);
  expect(outcome.state).toBe('sent');
  expect((outcome.result as { id: string }).id).toBe((written as { newIds: string[] }).newIds[0]);

  const bob = await open('bob', bobDir);
  expect(await bob.sync()).toEqual({ ok: true });
  expect(bodies((await bob.read<{ body: string }[]>('inbox', {}, Date.now())).data!)).toEqual([['hello', false]]);
  await bob.close();
  // The partition is bound to its origin: another origin cannot open it.
  await expect(open('bob', bobDir, 'http://127.0.0.1:9')).rejects.toThrow(/another app, origin, or viewer/);
  await alice.close();
}, 60_000);

test('offline writes survive a reopen and send later; refusals withdraw; the poll sees others', async () => {
  const carolDir = join(scratch, 'carol');
  let carol = await open('carol', carolDir);
  expect(await carol.sync()).toEqual({ ok: true });
  // Lose the link: every request fails before it reaches the server.
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    if (String(input).startsWith(origin)) throw new TypeError('offline');
    return realFetch(input as never, init);
  }) as typeof fetch;
  let pending;
  try {
    pending = await carol.write('send', { body: 'from the tunnel' }, Date.now());
    const done = await carol.sync();
    expect(done.ok).toBe(false);
    expect(done.offline).toBe(true);
  } finally { globalThis.fetch = realFetch; }
  await carol.close();
  carol = await open('carol', carolDir);
  expect(carol.isOpen).toBe(true);
  const status = await carol.status();
  expect(status.queued?.[0]?.id).toBe(pending!.id);
  expect(bodies((await carol.read<{ body: string; pending?: boolean }[]>('inbox', {}, Date.now())).data!)[0]).toEqual(['from the tunnel', true]);

  const dave = await open('dave', join(scratch, 'dave'));
  expect(await dave.sync()).toEqual({ ok: true });
  expect(await dave.poll(0)).toBe(false);
  expect(await carol.sync()).toEqual({ ok: true });
  expect((await carol.status()).queued).toEqual([]);
  expect(await dave.poll(0)).toBe(true);
  expect(await dave.sync()).toEqual({ ok: true });
  expect(bodies((await dave.read<{ body: string }[]>('inbox', {}, Date.now())).data!)[0]).toEqual(['from the tunnel', false]);

  const refused = await carol.write('send', { body: '' }, Date.now());
  if (refused.state === 'pending') {
    expect(await carol.sync()).toEqual({ ok: true });
    const outcome = await carol.outcome(refused.id);
    expect(outcome.state).toBe('failed');
    expect(outcome.why?.code).toBe('EMPTY');
  } else expect(refused.why.code).toBe('EMPTY');
  expect((await carol.status()).queued).toEqual([]);
  await carol.close();
  await dave.close();
}, 60_000);

test('commits and reopens stay within the storage bounds, in order', async () => {
  const db = await storage(join(scratch, 'bounds')).sqlite.open('app:/data/bounds.sqlite');
  await db.execute('CREATE TABLE IF NOT EXISTS snapback_device (s TEXT NOT NULL, k TEXT NOT NULL, v TEXT NOT NULL, PRIMARY KEY (s, k)) WITHOUT ROWID');
  // 130,000 changes, more than a transaction's commands or a result's rows;
  // keys set, overwritten and deleted within one drain end as the last says.
  const changes: Change[] = [];
  for (let i = 0; i < 120_000; i++) changes.push({ s: 'r', k: `row:${i}`, v: JSON.stringify({ i, pad: 'x'.repeat(40) }) });
  for (let i = 0; i < 5_000; i++) changes.push({ s: 'r', k: `row:${i}`, v: null });
  for (let i = 0; i < 5_000; i++) changes.push({ s: 'r', k: `row:${i * 2}`, v: '"back"' });
  await commitChanges(db, changes);
  const kept = await readKept(db);
  expect(kept.length).toBe(120_000 - 5_000 + 2_500);
  expect(kept.find(c => c.k === 'row:2')?.v).toBe('"back"');
  expect(kept.find(c => c.k === 'row:1')).toBeUndefined();
  expect(kept.find(c => c.k === 'row:119999')?.v).toContain('"i":119999');
  // Large values: pages shrink to fit the 16 MiB result.
  const big = Array.from({ length: 40 }, (_, i) => ({ s: 'b', k: `big:${i}`, v: 'y'.repeat(1 << 20) }));
  await commitChanges(db, big);
  expect((await readKept(db)).filter(c => c.s === 'b').length).toBe(40);
  await db.close();
}, 120_000);

test('calls from overlapping answers wait their turn; a failed commit rebuilds from the disk', async () => {
  const dir = join(scratch, 'jules'), faults: Faults = { delay: 30 };
  let jules = await open('jules', dir, origin, faults);
  expect(await jules.sync()).toEqual({ ok: true });
  // Two answers at once: the read waits for the write's commit.
  // Started in the same turn: the read runs only after the write's commit.
  faults.log = [];
  const [written, read] = await Promise.all([
    jules.write('send', { body: 'one at a time' }, Date.now() + 0.5).then(w => { faults.log!.push('write'); return w; }),
    jules.read<{ body: string; pending?: boolean }[]>('inbox', {}, Date.now()).then(r => { faults.log!.push('read'); return r; }),
  ]);
  expect(faults.log).toEqual(['commit', 'write', 'read']);
  faults.log = undefined;
  expect(written.state).toBe('pending');
  expect(read.data?.[0]?.body).toBe('one at a time');
  // The next commit fails as Exact fails one: the call throws, the device
  // rebuilds from the disk, and nothing is lost or sent twice.
  faults.delay = 0;
  faults.failNext = true;
  await expect(jules.write('send', { body: 'never saved' }, Date.now())).rejects.toThrow(/storage write failed/);
  const after = (await jules.read<{ body: string }[]>('inbox', {}, Date.now())).data!.map(row => row.body);
  expect(after).toContain('one at a time');
  expect(after).not.toContain('never saved');
  expect(await jules.sync()).toEqual({ ok: true });
  expect((await jules.status()).queued).toEqual([]);
  await jules.close();
  jules = await open('jules', dir);
  const reopened = (await jules.read<{ body: string; pending?: boolean }[]>('inbox', {}, Date.now())).data!;
  expect(reopened.filter(r => r.body === 'one at a time').map(r => r.pending === true)).toEqual([false]);
  expect(reopened.some(r => r.body === 'never saved')).toBe(false);
  await jules.close();
}, 60_000);

test('a failed save after the server answered keeps the answer and never resends', async () => {
  const dir = join(scratch, 'kim'), faults: Faults = {};
  const kim = await open('kim', dir, origin, faults);
  expect(await kim.sync()).toEqual({ ok: true });
  const written = await kim.write('send', { body: 'answered, then the disk failed' }, Date.now());
  // The server confirms the send; the device's next commit (keep and settle)
  // fails, as a full disk would fail it.
  let sends = 0;
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const response = await realFetch(input as never, init);
    if (String(input).includes('/m/')) { sends++; faults.failNext = true; }
    return response;
  }) as typeof fetch;
  try {
    await expect(kim.sync()).rejects.toThrow(/storage write failed/);
    expect((await kim.outcome(written.id)).state).toBe('sent');
    expect(await kim.sync()).toEqual({ ok: true });
  } finally { globalThis.fetch = realFetch; }
  expect(sends).toBe(1);
  expect((await kim.status()).queued).toEqual([]);
  const rows = (await kim.read<{ body: string; pending?: boolean }[]>('inbox', {}, Date.now())).data!;
  expect(rows.filter(r => r.body === 'answered, then the disk failed').map(r => r.pending === true)).toEqual([false]);
  await kim.close();
}, 60_000);

test('a closed client refuses every call; closing twice is harmless', async () => {
  const lou = await open('lou', join(scratch, 'lou'));
  expect(await lou.sync()).toEqual({ ok: true });
  await lou.close();
  await lou.close();
  await expect(lou.read('inbox', {}, Date.now())).rejects.toThrow(/closed/);
  await expect(lou.write('send', { body: 'too late' }, Date.now())).rejects.toThrow(/closed/);
  await expect(lou.sync()).rejects.toThrow(/closed/);
}, 60_000);

test('when the rebuild fails too, the page hands the answer to the next device', async () => {
  const dir = join(scratch, 'max'), faults: Faults = {};
  const max = await open('max', dir, origin, faults);
  expect(await max.sync()).toEqual({ ok: true });
  const written = await max.write('send', { body: 'saved by the page' }, Date.now());
  let sends = 0;
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const response = await realFetch(input as never, init);
    if (String(input).includes('/m/')) { sends++; faults.failNext = true; faults.failOpens = 1; }
    return response;
  }) as typeof fetch;
  try {
    // The save after the server's answer fails, and so does reopening.
    await expect(max.sync()).rejects.toThrow(/storage write failed/);
    await expect(max.read('inbox', {}, Date.now())).rejects.toThrow(/could not be reloaded/);
    // A new device in the same page takes the answer over: nothing is resent.
    const again = await open('max', dir, origin, faults);
    expect((await again.outcome(written.id)).state).toBe('sent');
    expect(await again.sync()).toEqual({ ok: true });
    expect((await again.status()).queued).toEqual([]);
    await again.close();
  } finally { globalThis.fetch = realFetch; }
  expect(sends).toBe(1);
}, 60_000);

test('a successor that cannot save either keeps the page’s answer; a handed answer is saved at once', async () => {
  const dir = join(scratch, 'pia'), faults: Faults = {};
  const pia = await open('pia', dir, origin, faults);
  expect(await pia.sync()).toEqual({ ok: true });
  const written = await pia.write('send', { body: 'kept through a full disk' }, Date.now());
  let sends = 0;
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const response = await realFetch(input as never, init);
    if (String(input).includes('/m/')) { sends++; faults.failNext = true; faults.failOpens = 1; }
    return response;
  }) as typeof fetch;
  try {
    await expect(pia.sync()).rejects.toThrow(/storage write failed/);
    // The disk is still full: a new device's own open cannot be saved.
    faults.failCommits = 1;
    await expect(open('pia', dir, origin, faults)).rejects.toThrow();
    // Space again: a device takes the answer over and saves it as it opens;
    // closing before any sync loses nothing.
    const taker = await open('pia', dir, origin, faults);
    await taker.close();
    const later = await open('pia', dir, origin, faults);
    expect((await later.outcome(written.id)).state).toBe('sent');
    expect(await later.sync()).toEqual({ ok: true });
    expect((await later.status()).queued).toEqual([]);
    await later.close();
  } finally { globalThis.fetch = realFetch; }
  expect(sends).toBe(1);
}, 60_000);

test('a close during a rebuild is carried out when the rebuild ends', async () => {
  const dir = join(scratch, 'quin'), faults: Faults = {};
  const quin = await open('quin', dir, origin, faults);
  expect(await quin.sync()).toEqual({ ok: true });
  faults.failNext = true;
  faults.openDelay = 200;
  const failing = quin.write('send', { body: 'rebuilt' }, Date.now()).catch(error => error);
  await new Promise(r => setTimeout(r, 50));
  await quin.close();
  expect(String(await failing)).toMatch(/storage write failed/);
  faults.openDelay = 0;
  expect(faults.handles).toBe(0);
}, 60_000);

test('a refresh that fails before its answer lets the next one go', async () => {
  let failing = true;
  const dir = join(scratch, 'refresh');
  const client = await Snapback.open({
    app: 'test.exact.snapback4', name: 'inbox', origin, viewer: 'dev:rory',
    headers: () => { if (failing) throw new Error('no credential yet'); return { 'x-snapback-persona': 'rory' }; },
    storage: storage(dir, {}), wasm: `file://${wasm}`,
  });
  await expect(client.refreshSession(Date.now())).rejects.toThrow(/no credential yet/);
  failing = false;
  // A persona cannot be refreshed, but the request goes: not E_BUSY.
  const second = await client.refreshSession(Date.now());
  expect(second.ok).toBe(false);
  expect(second.ok ? '' : second.denied.code).not.toBe('E_BUSY');
  await client.close();
}, 60_000);
