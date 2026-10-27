// A slow host's session read waits instead of failing (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: packages/
// client-runtime/src/state/session.ts fetchEnvironmentSessionState): a timeout asks again with double the time,
// from 6 s up to 60 s, so the grants still load and Settings stays writable.
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj } from '../../src/client/domain';
import { Backend, storage } from '../support/client-fixture';

/** `/api/auth/session` answers after `delay` ms; a shorter deadline fails as the transport reports it (reason timeout). */
class SlowSession extends Backend {
  delay = 10_000;
  deadlines: number[] = [];
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'http' && request.path === '/api/auth/session') {
      this.deadlines.push(Number(request.timeoutMs));
      if (Number(request.timeoutMs) < this.delay) return { ok: false, generation: this.generation, error: { kind: 'Network', message: `the request timed out after ${String(request.timeoutMs)} ms`, uncertain: false, reason: 'timeout' } };
    }
    return super.later(input);
  }
}

describe('the session read', () => {
  test('a read 10 s slow completes on its second try (6 s, then 12 s) and keeps the grants', async () => {
    const native = new SlowSession(), client = new T3Client();
    await client.refresh(native, storage().files);
    expect(native.deadlines).toEqual([6_000, 12_000]);
    expect(client.error).toBe('');
    expect(client.scopes).toContain('settings:write');
  });

  test('the ladder stops at 60 s, and a failure other than a timeout is not asked again', async () => {
    const native = new SlowSession(), client = new T3Client();
    native.delay = 120_000;
    await client.refresh(native, storage().files);
    expect(native.deadlines).toEqual([6_000, 12_000, 24_000, 48_000, 60_000]);
    expect(client.error).toContain('timed out after 60000 ms');
    const refused = new SlowSession(), other = new T3Client();
    refused.later = async (input: unknown) => {
      const request = obj(input);
      if (request.op === 'http' && request.path === '/api/auth/session') { refused.deadlines.push(Number(request.timeoutMs)); return refused.bad('The server returned HTTP 500.'); }
      return Backend.prototype.later.call(refused, input);
    };
    await other.refresh(refused, storage().files);
    expect(refused.deadlines).toEqual([6_000]);
  });
});
