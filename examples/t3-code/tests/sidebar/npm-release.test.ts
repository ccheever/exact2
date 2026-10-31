import { describe, expect, test } from 'bun:test';
import { cachedReleasePublished, npmReleaseAnswer, npmReleaseKey, npmReleasePublished } from '../../src/sidebar/npm-release';
import type { Native } from '../../src/client/protocol';

// An answer waits on its fetch; once the runner lets that answer go, the native fetch rejects as Aborted.
const aborted = () => Object.assign(new Error('the answer was let go before this reply; the request may already have been sent'), { name: 'FetchError', kind: 'Aborted' });

describe('npm publication probe', () => {
  for (const version of ['0.0.50', '0.0.50-nightly.20261015.2']) test(`accepts the published ${version} with a plain GET`, async () => {
    const calls: unknown[][] = [];
    const request = async (...args: Parameters<typeof fetch>) => { calls.push(args); return new Response(JSON.stringify({ version })); };
    expect(await npmReleasePublished(version, request as typeof fetch)).toBe(true);
    expect(calls).toEqual([[`https://registry.npmjs.org/t3/${encodeURIComponent(version)}`, { exactIndependentHttp: { maxResponseBytes: 1 << 20 } }]]);
  });
  test('rejects missing, mismatched, invalid, failed and unreachable releases', async () => {
    for (const response of [new Response('{}'), new Response('{"version":"0.0.49"}'), new Response('null'), new Response('invalid'),
      new Response('{}', { status: 404 }), new Response('{}', { status: 500 })]) {
      expect(await npmReleasePublished('0.0.50', (async () => response) as typeof fetch)).toBe(false);
    }
    expect(await npmReleasePublished('0.0.50', (async () => { throw new Error('offline'); }) as typeof fetch)).toBe(false);
  });
  test('a let-go answer is no verdict', async () => {
    expect(await npmReleasePublished('0.1.5', (async () => { throw aborted(); }) as typeof fetch)).toBeNull();
  });
});

describe('the npmRelease resource', () => {
  const native = (asked: unknown[]): Native => ({ available: true, watch() {}, later: async request => { asked.push(request); return {}; } });
  test('asks npm only for a key, once per five-minute window', () => {
    expect(npmReleaseKey(null, 1_000)).toBe('');
    expect(npmReleaseKey('0.1.5', 299_999)).toBe('0.1.5|0');
    expect(npmReleaseKey('0.1.5', 300_000)).toBe('0.1.5|1');
  });
  test('no key asks nothing', async () => {
    let calls = 0;
    expect(await npmReleaseAnswer('', null, (async () => { calls++; return new Response('{}'); }) as typeof fetch)).toBe(false);
    expect(calls).toBe(0);
  });
  test('a published release is cached, and the window asked to read it', async () => {
    const asked: unknown[] = [];
    expect(cachedReleasePublished('0.1.7')).toBe(false);
    expect(await npmReleaseAnswer('0.1.7|5', native(asked), (async () => new Response('{"version":"0.1.7"}')) as typeof fetch)).toBe(true);
    expect([cachedReleasePublished('0.1.7'), asked]).toEqual([true, [{ op: 'sidebarNotify' }]]);
    // The next window's answer agrees: nothing to read again.
    expect(await npmReleaseAnswer('0.1.7|6', native(asked), (async () => new Response('{"version":"0.1.7"}')) as typeof fetch)).toBe(true);
    expect(asked.length).toBe(1);
    // A key that comes back is not asked again.
    let calls = 0;
    expect(await npmReleaseAnswer('0.1.7|6', native(asked), (async () => { calls++; return new Response('{}', { status: 404 }); }) as typeof fetch)).toBe(true);
    expect(calls).toBe(0);
  });
  test('a let-go answer keeps what npm said last', async () => {
    const asked: unknown[] = [];
    expect(await npmReleaseAnswer('0.1.8|5', native(asked), (async () => new Response('{"version":"0.1.8"}')) as typeof fetch)).toBe(true);
    expect(await npmReleaseAnswer('0.1.8|6', native(asked), (async () => { throw aborted(); }) as typeof fetch)).toBe(true);
    expect(cachedReleasePublished('0.1.8')).toBe(true);
    expect(await npmReleaseAnswer('0.1.9|5', native(asked), (async () => { throw aborted(); }) as typeof fetch)).toBe(false);
    expect(cachedReleasePublished('0.1.9')).toBe(false);
  });
});
