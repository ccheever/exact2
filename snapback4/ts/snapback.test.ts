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
import type { Change, SqliteStorage } from './web.ts';
import type { MutationOp, QueryOp } from './snapback.ts';

const binary = process.env.SNAPBACK4_BIN ?? resolve(import.meta.dir, `../../node_modules/snapback4-${process.platform}-${process.arch}/snapback4${process.platform === 'win32' ? '.exe' : ''}`);
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

table drafts:
  author: principal
  body: text <=50
  at: time
  by byTime: at, id
  read <- .author = viewer
  insert <- .author = viewer
  update <- deny
  delete <- deny
  online only

query myDrafts():
  return drafts last 50 by byTime

query total():
  return count(messages)

query oldest():
  return messages first 5 by byTime

table pings:
  author: principal
  public 'pings are public'
  insert <- .author = viewer
  update <- deny
  delete <- .author = viewer
  ephemeral 4s

mutation ping():
  row = insert pings { author: viewer }
  return { id: row.id }

mutation draft(body: text <=50):
  row = insert drafts { author: viewer, body, at: now }
  return { id: row.id }

-- What the device refuses itself: another's ticket (a row rule), and an
-- approval by a member it knows is not a lead (a require).
table members:
  person: principal
  role: enum('member', 'lead')
  unique person
  public 'the directory is public'
  insert <- .person = viewer
  update <- deny
  delete <- deny
  sync public last 100 by .id

mutation join(role: enum('member', 'lead')):
  return insert members { person: viewer, role }

query leads():
  require exists members[person = viewer, role = 'lead'] else NOT_LEAD
  return members first 5 by .id

mutation approve(body: text <=50):
  require exists members[person = viewer, role = 'lead'] else NOT_LEAD
  row = insert messages { author: viewer, body, at: now }
  return { id: row.id }

table tickets:
  title: text <=50
  status: enum('open', 'closed')
  owner: principal
  public 'tickets are shared'
  insert <- .owner = viewer
  update (old, next) <- old.owner = viewer and next.owner = viewer
  delete <- deny
  sync public last 100 by .id

mutation file(title: text <=50):
  return insert tickets { title, status: 'open', owner: viewer }

mutation close(t: tickets):
  row = tickets[t] ! else NOT_FOUND
  update tickets[row.id] { status: 'closed' }
  return null

table photos:
  owner: principal
  photo: image <=1000000
  read <- .owner = viewer
  insert <- .owner = viewer
  update <- deny
  delete <- .owner = viewer
  sync to .owner last 100 by .id

mutation addPhoto(photo: image <=1000000):
  return insert photos { owner: viewer, photo }

table documents:
  owner: principal
  attachment: file <=1000000
  read <- .owner = viewer
  insert <- .owner = viewer
  update <- deny
  delete <- .owner = viewer
  sync to .owner last 100 by .id

mutation addDocument(attachment: file <=1000000):
  return insert documents { owner: viewer, attachment }

query documents():
  return documents first 100 by .id

query myDocuments():
  return documents[owner = viewer] last 100 by .id
`;

const scratch = mkdtempSync(join(tmpdir(), 'exact-snapback4-ts-'));
const wasm = join(scratch, 'snapback4.wasm');
let server: ChildProcess;
let origin = '';

// The driver imports the wasm glue this writes (ts/generated/), so it is
// built before the driver is loaded, in a fresh checkout too.
const built = spawnSync('bun', [resolve(import.meta.dir, '../web/build.mjs'), wasm], { stdio: 'inherit' });
if (built.status !== 0) throw new Error('web/build.mjs failed');
const { Snapback } = await import('./snapback.ts');
const { commitChanges, readKept } = await import('./web.ts');

beforeAll(async () => {
  const project = join(scratch, 'server');
  mkdirSync(join(project, 'snapback'), { recursive: true });
  writeFileSync(join(project, 'snapback/schema.q'), SCHEMA);
  server = spawn(binary, ['dev', '--port', '0', '--memory', '--no-watch'], { cwd: project, stdio: ['ignore', 'pipe', 'pipe'] });
  origin = await new Promise<string>((done, fail) => {
    const lines = createInterface({ input: server.stdout! });
    lines.on('line', line => { if (/^http:\/\/127\.0\.0\.1:\d+$/.test(line.trim())) done(line.trim()); });
    let diagnostic = '';
    server.stderr!.on('data', chunk => {
      diagnostic += String(chunk);
      const ready = /SNAPBACK4_READY .*url=(http:\/\/127\.0\.0\.1:\d+)/.exec(diagnostic);
      if (ready) done(ready[1]);
    });
    server.on('error', fail);
    server.on('exit', code => fail(new Error(`snapback4 dev exited ${code}: ${diagnostic}`)));
  });
}, 30_000);

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

test('an app with an unrelated native hatch can explicitly select the web device', async () => {
  let calls = 0;
  const native = { available: true, call() { calls++; throw new Error('camera is not Snapback'); } };
  const db = await Snapback.open({app: 'test', name: 'camera', origin, viewer: 'dev:alice',
    headers: () => ({'x-snapback-persona': 'alice'}), storage: storage(join(scratch, 'camera')),
    native, device: 'web', wasm: `file://${wasm}`});
  try {
    expect(await db.sync()).toEqual({ok: true});
    expect(Array.isArray((await db.read('inbox', {}, Date.now())).data)).toBe(true);
    expect(calls).toBe(0);
  } finally { await db.close(); }
});

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

