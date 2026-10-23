import { describe, expect, test } from 'bun:test';
import { constants, createDeflate } from 'node:zlib';
import { identifyPayload, identity, restHeaders } from './client.ts';
import {
  heartbeatDue, initialGateway, nextHeartbeatAt, snapshot, step,
  type GatewayState, type StepResult,
} from './gateway.ts';

const id = identity('ios', { launchId: 'fixture-launch', heartbeatSessionId: 'fixture-heartbeat' });
const token = 'fixture-token';
const gatewayUrl = 'wss://gateway.fixture.invalid/?fixture=1&v=8';
const ready = { op: 0, t: 'READY', s: 1, d: { session_id: 'fixture-session', resume_gateway_url: 'wss://resume.fixture.invalid' } };
const hello = '{"op":10,"d":{"heartbeat_interval":1000}}';

function fresh(options: Partial<Parameters<typeof initialGateway>[0]> = {}): GatewayState {
  return initialGateway({ identity: id, token, gatewayUrl, ...options });
}

function open(state: GatewayState, nowMs = 0, jitter = 0.25): void {
  step(state, { type: 'connect', nowMs, jitter });
  step(state, { type: 'open', nowMs });
}

function message(state: GatewayState, payload: unknown, nowMs = 0, jitter?: number): StepResult {
  return step(state, { type: 'message', data: JSON.stringify(payload), nowMs, jitter });
}

function established(): GatewayState {
  const state = fresh();
  open(state);
  step(state, { type: 'message', data: hello, nowMs: 0 });
  message(state, ready, 1);
  return state;
}

function retry(state: GatewayState, jitter = 0.25): number {
  const nowMs = state.retryAtMs!;
  const result = step(state, { type: 'retry', nowMs, jitter });
  expect(result.connect?.atMs).toBe(nowMs);
  expect(state.phase).toBe('connecting');
  step(state, { type: 'open', nowMs });
  return nowMs;
}

function sent(result: StepResult): any[] {
  return result.send.map(frame => JSON.parse(frame));
}

function dispatches(result: StepResult) {
  return result.changes.filter(change => change.kind === 'dispatch');
}

// One zlib context, with a sync flush after each gateway JSON document. Independent
// deflateSync calls would miss dictionary reuse, the failure these fixtures exercise.
async function compressedFrames(payloads: unknown[]): Promise<Uint8Array[]> {
  const compressor = createDeflate();
  let chunks: Uint8Array[] = [];
  compressor.on('data', chunk => chunks.push(new Uint8Array(chunk)));
  const frames: Uint8Array[] = [];
  try {
    for (const payload of payloads) {
      await new Promise<void>((resolve, reject) => {
        compressor.once('error', reject);
        compressor.write(JSON.stringify(payload));
        compressor.flush(constants.Z_SYNC_FLUSH, () => {
          compressor.off('error', reject);
          resolve();
        });
      });
      const frame = new Uint8Array(chunks.reduce((size, chunk) => size + chunk.length, 0));
      let offset = 0;
      for (const chunk of chunks) {
        frame.set(chunk, offset);
        offset += chunk.length;
      }
      expect([...frame.slice(-4)]).toEqual([0, 0, 255, 255]);
      frames.push(frame);
      chunks = [];
    }
  } finally {
    compressor.destroy();
  }
  return frames;
}

