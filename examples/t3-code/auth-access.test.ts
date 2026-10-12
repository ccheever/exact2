// 20261005-this-machine-network-access item 4: the access stream's reducer, ported with the
// reference's test names (MIT, see LICENSE-T3; reference 1e2ecbd975: packages/client-runtime/src/
// state/auth.test.ts), then the clone's wiring: the subscription follows the page, events land only
// for this machine, and the six HTTP calls go to the embedded server.
import { afterEach, describe, expect, it } from 'bun:test';
import { EMPTY_AUTH_ACCESS_SNAPSHOT, access, accessEvent, accessFleetPass, applyAuthAccessStreamEvent, createPairingCredential, decodeAuthAccessEvent, revokeClientSession,
  revokeOtherClientSessions, revokePairingLink, sortClientSessions, summarizeAuthorizedClients, watchAccess, AUTH_ACCESS_KEY, type AuthClientSession } from './auth-access';
import { primaryAt, resetPrimary } from './local-primary-fixture';
import type { Obj } from './domain';
import type { Native } from './protocol';

afterEach(() => { resetPrimary(); Object.assign(access, { environmentId: '', snapshot: EMPTY_AUTH_ACCESS_SNAPSHOT, loaded: false, error: '', wanted: false, changed: () => {} }); });

const pairingLink = { id: 'pairing-link', scopes: ['orchestration:read'], subject: 'subject', label: 'Phone', createdAt: '2036-04-07T00:00:00.000Z', expiresAt: '2036-04-07T00:05:00.000Z' };
const clientSession: AuthClientSession = { sessionId: 'session-client', subject: 'subject', scopes: ['orchestration:read'], method: 'browser-session-cookie',
  client: { label: 'Phone', deviceType: 'mobile' }, issuedAt: '2036-04-07T00:00:00.000Z', expiresAt: '2036-05-07T00:00:00.000Z', lastConnectedAt: null, connected: true, current: false };

describe('applyAuthAccessStreamEvent', () => {
  it('accumulates rapid pairing-link and client updates into one snapshot', () => {
    const withPairingLink = applyAuthAccessStreamEvent(EMPTY_AUTH_ACCESS_SNAPSHOT, { version: 1, revision: 1, type: 'pairingLinkUpserted', payload: pairingLink });
    const withClient = applyAuthAccessStreamEvent(withPairingLink, { version: 1, revision: 2, type: 'clientUpserted', payload: clientSession });
    expect(withClient).toEqual({ pairingLinks: [pairingLink], clientSessions: [clientSession] });
  });

  it('applies removals without disturbing unrelated access state', () => {
    const snapshot = applyAuthAccessStreamEvent({ pairingLinks: [pairingLink], clientSessions: [] }, { version: 1, revision: 2, type: 'pairingLinkRemoved', payload: { id: 'pairing-link' } });
    expect(snapshot).toEqual(EMPTY_AUTH_ACCESS_SNAPSHOT);
  });
});

describe('the access stream (subscribeAuthAccess on this machine)', () => {
  const calls: Obj[] = [];
  const call = async (request: Obj): Promise<Obj> => { calls.push(request); return request.op === 'subscribe' ? { id: '3-7' } : {}; };
  afterEach(() => { calls.length = 0; });

  it('subscribes only while wanted and only on this machine, then unsubscribes when the page closes', async () => {
    primaryAt('http://127.0.0.1:16101', 'env-local');
    const client = { environmentId: 'env-local', origin: 'http://127.0.0.1:16101', generation: 3 };
    await watchAccess(client, call);
    expect(calls).toEqual([]);
    access.wanted = true;
    await watchAccess({ ...client, environmentId: 'env-other', origin: 'http://192.168.1.9:3773' }, call);
    expect(calls).toEqual([]);
    await watchAccess(client, call);
    await watchAccess(client, call);
    expect(calls).toEqual([{ op: 'subscribe', key: AUTH_ACCESS_KEY, method: 'subscribeAuthAccess', payload: {} }]);
    access.wanted = false;
    await watchAccess(client, call);
    expect(calls.at(-1)).toEqual({ op: 'unsubscribe', key: AUTH_ACCESS_KEY });
  });

  it('reduces the focused primary’s events, redraws, and ignores other generations and machines', async () => {
    primaryAt('http://127.0.0.1:16101', 'env-local');
    let redraws = 0;
    access.wanted = true; access.changed = () => { redraws++; };
    const client = { environmentId: 'env-local', origin: 'http://127.0.0.1:16101', generation: 3 };
    await watchAccess(client, call);
    const entry = (value: Obj, generation = 3) => ({ key: AUTH_ACCESS_KEY, generation, subscriptionId: '3-7', value });
    expect(accessEvent(client, entry({ version: 1, revision: 1, type: 'snapshot', payload: { pairingLinks: [pairingLink], clientSessions: [] } }))).toBe(true);
    expect(accessEvent(client, entry({ version: 1, revision: 2, type: 'clientUpserted', payload: clientSession as unknown as Obj }))).toBe(true);
    expect(accessEvent(client, entry({ version: 1, revision: 3, type: 'pairingLinkRemoved', payload: { id: 'pairing-link' } }, 2))).toBe(true); // an old generation
    expect(accessEvent(client, { key: 'config', generation: 3, value: {} })).toBe(false);
    expect([access.loaded, access.snapshot.pairingLinks.length, access.snapshot.clientSessions.map(session => session.sessionId), redraws]).toEqual([true, 1, ['session-client'], 2]);
    // A transport failure resets the stream so the next pass subscribes again, and says why.
    accessEvent(client, entry({ _transportError: { message: 'The access stream ended.' } }));
    expect(access.error).toBe('The access stream ended.');
    await watchAccess(client, call);
    expect(calls.filter(request => request.op === 'subscribe')).toHaveLength(2);
  });

  it('follows the primary on its fleet transport', async () => {
    primaryAt('http://127.0.0.1:16101', 'env-local');
    access.wanted = true;
    await accessFleetPass(call, { primary: true, environmentId: 'env-local', generation: 1 } as never);
    await accessFleetPass(call, { primary: false, environmentId: 'env-remote', generation: 1 } as never);
    expect(calls.filter(request => request.op === 'subscribe')).toHaveLength(1);
  });

  it('decodes the wire events and drops malformed ones', () => {
    expect(decodeAuthAccessEvent({ version: 1, revision: 4, type: 'clientRemoved', payload: { sessionId: 's' } })).toEqual({ version: 1, revision: 4, type: 'clientRemoved', payload: { sessionId: 's' } });
    expect(decodeAuthAccessEvent({ type: 'pairingLinkUpserted', payload: { scopes: [] } })).toBeNull();
    expect(decodeAuthAccessEvent({ type: 'unknown' })).toBeNull();
  });
});