test('a sync after a write settles it while other sources sync at once; send is all three; refusals reach failure(x)', async () => {
  const { failureCode, failureReason } = await import('../../host/web-js/shape.js');
  const { refused } = await import('./snapback.ts');
  const db = await open('nell', join(scratch, 'nell'));
  expect(await db.sync()).toEqual({ ok: true });
  // Three resources syncing and reading over and over, on a slow link.
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    await new Promise(r => setTimeout(r, Math.random() * 15));
    return realFetch(input as never, init);
  }) as typeof fetch;
  let stop = false;
  const sources = [0, 1, 2].map(async () => {
    while (!stop) { await db.sync(); await db.read('inbox', {}, Date.now()); await new Promise(r => setTimeout(r, Math.random() * 10)); }
  });
  try {
    for (let i = 0; i < 6; i++) {
      // write admits only: asked at once, its fate is pending.
      const written = await db.write('send', { body: `busy ${i}` }, Date.now());
      expect((await db.outcome(written.id)).state).toBe('pending');
      // A round asked for after the write sends it, whatever runs meanwhile.
      expect(await db.sync()).toEqual({ ok: true });
      expect((await db.outcome(written.id)).state).toBe('sent');
    }
    const sent = await db.send<{ id: string }>('send', { body: 'in one call' }, Date.now());
    expect([sent.state, sent.round]).toEqual(['sent', { ok: true }]);
    expect(typeof sent.result?.id).toBe('string');
    // The server's refusal, with the mutation's own code.
    const empty = await db.send('send', { body: '' }, Date.now());
    expect([empty.state, empty.why?.code]).toEqual(['failed', 'EMPTY']);
  } finally { stop = true; await Promise.all(sources); globalThis.fetch = realFetch; }
  // Offline, send keeps the write and says why it is still pending.
  globalThis.fetch = (async () => { throw new TypeError('offline'); }) as unknown as typeof fetch;
  try {
    const kept = await db.send('send', { body: 'later' }, Date.now());
    expect([kept.state, kept.round?.offline]).toEqual(['pending', true]);
  } finally { globalThis.fetch = realFetch; }
  expect((await db.send('send', { body: 'now' }, Date.now())).state).toBe('sent');
  expect((await db.status()).queued).toEqual([]);

  // Generated handles (snapback/generated/api.ts's `api`) go where a name goes, typed by their `Query`/`Mutation`.
  const api = {
    inbox: { kind: 'query', name: 'inbox', runs: 'device', rowKey: 'id' } as QueryOp<Record<string, never>, { id: string; body: string; pending?: boolean }[]>,
    send: { kind: 'mutation', name: 'send', predicts: 'never' } as MutationOp<{ body: string }, { id: string }>,
  };
  const typed = await db.send(api.send, { body: 'by handle' }, Date.now());
  expect([typed.state, typeof typed.result?.id]).toEqual(['sent', 'string']);
  const byHandle = await db.read(api.inbox, {}, Date.now());
  expect(byHandle.data?.some(row => row.id === typed.result?.id && row.body === 'by handle')).toBe(true);
  expect((await db.readAll(api.inbox, {}, Date.now())).map(row => row.id)).toEqual(byHandle.data!.map(row => row.id));
  expect((await db.write(api.send, { body: 'admitted by handle' }, Date.now())).state).toBe('pending');
  await db.sync();

  // A query's `require … else CODE`, thrown as the driver throws it: the
  // code is failure(x).reason, under the closed code `error`, and leads the
  // message.
  const read = await db.read('leads', {}, Date.now());
  expect(read.denied?.code).toBe('NOT_LEAD');
  const error = refused(read.denied!);
  expect([error.message.startsWith('NOT_LEAD: '), (error as { code?: string }).code, failureCode(error), failureReason(error)]).toEqual([true, 'NOT_LEAD', 'error', 'NOT_LEAD']);
  await expect(db.readAll('leads', {}, Date.now())).rejects.toMatchObject({ reason: 'NOT_LEAD' });
  const optional = await db.readOptional('leads', {}, Date.now(), ['NOT_LEAD']);
  expect(optional.data).toBeNull();
  expect(optional.complete).toBe(false);
  expect(optional.denied?.code).toBe('NOT_LEAD');
  await expect(db.readOptional('leads', {}, Date.now(), [])).rejects.toMatchObject({reason: 'NOT_LEAD'});
  expect(Array.isArray((await db.readOptional(api.inbox, {}, Date.now(), [])).data)).toBe(true);
  // Unreached is `offline`, as a lost fetch is; its reason is still its code.
  const lost = refused({ code: 'E_OFFLINE', message: 'not reached' });
  expect([failureCode(lost), failureReason(lost)]).toEqual(['offline', 'E_OFFLINE']);
  // Natively Error.prototype is frozen, so `name` is defined, not assigned.
  const frozen = Object.getOwnPropertyDescriptor(Error.prototype, 'name')!;
  Object.defineProperty(Error.prototype, 'name', { ...frozen, writable: false });
  try { expect(failureCode(refused({ code: 'E_OFFLINE', message: 'not reached' }))).toBe('offline'); }
  finally { Object.defineProperty(Error.prototype, 'name', frozen); }
  const never = await open('olive', join(scratch, 'olive'), 'http://127.0.0.1:9');
  const unreached = await never.read('inbox', {}, Date.now()).catch(e => e as Error);
  expect([unreached.message.startsWith('E_OFFLINE: '), failureCode(unreached)]).toEqual([true, 'offline']);
  await never.close();
  await db.close();
}, 120_000);

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

/** One page's storage, shared by every source that opens a partition in it. */
const onePage = (persona: string, dir: string, faults: Faults = {}) => {
  const page = storage(dir, faults);
  return (viewer = persona) => Snapback.open({
    app: 'test.exact.snapback4', name: 'inbox', origin, viewer: `dev:${viewer}`,
    headers: () => ({ 'x-snapback-persona': viewer }), storage: page, wasm: `file://${wasm}`,
  });
};

