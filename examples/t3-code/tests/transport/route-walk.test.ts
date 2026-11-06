// The route walk's shared head start (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: packages/client-runtime/src/
// connection/driver.ts connectOverRoutes): every direct check gets 500 ms; a route silent by then waits for a second
// pass, where those that answered late go before those still silent. Time is the test's: each check answers after
// its route's latency, or fails as exact2's fetch does once its deadline passes.
import { describe, expect, test } from 'bun:test';
import { CHECK_HEAD_START_MS, CHECK_TIMEOUT_MS, RouteState, type HttpInit, type HttpReply } from '../../src/transport/transport-routes';
import type { Catalog } from '../../src/transport/transport-catalog';

const flush = () => new Promise(resolve => setTimeout(resolve, 0));

/** Routes by origin with their latency (ms; Infinity for a dead one), and a clock the test moves. */
function walk(latencies: Record<string, number>) {
  let now = 0;
  const waiting: { at: number; settle: () => void }[] = [];
  const checks: string[] = [];
  const http = (url: string, init: HttpInit) => new Promise<HttpReply>((resolve, reject) => {
    const origin = new URL(url).origin, latency = latencies[origin] ?? Infinity;
    checks.push(`${origin} ${init.timeoutMs}`);
    waiting.push({ at: now + Math.min(latency, init.timeoutMs), settle: () => latency <= init.timeoutMs
      ? resolve({ status: 200, text: JSON.stringify({ environmentId: 'env-1' }) })
      : reject(Object.assign(new Error('timed out'), { name: 'FetchError', kind: 'Timeout' })) });
  });
  const routes = Object.keys(latencies).map(origin => ({ id: origin, origin, kind: '', learned: false, credential: origin }));
  const entry = { origin: routes[0]!.origin, environmentId: 'env-1', label: 'Studio', routes };
  const catalog = { entryForOrigin: () => entry, entryForEnvironment: () => entry } as unknown as Catalog;
  const state = new RouteState(http);
  state.load(routes[0]!.origin, catalog);
  state.beginWalk(catalog);
  const advance = async (ms: number) => {
    const end = now + ms;
    for (;;) {
      await flush();
      const due = waiting.filter(item => item.at <= end).sort((left, right) => left.at - right.at)[0];
      if (!due) break;
      now = due.at; waiting.splice(waiting.indexOf(due), 1); due.settle();
    }
    now = end; await flush();
  };
  /** The next route and when it came. */
  const next = async (limit = 5_000) => {
    let found: string | null | undefined;
    void state.next().then(route => { found = route?.origin ?? null; });
    for (let moved = 0; ; moved += 50) {
      await flush();
      if (found !== undefined || moved >= limit) break;
      await advance(50);
    }
    return { origin: found, at: now };
  };
  return { next, advance, checks };
}

const A = 'http://10.0.0.2:3773', B = 'http://100.64.0.2:3773', C = 'http://192.168.1.4:3773';

describe('the head start', () => {
  test('a dead first route costs half a second, not the full check', async () => {
    const { next, checks } = walk({ [A]: Infinity, [B]: 20 });
    expect(await next()).toEqual({ origin: B, at: CHECK_HEAD_START_MS });
    // Each direct route is asked twice at the start: the head start's check and the full one.
    expect(checks).toEqual([`${A} ${CHECK_HEAD_START_MS}`, `${A} ${CHECK_TIMEOUT_MS}`, `${B} ${CHECK_HEAD_START_MS}`, `${B} ${CHECK_TIMEOUT_MS}`]);
  });

  test('a route that answers within the head start goes first, in order', async () => {
    const { next } = walk({ [A]: 100, [B]: 20 });
    expect(await next()).toEqual({ origin: A, at: 100 });
  });

  test('a route reached after the head start counts by its own check once that is in', async () => {
    const { next, advance } = walk({ [A]: 100, [B]: 1_000, [C]: Infinity });
    expect((await next()).origin).toBe(A);
    await advance(1_900); // the attempt over A takes two seconds and fails
    expect(await next()).toEqual({ origin: B, at: 2_000 });
  });

  test('the second pass tries late answerers before routes still silent', async () => {
    const { next } = walk({ [A]: Infinity, [B]: 1_500, [C]: 800 });
    expect(await next()).toEqual({ origin: B, at: CHECK_TIMEOUT_MS });
    expect((await next()).origin).toBe(C);
    expect((await next()).origin).toBe(A);
    expect((await next()).origin).toBeNull();
  });
});
