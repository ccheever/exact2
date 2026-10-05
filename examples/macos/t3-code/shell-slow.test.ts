// The client's side of "Some requests are slow" (lane r13-slow, round-12 F4): each request is timed from the
// moment it was sent and ends with its own reply; the requests of an environment that was switched off,
// disconnected or replaced end with it (the reference ends a request when its session closes).
import { describe, expect, test } from 'bun:test';
import { toasts } from './toast';
import { slowRequests, tracking, trackRpc, isTracked, TRANSPORT_REQUEST_DEADLINE_MS } from './shell-slow';
import { traceRpc, statusTicket, settleTraces, settleLiveTraces } from './r3-protocol-reader';
import { T3Client } from './client';
import { shellView } from './shell';
import type { Obj } from './domain';

function fakeClient(extra: Obj = {}): T3Client {
  return { threadId: '', projectId: '', ready: true, environmentId: 'env-a', generation: 4, connection: 'connected', shell: { threads: [], projects: [] },
    config: { environment: { capabilities: {} } }, local: { clientSettings: {}, deviceSettings: { timestampFormat: '24-hour' } }, ...extra } as unknown as T3Client;
}
const request = (method = 'orchestration.getFullThreadDiff') => ({ op: 'request', method, payload: {} });
const slowToasts = (client: T3Client) => toasts(client).filter(toast => toast.title === 'Some requests are slow');
const T0 = Date.UTC(2026, 9, 5, 14, 0, 0);

describe('request start', () => {
  test('a request is timed from the first shell time after it was sent, not from a stale one', () => {
    const client = fakeClient();
    slowRequests(client, T0); // the last time the shell ran; nothing ticks afterwards
    const ack = trackRpc(client, request()); // sent ten minutes later
    expect(tracking(client)).toBe(true);
    slowRequests(client, T0); // the shell asks again with the same, stale time: the clock has not started
    slowRequests(client, T0 + 600_000); // the first tick after the idle gap
    expect(slowToasts(client)).toHaveLength(0);
    slowRequests(client, T0 + 600_000 + 14_999);
    expect(slowToasts(client)).toHaveLength(0);
    slowRequests(client, T0 + 600_000 + 15_000);
    expect(slowToasts(client).map(toast => toast.description)).toEqual(['1 request waiting longer than 15s.']);
    ack();
    slowRequests(client, T0 + 600_000 + 15_500);
    expect(slowToasts(client)).toHaveLength(0);
  });

  test('a request answered before the shell ticks is never timed', () => {
    const client = fakeClient();
    slowRequests(client, T0);
    trackRpc(client, request())();
    expect(tracking(client)).toBe(false);
    slowRequests(client, T0 + 600_000);
    slowRequests(client, T0 + 700_000);
    expect(slowToasts(client)).toHaveLength(0);
  });

  test('a request sent while the shell ticks is timed from the next tick', () => {
    const client = fakeClient();
    slowRequests(client, T0);
    slowRequests(client, T0 + 500);
    trackRpc(client, request());
    slowRequests(client, T0 + 1000); // the next tick starts it
    slowRequests(client, T0 + 1000 + 14_999);
    expect(slowToasts(client)).toHaveLength(0);
    slowRequests(client, T0 + 1000 + 15_000);
    expect(slowToasts(client)).toHaveLength(1);
  });

  test('before the shell has any time the first time starts the clock', () => {
    const client = fakeClient();
    const ack = trackRpc(client, request());
    slowRequests(client, T0);
    slowRequests(client, T0 + 15_000);
    expect(slowToasts(client)).toHaveLength(1);
    ack();
  });
});