/** Every request to the server fails, as with no network, while `work` runs. */
async function offline<T>(work: () => Promise<T>): Promise<T> {
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    if (String(input).startsWith(origin)) throw new TypeError('offline');
    return realFetch(input as never, init);
  }) as typeof fetch;
  try { return await work(); } finally { globalThis.fetch = realFetch; }
}

test('sources share a page\'s partition: concurrent opens, rounds that wait their turn, a first read that syncs', async () => {
  const openErin = onePage('erin', join(scratch, 'erin'));
  const [a, b] = await Promise.all([openErin(), openErin()]);
  expect(a.isOpen).toBe(false);
  // Never synced: reads start (or wait for) the first round, and rounds asked
  // for at once run one after another, none answering busy.
  const [ra, rb, sa, sb] = await Promise.all([
    a.read<{ body: string }[]>('inbox', {}, Date.now()), b.read<{ body: string }[]>('inbox', {}, Date.now()), a.sync(), b.sync()]);
  expect(Array.isArray(ra.data) && Array.isArray(rb.data)).toBe(true);
  expect(sa).toEqual({ ok: true });
  expect(sb).toEqual({ ok: true });
  expect(a.isOpen && b.isOpen).toBe(true);

  const shared = (rows?: { body: string; pending?: boolean }[]) => bodies(rows!.filter(row => row.body === 'shared'));
  const written = await a.write('send', { body: 'shared' }, Date.now());
  expect(shared((await b.read<{ body: string; pending?: boolean }[]>('inbox', {}, Date.now())).data)).toEqual([['shared', true]]);
  expect((await Promise.all([a.sync(), b.sync(), a.sync()])).every(round => round.ok)).toBe(true);
  expect((await b.outcome(written.id)).state).toBe('sent');

  // A partition belongs to one viewer: another needs its own name.
  await expect(openErin('frank')).rejects.toThrow(/E_PARTITION_VIEWER/);

  // Closing one client leaves the partition to the other; the last lets it go.
  await a.close();
  expect(a.isOpen).toBe(false);
  await expect(a.read('inbox', {}, Date.now())).rejects.toThrow(/closed/);
  expect(b.usable).toBe(true);
  expect(shared((await b.read<{ body: string }[]>('inbox', {}, Date.now())).data)).toEqual([['shared', false]]);
  await b.close();
  const again = await openErin();
  expect(again.isOpen).toBe(true);
  expect(shared((await again.read<{ body: string }[]>('inbox', {}, Date.now())).data)).toEqual([['shared', false]]);
  await again.close();
}, 60_000);

test('a query over an online only table is the server\'s to answer, and says so when unreached', async () => {
  const gina = await open('gina', join(scratch, 'gina'));
  const kept = await gina.write('draft', { body: 'only mine' }, Date.now());
  expect(kept.state).toBe('pending');
  expect(await gina.sync()).toEqual({ ok: true });
  expect((await gina.outcome(kept.id)).state).toBe('sent');
  const mine = await gina.read<{ body: string }[]>('myDrafts', {}, Date.now());
  expect(mine.server).toBe(true);
  expect(mine.data!.map(row => row.body)).toEqual(['only mine']);
  expect((await gina.readAll<{ body: string }>('myDrafts', {}, Date.now())).map(row => row.body)).toEqual(['only mine']);

  // The server's rules answer for another viewer.
  const hank = await open('hank', join(scratch, 'hank'));
  expect((await hank.read<{ body: string }[]>('myDrafts', {}, Date.now())).data).toEqual([]);
  await hank.close();

  // Unreached: a refusal, never an empty page; the device's own queries
  // still answer from the partition.
  await offline(async () => {
    const unreached = await gina.read('myDrafts', {}, Date.now());
    expect(unreached.data).toBeUndefined();
    expect(unreached.denied?.code).toBe('E_OFFLINE');
    await expect(gina.readAll('myDrafts', {}, Date.now())).rejects.toThrow(/E_OFFLINE/);
    await expect(gina.readOptional('myDrafts', {}, Date.now(), ['E_OFFLINE'])).rejects.toThrow(/E_OFFLINE/);
    expect(Array.isArray((await gina.read('inbox', {}, Date.now())).data)).toBe(true);
  });
  await gina.close();
}, 60_000);

test('a partition that has never synced, offline, refuses reads and writes with E_OFFLINE', async () => {
  const ivy = await open('ivy', join(scratch, 'ivy'));
  await offline(async () => {
    await expect(ivy.read('inbox', {}, Date.now())).rejects.toThrow(/E_OFFLINE/);
    await expect(ivy.write('send', { body: 'too soon' }, Date.now())).rejects.toThrow(/E_OFFLINE/);
  });
  // Back online, the next read opens it.
  expect((await ivy.read('inbox', {}, Date.now())).data).toBeDefined();
  expect(ivy.isOpen).toBe(true);
  await ivy.close();
}, 60_000);

