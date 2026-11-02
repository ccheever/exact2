// The session's grants load apart from the connection (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: packages/
// client-runtime/src/state/session.ts sessionStateAtom): the client asks for them (`readSession`) and goes on
// to the config and the shell; the transport files the reply under `session-state` (its ladder is tested in
// tests/transport/transport-session.test.ts).
import { describe, expect, test } from 'bun:test';
import { T3Client } from '../../src/client/client';
import { obj } from '../../src/client/domain';
import { Backend, storage } from '../support/client-fixture';

/** A host whose session read is still out: `readSession` answers, and the reply comes when the test says; a direct read never answers. */
class SlowSession extends Backend {
  asked = 0;
  override async later(input: unknown): Promise<unknown> {
    const request = obj(input);
    if (request.op === 'readSession') { this.calls.push(request); this.asked += 1; return this.good({ id: `session-${this.asked}` }); }
    if (request.op === 'http' && request.path === '/api/auth/session') return new Promise(() => {});
    return super.later(input);
  }
  answer(value: unknown) { this.emit('session-state', value as Record<string, unknown>); }
}

describe('the session read', () => {
  test('the config and the shell load while the read is out; the grants come in after', async () => {
    const native = new SlowSession(), client = new T3Client(), files = storage().files;
    const outcome = await Promise.race([client.refresh(native, files).then(() => 'loaded'), new Promise(resolve => setTimeout(resolve, 1_000, 'still waiting on the session read'))]);
    expect(outcome).toBe('loaded');
    const ops = native.calls.map(call => String(call.op === 'request' || call.op === 'subscribe' ? call.method : call.op === 'http' ? call.path : call.op));
    expect(ops.indexOf('readSession')).toBeLessThan(ops.indexOf('server.getConfig'));
    expect([client.config.environment !== undefined, client.shellLoaded, client.scopesKnown, client.scopes, client.error]).toEqual([true, true, false, [], '']);
    native.answer({ _reply: { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate', 'settings:write'] } });
    await client.refresh(native, files);
    expect([client.scopesKnown, client.scopes.includes('settings:write'), client.error]).toEqual([true, true, '']);
    expect(native.asked).toBe(1);
  });

  test('a failed read grants nothing and shows no banner, as the reference reads a failure', async () => {
    const native = new SlowSession(), client = new T3Client(), files = storage().files;
    await client.refresh(native, files);
    native.answer({ _replyError: { kind: 'Network', message: 'the request timed out after 60000 ms', reason: 'timeout' } });
    await client.refresh(native, files);
    expect([client.scopesKnown, client.scopes, client.error, client.shellLoaded]).toEqual([false, [], '', true]);
    // SessionPermissions says the check is being retried (the transport asks again on this connection).
    expect(client.sessionCheck).toBe('failed');
  });

  test('the session check reads pending, then failed, rejected or granted', async () => {
    const native = new SlowSession(), client = new T3Client(), files = storage().files;
    await client.refresh(native, files);
    expect(client.sessionCheck).toBe('pending');
    for (const [reply, check] of [[{ _replyError: { kind: 'HTTP', message: 'Internal server error' } }, 'failed'], [{ _reply: { authenticated: false } }, 'rejected'],
      [{ _replyError: { kind: 'HTTP', message: 'Internal server error' } }, 'failed'], [{ _reply: { authenticated: true, scopes: ['orchestration:read'] } }, 'granted']] as const) {
      native.answer(reply);
      await client.refresh(native, files);
      expect(client.sessionCheck).toBe(check);
    }
  });

  test('a session the server no longer accepts asks to pair again, until a read it accepts', async () => {
    const native = new SlowSession(), client = new T3Client(), files = storage().files;
    await client.refresh(native, files);
    native.answer({ _reply: { authenticated: false } });
    await client.refresh(native, files);
    expect([client.error, client.scopesKnown]).toEqual(['Pair with the T3 server again.', false]);
    native.answer({ _reply: { authenticated: true, scopes: ['orchestration:read'] } });
    await client.refresh(native, files);
    expect([client.error, client.scopesKnown]).toEqual(['', true]);
  });

  test('the grants decide the notice of a server that cannot resolve a command\'s context', async () => {
    const native = new SlowSession(), client = new T3Client(), files = storage().files;
    native.config = { ...native.config, environment: { ...obj(native.config.environment), capabilities: {} } };
    await client.refresh(native, files);
    expect(client.error).toBe('');
    native.answer({ _reply: { authenticated: true, scopes: ['orchestration:read', 'orchestration:operate'] } });
    await client.refresh(native, files);
    expect(client.error).toBe('Update the T3 server to send messages from this client. Existing threads remain readable.');
  });
});