describe('gateway session', () => {
  test('text HELLO identifies with the host payload; READY and every dispatch advance seq', () => {
    const state = fresh();
    open(state, 100);
    const identified = step(state, { type: 'message', data: hello, nowMs: 100 });
    expect(sent(identified)).toEqual([{ op: 2, d: identifyPayload(id, token) }]);
    expect(state.phase).toBe('identifying');
    expect(state.identifies).toEqual([100]);
    const received = message(state, ready, 101);
    expect(dispatches(received)).toEqual([{ kind: 'dispatch', t: 'READY', s: 1, d: ready.d }]);
    expect(snapshot(state)).toMatchObject({ phase: 'ready', seq: 1, sessionId: 'fixture-session' });
    expect(state.resumeGatewayUrl).toBe(ready.d.resume_gateway_url);
    const unknown = message(state, { op: 0, t: 'FUTURE_EVENT', s: 19, d: { value: 42 } }, 102);
    expect(state.seq).toBe(19);
    expect(dispatches(unknown)[0]).toMatchObject({ t: 'FUTURE_EVENT', s: 19, d: { value: 42 } });
    expect(step(state, { type: 'message', data: hello, nowMs: 103 }).send).toEqual([]);
    expect(state.identifies).toEqual([100]);
  });

  test('first heartbeat uses supplied jitter, ACK permits the next, and zero jitter sends immediately', () => {
    const state = fresh();
    open(state, 100, 0.25);
    step(state, { type: 'message', data: hello, nowMs: 100 });
    expect(nextHeartbeatAt(state)).toBe(350);
    expect(heartbeatDue(state, 349)).toBe(false);
    expect(step(state, { type: 'heartbeat', nowMs: 349 }).send).toEqual([]);
    expect(heartbeatDue(state, 350)).toBe(true);
    expect(sent(step(state, { type: 'heartbeat', nowMs: 350 }))).toEqual([{ op: 1, d: null }]);
    expect(nextHeartbeatAt(state)).toBe(1350);
    message(state, { op: 11 }, 360);
    expect(state.awaitingAck).toBe(false);
    expect(state.lastAckAtMs).toBe(360);
    expect(sent(step(state, { type: 'heartbeat', nowMs: 1350 }))).toEqual([{ op: 1, d: null }]);
    const immediate = fresh();
    open(immediate, 200, 0);
    step(immediate, { type: 'message', data: hello, nowMs: 200 });
    expect(nextHeartbeatAt(immediate)).toBe(200);
    expect(sent(step(immediate, { type: 'heartbeat', nowMs: 200 }))).toEqual([{ op: 1, d: null }]);
  });

  test('missing ACK closes non-normally and resumes without spending an identify', () => {
    const state = established();
    expect(sent(step(state, { type: 'heartbeat', nowMs: 250 }))).toEqual([{ op: 1, d: 1 }]);
    const zombie = step(state, { type: 'heartbeat', nowMs: 1250 });
    expect(zombie.close).toEqual({ code: 4000, reason: 'heartbeat ACK missing' });
    expect(zombie.send).toEqual([]);
    expect(zombie.connect?.url).toBe('wss://resume.fixture.invalid/?encoding=json&v=9&compress=zlib-stream');
    expect(nextHeartbeatAt(state)).toBeNull();
    const deadline = state.retryAtMs;
    expect(step(state, { type: 'close', code: 4000, nowMs: 1251 }).connect).toBeUndefined();
    expect(state.retryAtMs).toBe(deadline);
    const nowMs = retry(state, 0.75);
    const resumed = step(state, { type: 'message', data: hello, nowMs });
    expect(sent(resumed)).toEqual([{ op: 6, d: { token, session_id: 'fixture-session', seq: 1 } }]);
    expect(nextHeartbeatAt(state)).toBe(nowMs + 750);
    expect(state.identifies).toEqual([0]);
    message(state, { op: 0, t: 'RESUMED', s: 2, d: {} }, nowMs + 1);
    expect(state.phase).toBe('ready');
    expect(state.reconnectAttempts).toBe(0);
  });

  test('server-requested heartbeat is immediate and resets the periodic deadline', () => {
    const state = established();
    const result = message(state, { op: 1, d: null }, 100);
    expect(sent(result)).toEqual([{ op: 1, d: 1 }]);
    expect(state.awaitingAck).toBe(true);
    expect(nextHeartbeatAt(state)).toBe(1100);
    message(state, { op: 11 }, 101);
    expect(step(state, { type: 'heartbeat', nowMs: 250 }).send).toEqual([]);
    expect(step(state, { type: 'heartbeat', nowMs: 1100 }).send).toHaveLength(1);
    expect(message(state, { op: 1 }, 1101).send).toHaveLength(1);
    expect(nextHeartbeatAt(state)).toBe(2101);
  });

  for (const nowMs of [249, 250, 251]) {
    test(`server heartbeat at ${nowMs} grants a full interval for its ACK`, () => {
      const state = established();
      const requested = message(state, { op: 1 }, nowMs);
      expect(sent(requested)).toEqual([{ op: 1, d: 1 }]);
      expect(requested.close).toBeUndefined();
      expect(state.awaitingAck).toBe(true);
      expect(state.lastHeartbeatAtMs).toBe(nowMs);
      expect(nextHeartbeatAt(state)).toBe(nowMs + 1000);
      for (const tick of [Math.max(250, nowMs), nowMs + 999]) {
        const waiting = step(state, { type: 'heartbeat', nowMs: tick });
        expect(waiting.close).toBeUndefined();
        expect(waiting.send).toEqual([]);
        expect(state.awaitingAck).toBe(true);
      }
      const missed = step(state, { type: 'heartbeat', nowMs: nowMs + 1000 });
      expect(missed.close).toEqual({ code: 4000, reason: 'heartbeat ACK missing' });
      expect(missed.send).toEqual([]);
    });
  }

  test('READY refreshes authentication for REST, RESUME and the next fresh IDENTIFY', () => {
    const state = fresh();
    open(state);
    step(state, { type: 'message', data: hello, nowMs: 0 });
    const refreshedToken = 'fixture-refreshed-token';
    const received = message(state, { ...ready, d: { ...ready.d, auth_token: refreshedToken } }, 1);
    expect(received.changes).toContainEqual({ kind: 'token', token: refreshedToken });
    let credential = token;
    let headers = restHeaders(id, credential);
    for (const change of received.changes) {
      if (change.kind === 'token') {
        credential = change.token;
        headers = restHeaders(id, credential);
      }
    }
    expect(credential).toBe(refreshedToken);
    expect(headers.Authorization).toBe(refreshedToken);
    const reconnect = step(state, { type: 'close', code: 1006, reason: refreshedToken, nowMs: 10 });
    let nowMs = retry(state);
    const resumed = step(state, { type: 'message', data: hello, nowMs });
    expect(sent(resumed)).toEqual([{ op: 6, d: { token: refreshedToken, session_id: 'fixture-session', seq: 1 } }]);
    const invalid = message(state, { op: 9, d: false }, nowMs + 1);
    nowMs = retry(state);
    const identified = step(state, { type: 'message', data: hello, nowMs });
    expect(sent(identified)).toEqual([{ op: 2, d: identifyPayload(id, refreshedToken) }]);
    const statusChanges = [received, reconnect, resumed, invalid, identified]
      .flatMap(result => result.changes.filter(change => change.kind === 'status'));
    for (const secret of [token, refreshedToken]) {
      expect(JSON.stringify(snapshot(state))).not.toContain(secret);
      expect(JSON.stringify(statusChanges)).not.toContain(secret);
    }
    for (const authToken of [undefined, null, '', 123, refreshedToken]) {
      const unchanged = message(state, { ...ready, d: { ...ready.d, auth_token: authToken } }, nowMs + 1);
      expect(state.token).toBe(refreshedToken);
      expect(unchanged.changes.filter(change => change.kind === 'token')).toEqual([]);
    }
  });

  test('RECONNECT preserves the session and uses its resume URL', () => {
    const state = established();
    message(state, { op: 0, t: 'MESSAGE_CREATE', s: 44, d: {} }, 10);
    const result = message(state, { op: 7, d: null }, 20);
    expect(result.close?.code).toBe(4000);
    expect(result.connect?.url).toContain('resume.fixture.invalid');
    expect(state.phase).toBe('backoff');
    const nowMs = retry(state);
    expect(sent(step(state, { type: 'message', data: hello, nowMs }))[0]).toEqual({
      op: 6, d: { token, session_id: 'fixture-session', seq: 44 },
    });
  });

  test('INVALID_SESSION true resumes; false forgets and waits 1–5 seconds before identifying', () => {
    for (const resumable of [true, false]) {
      const state = established();
      const result = message(state, { op: 9, d: resumable }, 100, 0.75);
      expect(result.close?.code).toBe(4000);
      if (!resumable) {
        expect(state.sessionId).toBeNull();
        expect(state.seq).toBeNull();
        expect(state.resumeGatewayUrl).toBeNull();
        expect(result.connect?.atMs).toBe(4100);
        expect(result.connect?.url).toBe('wss://gateway.fixture.invalid/?fixture=1&v=9&encoding=json&compress=zlib-stream');
      }
      const nowMs = retry(state);
      const next = step(state, { type: 'message', data: hello, nowMs });
      expect(sent(next)[0].op).toBe(resumable ? 6 : 2);
      expect(state.identifies).toHaveLength(resumable ? 1 : 2);
    }
  });

  test('INVALID_SESSION false keeps its 1–5 second delay after repeated failures', () => {
    for (const jitter of [0, 0.999]) {
      const state = established();
      state.reconnectAttempts = 8;
      const result = message(state, { op: 9, d: false }, 100, jitter);
      expect(result.connect?.atMs).toBe(100 + 1000 + 4000 * jitter);
      const nowMs = retry(state);
      expect(sent(step(state, { type: 'message', data: hello, nowMs }))[0].op).toBe(2);
    }
  });

  test('fatal close codes stop permanently, including 4004', () => {
    for (const code of [4004, 4010, 4011, 4012, 4013, 4014]) {
      const state = established();
      const result = step(state, { type: 'close', code, nowMs: 100, reason: 'untrusted detail' });
      expect(result.connect).toBeUndefined();
      expect(result.send).toEqual([]);
      expect(state.phase).toBe('stopped');
      expect(state.stoppedReason).toContain(String(code));
      expect(state.stoppedReason).not.toContain('untrusted');
      expect(state.sessionId).toBeNull();
      expect(step(state, { type: 'retry', nowMs: 10_000, jitter: 0 }).connect).toBeUndefined();
      expect(step(state, { type: 'connect', nowMs: 10_001 }).changes).toEqual([]);
    }
  });

  test('1000/1001/4007/4009 identify fresh; every other recoverable close resumes', () => {
    for (const code of [4007, 4009, 1000, 1001, 1006, 4000, 4001, 4002, 4003, 4005, 4008]) {
      const state = established();
      const closed = step(state, { type: 'close', code, nowMs: 10 });
      const freshSession = [1000, 1001, 4007, 4009].includes(code);
      if (freshSession) {
        expect(state.sessionId).toBeNull();
        expect(state.seq).toBeNull();
        expect(state.resumeGatewayUrl).toBeNull();
        expect(closed.connect?.url).toContain('gateway.fixture.invalid');
      }
      const nowMs = retry(state);
      const next = step(state, { type: 'message', data: hello, nowMs });
      expect(sent(next)[0].op).toBe(freshSession ? 2 : 6);
      expect(state.identifies.length).toBe(freshSession ? 2 : 1);
    }
  });

  test('backoff doubles, caps at 60 seconds, uses retry jitter, and resets on READY', () => {
    const state = fresh();
    open(state, 0, 0);
    let nowMs = 10;
    const delays: number[] = [];
    for (let i = 0; i < 9; i++) {
      const result = step(state, { type: 'close', code: 1006, nowMs });
      delays.push(result.connect!.atMs - nowMs);
      nowMs = retry(state, i === 0 ? 0.5 : 0) + 1;
    }
    expect(delays).toEqual([1000, 3000, 4000, 8000, 16000, 32000, 60000, 60000, 60000]);
    step(state, { type: 'message', data: hello, nowMs });
    message(state, ready, nowMs + 1);
    const result = step(state, { type: 'close', code: 1006, nowMs: nowMs + 2 });
    expect(result.connect!.atMs - (nowMs + 2)).toBe(1000);
  });

  test('early retry/connect cannot bypass a deadline; late ticks send only one heartbeat', () => {
    const state = established();
    step(state, { type: 'close', code: 1006, nowMs: 10 });
    const atMs = state.retryAtMs!;
    expect(step(state, { type: 'retry', nowMs: 11, jitter: 0 }).connect?.atMs).toBe(atMs);
    expect(state.phase).toBe('backoff');
    expect(step(state, { type: 'connect', nowMs: 12 }).connect?.atMs).toBe(atMs);
    expect(state.phase).toBe('backoff');
    const nowMs = retry(state);
    step(state, { type: 'message', data: hello, nowMs });
    expect(step(state, { type: 'heartbeat', nowMs: nowMs + 10_000 }).send).toHaveLength(1);
    expect(nextHeartbeatAt(state)).toBe(nowMs + 11_000);
  });

  test('the rolling identify budget stops a reconnect loop and exposes a persistable history', () => {
    const state = fresh({ identifyBudget: { max: 2, windowMs: 3_600_000 } });
    open(state, 0, 0);
    step(state, { type: 'message', data: hello, nowMs: 0 });
    step(state, { type: 'close', code: 1006, nowMs: 1 });
    let nowMs = retry(state, 0);
    step(state, { type: 'message', data: hello, nowMs });
    step(state, { type: 'close', code: 1006, nowMs: nowMs + 1 });
    nowMs = retry(state, 0);
    const exhausted = step(state, { type: 'message', data: hello, nowMs });
    expect(exhausted.send).toEqual([]);
    expect(exhausted.close?.code).toBe(4000);
    expect(state.stoppedReason).toBe('identify budget exhausted: 2 in the last hour');
    expect(exhausted.changes).toContainEqual({
      kind: 'status', phase: 'stopped', detail: 'identify budget exhausted: 2 in the last hour',
    });
    const view = snapshot(state);
    expect(view.identifies).toEqual([0, 1001]);
    expect(view.identifiesInWindow).toBe(2);
    expect(JSON.stringify(view)).not.toContain(token);
    view.identifies.length = 0;
    expect(state.identifies).toHaveLength(2);
    expect(step(state, { type: 'retry', nowMs: nowMs + 10_000, jitter: 0 }).connect).toBeUndefined();
  });

  test('persisted identifies count, expire exactly at the boundary, and do not charge resumes', () => {
    const prior = [0, 1, 2, 3];
    const stopped = fresh({ identifies: prior });
    open(stopped, 100);
    expect(step(stopped, { type: 'message', data: hello, nowMs: 100 }).send).toEqual([]);
    expect(stopped.stoppedReason).toBe('identify budget exhausted: 4 in the last hour');
    expect(prior).toEqual([0, 1, 2, 3]);
    const expired = fresh({ identifies: prior });
    open(expired, 3_600_000);
    expect(sent(step(expired, { type: 'message', data: hello, nowMs: 3_600_000 }))[0].op).toBe(2);
    expect(expired.identifies).toEqual([1, 2, 3, 3_600_000]);
    const state = established();
    state.identifyBudget.max = 1;
    step(state, { type: 'close', code: 1006, nowMs: 100 });
    const nowMs = retry(state);
    expect(sent(step(state, { type: 'message', data: hello, nowMs }))[0].op).toBe(6);
    expect(state.identifies).toEqual([0]);
  });

  test('malformed frames reconnect without leaking the frame or losing the resumable session', () => {
    const state = established();
    const result = step(state, { type: 'message', data: '{account-private-data', nowMs: 10 });
    expect(result.close).toEqual({ code: 4000, reason: 'invalid gateway payload' });
    expect(state.sessionId).toBe('fixture-session');
    expect(result.connect).toBeDefined();
    expect(JSON.stringify(result.changes)).not.toContain('account-private-data');
  });
});

