// The Effect RPC wire, addresses, the token exchange form and the retry policy (transport-wire.ts).
import { describe, test, expect } from 'bun:test';
import { decode, encode, failure, origin, path, credential, exchangeForm, exchangeScopes, failureMessage, compatibility, canSelfUpdate,
  retryDelayMs, streamRetryMs, retriesStream, request, TransportFailure, pullRequestUnavailable, random, randomBytes, refillRandom, uuid } from '../../src/transport/transport-wire';
import { Inbox } from '../../src/transport/transport-inbox';
import { stableJSON, pullRequestDiffBody, socketURL } from '../../src/transport/transport-session';

describe('frames', () => {
  test('a message is one frame or a batch, each tagged', () => {
    expect(decode('{"_tag":"Pong"}')).toEqual([{ _tag: 'Pong' }]);
    expect(decode('[{"_tag":"Ack","requestId":"1"},{"_tag":"Pong"}]')).toHaveLength(2);
    expect(() => decode('{"x":1}')).toThrow('untagged');
    expect(() => decode('nope')).toThrow('invalid RPC frame');
  });
  test('a request carries the orchestration protocol header', () => {
    expect(request('1-1', 'server.getConfig', {})).toEqual({ _tag: 'Request', id: '1-1', tag: 'server.getConfig', payload: {}, headers: [['x-t3-orchestration-protocol', '2']] });
    expect(() => encode('x'.repeat(16 * 1024 * 1024 + 1))).toThrow('too large');
  });
});

describe('Exit failures as Cause.squash reports them', () => {
  test('the first typed Fail wins over a Die, and keeps its tag, reason and detail', () => {
    const squashed = failure({ _tag: 'Failure', cause: [{ _tag: 'Die', defect: 'boom' }, { _tag: 'Fail', error: { _tag: 'PullRequestOperationError', operation: 'getDetail', detail: 'nope', reason: 'not-found' } }] });
    expect([squashed.kind, squashed.message, squashed.reason, squashed.detail]).toEqual(['PullRequestOperationError', 'Pull request operation getDetail failed: nope', 'not-found', 'nope']);
  });
  test('an unavailable pull request host reads its stable sentence; an untagged error its message', () => {
    expect(failure({ _tag: 'Fail', error: { _tag: 'PullRequestUnavailableError', reason: 'cli-missing', provider: 'gitlab' } }).message).toBe(pullRequestUnavailable('cli-missing', 'gitlab'));
    expect(failure({ name: 'Error', message: 'plain' }).message).toBe('plain');
    expect(failure({ _tag: 'Fail', error: { _tag: 'DeviceEasAccountError', reason: 'Expo rejected the credential (HTTP 401).' } }).message).toBe('Expo rejected the credential (HTTP 401).');
    expect(failure({ _tag: 'Interrupt' }).kind).toBe('Interrupt');
    expect(failure({ _tag: 'Fail', error: { _tag: 'Mystery' } }).message).toBe('The server refused the request (Mystery).');
  });
});

test('secret request errors use stable reason text without exposing their cause', () => {
  expect(failure({ _tag: 'Fail', error: { _tag: 'SecretRequestError', reason: 'store_failed', cause: { message: 'private-value' } } }).message).toBe('Could not store the secret.');
  expect(failure({ _tag: 'SecretRequestError', reason: 'unknown', message: 'private-value', detail: 'private-value', cause: 'private-value' }).message).toBe('Could not answer the request. Try again.');
});

describe('addresses', () => {
  test('an origin is http(s), lowercased, without a path', () => {
    expect(origin('HTTP://Example.COM:3773/path?x#y')).toBe('http://example.com:3773');
    expect(() => origin('ftp://x')).toThrow('http://');
    expect(() => origin('http://user:pw@x')).toThrow();
  });
  test('a path stays on the connected origin', () => {
    expect(path('/api/auth/session', 'http://127.0.0.1:4001')).toBe('http://127.0.0.1:4001/api/auth/session');
    expect(() => path('//evil.test/x', 'http://127.0.0.1:4001')).toThrow('stay on the connected server');
  });
  test('a pairing link gives its token only for its own origin', () => {
    expect(credential('abc', 'http://h:1')).toBe('abc');
    expect(credential('http://h:1/pair#token=xyz', 'http://h:1')).toBe('xyz');
    expect(credential('http://h:1/pair?token=q', 'http://h:1')).toBe('q');
    expect(() => credential('http://other:1/pair#token=xyz', 'http://h:1')).toThrow('pairing link from this server');
    expect(() => credential('http://h:1/pair', 'http://h:1')).toThrow('no token');
  });
  test('the socket URL carries the ticket, the protocol gate and this client', () => {
    const url = new URL(socketURL('https://h.test:8443', 't', true));
    expect([url.protocol, url.pathname, url.searchParams.get('wsTicket'), url.searchParams.get('orchestrationProtocol'), url.searchParams.get('clientOs')]).toEqual(['wss:', '/ws', 't', '2', 'macos']);
    expect(new URL(socketURL('http://h:1', 't', false)).searchParams.has('orchestrationProtocol')).toBe(false);
  });
});

