import { expect, test } from 'bun:test';
import { Snapback, type NativeModule, type Request as DeviceRequest } from './snapback.ts';
import { nativeDevice } from './native.ts';

function gate() {
  let resolve!: () => void;
  const promise = new Promise<void>(done => { resolve = done; });
  return { promise, resolve };
}

const busy = { denied: { code: 'E_BUSY', family: 'store', retryable: true, message: 'device work is running' } };
const acquisition = { acquired: true, exhausted: true, cursor: null, acquiredTables: ['messages'], flooredGroups: 0, rows: 0, tables: {} };

// A native facade with synchronous calls and host work delivered on a later
// tick. Its apply gate represents a worker in SQLite, outside the source turn.
class Native implements NativeModule {
  readonly available = true;
  calls: string[] = [];
  laters: string[] = [];
  owner?: string;
  exchange = 0;
  rows: unknown[] = [];
  ended = false;
  applying = gate();
  resume = gate();
  lost = gate();
  loseApply = false;
  failSync = false;
  tail: Promise<unknown> = Promise.resolve();
  busy = false;

  call(request: DeviceRequest): DeviceRequest {
    this.calls.push(String(request.op));
    if (this.ended) throw new Error('native call outside an answer');
    if (this.busy) return busy;
    if (['open', 'sync', 'deliver', 'apply', 'adopt', 'close'].includes(String(request.op)))
      throw new Error(`heavy ${request.op} ran inside the source turn`);
    return this.dispatch(request);
  }

  later(request: DeviceRequest): Promise<DeviceRequest> {
    this.laters.push(String(request.op));
    if (this.ended) return Promise.reject(new Error('native call outside an answer'));
    const work = this.tail.then(async () => {
      await new Promise<void>(resolve => setImmediate(resolve));
      this.busy = true;
      try {
        if (request.op === 'deliver' && !this.rows.length) {
          this.applying.resolve();
          await this.resume.promise;
        }
        const answer = this.dispatch(request);
        if (request.op === 'sync' && this.failSync) {
          this.failSync = false;
          throw new Error('native reply lost before its first exchange');
        }
        return answer;
      } finally { this.busy = false; }
    });
    this.tail = work.catch(() => undefined);
    return request.op === 'deliver' && this.loseApply
      ? Promise.race([work, this.lost.promise.then(() => { throw Object.assign(new Error('the answer was let go before this reply; the request may already have been sent'), { kind: 'Aborted' }); })])
      : work;
  }

  dispatch(request: DeviceRequest): DeviceRequest {
    switch (request.op) {
      case 'open': return { ok: { opened: true } };
      case 'close': this.owner = undefined; return { ok: null };
      case 'status': return { ok: { open: true, online: true, syncing: !!this.owner, denied: null, revision: this.rows.length, acquired: true, acquisition } };
      case 'cancel': {
        const owns = this.owner === request.round;
        if (owns) this.owner = undefined;
        return { ok: owns };
      }
      case 'sync':
        if (this.owner) return { ok: { done: { ok: false, busy: true } } };
        this.owner = String(request.round);
        return { ok: { fetch: { method: 'POST', path: '/sync', exchange: String(++this.exchange) } } };
      case 'deliver': {
        const reply = request.reply as DeviceRequest;
        this.rows = (reply.body as DeviceRequest).rows as unknown[];
        // A lost apply can advance to a new exchange before cancellation arrives.
        if (this.loseApply) return { ok: { fetch: { method: 'GET', path: '/next', exchange: String(++this.exchange) } } };
        this.owner = undefined;
        return { ok: { done: { ok: true, acquired: true } } };
      }
      default: return { ok: true };
    }
  }
}