describe('gateway zlib-stream', () => {
  test('sync-flushed HELLO, READY, dispatches and ACK are each available before the next input', async () => {
    const payloads = [
      JSON.parse(hello), ready,
      { op: 0, t: 'MESSAGE_CREATE', s: 2, d: { content: 'repeat dictionary words 🦊 日本語 '.repeat(1000) } },
      { op: 0, t: 'MESSAGE_CREATE', s: 3, d: { content: 'repeat dictionary words 🦊 日本語 '.repeat(1000) } },
      { op: 11 },
    ];
    const frames = await compressedFrames(payloads);
    const state = fresh();
    open(state);
    const identified = step(state, { type: 'message', data: frames[0], nowMs: 0 });
    expect(sent(identified)).toEqual([{ op: 2, d: identifyPayload(id, token) }]);
    for (let i = 1; i < 4; i++) {
      const result = step(state, { type: 'message', data: frames[i], nowMs: i });
      expect(dispatches(result)).toEqual([{ kind: 'dispatch', t: payloads[i].t!, s: payloads[i].s!, d: payloads[i].d }]);
    }
    step(state, { type: 'heartbeat', nowMs: 250 });
    step(state, { type: 'message', data: frames[4], nowMs: 251 });
    expect(state.awaitingAck).toBe(false);
    expect(state.seq).toBe(3);
  });

  test('a dispatch split over socket messages waits for the complete flush marker', async () => {
    const payload = { op: 0, t: 'MESSAGE_CREATE', s: 2, d: { content: 'split 🦊 '.repeat(2000) } };
    const frames = await compressedFrames([JSON.parse(hello), ready, payload]);
    const state = fresh();
    open(state);
    step(state, { type: 'message', data: frames[0], nowMs: 0 });
    step(state, { type: 'message', data: frames[1], nowMs: 1 });
    const compressed = frames[2];
    const ends = [1, 3, Math.floor(compressed.length / 2), compressed.length - 3, compressed.length - 1];
    let offset = 0;
    for (const end of ends) {
      const part = compressed.slice(offset, end);
      expect(step(state, { type: 'message', data: part.buffer, nowMs: 2 }).changes).toEqual([]);
      expect(state.seq).toBe(1);
      offset = end;
    }
    const result = step(state, { type: 'message', data: compressed.slice(offset), nowMs: 2 });
    expect(dispatches(result)).toEqual([{ kind: 'dispatch', t: payload.t, s: 2, d: payload.d }]);
  });

  test('one JSON document above 3 MiB inflates and parses completely in the same step', async () => {
    const content = 'large READY dictionary 🦊 日本語|'.repeat(120_000);
    const payload = { ...ready, d: { ...ready.d, guilds: [{ id: '1', description: content }] } };
    const size = new TextEncoder().encode(JSON.stringify(payload)).length;
    expect(size).toBeGreaterThan(3 * 1024 * 1024);
    const frames = await compressedFrames([
      JSON.parse(hello), payload,
      { op: 0, t: 'MESSAGE_CREATE', s: 2, d: { content: content.slice(-1000) } },
    ]);
    const state = fresh();
    open(state);
    step(state, { type: 'message', data: frames[0], nowMs: 0 });
    const start = performance.now();
    const result = step(state, { type: 'message', data: frames[1], nowMs: 1 });
    const elapsed = performance.now() - start;
    console.log(`gateway inflate+parse: ${size} bytes (${frames[1].length} compressed), ${elapsed.toFixed(2)} ms`);
    expect(dispatches(result)[0]?.d.guilds[0].description).toBe(content);
    expect(state.phase).toBe('ready');
    expect(state.seq).toBe(1);
    const after = step(state, { type: 'message', data: frames[2], nowMs: 2 });
    expect(dispatches(after)[0]?.d.content).toBe(content.slice(-1000));
    expect(state.seq).toBe(2);
  });

  test('new connections reset the inflater and discard an unfinished compressed dispatch', async () => {
    const first = await compressedFrames([
      JSON.parse(hello), ready,
      { op: 0, t: 'MESSAGE_CREATE', s: 2, d: { content: 'old dictionary '.repeat(10_000) } },
    ]);
    const second = await compressedFrames([
      JSON.parse(hello), { op: 0, t: 'RESUMED', s: 3, d: {} },
      { op: 0, t: 'MESSAGE_CREATE', s: 4, d: { content: 'new connection' } },
    ]);
    const state = fresh();
    open(state);
    step(state, { type: 'message', data: first[0], nowMs: 0 });
    step(state, { type: 'message', data: first[1], nowMs: 1 });
    expect(step(state, { type: 'message', data: first[2].slice(0, -2), nowMs: 2 }).changes).toEqual([]);
    step(state, { type: 'close', code: 1006, nowMs: 3 });
    const nowMs = retry(state);
    const helloResult = step(state, { type: 'message', data: second[0], nowMs });
    expect(sent(helloResult)).toEqual([{ op: 6, d: { token, session_id: 'fixture-session', seq: 1 } }]);
    step(state, { type: 'message', data: second[1], nowMs: nowMs + 1 });
    const result = step(state, { type: 'message', data: second[2], nowMs: nowMs + 2 });
    expect(dispatches(result)[0]).toEqual({ kind: 'dispatch', t: 'MESSAGE_CREATE', s: 4, d: { content: 'new connection' } });
    expect(state.phase).toBe('ready');
  });
});