test('a read the device cannot vouch for asks the server; offline it says so', async () => {
  const jo = await open('jo', join(scratch, 'jo'));
  expect(await jo.sync()).toEqual({ ok: true });
  // Past the inbox's sync horizon (last 100), a total over every message is
  // not the device's to answer.
  for (let i = 0; i < 105; i++) await jo.write('send', { body: `n${i}` }, Date.now());
  expect(await jo.sync()).toEqual({ ok: true });
  const local = await jo.read<number>('total', {}, Date.now());
  if (local.server) {
    expect(typeof local.data).toBe('number');
    expect(local.data!).toBeGreaterThanOrEqual(105);
    await offline(async () => {
      const unreached = await jo.read<number>('total', {}, Date.now());
      expect(unreached.offline).toBe(true);
      expect(unreached.server).toBeUndefined();
    });
    // A page ordered against the horizon (`first 5` of a `last 100` sync):
    // the device holds none of those rows and says `unknown`; the server
    // answers it whole. Offline, the device's rows are a partial, marked.
    const first = await jo.read<{ body: string }[]>('oldest', {}, Date.now());
    expect(first.server).toBe(true);
    expect(first.data!.length).toBe(5);
    const whole = await jo.readAll<{ body: string }>('oldest', {}, Date.now());
    expect([whole.length, whole.complete, whole.offline]).toEqual([5, true, false]);
    await offline(async () => {
      const partial = await jo.read<{ body: string }[]>('oldest', {}, Date.now());
      expect(partial.offline).toBe(true);
      expect(partial.complete).not.toBe(true);
      // readAll keeps what each page said: these rows are not all of them.
      const rows = await jo.readAll<{ body: string }>('oldest', {}, Date.now());
      expect([rows.complete, rows.offline]).toEqual([false, true]);
    });
  } else {
    // The device vouched for it (complete coverage): then it must be the truth.
    expect(local.loading).not.toBe(true);
    expect(local.data!).toBeGreaterThanOrEqual(105);
  }
  await jo.close();
}, 120_000);

test('a write the device refuses is failed at once, never sent, and answered by outcome and refusals', async () => {
  const ned = await open('ned', join(scratch, 'ned'));
  expect(await ned.sync()).toEqual({ ok: true });
  const filed = await ned.write('file', { title: 'disk full' }, Date.now());
  expect(await ned.sync()).toEqual({ ok: true });
  const ticket = ((await ned.outcome(filed.id)).result as { id: string }).id;
  await ned.close();

  const olga = await open('olga', join(scratch, 'olga'));
  await olga.write('join', { role: 'member' }, Date.now());
  expect(await olga.sync()).toEqual({ ok: true });
  // Another's ticket (a row rule) and an approval by a known non-lead (a
  // require): the device refuses both, as the server would.
  const closed = await olga.write('close', { t: ticket }, Date.now());
  const approved = await olga.write('approve', { body: 'ok' }, Date.now());
  expect(closed.state === 'failed' && closed.why.code).toBe('E_RULE');
  expect(approved.state === 'failed' && approved.why.code).toBe('NOT_LEAD');
  const next = await olga.write('send', { body: 'after two refusals' }, Date.now());
  expect(new Set([closed.id, approved.id, next.id]).size).toBe(3);
  const sends: string[] = [];
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    if (String(input).includes('/m/')) sends.push(new URL(String(input)).pathname);
    return realFetch(input as never, init);
  }) as typeof fetch;
  try { expect(await olga.sync()).toEqual({ ok: true }); } finally { globalThis.fetch = realFetch; }
  expect(sends).toEqual(['/m/send']);
  expect(await olga.outcome(closed.id)).toMatchObject({ state: 'failed', why: { code: 'E_RULE' } });
  expect(await olga.outcome(approved.id)).toMatchObject({ state: 'failed', why: { code: 'NOT_LEAD' } });
  expect((await olga.refusals()).map(r => [r.op, r.why.code])).toEqual([['close', 'E_RULE'], ['approve', 'NOT_LEAD']]);
  await olga.dismiss();
  expect(await olga.refusals()).toEqual([]);
  expect((await olga.outcome(approved.id)).state).toBe('failed');
  await olga.close();
}, 60_000);

test('two sources polling one partition at once share the poll', async () => {
  const openKim = onePage('kim', join(scratch, 'kim'));
  const [a, b] = await Promise.all([openKim(), openKim()]);
  expect(await a.sync()).toEqual({ ok: true });
  // Two polls at once: neither supersedes the other (E_STALE before sharing).
  const both = await Promise.all([a.poll(0), b.poll(0)]);
  expect(both[0]).toBe(both[1]);
  await a.close();
  await b.close();
}, 60_000);

test('an ephemeral write is refused at once, not queued; poll opens a never-synced partition', async () => {
  const lee = await open('lee', join(scratch, 'lee'));
  // poll() before any sync opens the partition first (it threw E_OFFLINE).
  expect(typeof await lee.poll(0)).toBe('boolean');
  const pinged = await lee.write('ping', {}, Date.now());
  expect(pinged.state).toBe('failed');
  if (pinged.state === 'failed') expect(pinged.why.code).toBe('E_CLIENT_UNSUPPORTED');
  expect((await lee.outcome(pinged.id)).state).toBe('failed');
  expect((await lee.refusals()).map(r => r.id)).toContain(pinged.id);
  expect((await lee.status()).queued).toEqual([]);
  // The next write is unaffected and takes its own id.
  const sent = await lee.write('send', { body: 'after a ping' }, Date.now());
  expect(sent.state).toBe('pending');
  expect(sent.id).not.toBe(pinged.id);
  expect(await lee.sync()).toEqual({ ok: true });
  expect((await lee.outcome(sent.id)).state).toBe('sent');
  await lee.close();
}, 60_000);

test('a round its answer could not finish is cancelled by the next round, so the client is not left busy', async () => {
  const { webDevice } = await import('./web.ts');
  const page = storage(join(scratch, 'una'));
  const device = await webDevice(page, 'test.exact.snapback4', 'app:/data/inbox.sqlite', `file://${wasm}`);
  // Natively the device refuses a call once the answer that made it has
  // ended, as a superseded answer's round finds when its reply comes.
  let ended = false;
  const native = { call: (request: Record<string, unknown>) => {
    if (ended) throw new Error('native call outside an answer');
    return device.call(request as never);
  } };
  const una = await Snapback.open({ app: 'test.exact.snapback4', name: 'inbox', origin, viewer: 'dev:una',
    headers: () => ({ 'x-snapback-persona': 'una' }), storage: page, native, wasm: `file://${wasm}` });
  expect(await una.sync()).toEqual({ ok: true });
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const reply = await realFetch(input as never, init);
    if (String(input).endsWith('/sync')) ended = true;
    return reply;
  }) as typeof fetch;
  try { await expect(una.sync()).rejects.toThrow(/outside an answer/); }
  finally { globalThis.fetch = realFetch; ended = false; }
  expect(await una.sync()).toEqual({ ok: true });
  await una.close();
  await device.close?.();
}, 60_000);

