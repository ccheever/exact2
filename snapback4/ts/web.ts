// The web's device: Snapback4's device and client as wasm
// (`exact-snapback4-web`), its partition kept in Exact SQLite. After each
// call the changes drain into one transaction; memory never runs ahead of
// the disk for long:
//
// - One call at a time. A call that finds another answer's call still
//   committing waits by issuing a storage read of its own: Exact runs a
//   module's storage in issue order, so that read lands after the commit.
//   It never awaits another answer's promise.
// - A failed commit rebuilds the device from the disk (Exact closes a
//   connection whose file write failed), carrying over what the server said
//   that the device had not yet saved; until that succeeds, calls refuse.
// - Writes and reads stay within Exact's bounds (10,000 commands a
//   transaction, 16 MiB a statement or a result, 100,000 rows a result).
//
// `snapback4/web/build.mjs` writes the wasm an app serves and `./generated/`.

import { initSync, WebSnapback } from './generated/device.ts';
import type { Device, Request } from './snapback.ts';

/** The part of Exact's `storage` this needs (`storage.sqlite`). */
export interface SqliteDatabase {
  execute(sql: string): Promise<unknown>;
  query(sql: string, params?: unknown[]): Promise<{ rows: unknown[][] }>;
  transaction(commands: { sql: string; params?: unknown[] }[]): Promise<unknown>;
  close(): Promise<unknown>;
}
export interface SqliteStorage { sqlite: { open(path: string): Promise<SqliteDatabase> } }

export type Change = { s: string; k: string; v: string | null };

const encoder = new TextEncoder();
/** Bytes of change JSON a statement carries: well under the 16 MiB bound. */
const CHUNK_BYTES = 4 << 20;
/** Exact's bound on one statement's parameters, less room for the array. */
const MAX_PARAMETER = (16 << 20) - 64;
/** Rows a page of the kept image starts at; halved while a page is too big. */
const PAGE_ROWS = 2000;
/** How many of its own storage reads a call issues waiting for another. */
const WAITS = 10_000;

let loaded = '';

/** What the server said that a device of this page could not make durable
 * (its save failed, and rebuilding failed too), by partition path: handed to
 * the next device this page opens there, before it sends anything. */
const carried = new Map<string, unknown>();
/** Keep `held` for this path too, beside what is already kept (by id). */
function carryInto(path: string, held: unknown) {
  const kept = new Map<string, unknown>();
  for (const list of [carried.get(path), held]) {
    for (const item of Array.isArray(list) ? list : []) {
      const id = (item as { id?: unknown })?.id;
      if (typeof id === 'string') kept.set(id, item);
    }
  }
  if (kept.size) carried.set(path, [...kept.values()]);
}

async function load(url: string): Promise<void> {
  if (loaded === url) return;
  // The device indexes words by the page's NFC; refuse a page that disagrees
  // with the server's, as Snapback's own web device does.
  if ('Å'.normalize('NFC') !== 'Å' || '가'.normalize('NFC') !== '가') {
    throw new Error('E_HOST_UNICODE: this browser lacks standard NFC normalization');
  }
  // Each answer fetches with its own ticket; a shared pending promise would
  // leave another answer waiting on this one's host operation.
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Snapback4 wasm: HTTP ${response.status} for ${url}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (loaded !== url) {
    initSync({ module: bytes });
    loaded = url;
  }
}

const TABLE = 'CREATE TABLE IF NOT EXISTS snapback_device (s TEXT NOT NULL, k TEXT NOT NULL, v TEXT NOT NULL, PRIMARY KEY (s, k)) WITHOUT ROWID';

/** Commit drained changes as one transaction: the last value of each key,
 * upserts and deletes in bulk statements of bounded size. */
export async function commitChanges(db: SqliteDatabase, changes: Change[]): Promise<void> {
  if (!changes.length) return;
  const last = new Map<string, Change>();
  for (const change of changes) last.set(`${change.s}\u0000${change.k}`, change);
  const commands: { sql: string; params: unknown[] }[] = [];
  const chunk = (items: Change[], sql: string) => {
    let batch: Change[] = [], bytes = 0;
    const flush = () => { if (batch.length) commands.push({ sql, params: [JSON.stringify(batch)] }); batch = []; bytes = 0; };
    for (const item of items) {
      // The parameter as Exact counts it: the encoded JSON, escapes included.
      const size = encoder.encode(JSON.stringify(item)).length + 1;
      if (size > MAX_PARAMETER) throw new Error(`Snapback4: the value at ${item.s}/${item.k} exceeds Exact's 16 MiB statement bound`);
      if (bytes + size > CHUNK_BYTES) flush();
      batch.push(item); bytes += size;
    }
    flush();
  };
  const all = [...last.values()];
  chunk(all.filter(c => c.v !== null), "INSERT OR REPLACE INTO snapback_device (s, k, v) SELECT json_extract(value, '$.s'), json_extract(value, '$.k'), json_extract(value, '$.v') FROM json_each(?)");
  chunk(all.filter(c => c.v === null), "DELETE FROM snapback_device WHERE (s, k) IN (SELECT json_extract(value, '$.s'), json_extract(value, '$.k') FROM json_each(?))");
  if (commands.length > 10_000) throw new Error('Snapback4: a commit exceeds Exact\'s 10,000-command transaction');
  await db.transaction(commands);
}