describe('Authorized clients helpers (ConnectionsSettings.tsx)', () => {
  it('lists this device first, then connected ones, newest first, and summarizes the fold', () => {
    const older = { ...clientSession, sessionId: 'older', connected: false, issuedAt: '2036-04-01T00:00:00.000Z' };
    const newer = { ...clientSession, sessionId: 'newer', connected: false, issuedAt: '2036-04-09T00:00:00.000Z' };
    const self = { ...clientSession, sessionId: 'self', current: true, connected: false };
    expect(sortClientSessions([older, clientSession, newer, self]).map(session => session.sessionId)).toEqual(['self', 'session-client', 'newer', 'older']);
    expect(summarizeAuthorizedClients([self], [])).toBe('1 client');
    expect(summarizeAuthorizedClients([self, older], [pairingLink])).toBe('2 clients · 1 pairing link');
    expect(summarizeAuthorizedClients([], [pairingLink, pairingLink])).toBe('0 clients · 2 pairing links');
  });
});

describe('the six /api/auth calls (EnvironmentAuthHttpApi through the embedded server)', () => {
  function native(answer: (request: Obj) => Obj): { native: Native; requests: Obj[] } {
    const requests: Obj[] = [];
    return { requests, native: { available: true, watch: () => {}, later: async request => { requests.push(request as Obj); return answer(request as Obj); } } };
  }
  it('sends the reference payloads and returns the credential once', async () => {
    const { native: bridge, requests } = native(request => ({ ok: true, generation: 0, value: request.path === '/api/auth/pairing-token'
      ? { id: 'link-1', credential: 'SECRET', expiresAt: '2036-04-07T00:05:00.000Z' } : request.path === '/api/auth/clients/revoke-others' ? { revokedCount: 2 } : { revoked: true } }));
    expect(await createPairingCredential(bridge, { label: '  Living room iPad ', scopes: ['orchestration:read'] })).toEqual({ id: 'link-1', credential: 'SECRET', expiresAt: '2036-04-07T00:05:00.000Z' });
    await createPairingCredential(bridge, { label: '   ', scopes: ['orchestration:read'] });
    await revokePairingLink(bridge, 'link-1');
    await revokeClientSession(bridge, 'session-1');
    expect(await revokeOtherClientSessions(bridge)).toBe(2);
    expect(requests).toEqual([
      { op: 'localAccess', method: 'POST', path: '/api/auth/pairing-token', body: { label: 'Living room iPad', scopes: ['orchestration:read'] } },
      { op: 'localAccess', method: 'POST', path: '/api/auth/pairing-token', body: { scopes: ['orchestration:read'] } },
      { op: 'localAccess', method: 'POST', path: '/api/auth/pairing-links/revoke', body: { id: 'link-1' } },
      { op: 'localAccess', method: 'POST', path: '/api/auth/clients/revoke', body: { sessionId: 'session-1' } },
      { op: 'localAccess', method: 'POST', path: '/api/auth/clients/revoke-others' },
    ]);
  });
  it('names the operation and status of a failure, never the server text', async () => {
    const { native: bridge } = native(() => ({ ok: false, generation: 0, error: { kind: 'Http', message: 'raw server text', detail: '403' } }));
    const error = await revokePairingLink(bridge, 'x').then(() => null, caught => caught as Error);
    expect(error?.message).toBe('Primary environment request failed during revoke-pairing-link (HTTP 403).');
    const unanswered = native(() => ({ ok: false, generation: 0, error: { kind: 'Http', message: 'offline' } }));
    expect((await createPairingCredential(unanswered.native, { scopes: ['orchestration:read'] }).then(() => null, caught => caught as Error))?.message)
      .toBe('Primary environment request failed during create-pairing-credential (HTTP 500).');
  });
});