test('Snapback.switcher waits for the other persona\'s write in flight before closing its partition', async () => {
  const personas = Snapback.switcher((persona: string) => ({ app: 'test.exact.snapback4', name: `inbox-${persona}`, origin,
    viewer: `dev:${persona}`, headers: () => ({ 'x-snapback-persona': persona }), wasm: `file://${wasm}` }));
  // A native module: one partition at a time, refusing a second open.
  const { webDevice } = await import('./web.ts');
  const page = storage(join(scratch, 'switch'));
  let open: Awaited<ReturnType<typeof webDevice>> | undefined;
  const opens: string[] = [];
  let failOpen = false;
  const native = { call: async (request: Record<string, unknown>) => {
    if (request.op === 'open') {
      if (open) throw new Error('close the current Snapback4 partition before opening another');
      if (failOpen) { failOpen = false; throw new Error('storage unavailable'); }
      open = await webDevice(page, 'test.exact.snapback4', String(request.path), `file://${wasm}`);
      opens.push(String(request.path));
    }
    if (!open) throw new Error('Snapback4 partition is not open');
    const answer = await open.call(request as never);
    if (request.op === 'close') { await open.close?.(); open = undefined; }
    return answer;
  } };
  expect(await personas.use('alice', page, native, db => db.sync())).toEqual({ ok: true });
  expect(personas.key).toBe('alice');
  // Alice's write is on the network when the app switches to Bob.
  const realFetch = globalThis.fetch;
  let release!: () => void;
  const held = new Promise<void>(done => { release = done; });
  globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    if (String(input).includes('/m/send')) await held;
    return realFetch(input as never, init);
  }) as typeof fetch;
  try {
    const posting = personas.use('alice', page, native, async db => {
      const written = await db.write('send', { body: 'switched' }, Date.now());
      await db.sync();
      return (await db.outcome(written.id)).state;
    });
    await new Promise(r => setTimeout(r, 50));
    const bob = personas.use('bob', page, native, db => db.sync());
    // And back to Alice before Bob has even opened: each waits its turn.
    const again = personas.use('alice', page, native, async db => (await db.read<{ body: string }[]>('inbox', {}, Date.now())).data?.map(row => row.body));
    release();
    expect(await posting).toBe('sent');
    expect(await bob).toEqual({ ok: true });
    expect(await again).toContain('switched');
  } finally { globalThis.fetch = realFetch; }
  expect(opens.map(path => path.replace(/^.*\//, ''))).toEqual(['inbox-alice.sqlite', 'inbox-bob.sqlite', 'inbox-alice.sqlite']);
  // An open that fails rejects that call only; the next is tried afresh.
  failOpen = true;
  expect(await personas.use('bob', page, native, db => db.sync()).catch(error => (error as Error).message)).toBe('storage unavailable');
  expect(personas.key).toBe(null);
  expect(await personas.use('bob', page, native, db => db.sync())).toEqual({ ok: true });
  await personas.close();
  expect(open).toBeUndefined();
  expect(personas.key).toBe(null);
}, 60_000);

test('upload sends bytes to /assets and download writes the asset to an app file, under the row\'s read rule', async () => {
  const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='), c => c.charCodeAt(0));
  const files = new Map<string, Uint8Array>();
  const withFiles = (persona: string, dir: string, at = origin) => Snapback.open({
    app: 'test.exact.snapback4', name: `photos-${persona}`, origin: at, viewer: `dev:${persona}`,
    headers: () => ({ 'x-snapback-persona': persona }), wasm: `file://${wasm}`,
    storage: { ...storage(dir), fs: { async writeFile(path: string, data: Uint8Array) { files.set(path, data); } } },
  });
  const alice = await withFiles('alice', join(scratch, 'photos-alice'));
  const sent = await alice.upload(png);
  if (!sent.ok) throw new Error(sent.why.message);
  expect([sent.asset.type, sent.asset.bytes, sent.asset.width, sent.asset.height]).toEqual(['image/png', png.byteLength, 1, 1]);
  const refused = await alice.upload(new TextEncoder().encode('not a picture'));
  expect(refused.ok ? null : refused.why.code).toBe('E_ASSET_TYPE');
  // Placed by a mutation, read back as its row's facts, then fetched as the row's reader.
  const placed = await alice.send('addPhoto', { photo: { id: sent.asset.id } }, Date.now());
  expect(placed.state).toBe('sent');
  const shown = await alice.download(sent.asset.id);
  expect(shown).toEqual({ ok: true, path: `app:/cache/snapback4-photos-alice-${sent.asset.id}.png`, type: 'image/png', bytes: png.byteLength });
  expect([...files.get(`app:/cache/snapback4-photos-alice-${sent.asset.id}.png`)!]).toEqual([...png]);
  expect((await alice.download(sent.asset.id, 'app:/tmp/one.png')).ok).toBe(true);
  const pdf = new TextEncoder().encode('%PDF-1.4\nExact attachment\n%%EOF');
  const uploaded = await alice.upload(pdf, { file: { type: 'application/pdf', name: 'review notes.pdf' } });
  if (!uploaded.ok) throw new Error(uploaded.why.message);
  expect(uploaded.asset).toMatchObject({ type: 'application/pdf', bytes: pdf.byteLength, name: 'review notes.pdf' });
  expect((await alice.send('addDocument', { attachment: { id: uploaded.asset.id } }, Date.now())).state).toBe('sent');
  const rows = await alice.read<{ attachment: { id: string; name: string; type: string } }[]>('documents', {}, Date.now());
  expect(rows.data?.[0].attachment).toMatchObject({ id: uploaded.asset.id, name: 'review notes.pdf', type: 'application/pdf' });
  const downloaded = await alice.download(uploaded.asset.id);
  expect(downloaded).toEqual({ ok: true, path: `app:/cache/snapback4-photos-alice-${uploaded.asset.id}-review notes.pdf`, name: 'review notes.pdf', type: 'application/pdf', bytes: pdf.byteLength });
  if (downloaded.ok) expect(files.get(downloaded.path)).toEqual(pdf);
  // Another viewer cannot read the row, so the bytes are not there for them.
  const bob = await withFiles('bob', join(scratch, 'photos-bob'));
  const hidden = await bob.download(sent.asset.id);
  expect(hidden.ok ? null : hidden.why.code).toBe('E_ASSET_UNKNOWN');
  await bob.close();
  await alice.close();
  // Unreached: offline, and nothing kept.
  const away = await withFiles('alice', join(scratch, 'photos-away'), 'http://127.0.0.1:9');
  const lost = await away.upload(png);
  expect(lost.ok ? null : lost.why.code).toBe('E_OFFLINE');
  await away.close();
}, 60_000);

test('fresh uses the live reset door, empties rows, and seeds the next test', async () => {
  const { resetBackends } = await import('../../scripts/agent-test.mjs');
  const first = await open('alice', join(scratch, 'fresh-first'));
  expect((await first.send('send', { body: 'before reset' }, Date.now())).state).toBe('sent');
  await first.close();
  await resetBackends([origin]);
  const empty = await open('alice', join(scratch, 'fresh-empty'));
  expect((await empty.read('inbox', {}, Date.now())).data).toEqual([]);
  await empty.close();
  await resetBackends([origin], [{ table: 'messages', row: { id: 'seed-note', author: 'dev:alice', body: 'seeded', at: Date.now() } }]);
  const seeded = await open('alice', join(scratch, 'fresh-seeded'));
  expect((await seeded.read<{ body: string }[]>('inbox', {}, Date.now())).data?.map(row => row.body)).toEqual(['seeded']);
  await seeded.close();
}, 60_000);

test('at bake (a native module that is not available) open refuses with code bake and calls nothing', async () => {
  const calls: unknown[] = [];
  const native = { available: false, call: (request: unknown) => { calls.push(request); throw new Error('native storage is unavailable during bake or in an unconfigured host'); } };
  const refused = await Snapback.open({ app: 'test.exact.snapback4', name: 'baked', origin, viewer: 'dev:bake',
    storage: storage(join(scratch, 'bake')), native, wasm: `file://${wasm}` }).catch(error => error);
  expect(refused).toBeInstanceOf(Error);
  expect((refused as { code?: string; kind?: string }).code).toBe('bake');
  expect((refused as { code?: string; kind?: string }).kind).toBe('Unavailable');
  expect(calls).toEqual([]);
}, 60_000);

test('password accounts on serve: signup, reopen, refresh, login, logout and isolated safe partitions', async () => {
  const project = join(scratch, 'password-server'), deployed = join(project, 'production');
  mkdirSync(join(project, 'snapback'), { recursive: true });
  writeFileSync(join(project, 'snapback/schema.q'), SCHEMA.replace('use identity', 'use identity password'));
  const release = spawnSync(binary, ['deploy', deployed], { cwd: project, encoding: 'utf8' });
  if (release.status !== 0) throw new Error(`deploy: ${release.stderr}`);
  const serve = spawn(binary, ['serve', deployed, '--port', '0'], { cwd: project, stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    const url = await new Promise<string>((done, fail) => {
      let output = '';
      for (const stream of [serve.stdout!, serve.stderr!]) stream.on('data', chunk => {
        output += String(chunk);
        const ready = /SNAPBACK4_READY .*url=(http:\/\/127\.0\.0\.1:\d+)/.exec(output) ?? /^(http:\/\/127\.0\.0\.1:\d+)$/m.exec(output);
        if (ready) done(ready[1]);
      });
      serve.on('error', fail);
      serve.on('exit', code => fail(new Error(`serve exited ${code}: ${output}`)));
    });
    const secretsPath = join(scratch, 'password-secrets.sqlite');
    let secrets = new Database(secretsPath);
    secrets.run('CREATE TABLE secrets (key TEXT PRIMARY KEY, value TEXT)');
    const store = {
      get: (key: string) => (secrets.query('SELECT value FROM secrets WHERE key=?').get(key) as { value: string } | null)?.value ?? null,
      set: (key: string, value: string) => { secrets.run('INSERT OR REPLACE INTO secrets VALUES (?, ?)', [key, value]); },
      forget: (key: string) => { secrets.run('DELETE FROM secrets WHERE key=?', [key]); },
    };
    const page = storage(join(scratch, 'password-device'));
    const options = { app: 'test.exact.password', name: 'account', origin: url, storage: page,
      auth: { store, key: 'session', now: Date.now() }, wasm: `file://${wasm}` };
    let db = await Snapback.open(options);
    expect(db.session).toBeNull();
    await expect(db.read('inbox', {}, Date.now())).rejects.toThrow(/E_AUTH/);
    const signed = await db.signup('alice@example.com', 'correct-horse-battery-staple');
    if (!signed.ok) throw new Error(JSON.stringify(signed.why));
    const principal = signed.session.principal, token = signed.session.token;
    expect(principal.startsWith('user:')).toBe(true);
    expect(Snapback.partitionName(principal)).toMatch(/^[a-z0-9-]+$/);
    expect((await db.send('send', { body: 'password hello' }, Date.now())).state).toBe('sent');
    const name = db.options.name;
    await db.close();
    secrets.close(); secrets = new Database(secretsPath);
    // A new store and a new driver restore the durable session and partition.
    db = await Snapback.open({ ...options, auth: { ...options.auth, now: Date.now() } });
    expect(db.session?.principal).toBe(principal);
    expect(db.options.name).toBe(name);
    expect((await db.read<{ author: string }[]>('inbox', {}, Date.now())).data?.some(row => row.author === principal)).toBe(true);
    const renewed = await Promise.all([db.refreshSession(Date.now()), db.refreshSession(Date.now())]);
    expect(renewed.every(r => r.ok)).toBe(true);
    expect(db.session?.token).not.toBe(token);
    const old = await fetch(`${url}/q/inbox`, { method: 'POST', headers: { authorization: `Bearer ${token}`, 'content-type': 'application/json' }, body: '{"args":{}}' });
    expect((await old.json() as { denied: { code: string } }).denied.code).toBe('E_AUTH');
    const latestToken = db.session!.token;
    expect(await db.logout()).toEqual({ ok: true });
    expect(store.get('session')).toBeNull();
    expect(db.session).toBeNull();
    const revoked = await fetch(`${url}/q/inbox`, { method: 'POST', headers: { authorization: `Bearer ${latestToken}`, 'content-type': 'application/json' }, body: '{"args":{}}' });
    expect((await revoked.json() as { denied: { code: string } }).denied.code).toBe('E_AUTH');
    expect((await db.login('alice@example.com', 'wrong-password')).ok).toBe(false);
    expect(db.session).toBeNull();
    expect((await db.login('alice@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    expect(db.options.name).toBe(name);
    const held = db.session!;
    await db.close();
    // Contract time triggers refresh just before expiry; it never reads Date.now().
    db = await Snapback.open({ ...options, auth: { ...options.auth, now: held.expiresAt - 30_000 } });
    expect(db.session?.token).not.toBe(held.token);
    // Offline sign-out forgets the local token regardless of revocation.
    const realFetch = globalThis.fetch;
    globalThis.fetch = (async () => { throw new TypeError('offline'); }) as typeof fetch;
    try { expect(await db.logout()).toEqual({ ok: false, offline: true }); } finally { globalThis.fetch = realFetch; }
    expect(store.get('session')).toBeNull();
    expect((await db.signup('bob@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    expect(db.options.name).not.toBe(name);
    expect((await db.send('send', { body: 'bob hello' }, Date.now())).state).toBe('sent');
    await db.logout();
    expect((await db.login('alice@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    expect((await db.send('draft', { body: 'Alice private' }, Date.now())).state).toBe('sent');
    // A request already carrying the old token cannot erase a refresh's replacement.
    for (const [path, request] of [
      ['/q/myDrafts', () => db.read('myDrafts', {}, Date.now())],
      ['/sync', () => db.sync()],
    ] as const) {
      let enter!: () => void, release!: () => void;
      const entered = new Promise<void>(done => { enter = done; });
      const held = new Promise<void>(done => { release = done; });
      globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
        if (String(input).endsWith(path)) { enter(); await held; }
        return realFetch(input as never, init);
      }) as typeof fetch;
      try {
        const pending = request();
        await entered;
        const renewed = await db.refreshSession(Date.now());
        if (!renewed.ok) throw new Error(renewed.denied.code);
        release();
        expect((await pending).denied?.code).toBe('E_AUTH');
        expect(db.session?.token).toBe(renewed.session.token);
      } finally { release(); globalThis.fetch = realFetch; }
      expect(await db.sync()).toEqual({ ok: true });
    }
    const staleAlice = await Snapback.open({ ...options, auth: { ...options.auth, now: Date.now() } });
    let enter!: () => void, enterSync!: () => void, release!: () => void;
    const entered = new Promise<void>(done => { enter = done; });
    const syncEntered = new Promise<void>(done => { enterSync = done; });
    const heldReply = new Promise<void>(done => { release = done; });
    globalThis.fetch = (async (input: string | URL | Request, init?: RequestInit) => {
      const response = await realFetch(input as never, init);
      if (String(input).endsWith('/q/myDrafts')) { enter(); await heldReply; }
      if (String(input).endsWith('/sync')) { enterSync(); await heldReply; }
      return response;
    }) as typeof fetch;
    const reading = db.read<{ body: string }[]>('myDrafts', {}, Date.now()).catch(error => error);
    try {
      await entered;
      const syncing = db.sync().catch(error => error);
      await syncEntered;
      const queuedSync = db.sync().catch(error => error);
      await db.logout();
      expect((await db.login('bob@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
      await expect(staleAlice.download('not-even-fetched')).rejects.toThrow(/E_AUTH/);
      await expect(staleAlice.upload(new Uint8Array([1]))).rejects.toThrow(/E_AUTH/);
      release();
      expect((await reading as { code: string }).code).toBe('E_STALE');
      expect((await syncing as { code: string }).code).toBe('E_STALE');
      expect((await queuedSync as { code: string }).code).toBe('E_STALE');
    } finally { release(); globalThis.fetch = realFetch; }
    await staleAlice.close();
    // A revoked session, including one kept across a backend reset, is forgotten.
    await realFetch(`${url}/auth/logout`, { method: 'POST', headers: { authorization: `Bearer ${db.session!.token}` } });
    expect((await db.sync()).denied?.code).toBe('E_AUTH');
    expect(db.session).toBeNull();
    expect(store.get('session')).toBeNull();
    await db.logout(); await db.close();
    store.set('session', JSON.stringify({ ...held, expiresAt: Date.now() - 1 }));
    db = await Snapback.open({ ...options, auth: { ...options.auth, now: Date.now() } });
    expect(db.session).toBeNull();
    expect(store.get('session')).toBeNull();
    await db.close();
    store.set('session', '{broken');
    db = await Snapback.open({ ...options, auth: { ...options.auth, now: Date.now() } });
    expect(db.session).toBeNull();
    expect(store.get('session')).toBeNull();
    await db.close();
    // The native module has one open partition, even when several sources share it.
    const { webDevice } = await import('./web.ts');
    let nativeDevice: Awaited<ReturnType<typeof webDevice>> | undefined;
    const native = { call: async (request: Record<string, unknown>) => {
      if (request.op === 'open') {
        if (nativeDevice) throw new Error('close the current Snapback4 partition before opening another');
        nativeDevice = await webDevice(page, options.app, String(request.path), `file://${wasm}`);
      }
      if (!nativeDevice) throw new Error('no native partition');
      const answer = await nativeDevice.call(request as never);
      if (request.op === 'close') { await nativeDevice.close?.(); nativeDevice = undefined; }
      return answer;
    } };
    const nativeOptions = { ...options, name: 'native-account', native, auth: { store, key: 'native-session', now: Date.now() } };
    const owner = await Snapback.open(nativeOptions);
    expect((await owner.login('alice@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    const oldHandle = await Snapback.open(nativeOptions);
    await owner.logout();
    expect(nativeDevice).toBeUndefined();
    expect(oldHandle.usable).toBe(false);
    expect((await owner.login('bob@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    await oldHandle.close();                            // must not close Bob's current native device
    expect((await owner.send('send', { body: 'native Bob' }, Date.now())).state).toBe('sent');
    await owner.logout();
    expect((await owner.login('alice@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    const refusedHandle = await Snapback.open(nativeOptions);
    await realFetch(`${url}/auth/logout`, { method: 'POST', headers: { authorization: `Bearer ${owner.session!.token}` } });
    expect((await owner.sync()).denied?.code).toBe('E_AUTH');
    expect((await owner.login('bob@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    expect(refusedHandle.usable).toBe(false);
    await refusedHandle.close();
    expect((await owner.send('send', { body: 'native recovery' }, Date.now())).state).toBe('sent');
    await owner.logout(); await owner.close();
    expect(nativeDevice).toBeUndefined();
    // An auth source opened before sign-in owns no partition; a data source
    // signs in later, and a third source uses manually supplied credentials.
    const authOnly = await Snapback.open(nativeOptions);
    expect(authOnly.session).toBeNull();
    const signedIn = await Snapback.open(nativeOptions);
    expect((await signedIn.login('alice@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    const authenticatedSibling = await Snapback.open(nativeOptions);
    const aliceSession = signedIn.session!;
    const headersSibling = await Snapback.open({ ...nativeOptions, auth: undefined,
      name: signedIn.options.name, viewer: aliceSession.principal,
      headers: () => ({ authorization: `Bearer ${aliceSession.token}` }) });
    const privatePdf = new TextEncoder().encode('%PDF-1.4\nPrivate native document\n%%EOF');
    const privateAsset = await signedIn.upload(privatePdf, { file: { type: 'application/pdf', name: 'native-private.pdf' } });
    if (!privateAsset.ok) throw new Error(privateAsset.why.code);
    expect((await signedIn.send('addDocument', { attachment: { id: privateAsset.asset.id } }, Date.now())).state).toBe('sent');
    const cached = await headersSibling.read<{ attachment: { id: string } }[]>('myDocuments', {}, Date.now());
    expect(cached.server).not.toBe(true);
    expect(cached.data?.some(row => row.attachment.id === privateAsset.asset.id)).toBe(true);
    expect(authOnly.isOpen).toBe(false);
    expect(await authOnly.logout()).toEqual({ ok: true });
    expect(store.get('native-session')).toBeNull();
    expect(nativeDevice).toBeUndefined();
    expect(authenticatedSibling.usable).toBe(false);
    expect(headersSibling.usable).toBe(false);
    await expect(headersSibling.read('myDocuments', {}, Date.now())).rejects.toThrow(/E_STALE/);
    await expect(authenticatedSibling.read('myDocuments', {}, Date.now())).rejects.toThrow(/E_AUTH/);
    // Same-account login must create a new partition owner, not revive siblings.
    expect((await authOnly.login('alice@example.com', 'correct-horse-battery-staple')).ok).toBe(true);
    await expect(authenticatedSibling.read('myDocuments', {}, Date.now())).rejects.toThrow(/E_STALE/);
    await signedIn.close(); await authenticatedSibling.close(); await headersSibling.close();
    expect((await authOnly.send('send', { body: 'unbound logout recovered' }, Date.now())).state).toBe('sent');
    await authOnly.logout(); await authOnly.close();
    expect(nativeDevice).toBeUndefined();
    secrets.close();
    const mixed = Snapback.switcher((viewer: string) => ({ app: 'test.exact.password', origin: url,
      viewer, wasm: `file://${wasm}`, headers: () => ({ authorization: `Bearer ${held.token}`, 'x-snapback-persona': undefined }) }));
    expect(await mixed.use(principal, page, null, async client => client.options.name)).toBe(Snapback.partitionName(principal));
    await mixed.close();
    await expect(Snapback.open({ app: 'test', name: principal, origin: url, viewer: principal, storage: page })).rejects.toThrow(principal);
  } finally { serve.kill(); await new Promise<void>(done => serve.exitCode === null ? serve.once('exit', () => done()) : done()); }
}, 60_000);