/** The kept image, in key order, a bounded page at a time. */
export async function readKept(db: SqliteDatabase): Promise<Change[]> {
  const kept: Change[] = [];
  let s = '', k = '', first = true, rows = PAGE_ROWS;
  for (;;) {
    let page: unknown[][];
    try {
      page = (await db.query(first
        ? 'SELECT s, k, v FROM snapback_device ORDER BY s, k LIMIT ?'
        : 'SELECT s, k, v FROM snapback_device WHERE s > ? OR (s = ? AND k > ?) ORDER BY s, k LIMIT ?',
      first ? [rows] : [s, s, k, rows])).rows;
    } catch (error) {
      // A page past the 16 MiB result bound: ask again for fewer rows.
      if (rows > 1 && /exceeds 16 MiB/.test(String((error as Error)?.message))) { rows = Math.max(1, rows >> 1); continue; }
      throw error;
    }
    for (const [ps, pk, pv] of page) kept.push({ s: String(ps), k: String(pk), v: String(pv) });
    if (page.length < rows) return kept;
    [s, k] = [String(page[page.length - 1][0]), String(page[page.length - 1][1])];
    first = false;
  }
}

function busy(): Error {
  return Object.assign(new Error('E_BUSY: saved data is still being written; try again'), { code: 'E_BUSY', retryable: true });
}

/** Open the partition's database and a wasm device over what it keeps. */
export async function webDevice(storage: SqliteStorage, app: string, path: string, url: string): Promise<Device> {
  await load(url);
  let db = await storage.sqlite.open(path);
  let device: WebSnapback | undefined;
  try {
    await db.execute(TABLE);
    device = new WebSnapback(app, JSON.stringify(await readKept(db)));
  } catch (error) { await db.close(); throw error; }
  let opened: Request | undefined;
  let owner = false;
  let closed = false;
  // A close that came while another call owned the device: that call, when
  // it ends, does the cleanup.
  let closeRequested = false;
  let broken: Error | undefined;
  const carry = (held: unknown) => carryInto(path, held);
  const cleanup = async () => {
    closed = true;
    device?.free();
    device = undefined;
    await db.close().catch(() => undefined);
  };

  // Rebuild from the disk after a failed commit. The old device is still
  // sound in memory: take what the server said that it has not saved.
  const rebuild = async (old: WebSnapback) => {
    let held: unknown = [];
    try { held = (JSON.parse(old.call(JSON.stringify({ op: 'held' }))) as Request).ok ?? []; } catch { held = []; }
    // Kept in the page, with any it already kept, until a device here has
    // saved them again (a commit that includes them succeeded).
    carry(held);
    old.free();
    device = undefined;
    await db.close().catch(() => undefined);
    db = await storage.sqlite.open(path);
    const fresh = new WebSnapback(app, JSON.stringify(await readKept(db)));
    try {
      let handed = false;
      if (opened) {
        const reopened = JSON.parse(fresh.call(JSON.stringify(opened))) as Request;
        if (reopened.error !== undefined || reopened.denied !== undefined) throw new Error(String(reopened.error ?? JSON.stringify(reopened.denied)));
        handed = handOver(fresh);
      }
      // The reopen and the saved outcomes belong on the disk: only once they
      // are there does the page let go of its copy.
      await commitChanges(db, JSON.parse(fresh.drain()) as Change[]);
      if (handed) carried.delete(path);
    } catch (error) { fresh.free(); throw error; }
    device = fresh;
  };
  // Outcomes an earlier device of this page could not save, handed to this
  // device and saved by it now (its client keeps and settles them), so the
  // commit that follows carries them.
  // True only when the device saved them all: then the commit that follows
  // carries them, and the page may let go of its copy once it succeeds.
  const handOver = (target: WebSnapback): boolean => {
    const held = carried.get(path);
    if (held === undefined) return false;
    target.call(JSON.stringify({ op: 'hold', held }));
    const saved = JSON.parse(target.call(JSON.stringify({ op: 'persist' }))) as Request;
    return (saved.ok as { ok?: boolean } | undefined)?.ok === true;
  };

  // Take the device, waiting out a call in progress: the read lands after
  // it commits. The claim is made in the same synchronous step as the check
  // that found it free, so two callers can never both pass.
  const claim = async () => {
    for (let waits = 0; ; waits++) {
      if (!owner) { owner = true; return; }
      if (waits >= WAITS) throw busy();
      // A probe that fails finds the owner rebuilding (its connection is
      // closed): there is nothing to queue behind, so say busy at once.
      if (!await db.query('SELECT 1').then(() => true, () => false)) throw busy();
    }
  };

  return {
    async call(request) {
      await claim();
      try {
        if (closed) throw new Error('Snapback4: this partition is closed');
        if (broken || !device) throw broken ?? new Error('Snapback4: the device is unavailable');
        const current = device;
        const answer = JSON.parse(current.call(JSON.stringify(request))) as Request;
        // A device that just opened takes over what an earlier one could
        // not save, inside this same call's commit.
        const handed = request.op === 'open' && answer.ok !== undefined && answer.error === undefined && handOver(current);
        try {
          await commitChanges(db, JSON.parse(current.drain()) as Change[]);
          if (handed) carried.delete(path);
        }
        catch (error) {
          try { await rebuild(current); }
          catch (failure) {
            broken = Object.assign(new Error(`Snapback4: saved data could not be reloaded (${(failure as Error)?.message ?? failure}); reopen the app`), { code: 'E_UNAVAILABLE' });
          }
          throw error;
        }
        if (answer.error !== undefined) throw new Error(String(answer.error));
        if (request.op === 'open' && answer.ok) {
          opened = { op: 'open', path: request.path, origin: request.origin, viewer: request.viewer } as Request;
        }
        return answer;
      } finally {
        owner = false;
        if (closeRequested && !closed) await cleanup();
      }
    },
    healthy: () => !closed && !broken,
    // True once cleaned up; false when deferred to the call that owns the
    // device (a rebuild), which cleans up as it ends.
    async close() {
      if (closed) return true;
      try { await claim(); }
      catch {
        closeRequested = true;
        return false;
      }
      try { await cleanup(); return true; } finally { owner = false; }
    },
  };
}
