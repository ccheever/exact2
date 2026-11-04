// The web path end to end: this driver and the wasm (built by web/build.mjs)
// against a real `snapback4 dev`, the partition persisted in SQLite as
// Exact's storage keeps it. `bun test snapback4/ts` from the checkout root.

import { afterAll, beforeAll } from 'bun:test';
import { Database } from 'bun:sqlite';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import type { SqliteStorage } from './web.ts';

export const binary = process.env.SNAPBACK4_BIN ?? resolve(import.meta.dir, `../../node_modules/snapback4-${process.platform}-${process.arch}/snapback4${process.platform === 'win32' ? '.exe' : ''}`);
// Since 0.4.33 dev refuses a persona the schema does not declare: these are every one the tests send.
const PERSONAS = 'use personas alice, bob, carol, dave, erin, gina, hank, ivy, jo, jules, kim, lee, lou, max, ned, nell, olga, olive, pia, quin, rory, una';
export const SCHEMA = `
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

export const scratch = mkdtempSync(join(tmpdir(), 'exact-snapback4-ts-'));
export const wasm = join(scratch, 'snapback4.wasm');
let server: ChildProcess;
export let origin = '';

// The driver imports the wasm glue this writes (ts/generated/), so it is
// built before the driver is loaded, in a fresh checkout too.
const built = spawnSync('bun', [resolve(import.meta.dir, '../web/build.mjs'), wasm], { stdio: 'inherit' });
if (built.status !== 0) throw new Error('web/build.mjs failed');
export const { Snapback } = await import('./snapback.ts');
export const { commitChanges, readKept } = await import('./web.ts');

beforeAll(async () => {
  const project = join(scratch, 'server');
  mkdirSync(join(project, 'snapback'), { recursive: true });
  writeFileSync(join(project, 'snapback/schema.q'), SCHEMA.replace('use identity', `use identity\n${PERSONAS}`));
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
export type Faults = { failNext?: boolean; failCommits?: number; failOpens?: number; openDelay?: number; delay?: number; commits?: number; log?: string[]; handles?: number };
export function storage(dir: string, faults: Faults = {}): SqliteStorage {
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

export const open = (persona: string, dir: string, at = origin, faults: Faults = {}) => Snapback.open({
  app: 'test.exact.snapback4', name: 'inbox', origin: at, viewer: `dev:${persona}`,
  headers: () => ({ 'x-snapback-persona': persona }), storage: storage(dir, faults), wasm: `file://${wasm}`,
});

export const bodies = (rows: { body: string; pending?: boolean }[]) => rows.map(row => [row.body, row.pending === true]);
