import { describe, expect, test } from 'bun:test';
import { npmReleasePublished } from '../../src/sidebar/npm-release';

describe('npm publication probe', () => {
  for (const version of ['0.0.50', '0.0.50-nightly.20261015.2']) test(`accepts the published ${version} with a plain GET`, async () => {
    const calls: unknown[][] = [];
    const request = async (...args: Parameters<typeof fetch>) => { calls.push(args); return new Response(JSON.stringify({ version })); };
    expect(await npmReleasePublished(version, request as typeof fetch)).toBe(true);
    expect(calls).toEqual([[`https://registry.npmjs.org/t3/${encodeURIComponent(version)}`]]);
  });
  test('rejects missing, mismatched, invalid, failed and unreachable releases', async () => {
    for (const response of [new Response('{}'), new Response('{"version":"0.0.49"}'), new Response('null'), new Response('invalid'),
      new Response('{}', { status: 404 }), new Response('{}', { status: 500 })]) {
      expect(await npmReleasePublished('0.0.50', (async () => response) as typeof fetch)).toBe(false);
    }
    expect(await npmReleasePublished('0.0.50', (async () => { throw new Error('offline'); }) as typeof fetch)).toBe(false);
  });
});