describe('a request held past its threshold', () => {
  test('shows exactly one toast, however often the shell runs, and clears on the reply', () => {
    const client = fakeClient();
    slowRequests(client, T0);
    const ack = trackRpc(client, request('git.status'), T0);
    for (let at = 500; at <= 14_500; at += 500) slowRequests(client, T0 + at);
    expect(slowToasts(client)).toHaveLength(0);
    for (let at = 15_000; at <= 30_000; at += 500) slowRequests(client, T0 + at);
    expect(slowToasts(client)).toHaveLength(1);
    expect(slowToasts(client)[0]).toMatchObject({ kind: 'warning', description: '1 request waiting longer than 15s.', timeoutMs: 0 });
    ack(); // the reply
    slowRequests(client, T0 + 30_500);
    expect(slowToasts(client)).toHaveLength(0);
    expect(tracking(client)).toBe(false);
    for (let at = 31_000; at <= 60_000; at += 500) slowRequests(client, T0 + at);
    expect(slowToasts(client)).toHaveLength(0);
  });

  test('a second held request joins the same toast', () => {
    const client = fakeClient();
    slowRequests(client, T0);
    const first = trackRpc(client, request('git.status'), T0);
    const second = trackRpc(client, request('git.pull'), T0 + 5000);
    slowRequests(client, T0 + 15_000);
    slowRequests(client, T0 + 20_000);
    expect(slowToasts(client)).toHaveLength(1);
    expect(slowToasts(client)[0]!.description).toBe('2 requests waiting longer than 15s.');
    first();
    slowRequests(client, T0 + 21_000);
    expect(slowToasts(client)[0]!.description).toBe('1 request waiting longer than 15s.');
    second();
    slowRequests(client, T0 + 22_000);
    expect(slowToasts(client)).toHaveLength(0);
  });
});

describe('a request the transport has failed', () => {
  test('ends when the transport has timed it out, even though the answer that sent it never hears the failure', () => {
    const client = fakeClient();
    slowRequests(client, T0);
    trackRpc(client, request('vcs.refreshStatus'), T0); // its answer was let go: no acknowledgement will come
    slowRequests(client, T0 + 15_000);
    expect(slowToasts(client)).toHaveLength(1);
    slowRequests(client, T0 + TRANSPORT_REQUEST_DEADLINE_MS - 1);
    expect(slowToasts(client)).toHaveLength(1);
    slowRequests(client, T0 + TRANSPORT_REQUEST_DEADLINE_MS);
    expect(slowToasts(client)).toHaveLength(0);
    expect(tracking(client)).toBe(false);
  });

  test('is older than the deadline T3Transport.rpc gives a request', () => {
    expect(TRANSPORT_REQUEST_DEADLINE_MS).toBeGreaterThan(30_000);
    expect(TRANSPORT_REQUEST_DEADLINE_MS).toBeLessThan(35_000);
  });
});

