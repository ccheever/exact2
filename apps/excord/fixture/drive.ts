// A test driver for `discord/gateway.ts` over a real socket: it does what the Contract stream
// resource and its `task every` will do in the app (open the URL a step names, hand every socket
// event to `step`, send and close what it says, tick the heartbeat), on Bun's WebSocket and the
// wall clock. Only tests use it.
import { identity, type Host } from '../discord/client.ts';
import {
  heartbeatDue, initialGateway, step, type GatewayChange, type GatewayState, type StepResult,
} from '../discord/gateway.ts';
import { admit, onResponse, type Limiter, type RestRequest } from '../discord/rest.ts';
import { seededRandom } from './world.ts';

export interface Drive {
  state: GatewayState;
  changes: GatewayChange[];
  /** Every frame the client sent, parsed. */
  sent: any[];
  /** Resolves once `test` holds of the changes so far; rejects after `timeoutMs`. */
  until(test: (changes: GatewayChange[]) => boolean, what: string, timeoutMs?: number): Promise<void>;
  dispatches(t?: string): Extract<GatewayChange, { kind: 'dispatch' }>[];
  close(): void;
}

export function drive(opts: {
  gatewayUrl: string; token?: string; host?: Host; seed?: number; heartbeatTickMs?: number;
  identifyBudget?: { max: number; windowMs: number };
  onChange?: (change: GatewayChange) => void;
}): Drive {
  const random = seededRandom(opts.seed ?? 7);
  const state = initialGateway({
    identity: identity(opts.host ?? 'ios', { launchId: 'drive-launch', heartbeatSessionId: 'drive-heartbeat' }),
    token: opts.token ?? 'fixture-token', gatewayUrl: opts.gatewayUrl, identifyBudget: opts.identifyBudget,
  });
  const changes: GatewayChange[] = [];
  const sent: any[] = [];
  const waiters = new Set<() => void>();
  let socket: WebSocket | null = null;
  let stopped = false;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;

  const apply = (result: StepResult) => {
    for (const frame of result.send) {
      sent.push(JSON.parse(frame));
      socket?.send(frame);
    }
    if (result.close && socket) {
      // Events from a socket we closed belong to no connection the session has.
      const old = socket;
      socket = null;
      old.close(result.close.code, result.close.reason);
    }
    for (const change of result.changes) {
      changes.push(change);
      opts.onChange?.(change);
    }
    if (result.connect && state.phase === 'connecting') open(result.connect.url);
    else if (result.connect && state.phase === 'backoff') {
      if (retryTimer) clearTimeout(retryTimer);
      retryTimer = setTimeout(() => {
        retryTimer = null;
        if (!stopped) apply(step(state, { type: 'retry', nowMs: Date.now(), jitter: random() }));
      }, Math.max(0, result.connect.atMs - Date.now()));
    }
    for (const wake of [...waiters]) wake();
  };

  const open = (url: string) => {
    const ws = new WebSocket(url);
    ws.binaryType = 'arraybuffer';
    socket = ws;
    const mine = () => socket === ws && !stopped;
    ws.onopen = () => { if (mine()) apply(step(state, { type: 'open', nowMs: Date.now() })); };
    ws.onmessage = event => {
      if (mine()) apply(step(state, { type: 'message', data: event.data as ArrayBuffer | string, nowMs: Date.now(), jitter: random() }));
    };
    ws.onclose = event => {
      if (!mine()) return;
      socket = null;
      apply(step(state, { type: 'close', code: event.code, reason: event.reason, nowMs: Date.now() }));
    };
  };

  const ticker = setInterval(() => {
    if (!stopped && heartbeatDue(state, Date.now())) apply(step(state, { type: 'heartbeat', nowMs: Date.now() }));
  }, opts.heartbeatTickMs ?? 10);

  apply(step(state, { type: 'connect', nowMs: Date.now(), jitter: random() }));

  const dispatches = (t?: string) => changes.filter((c): c is Extract<GatewayChange, { kind: 'dispatch' }> =>
    c.kind === 'dispatch' && (t === undefined || c.t === t));
  return {
    state, changes, sent, dispatches,
    until(test, what, timeoutMs = 10_000) {
      return new Promise<void>((resolve, reject) => {
        const check = () => {
          if (!test(changes)) return;
          waiters.delete(check); clearTimeout(timer); resolve();
        };
        const timer = setTimeout(() => {
          waiters.delete(check);
          reject(new Error(`timed out waiting for ${what}; phase ${state.phase}, ${changes.length} changes`));
        }, timeoutMs);
        waiters.add(check);
        check();
      });
    },
    close() {
      stopped = true;
      clearInterval(ticker);
      if (retryTimer) clearTimeout(retryTimer);
      socket?.close(4000, 'test over');
      socket = null;
    },
  };
}

/** POST a control call to the fixture and return its JSON. */
export async function control(origin: string, name: string, body: object = {}): Promise<any> {
  const reply = await fetch(`${origin}/control/${name}`, { method: 'POST', body: JSON.stringify(body) });
  if (!reply.ok) throw new Error(`control ${name}: ${reply.status} ${await reply.text()}`);
  return reply.json();
}

export async function fixtureState(origin: string): Promise<any> {
  return (await fetch(`${origin}/control/state`)).json();
}

/** Polls the fixture's state every 50 ms until `test` holds: the server side notifies no one. */
export async function untilFixture(origin: string, test: (state: any) => boolean, what: string, timeoutMs = 20_000): Promise<any> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const state = await fixtureState(origin);
    if (test(state)) return state;
    if (Date.now() > deadline) throw new Error(`timed out waiting for the fixture: ${what}`);
    await Bun.sleep(50);
  }
}

/** One REST call as the app makes it: admitted by the limiter, waiting out what it says, then
 * reported back to it. A wait the limiter cannot bound (an unknown bucket already in flight) throws. */
export async function restCall(limiter: Limiter, request: RestRequest): Promise<{ status: number; body: any; retryAfterMs?: number }> {
  for (;;) {
    const admitted = admit(limiter, request, Date.now());
    if (admitted.ok) break;
    if (!Number.isFinite(admitted.waitMs)) throw new Error(`limiter cannot admit ${request.route}`);
    await Bun.sleep(admitted.waitMs);
  }
  let reply: Response;
  try {
    reply = await fetch(request.url, { method: request.method, headers: request.headers, body: request.body });
  } catch (error) {
    onResponse(limiter, request, 0, () => null, null, Date.now());
    throw error;
  }
  const text = await reply.text();
  const body = text ? JSON.parse(text) : null;
  const limits = onResponse(limiter, request, reply.status, name => reply.headers.get(name), body, Date.now());
  return { status: reply.status, body, ...limits };
}