describe('the token exchange (authorization/remote.ts)', () => {
  test('sends the scope only when named, and a literal plus stays a plus', () => {
    const form = new URLSearchParams(exchangeForm('a+b', 'orchestration:read'));
    expect([form.get('subject_token'), form.get('scope'), form.get('client_label'), form.get('client_os')]).toEqual(['a+b', 'orchestration:read', 'Exact T3 for Mac', 'macos']);
    expect(new URLSearchParams(exchangeForm('c')).has('scope')).toBe(false);
  });
  test('a refusal reads the server, then mapRemoteEnvironmentError, then the status', () => {
    expect(failureMessage({ message: 'Nope' }, 401)).toBe('Nope');
    expect(failureMessage({ reason: 'invalid_credential' }, 401)).toBe('The environment credential is invalid.');
    expect(failureMessage({ reason: 'scope_not_granted' }, 403)).toBe('The environment rejected the authentication request.');
    expect(failureMessage({ _tag: 'PullRequestUnavailableError', reason: 'provider-unsupported' }, 503)).toBe("Change requests cannot be browsed for this project's host yet.");
    expect(failureMessage(undefined, 500)).toBe('The server returned HTTP 500.');
  });
});

describe('compatibility (connection/compatibility.ts)', () => {
  test('names the direction; an older host that updates itself can be updated from here', () => {
    expect(compatibility({ orchestrationProtocolVersion: 2 })).toBeNull();
    expect(compatibility({ orchestrationProtocolVersion: 3, label: 'L' })!.message).toContain('not supported by this server');
    expect(compatibility({ label: 'L', capabilities: { serverSelfUpdate: 'respawn' } })).toEqual({ message: 'This client requires a newer server. Update T3 Code on L to connect.', serverUpdateRequired: true });
    expect(canSelfUpdate({ capabilities: { serverSelfUpdate: 'desktop-managed' } })).toBe(false);
    expect(canSelfUpdate({ capabilities: { serverSelfUpdate: 'desktop-managed', desktopAppUpdate: true } })).toBe(true);
  });
});

describe('the retry policy (supervisor.ts retryDelayMs, rpc/client.ts)', () => {
  test('the ladder: a point in the upper half of min(300 s, 1 s·2^(n+1))', () => {
    expect([retryDelayMs(0, 0), retryDelayMs(0, 0.999)]).toEqual([1000, 1999]);
    expect([retryDelayMs(1, 0), retryDelayMs(2, 0.5)]).toEqual([2000, 6000]);
    expect(retryDelayMs(20, 1)).toBe(300_000);
  });
  test('a stream resubscribes after 250 ms doubling to 30 s; an authorization failure waits', () => {
    expect([streamRetryMs(0), streamRetryMs(1), streamRetryMs(10)]).toEqual([250, 500, 30_000]);
    expect([retriesStream('Network'), retriesStream('EnvironmentAuthorizationError')]).toEqual([true, false]);
  });
});

describe('the inbox', () => {
  test('reading keeps entries until acknowledged; one notification until then', () => {
    const inbox = new Inbox();
    expect(inbox.append(1, 'shell', 's', { a: 1 })).toBe(true);
    expect(inbox.append(1, 'shell', 's', { a: 2 })).toBe(false);
    expect(inbox.read(0).events.map(event => event.seq)).toEqual([1, 2]);
    expect(inbox.read(0).events.length).toBe(2);
    inbox.acknowledge(2);
    expect(inbox.read(2)).toEqual({ events: [], latest: 2, reset: false });
    expect(inbox.append(1, 'shell', 's', { a: 3 })).toBe(true);
  });
  test('overflow resets explicitly, so the reader asks for snapshots', () => {
    const inbox = new Inbox(2);
    for (let index = 0; index < 3; index++) inbox.append(1, 'k', '', index);
    expect(inbox.read(1).reset).toBe(true);
    expect(inbox.read(2).events.map(event => event.value)).toEqual([2]);
  });
});

describe('randomness drawn ahead', () => {
  test('ids and jitter come from the pool, which an answer tops up', () => {
    refillRandom();
    const ids = Array.from({ length: 64 }, () => uuid());
    expect(new Set(ids).size).toBe(64);
    for (const id of ids) expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    const value = random();
    expect(value >= 0 && value < 1).toBe(true);
    expect(randomBytes(3).length).toBe(3);
  });
});

describe('request bodies', () => {
  test('a shared read is keyed by its method and its payload with keys sorted', () => {
    expect(stableJSON({ b: 1, a: { d: [1, { f: 2, e: 1 }], c: 0 } })).toBe('{"a":{"c":0,"d":[1,{"e":1,"f":2}]},"b":1}');
  });
  test('the pull request diff takes only its fields', () => {
    expect(pullRequestDiffBody({ number: 4, repository: 'o/r', projectId: 'p' })).toBe('{"number":4,"projectId":"p","repository":"o/r"}');
    expect(() => pullRequestDiffBody({ number: 4, repository: 'o/r', projectId: 'p', path: '/etc' })).toThrow(TransportFailure);
    expect(() => pullRequestDiffBody({ number: 0, repository: 'o/r', projectId: 'p' })).toThrow('prDiff takes');
  });
});

test('pairing retries only an invalid_scope refusal with the legacy grant', async () => {
  const attempts: string[] = [];
  const exchange = async (scope: string) => { attempts.push(scope); if (scope === 'new') throw new TransportFailure('HTTP', 'Unknown scope', false, 'invalid_scope'); return 'bearer'; };
  expect(await exchangeScopes('new', 'old', exchange)).toBe('bearer');
  expect(attempts).toEqual(['new', 'old']);
  attempts.length = 0;
  await expect(exchangeScopes('new', 'old', async scope => { attempts.push(scope); throw new TransportFailure('Authentication', 'Spent', false, 'invalid_credential'); })).rejects.toThrow('Spent');
  expect(attempts).toEqual(['new']);
});