describe('requests of an environment that is gone', () => {
  function held(client: T3Client) {
    slowRequests(client, T0);
    const acks = [trackRpc(client, request('server.getConfig'), T0), trackRpc(client, request('git.status'), T0 + 1000)];
    slowRequests(client, T0 + 16_000);
    expect(slowToasts(client)[0]!.description).toBe('2 requests waiting longer than 15s.');
    return acks;
  }

  test('switching environment A off and B on ends the slow requests of A', () => {
    const client = fakeClient();
    held(client);
    // A off: the transport follows B (a new environment id and connection generation).
    Object.assign(client, { connection: 'connecting' });
    slowRequests(client, T0 + 17_000);
    expect(slowToasts(client)).toHaveLength(0);
    expect(tracking(client)).toBe(false);
    Object.assign(client, { environmentId: 'env-b', generation: 5, connection: 'connected' });
    slowRequests(client, T0 + 18_000);
    slowRequests(client, T0 + 60_000);
    expect(slowToasts(client)).toHaveLength(0);
  });

  test('requests that were sent to A and are still pending are not timed once B is the environment', () => {
    const client = fakeClient();
    slowRequests(client, T0);
    trackRpc(client, request('server.getConfig')); // sent to A, never answered (its answer was let go)
    Object.assign(client, { environmentId: 'env-b', generation: 5 });
    slowRequests(client, T0 + 500);
    slowRequests(client, T0 + 20_000);
    expect(slowToasts(client)).toHaveLength(0);
    expect(tracking(client)).toBe(false);
  });

  test('a request sent to B after the switch is timed on its own', () => {
    const client = fakeClient();
    held(client);
    Object.assign(client, { environmentId: 'env-b', generation: 5 });
    slowRequests(client, T0 + 17_000);
    expect(slowToasts(client)).toHaveLength(0);
    trackRpc(client, request('git.status'), T0 + 17_000);
    slowRequests(client, T0 + 31_999);
    expect(slowToasts(client)).toHaveLength(0);
    slowRequests(client, T0 + 32_000);
    expect(slowToasts(client)[0]!.details!.map(detail => detail.title)).toEqual(['git.status · env-b']);
  });

  test('a replaced connection of the same environment ends the requests of the old one', () => {
    const client = fakeClient();
    held(client);
    Object.assign(client, { generation: 5 });
    slowRequests(client, T0 + 17_000);
    expect(slowToasts(client)).toHaveLength(0);
  });

  test('a client that is not connected does not time requests', () => {
    for (const connection of ['disconnected', 'connecting', 'reconnecting', 'error']) {
      const client = fakeClient();
      held(client);
      Object.assign(client, { connection });
      slowRequests(client, T0 + 17_000);
      expect(slowToasts(client)).toHaveLength(0);
      expect(tracking(client)).toBe(false);
    }
  });

  test('an acknowledgement of a request that already ended is harmless', () => {
    const client = fakeClient();
    const acks = held(client);
    Object.assign(client, { environmentId: 'env-b' });
    slowRequests(client, T0 + 17_000);
    for (const ack of acks) ack();
    expect(isTracked(client, acks[0]!.requestId)).toBe(false);
    slowRequests(client, T0 + 18_000);
    expect(slowToasts(client)).toHaveLength(0);
  });
});