let number = 0;
async function withNative(work: (native: Native, db: Snapback) => Promise<void>) {
  const native = new Native();
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async () => new Response(JSON.stringify({ rows: Array.from({ length: 5000 }, (_, id) => ({ id, body: 'a large history page' })) }))) as typeof fetch;
  const db = await Snapback.open({ app: `test.native.${++number}`, name: 'history', origin: 'https://native.test',
    viewer: 'dev:alice', now: 100, native, storage: { sqlite: { async open() { throw new Error('native must not open wasm storage'); } } } });
  try { await work(native, db); }
  finally { native.ended = false; native.resume.resolve(); await native.tail; await db.close(100); globalThis.fetch = realFetch; }
}

test('native large apply yields, busy cheap reads retry off-turn, and the next round runs', async () => {
  await withNative(async (native, db) => {
    const first = db.sync({ now: 100 });
    await native.applying.promise;
    expect(native.busy).toBe(true);
    const status = db.status(100);
    expect(native.calls).toContain('status');
    expect(native.laters).toContain('status');
    const second = db.sync({ now: 100 });
    native.resume.resolve();
    expect(await first).toMatchObject({ ok: true });
    expect((await status).revision).toBe(5000);
    expect(await second).toMatchObject({ ok: true });
    expect(native.owner).toBeUndefined();
    expect(native.rows).toHaveLength(5000);
    expect(native.calls).not.toContain('deliver');
    expect(native.calls).not.toContain('sync');
  });
});

test('a refusal from an ended answer releases the round queue without advancing time', async () => {
  await withNative(async (native, db) => {
    native.ended = true;
    await expect(db.sync({ now: 100 })).rejects.toThrow('native call outside an answer');
    native.ended = false;
    native.resume.resolve();
    expect(await db.sync({ now: 100 })).toMatchObject({ ok: true });
    expect(native.calls).toContain('cancel');
    // The synchronous host refusal has the same immediate recovery.
    native.ended = true;
    await expect(db.status(100)).rejects.toThrow('native call outside an answer');
    native.ended = false;
    expect(await db.sync({ now: 100 })).toMatchObject({ ok: true });
  });
});

test('a lost apply reply settles immediately; a new answer cancels after apply advances', async () => {
  await withNative(async (native, db) => {
    native.loseApply = true;
    const first = db.sync({ now: 100 });
    const failed = first.catch(error => error);
    await native.applying.promise;
    native.lost.resolve();
    expect((await failed).message).toContain('the answer was let go');
    // No cleanup on the rejected answer, and no wait for the SQLite worker.
    expect(native.busy).toBe(true);
    expect(native.calls).not.toContain('cancel');
    const next = db.sync({ now: 100 });
    native.resume.resolve();
    await native.tail;
    native.loseApply = false;
    expect(await next).toMatchObject({ ok: true });
    expect(native.exchange).toBe(3);
    expect(native.owner).toBeUndefined();
  });
});

test('a lost first sync reply is cancelled by owner even without an exchange token', async () => {
  await withNative(async (native, db) => {
    native.failSync = true;
    await expect(db.sync({ now: 100 })).rejects.toThrow('before its first exchange');
    expect(native.owner).toBeDefined();
    native.resume.resolve();
    expect(await db.sync({ now: 100 })).toMatchObject({ ok: true });
    expect(native.owner).toBeUndefined();
  });
});

test('row-scaled device operations use later; SQLite metadata and query reads stay synchronous', async () => {
  const calls: string[] = [], laters: string[] = [];
  const device = nativeDevice({ call(request) { calls.push(String(request.op)); return { ok: true }; },
    async later(request) { laters.push(String(request.op)); return { ok: true }; } });
  const heavy = ['apply', 'adopt', 'observe_store', 'clear_rows', 'install_native', 'open', 'close', 'sync', 'deliver',
    'persist', 'hold', 'write', 'predict', 'admit', 'settle', 'withdraw', 'refusals', 'dismiss', 'replace', 'restore'];
  const cheap = ['status', 'sync_state', 'meta', 'queued', 'outcome', 'query', 'read'];
  for (const op of [...heavy, ...cheap]) await device.call({ op });
  expect(laters).toEqual(heavy);
  expect(calls).toEqual(cheap);
});