describe('status reads end traces with their environment', () => {
  test('a status that names another environment ends the traces of the old one on the first read', () => {
    const client = new T3Client(); client.environmentId = 'env-a';
    const first = traceRpc(client, { op: 'request', method: 'server.getConfig', payload: {} });
    const second = traceRpc(client, { op: 'request', method: 'git.status', payload: {} });
    expect(tracking(client)).toBe(true);
    // Both are still listed as pending by name only; the status names B, so A's sockets are gone.
    expect(settleTraces(client, { state: 'connected', environmentId: 'env-b', traces: [first.request.trace, second.request.trace] }, statusTicket(client))).toBe(2);
    expect(tracking(client)).toBe(false);
  });

  test('a status that says the transport is not connected ends every trace sent before the read', () => {
    const client = new T3Client(); client.environmentId = 'env-a';
    const sent = traceRpc(client, { op: 'request', method: 'server.getConfig', payload: {} });
    const ticket = statusTicket(client);
    const later = traceRpc(client, { op: 'request', method: 'git.status', payload: {} }); // sent after the read began
    expect(settleTraces(client, { state: 'reconnecting', environmentId: 'env-a', traces: [] }, ticket)).toBe(1);
    expect(tracking(client)).toBe(true);
    later.done(); sent.done();
    expect(tracking(client)).toBe(false);
  });

  test('a status of the same connected environment keeps a traced call the transport still lists', () => {
    const client = new T3Client(); client.environmentId = 'env-a';
    const call = traceRpc(client, { op: 'request', method: 'git.status', payload: {} });
    expect(settleTraces(client, { state: 'connected', environmentId: 'env-a', traces: [call.request.trace] }, statusTicket(client))).toBe(0);
    expect(settleTraces(client, { state: 'connected', environmentId: 'env-a', traces: [call.request.trace] }, statusTicket(client))).toBe(0);
    expect(tracking(client)).toBe(true);
    call.done();
    expect(tracking(client)).toBe(false);
  });

  test('polling the status ends answered traces whose answer was let go, and keeps the pending one', async () => {
    const client = new T3Client(); client.environmentId = 'env-a'; client.connection = 'connected';
    slowRequests(client, T0);
    const answered = traceRpc(client, { op: 'request', method: 'vcs.refreshStatus', payload: {} });
    const pending = traceRpc(client, { op: 'request', method: 'git.status', payload: {} });
    let listed: unknown[] = [answered.request.trace, pending.request.trace];
    const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 1, value: { state: 'connected', environmentId: 'env-a', traces: listed } }) };
    expect(await settleLiveTraces(client, native)).toBe(0); // the first read never settles
    listed = [pending.request.trace]; // the transport answered the first request
    expect(await settleLiveTraces(client, native)).toBe(1);
    expect(tracking(client)).toBe(true);
    expect(await settleLiveTraces(client, native)).toBe(0);
    pending.done();
    expect(tracking(client)).toBe(false);
    expect(await settleLiveTraces(client, native)).toBe(0); // nothing live: no status read
  });

  test('a failed status read settles nothing', async () => {
    const client = new T3Client(); client.environmentId = 'env-a';
    traceRpc(client, { op: 'request', method: 'git.status', payload: {} });
    const failing = { available: true, watch() {}, later: async () => { throw new Error('gone'); } };
    expect(await settleLiveTraces(client, failing)).toBe(0);
    expect(await settleLiveTraces(client, failing)).toBe(0);
    expect(tracking(client)).toBe(true);
  });

  test('a trace that slowRequests already ended leaves no entry behind', () => {
    const client = new T3Client(); client.environmentId = 'env-a'; client.connection = 'connected';
    slowRequests(client, T0);
    const call = traceRpc(client, { op: 'request', method: 'git.status', payload: {} });
    client.environmentId = 'env-b';
    slowRequests(client, T0 + 500);
    expect(tracking(client)).toBe(false);
    expect(settleTraces(client, { state: 'connected', environmentId: 'env-b', traces: [] }, statusTicket(client))).toBe(0);
    call.done();
  });
});

// The real T3Client.call (private) with a native whose reply the test holds: the plumbing between a request, its trace
// and the warning, without a transport.
describe('client.call end to end', () => {
  type Call = (native: unknown, request: unknown, expected?: number, write?: boolean) => Promise<Obj>;
  function connected(): T3Client {
    const client = new T3Client();
    Object.assign(client, { environmentId: 'env-a', generation: 7, connection: 'connected' });
    return client;
  }
  function held() {
    const sent: Obj[] = [];
    let release: (value: unknown) => void = () => undefined;
    const reply = new Promise(resolve => { release = resolve; });
    const native = { available: true, watch() {}, later: async (request: unknown) => { sent.push(request as Obj); return reply; } };
    return { native, sent, answer: () => release({ ok: true, generation: 7, value: { done: true } }), fail: () => release({ ok: false, generation: 7, error: { kind: 'transport', message: 'gone' } }) };
  }
  const call = (client: T3Client, native: unknown) => (client as unknown as { call: Call }).call.call(client, native, { op: 'request', method: 'git.status', payload: {} });

  test('a request is tracked from the call, shows one toast past 15 s and clears with its reply', async () => {
    const client = connected(), h = held();
    slowRequests(client, T0);
    const done = call(client, h.native);
    expect(h.sent[0]).toMatchObject({ op: 'request', method: 'git.status', trace: 1, generation: 7 });
    expect(tracking(client)).toBe(true);
    slowRequests(client, T0 + 500);
    for (let at = 1000; at <= 14_500; at += 500) slowRequests(client, T0 + at);
    expect(slowToasts(client)).toHaveLength(0);
    for (let at = 15_500; at <= 30_000; at += 500) slowRequests(client, T0 + at);
    expect(slowToasts(client)).toHaveLength(1);
    expect(slowToasts(client)[0]!.details!.map(detail => detail.title)).toEqual(['git.status · env-a']);
    h.answer();
    expect(await done).toEqual({ done: true });
    slowRequests(client, T0 + 30_500);
    expect(slowToasts(client)).toHaveLength(0);
    expect(tracking(client)).toBe(false);
  });

  test('a failed request ends the same way', async () => {
    const client = connected(), h = held();
    slowRequests(client, T0);
    const done = call(client, h.native);
    slowRequests(client, T0 + 500);
    slowRequests(client, T0 + 20_000);
    expect(slowToasts(client)).toHaveLength(1);
    h.fail();
    await expect(done).rejects.toThrow('gone');
    slowRequests(client, T0 + 20_500);
    expect(slowToasts(client)).toHaveLength(0);
  });

  test('a request whose answer was let go (its reply never arrives) stops counting when the environment switches', () => {
    const client = connected(), h = held();
    slowRequests(client, T0);
    void call(client, h.native); // the awaiting answer is dropped: nothing resumes it
    slowRequests(client, T0 + 500);
    slowRequests(client, T0 + 20_000);
    expect(slowToasts(client)).toHaveLength(1);
    // A is switched off and B is switched on.
    Object.assign(client, { connection: 'connecting' });
    slowRequests(client, T0 + 20_500);
    expect(slowToasts(client)).toHaveLength(0);
    Object.assign(client, { environmentId: 'env-b', generation: 8, connection: 'connected' });
    for (let at = 21_000; at <= 60_000; at += 1000) slowRequests(client, T0 + at);
    expect(slowToasts(client)).toHaveLength(0);
    expect(tracking(client)).toBe(false);
  });
});

// shellView polls the transport's status while it times requests (the hunk in shell.ts): an answered request whose
// answer was let go ends on the shell's next read, long before the transport's 30 s deadline.
describe('shellView settles live traces', () => {
  test('a request the transport answered stops counting as slow on the next shell read', async () => {
    const client = new T3Client(); Object.assign(client, { environmentId: 'env-a', generation: 3, connection: 'connected' });
    const files = { fs: { async mkdir() {}, async readFile(): Promise<ArrayBuffer> { throw new Error('missing'); }, async atomicWriteFile() {} } };
    let listed: number[] = [];
    const reads: string[] = [];
    const native = { available: true, watch() {}, later: async (input: unknown) => {
      const request = input as Obj; reads.push(String(request.op));
      if (request.op === 'status') return { ok: true, generation: 3, value: { state: 'connected', environmentId: 'env-a', traces: listed } };
      if (request.op === 'notifyStatus') return { ok: true, generation: 3, value: { active: true, authorization: 'unknown', agent: false, opened: '', openedThread: '' } };
      return { ok: true, generation: 3, value: {} };
    } };
    await shellView(client, native, files as never, T0, '', false);
    const call = traceRpc(client, { op: 'request', method: 'vcs.refreshStatus', payload: { cwd: '/x' } }); // its answer is let go
    listed = [call.request.trace as number];
    await shellView(client, native, files as never, T0 + 500, '', false);
    await shellView(client, native, files as never, T0 + 1000, '', false);
    expect(tracking(client)).toBe(true); // still pending at the transport
    listed = []; // the transport answered it
    await shellView(client, native, files as never, T0 + 1500, '', false);
    await shellView(client, native, files as never, T0 + 20_000, '', false);
    expect(tracking(client)).toBe(false);
    expect(slowToasts(client)).toHaveLength(0);
    const before = reads.filter(op => op === 'status').length;
    await shellView(client, native, files as never, T0 + 21_000, '', false);
    expect(reads.filter(op => op === 'status').length).toBe(before); // nothing live: no status read
  });
});
