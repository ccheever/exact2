// @exact/oauth against the fixture's provider profiles, in Bun (LLP 1146
// D6): the protocol alone, with `openAuthSession` standing in for the
// browser (it follows the authorization URL to its redirect). What Exact adds
// (a let-go answer, the store asking readers again) is tested by the cases
// in an app (`cases.template.contract`).
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, test } from 'bun:test';
import { handle, PROFILES } from './fixture.ts';
import { oauth, OAuthError, type OAuthAuthorization, type OAuthConfig, type OAuthEvent, type OAuthStore } from '../index.ts';

let server: ReturnType<typeof Bun.serve>;
let origin = '';
const CALLBACK = 'com.example.app:/oauth';
let tamper: ((url: string) => string) | null = null;
let cancel = false;

let second: ReturnType<typeof Bun.serve>;
beforeAll(() => {
  // Two adjacent ports: Google's token endpoint is on the second (GOOG-1). A
  // pair with a port in use is skipped; any other failure to listen ends setup.
  const listen = (port: number): ReturnType<typeof Bun.serve> | null => {
    try { return Bun.serve({ port, hostname: '127.0.0.1', idleTimeout: 0, fetch: handle }); } catch (e) {
      if ((e as { code?: string }).code === 'EADDRINUSE') return null;
      throw e;
    }
  };
  for (let p = 47100; ; p += 2) {
    if (p > 48998) throw new Error('No two adjacent ports are free from 47100 to 48999: stop what holds them, then run the tests again.');
    const first = listen(p);
    const next = first && listen(p + 1);
    if (first && next) { server = first; second = next; break; }
    first?.stop(true);
  }
  origin = `http://127.0.0.1:${server.port}`;
  const g = globalThis as Record<string, unknown>;
  g.authCallback = () => CALLBACK;
  g.openAuthSession = async (url: string, o: { callback: string; state: string }) => {
    if (cancel) throw Object.assign(new Error('cancelled'), { status: 499 });
    const r = await fetch(url, { redirect: 'manual' });
    const location = r.headers.get('location');
    if (r.status !== 302 || !location) throw Object.assign(new Error(`authorize answered ${r.status}`), { status: 502 });
    expect(location.startsWith(o.callback)).toBe(true);
    return tamper ? tamper(location) : location;
  };
});
afterAll(() => { server.stop(true); second.stop(true); });

// The fixture's PORT is fixed when it runs alone; here it is the test's port.
async function control(body: Record<string, unknown>): Promise<void> {
  const r = await fetch(`${origin}/control`, { method: 'POST', body: JSON.stringify(body) });
  expect(r.ok).toBe(true);
}
async function counts(profile: string): Promise<Record<string, number>> {
  return ((await (await fetch(`${origin}/control/counts`)).json()) as Record<string, Record<string, number>>)[profile] ?? {};
}

function memory(): OAuthStore & { map: Map<string, string> } {
  const map = new Map<string, string>();
  return { map, get: (k) => map.get(k) ?? null, set: (k, v) => void map.set(k, v), forget: (k) => void map.delete(k) };
}

function config(profile: string, extra: Partial<OAuthConfig> = {}): OAuthConfig {
  const b = `${origin}/p/${profile}`;
  return { issuer: b, clientId: `client-${profile}`, scopes: ['openid', 'profile', 'offline_access'], key: `${profile}.session`, apis: [origin], ...extra };
}

// Each profile configured as its provider's documentation says (PROVIDERS.md).
function configFor(profile: string): OAuthConfig {
  const b = `${origin}/p/${profile}`;
  switch (profile) {
    case 'google': return config(profile, { scopes: ['openid', 'profile', 'email'], authorizationParams: { access_type: 'offline', prompt: 'consent' }, clientSecret: 'fixture-secret', issuerAliases: ['accounts.google.com'] });
    case 'entra': case 'entra-v1': return config(profile, { issuer: `${b}/common/v2.0`, issuerTemplate: true });
    case 'auth0': return config(profile, { issuer: `${b}/` });
    case 'onelogin': return config(profile, { issuer: `${b}/oidc/2`, scopes: ['openid', 'profile'], endedErrors: [{ error: 'invalid_request', status: 401 }] });
    case 'okta': return config(profile, { retryLostRefresh: true });
    case 'github': return { clientId: 'client-github', clientSecret: 'fixture-secret', scopes: ['read:user'], key: 'github.session', apis: [origin], endpoints: { authorization: `${b}/authorize`, token: `${b}/token` }, endedErrors: ['bad_refresh_token'], commaScopes: true };
    case 'salesforce': return config(profile, { scopes: ['openid', 'api', 'refresh_token'] });
    case 'forgerock': return config(profile, { expiredStatus: [401, 403] });
    default: return config(profile);
  }
}

beforeEach(async () => {
  tamper = null;
  cancel = false;
  await control({ action: 'reset' });
});

describe.each(Object.keys(PROFILES))('%s', (profile) => {
  test('signs in, reads, refreshes on 401, signs out', async () => {
    const p = oauth(configFor(profile)), store = memory();
    expect((await p.session(store)).signedIn).toBe(false);
    const r = await p.signIn(store);
    expect(r).toEqual({ ok: true, code: '', error: '', retryable: false, status: 0 });
    const s = await p.session(store);
    expect(s.signedIn).toBe(true);
    const me = await (await p.fetch(store, `${origin}/p/${profile}/api/me`)).json();
    expect(me.name).toBe('Ada Lovelace');
    await control({ action: 'expire-access', profile });
    const again = await p.fetch(store, `${origin}/p/${profile}/api/me`);
    expect(again.status).toBe(200);
    expect((await counts(profile)).refresh).toBe(1);
    // A second refresh works with whatever the first left (rotated, same or omitted).
    await control({ action: 'expire-access', profile });
    expect((await p.fetch(store, `${origin}/p/${profile}/api/me`)).status).toBe(200);
    expect((await counts(profile)).refresh).toBe(2);
    expect((await p.signOut(store)).ok).toBe(true);
    expect((await p.session(store)).signedIn).toBe(false);
    // Sign-out revokes the token kept now (RFC 7009), where discovery names a revocation endpoint.
    const after = await counts(profile);
    expect([after['revoke-current'] ?? 0, after['revoke-stale'] ?? 0, after['revoke-unknown'] ?? 0]).toEqual([PROFILES[profile].discovery ? 1 : 0, 0, 0]);
  });
});

// A native host hardens the realm, freezing `Error.prototype`, before the
// module loads; an error the package throws is still its own.
test('an OAuthError is made where Error.prototype is frozen', () => {
  const index = new URL('../index.ts', import.meta.url).pathname;
  const made = Bun.spawnSync([process.execPath, '-e', `Object.freeze(Error.prototype);
    const { OAuthError } = await import(${JSON.stringify(index)});
    const e = new OAuthError('network', 'no reply', 0, true);
    console.log(e.name, e.code, e.message, e instanceof Error);`]);
  expect({ out: made.stdout.toString().trim(), err: made.stderr.toString() }).toEqual({ out: 'OAuthError network no reply true', err: '' });
});

describe('one session, several callers', () => {
  // Two providers made for one key renew it once and keep the session: both
  // reads are refused while the one refresh is held at the server, so the
  // second waits for it. With reuse detection, a second refresh of the spent
  // token would end the session.
  test('two providers for one key renew it once', async () => {
    const a = oauth(configFor('auth0')), b = oauth(configFor('auth0')), store = memory();
    await a.signIn(store);
    await control({ action: 'expire-access', profile: 'auth0' });
    await control({ action: 'hold-token', profile: 'auth0' });
    let refused = 0, bothRefused = (): void => {};
    const both = new Promise<void>((resolve) => { bothRefused = resolve; });
    const real = globalThis.fetch;
    globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const reply = await real(input, init);
      if (String(input).endsWith('/api/me') && reply.status === 401 && ++refused === 2) bothRefused();
      return reply;
    }) as typeof fetch;
    try {
      const reads = Promise.all([a, b].map((p) => p.fetch(store, `${origin}/p/auth0/api/me`)));
      await both;
      // A task later, each caller has done what it does on its 401 short of I/O: the second decided to wait or to refresh.
      await new Promise((resolve) => setImmediate(resolve));
      await control({ action: 'await', profile: 'auth0', held: 1 });
      await control({ action: 'release-token' });
      expect((await reads).map((r) => r.status)).toEqual([200, 200]);
    } finally {
      globalThis.fetch = real;
    }
    const got = await counts('auth0');
    expect([got.refresh, got.reuse ?? 0]).toEqual([1, 0]);
  });

  // A sign-out whose revoke got no reply (natively, one superseded, whose
  // request is refused) leaves what it forgot for the next sign-out.
  test('a sign-out revokes what an earlier one forgot and could not revoke', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const real = globalThis.fetch;
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) =>
      String(input).endsWith('/revoke') ? Promise.reject(new TypeError('no reply')) : real(input, init)) as typeof fetch;
    try { expect((await p.signOut(store)).ok).toBe(true); } finally { globalThis.fetch = real; }
    expect((await counts('spec'))['revoke-current'] ?? 0).toBe(0);
    expect((await p.signOut(store)).ok).toBe(true);
    const got = await counts('spec');
    expect([got['revoke-current'] ?? 0, got['revoke-stale'] ?? 0]).toEqual([1, 0]);
  });

  test('a sign-out revokes every session earlier ones forgot and could not revoke', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const real = globalThis.fetch;
    globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) =>
      String(input).endsWith('/revoke') ? Promise.reject(new TypeError('no reply')) : real(input, init)) as typeof fetch;
    try { expect((await p.signOut(store)).ok).toBe(true); } finally { globalThis.fetch = real; }
    await p.signIn(store);
    expect((await p.signOut(store)).ok).toBe(true);
    const got = await counts('spec');
    expect([got['revoke-current'] ?? 0, got['revoke-stale'] ?? 0]).toEqual([2, 0]);
  });
});

describe('spec edge cases', () => {
  test('a cancelled and a declined sign-in leave the person signed out', async () => {
    const p = oauth(configFor('spec')), store = memory();
    cancel = true;
    expect((await p.signIn(store)).code).toBe('cancelled');
    cancel = false;
    await control({ action: 'deny-authorize', profile: 'spec' });
    const r = await p.signIn(store);
    expect(r.code).toBe('access_denied');
    expect(r.ok).toBe(false);
    expect((await p.session(store)).signedIn).toBe(false);
  });

  test('a sign-in that cannot be prepared says why', async () => {
    const g = globalThis as Record<string, unknown>, callback = g.authCallback;
    g.authCallback = (): string => { throw new Error('authCallback(): the grants name no auth.callback'); };
    try {
      expect(await oauth(configFor('spec')).session(memory())).toMatchObject({ error: 'Sign-in cannot be prepared: authCallback(): the grants name no auth.callback', ended: false });
    } finally {
      g.authCallback = callback;
    }
  });

  test('a callback for another sign-in, a duplicated state and a wrong iss are refused', async () => {
    const p = oauth(configFor('spec')), store = memory();
    tamper = (u) => u.replace(/state=[^&]+/, 'state=forged');
    expect((await p.signIn(store)).code).toBe('state_mismatch');
    tamper = (u) => `${u}&state=${/state=([^&]+)/.exec(u)![1]}`;
    expect((await p.signIn(store)).code).toBe('state_mismatch');
    tamper = (u) => u.replace(/iss=[^&]+/, 'iss=' + encodeURIComponent('https://attacker.example'));
    expect((await p.signIn(store)).code).toBe('issuer_mismatch');
    tamper = (u) => u.replace(/&iss=[^&]+/, '');
    expect((await p.signIn(store)).code).toBe('issuer_mismatch');
    expect((await counts('spec')).exchange ?? 0).toBe(0);
  });

  test('two callers that need a refresh at once send one refresh', async () => {
    const p = oauth(configFor('auth0')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'auth0' });
    await control({ action: 'hold-token', profile: 'auth0' });
    let refused = 0, bothRefused = (): void => {};
    const seen = new Promise<void>((resolve) => { bothRefused = resolve; });
    const both = await withFetch((url, init, real) => (!url.endsWith('/api/me') ? undefined : real(url, init).then((reply) => {
      if (reply.status === 401 && ++refused === 2) bothRefused();
      return reply;
    })), async () => {
      const reads = Promise.all([p.fetch(store, `${origin}/p/auth0/api/me`), p.fetch(store, `${origin}/p/auth0/api/me`)]);
      await seen;
      // A task later, the second caller has done what it does on its 401 short of I/O: it waits behind the held refresh.
      await new Promise((resolve) => setImmediate(resolve));
      await control({ action: 'await', profile: 'auth0', held: 1 });
      await control({ action: 'release-token' });
      return reads;
    });
    expect(both.map((r) => r.status)).toEqual([200, 200]);
    expect((await counts('auth0')).refresh).toBe(1);
    expect((await counts('auth0')).reuse ?? 0).toBe(0);
  });

  test('503, 429 and lost replies on refresh keep the session', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    for (const action of [{ action: 'fail-token', status: 503 }, { action: 'fail-token', status: 429 }, { action: 'drop-token' }]) {
      await control({ action: 'expire-access', profile: 'spec' });
      await control({ ...action, profile: 'spec' });
      const e = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
      expect([action.action, e.retryable, e.signedOut]).toEqual([action.action, true, false]);
      expect((await p.session(store)).signedIn).toBe(true);
    }
    // The next try, with the token kept, succeeds.
    expect((await p.fetch(store, `${origin}/p/spec/api/me`)).status).toBe(200);
  });

  test('invalid_grant on refresh signs out with a reason', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    await control({ action: 'revoke', profile: 'spec' });
    const e = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
    expect(e.code).toBe('invalid_grant');
    expect(e.signedOut).toBe(true);
    const s = await p.session(store);
    expect(s.signedIn).toBe(false);
    expect(s.error).not.toBe('');
  });

  test('errors under HTTP 200 fail the exchange (GitHub)', async () => {
    const p = oauth(configFor('github')), store = memory();
    tamper = (u) => u.replace(/code=[^&]+/, 'code=bogus');
    const r = await p.signIn(store);
    expect(r.code).toBe('invalid_grant');
    expect((await p.session(store)).signedIn).toBe(false);
  });

  test('a discovery document naming another issuer is refused', async () => {
    const p = oauth(config('entra', { issuer: `${origin}/p/entra/common/v2.0` })), store = memory();
    await p.session(store);
    const r = await p.signIn(store);
    expect(r.code).toBe('config');
    const trailing = oauth(config('auth0', { issuer: `${origin}/p/auth0` }));
    expect((await trailing.signIn(memory())).code).toBe('config');
  });

  test('sign-out forgets locally when revocation fails, and a 503 leaves the token for the next one', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    await control({ action: 'fail-token', profile: 'spec', status: 503 });
    expect((await p.signOut(store)).ok).toBe(true);
    expect(store.map.size).toBe(0);
    // After a 503 the token still exists (RFC 7009 §2.2.1): the next sign-out revokes it.
    expect((await counts('spec'))['revoke-current'] ?? 0).toBe(0);
    expect((await p.signOut(store)).ok).toBe(true);
    expect((await counts('spec'))['revoke-current']).toBe(1);
  });

  test('a refresh whose ID token is for another account ends the session and uses none of its tokens', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'spec' });
    await control({ action: 'id-claims', profile: 'spec', claims: { sub: 'someone-else' } });
    const e = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
    expect([e.code, e.signedOut]).toEqual(['invalid_response', true]);
    const s = await p.session(store);
    expect([s.signedIn, s.error]).toEqual([false, 'The session cannot continue: the ID token is for another account. Sign in again.']);
    expect(JSON.parse(store.map.get('spec.session')!)).toEqual({ ended: s.error });
  });

  test('an ID token naming another authorized party is refused', async () => {
    await control({ action: 'id-claims', profile: 'spec', claims: { azp: 'another-client' } });
    const r = await oauth(configFor('spec')).signIn(memory());
    expect([r.ok, r.code, r.error]).toEqual([false, 'invalid_response', 'Sign-in failed: the ID token is for another app.']);
  });

  test('signing in again as the same account keeps a refresh token the reply leaves out, after a refresh too', async () => {
    const store = memory(), p = oauth(configFor('spec'));
    expect((await p.signIn(store)).ok).toBe(true);
    const nonce = (signedIn(store, 'spec.session').first as { nonce: string }).nonce;
    await control({ action: 'expire-access', profile: 'spec' });
    expect((await p.fetch(store, `${origin}/p/spec/api/me`)).status).toBe(200);
    // No `offline_access`: the spec profile sends no refresh token.
    const again = oauth(config('spec', { scopes: ['openid', 'profile'] }));
    expect((await again.signIn(store)).ok).toBe(true);
    await control({ action: 'expire-access', profile: 'spec' });
    // The kept token's refreshed ID token carries the nonce of the sign-in it came from (OIDC Core §12.2).
    await control({ action: 'id-claims', profile: 'spec', claims: { nonce } });
    expect(((await again.json(store, `${origin}/p/spec/api/me`)) as { name: string }).name).toBe('Ada Lovelace');
    expect((await counts('spec'))['refresh-ok']).toBe(2);
  });

  test('account_selection_required on refresh ends the session', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'spec' });
    await control({ action: 'error-token', profile: 'spec', error: 'account_selection_required' });
    const e = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
    expect([e.code, e.signedOut]).toEqual(['account_selection_required', true]);
    expect((await p.session(store)).signedIn).toBe(false);
  });

  test('an authorized fetch follows no redirect', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const before = (await counts('spec')).api ?? 0;
    expect((await p.fetch(store, `${origin}/p/spec/api/moved`)).status).toBe(302);
    expect((await counts('spec')).api ?? 0).toBe(before);
    expect((await counts('spec')).refresh ?? 0).toBe(0);
  });

  // The fake provider's own rules for a spent refresh token.
  test('a spent refresh token is reuse with no grace period, and Entra still takes one (ENTRA-9)', async () => {
    const spend = async (profile: string, cfg: OAuthConfig): Promise<Response> => {
      const store = memory(), p = oauth(cfg);
      await p.signIn(store);
      const spent = (JSON.parse(store.map.get(cfg.key)!) as { refreshToken: string }).refreshToken;
      await control({ action: 'expire-access', profile });
      await p.fetch(store, `${origin}/p/${profile}/api/me`);
      const token = profile === 'entra' ? `${origin}/p/entra/token` : `${origin}/p/${profile}/token`;
      return fetch(token, { method: 'POST', body: new URLSearchParams({ grant_type: 'refresh_token', refresh_token: spent, client_id: cfg.clientId }) });
    };
    expect((await spend('spec', configFor('spec'))).status).toBe(400);
    expect((await spend('entra', configFor('entra'))).status).toBe(200);
    expect([(await counts('spec')).reuse, (await counts('entra')).reuse]).toEqual([1, 1]);
  });

  test('GitHub refuses an exchange without its client secret (GH-7)', async () => {
    const { clientSecret: _, ...without } = configFor('github');
    const r = await oauth(without).signIn(memory());
    expect([r.ok, r.code]).toEqual([false, 'invalid_client']);
  });

  test('the provider\'s own terminal codes sign out (Auth0 403, OneLogin invalid_request, GitHub bad_refresh_token)', async () => {
    for (const profile of ['auth0', 'onelogin', 'github']) {
      const p = oauth(configFor(profile)), store = memory();
      expect((await p.signIn(store)).ok).toBe(true);
      await control({ action: 'revoke', profile });
      const e = await p.fetch(store, `${origin}/p/${profile}/api/me`).catch((x) => x);
      expect([profile, e.signedOut]).toEqual([profile, true]);
      expect((await p.session(store)).signedIn).toBe(false);
    }
  });

  test('OneLogin refuses offline_access at the authorization endpoint', async () => {
    const p = oauth(config('onelogin', { issuer: `${origin}/p/onelogin/oidc/2` })), store = memory();
    expect((await p.signIn(store)).code).toBe('invalid_scope');
  });

  test('a reply lost after the token was spent: one retry within the grace period keeps the session (Okta)', async () => {
    const p = oauth(configFor('okta')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'okta' });
    await control({ action: 'drop-token-after', profile: 'okta' });
    expect((await p.fetch(store, `${origin}/p/okta/api/me`)).status).toBe(200);
    const c = await counts('okta');
    expect([c['dropped-after'], c.grace, c.reuse ?? 0]).toEqual([1, 1, 0]);
  });

  test('sign-out while a refresh is in flight stays signed out', async () => {
    const p = oauth(configFor('auth0')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'auth0' });
    await control({ action: 'hold-token', profile: 'auth0' });
    const reading = p.fetch(store, `${origin}/p/auth0/api/me`).catch((x) => x);
    await control({ action: 'await', profile: 'auth0', held: 1 });
    // Sign-out waits behind the held refresh, then revokes the token it
    // issued: one that revoked first would leave the refresh nothing to renew.
    const leaving = p.signOut(store);
    await control({ action: 'release-token' });
    await Promise.all([reading, leaving]);
    expect((await p.session(store)).signedIn).toBe(false);
    expect(store.map.size).toBe(0);
    const got = await counts('auth0');
    expect([got['refresh-ok'] ?? 0, got['revoke-current'] ?? 0, got.revoke]).toEqual([1, 1, 1]);
  });

  test('the access token goes only to apis, over HTTPS or loopback', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const e = await p.fetch(store, 'https://cdn.example.com/a.png').catch((x) => x);
    expect(e.code).toBe('config');
    expect(() => oauth(config('spec', { apis: ['http://api.example.com'] }))).toThrow(/not HTTPS/);
    expect(() => oauth(config('spec', { authorizationParams: { state: 'x' } }))).toThrow(/may not set state/);
  });

  test('json throws for a status that is not 2xx and answers the body otherwise', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    expect(await p.json(store, `${origin}/p/spec/api/me`)).toMatchObject({ name: 'Ada Lovelace' });
    const e = await p.json(store, `${origin}/p/spec/nowhere`).catch((x) => x);
    expect([e.code, e.status]).toEqual(['server', 404]);
  });

  test('the press opens the browser before awaiting anything when prepared', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.session(store);
    let opened = false;
    const g = globalThis as Record<string, unknown>, real = g.openAuthSession as (...a: unknown[]) => Promise<string>;
    g.openAuthSession = (...a: unknown[]) => { opened = true; return real(...a); };
    const signing = p.signIn(store);
    expect(opened).toBe(true);
    g.openAuthSession = real;
    expect((await signing).ok).toBe(true);
  });

  test('without retryLostRefresh a reply lost after the token was spent keeps the session, retryable', async () => {
    const p = oauth(config('okta')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'okta' });
    await control({ action: 'drop-token-after', profile: 'okta' });
    const e = await p.fetch(store, `${origin}/p/okta/api/me`).catch((x) => x);
    expect([e.code, e.retryable, e.signedOut]).toEqual(['network', true, false]);
    expect((await p.session(store)).signedIn).toBe(true);
  });

  test('Entra throttling (invalid_grant with AADSTS50196) keeps the session', async () => {
    const p = oauth(configFor('entra')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'entra' });
    await control({ action: 'error-token', profile: 'entra', error: 'invalid_grant', codes: [50196] });
    const e = await p.fetch(store, `${origin}/p/entra/api/me`).catch((x) => x);
    expect([e.code, e.retryable, e.signedOut]).toEqual(['invalid_grant', true, false]);
    expect((await p.session(store)).signedIn).toBe(true);
    expect((await p.fetch(store, `${origin}/p/entra/api/me`)).status).toBe(200);
  });

  test('interaction_required and invalid_client on refresh: one ends the session, the other keeps it', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'spec' });
    await control({ action: 'error-token', profile: 'spec', error: 'invalid_client' });
    const kept = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
    expect([kept.code, kept.retryable, kept.signedOut]).toEqual(['invalid_client', false, false]);
    await control({ action: 'error-token', profile: 'spec', error: 'interaction_required' });
    const ended = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
    expect([ended.code, ended.signedOut]).toEqual(['interaction_required', true]);
    expect(await p.session(store)).toMatchObject({ signedIn: false, error: 'The provider asks you to sign in again.', ended: true });
  });

  test('a status other than 401 does not refresh, and a discovery failure is reported', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const r = await p.fetch(store, `${origin}/p/spec/api/forbidden`);
    expect(r.status).toBe(403);
    expect((await counts('spec')).refresh ?? 0).toBe(0);
    const nowhere = oauth(config('spec', { issuer: `${origin}/p/nope` }));
    expect((await nowhere.signIn(memory())).code).toBe('config');
  });

  test('an ID token is required when openid is asked; max_age is refused', async () => {
    expect(() => oauth(config('spec', { authorizationParams: { max_age: '0' } }))).toThrow(/max_age/);
    const p = oauth(config('github', { issuer: undefined, clientSecret: 'fixture-secret', endpoints: { authorization: `${origin}/p/github/authorize`, token: `${origin}/p/github/token` }, scopes: ['openid'] }));
    expect((await p.signIn(memory())).error).toMatch(/no ID token/);
  });

  test('a refresh reply refused after the provider rotated keeps the new refresh token', async () => {
    const p = oauth(configFor('auth0')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'auth0' });
    await control({ action: 'omit-token-type', profile: 'auth0' });
    const e = await p.fetch(store, `${origin}/p/auth0/api/me`).catch((x) => x);
    expect([e.code, e.retryable, e.signedOut]).toEqual(['invalid_response', true, false]);
    expect((await p.fetch(store, `${origin}/p/auth0/api/me`)).status).toBe(200);
    const c = await counts('auth0');
    expect([c.refresh, c.reuse ?? 0]).toEqual([2, 0]);
  });

  test('with an issuer template, a callback naming another issuer is refused', async () => {
    const p = oauth(configFor('entra')), store = memory();
    tamper = (u) => `${u}&iss=${encodeURIComponent('https://attacker.example/x/v2.0')}`;
    expect((await p.signIn(store)).code).toBe('issuer_mismatch');
    tamper = (u) => `${u}&iss=${encodeURIComponent(`${origin}/p/entra/72f988bf-0000-0000-0000-000000000000/v2.0`)}`;
    expect((await p.signIn(store)).ok).toBe(true);
  });

  test('after a blocked popup the next press opens at once; a failed preparation shows in session', async () => {
    const p = oauth(configFor('spec')), store = memory();
    const g = globalThis as Record<string, unknown>, real = g.openAuthSession as (...a: unknown[]) => Promise<string>;
    // Unprepared, the press prepares first, and the browser blocks the popup.
    g.openAuthSession = () => Promise.reject(Object.assign(new Error('popup blocked'), { status: 428 }));
    expect((await p.signIn(store)).code).toBe('popup_blocked');
    let opened = false;
    g.openAuthSession = (...a: unknown[]) => { opened = true; return real(...a); };
    const signing = p.signIn(store);
    expect(opened).toBe(true);
    g.openAuthSession = real;
    expect((await signing).ok).toBe(true);
    const nowhere = oauth(config('spec', { issuer: `${origin}/p/nope` }));
    expect((await nowhere.session(memory())).error).toMatch(/discovery document/);
  });
});

// Runs `run` with a fetch that answers what `answer` returns for a request it
// picks, and sends every other request to the fixture.
async function withFetch<T>(answer: (url: string, init: RequestInit | undefined, real: typeof fetch) => Promise<Response> | undefined, run: () => Promise<T>): Promise<T> {
  const real = globalThis.fetch;
  globalThis.fetch = ((input: RequestInfo | URL, init?: RequestInit) => answer(String(input), init, real) ?? real(input, init)) as typeof fetch;
  try { return await run(); } finally { globalThis.fetch = real; }
}
const refreshing = (url: string, init?: RequestInit): boolean => url.endsWith('/token') && String(init?.body).includes('grant_type=refresh_token');
const signedIn = (store: { map: Map<string, string> }, key: string): Record<string, unknown> => JSON.parse(store.map.get(key)!);
// A reply as the fixture sent it, dated `at`.
const dated = (reply: Promise<Response>, at: number): Promise<Response> => reply.then(async (r) => new Response(await r.text(), { status: r.status, headers: { 'Content-Type': r.headers.get('content-type') ?? 'application/json', Date: new Date(at).toUTCString() } }));

describe('what a reply must be', () => {
  test('a refresh reply whose ID token is for another account keeps none of its tokens, though it is unreadable too', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'spec' });
    await control({ action: 'id-claims', profile: 'spec', claims: { sub: 'someone-else' } });
    await control({ action: 'omit-token-type', profile: 'spec' });
    const e = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
    expect([e.code, e.signedOut]).toEqual(['invalid_response', true]);
    expect(Object.keys(signedIn(store, 'spec.session'))).toEqual(['ended']);
  });

  test('a refresh reply whose ID token is empty or not a string keeps none of its tokens', async () => {
    for (const idToken of ['', null, 7]) {
      await control({ action: 'reset' });
      const p = oauth(configFor('spec')), store = memory();
      await p.signIn(store);
      await control({ action: 'expire-access', profile: 'spec' });
      const e = await withFetch((url, init, real) => (!refreshing(url, init) ? undefined : (async () => {
        const body = await (await real(url, init)).json();
        return Response.json({ ...body, id_token: idToken });
      })()), () => p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x));
      expect([e.code, e.signedOut], JSON.stringify(idToken)).toEqual(['invalid_response', true]);
      expect(Object.keys(signedIn(store, 'spec.session'))).toEqual(['ended']);
    }
  });

  test('a refreshed ID token with another issuer, audience or azp ends the session', async () => {
    for (const claims of [{ iss: `${origin}/p/elsewhere` }, { aud: ['client-spec', 'client-other'], azp: 'client-spec' }, { azp: 'client-other' }]) {
      await control({ action: 'reset' });
      const p = oauth({ ...configFor('spec'), trustedAudiences: ['client-other'] }), store = memory();
      await p.signIn(store);
      await control({ action: 'expire-access', profile: 'spec' });
      await control({ action: 'id-claims', profile: 'spec', claims });
      const e = await p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x);
      expect([e.code, e.signedOut], JSON.stringify(claims)).toEqual(['invalid_response', true]);
      expect(Object.keys(signedIn(store, 'spec.session'))).toEqual(['ended']);
    }
  });

  test('an unchanged ID token in a refresh reply is checked as a new one is', async () => {
    const p = oauth(configFor('spec')), store = memory(), at = Date.now();
    // Four minutes before the sign-in reply's `Date`: past its expiry, within the five minutes of skew.
    await control({ action: 'id-claims', profile: 'spec', claims: { exp: Math.floor(at / 1000) - 240 } });
    expect((await withFetch((url, init, real) => (!url.endsWith('/token') ? undefined : dated(real(url, init), at)), () => p.signIn(store))).ok).toBe(true);
    const idToken = signedIn(store, 'spec.session').idToken;
    await control({ action: 'expire-access', profile: 'spec' });
    // The refresh reply carries that ID token, dated two minutes later: six past its expiry, past the skew.
    const e = await withFetch((url, init, real) => (!refreshing(url, init) ? undefined : (async () => {
      const body = await (await real(url, init)).json();
      return Response.json({ ...body, id_token: idToken }, { headers: { Date: new Date(at + 120_000).toUTCString() } });
    })()), () => p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x));
    expect([e.code, e.signedOut]).toEqual(['invalid_response', true]);
    expect((await p.session(store)).error).toMatch(/the ID token has expired/);
  });

  test('an ID token past its expiry by more than the skew is refused at sign-in', async () => {
    // Six minutes before its reply's `Date`.
    const at = Date.now();
    await control({ action: 'id-claims', profile: 'spec', claims: { exp: Math.floor(at / 1000) - 360 } });
    const r = await withFetch((url, init, real) => (!url.endsWith('/token') ? undefined : dated(real(url, init), at)), () => oauth(configFor('spec')).signIn(memory()));
    expect(r.error).toBe('Sign-in failed: the ID token has expired.');
  });

  test('an API origin matches as the URL Standard serializes origins, and no other port', async () => {
    const cases = [
      [['https://api.example:443'], 'https://api.example/me', 200], [['https://api.example'], 'https://API.example:443/me', 200],
      [['https://api.example'], 'https://api.example:0443/me', 200], [['https://bücher.example'], 'https://xn--bcher-kva.example/me', 200],
      [['https://api.example'], 'https://api.example:8443/me', 'config'],
    ] as const;
    for (const [apis, url, expected] of cases) {
      await control({ action: 'reset' });
      const p = oauth({ ...configFor('spec'), apis: [...apis] }), store = memory();
      expect((await p.signIn(store)).ok).toBe(true);
      const got = await withFetch((u) => (/^https:\/\/(api\.example|xn--bcher-kva\.example)/i.test(u) ? Promise.resolve(new Response('{}')) : undefined),
        () => p.fetch(store, url).then((r) => r.status, (e) => e.code));
      expect(got, url).toBe(expected);
    }
  });

  test('without a Date the expiry is unknown', async () => {
    const p = oauth(configFor('spec')), store = memory();
    const r = await withFetch((url, init, real) => (!url.endsWith('/token') ? undefined : (async () => {
      const reply = await real(url, init);
      return new Response(await reply.text(), { status: reply.status, headers: { 'Content-Type': 'application/json' } });
    })()), () => p.signIn(store));
    expect(r.ok).toBe(true);
    expect((await p.session(store)).expiresAt).toBe(0);
  });

  test('a refresh token held for another client or another issuer is not kept', async () => {
    const store = memory();
    expect((await oauth(configFor('spec')).signIn(store)).ok).toBe(true);
    // Another client, kept under the same key, with no refresh token of its own
    expect((await oauth(config('spec', { clientId: 'client-other', scopes: ['openid', 'profile'] })).signIn(store)).ok).toBe(true);
    expect(signedIn(store, 'spec.session').refreshToken).toBeUndefined();
    // The same subject in another Entra tenant
    const entra = memory();
    expect((await oauth(configFor('entra')).signIn(entra)).ok).toBe(true);
    const tid = '0e7a6b38-0000-0000-0000-000000000000';
    await control({ action: 'id-claims', profile: 'entra', claims: { tid, iss: `${origin}/p/entra/${tid}/v2.0` } });
    expect((await oauth({ ...configFor('entra'), scopes: ['openid', 'profile'] }).signIn(entra)).ok).toBe(true);
    expect(signedIn(entra, 'entra.session').refreshToken).toBeUndefined();
  });

  test('an ID token naming an audience the app does not trust is refused, one it trusts is not', async () => {
    const claims = { aud: ['client-spec', 'https://api.example.test'], azp: 'client-spec' };
    await control({ action: 'id-claims', profile: 'spec', claims });
    expect((await oauth(configFor('spec')).signIn(memory())).error).toBe('Sign-in failed: the ID token names an audience the app does not trust.');
    await control({ action: 'id-claims', profile: 'spec', claims });
    expect((await oauth({ ...configFor('spec'), trustedAudiences: ['https://api.example.test'] }).signIn(memory())).ok).toBe(true);
  });

  test('a form-encoded reply that repeats a parameter is refused', async () => {
    const r = await withFetch((url) => (!url.endsWith('/token') ? undefined : Promise.resolve(new Response('access_token=a&access_token=b&token_type=Bearer', { headers: { 'Content-Type': 'application/x-www-form-urlencoded' } }))),
      () => oauth(configFor('spec')).signIn(memory()));
    expect(r.code).toBe('invalid_response');
  });

  test('discovery falls back to RFC 8414, and an endpoint it names must be absolute', async () => {
    const asked: string[] = [];
    const openid = `${origin}/p/spec/.well-known/openid-configuration`, rfc8414 = `${origin}/.well-known/oauth-authorization-server/p/spec`;
    const r = await withFetch((url, init, real) => {
      if (url === openid) { asked.push(url); return Promise.resolve(new Response(null, { status: 404 })); }
      if (url === rfc8414) { asked.push(url); return real(openid, init); }
      return undefined;
    }, () => oauth(configFor('spec')).signIn(memory()));
    expect([r.ok, asked]).toEqual([true, [openid, rfc8414]]);
    const relative = await withFetch((url, init, real) => (url !== openid ? undefined : (async () => Response.json({ ...(await (await real(url, init)).json()), token_endpoint: '/token' }))()),
      () => oauth(configFor('spec')).signIn(memory()));
    expect([relative.code, relative.retryable]).toEqual(['config', false]);
  });
});

describe('the fake provider', () => {
  test("a token reply names its own origin's issuer, whatever another origin is asked meanwhile", async () => {
    // The exchange waits for its body while the second origin is asked: `handle` runs to its first await at once.
    const r = await withFetch((url, init) => {
      if (!url.endsWith('/token')) return undefined;
      let send!: ReadableStreamDefaultController<Uint8Array>;
      const body = new ReadableStream<Uint8Array>({ start(c) { send = c; } });
      const reply = Promise.resolve(handle(new Request(url, { ...init, body, duplex: 'half' } as RequestInit)));
      return Promise.resolve(handle(new Request(`http://127.0.0.1:${second.port}/control/counts`))).then(() => {
        send.enqueue(new TextEncoder().encode(String(init?.body)));
        send.close();
        return reply;
      });
    }, () => oauth(configFor('spec')).signIn(memory()));
    expect(r.ok).toBe(true);
  });

  test('a code or refresh token is redeemed only by the client it was issued to, whose secret is compared whole', async () => {
    let refresh = '';
    // A sign-in whose exchange sends `authorization` in place of the form's client credentials: the exchange's reply.
    const exchange = async (profile: string, authorization: string, keep = false): Promise<unknown[]> => {
      let got: unknown[] = [];
      await withFetch((url, init, real) => {
        if (!url.endsWith('/token')) return undefined;
        const body = new URLSearchParams(String(init?.body));
        if (!keep) body.delete('client_id');
        if (!keep) body.delete('client_secret');
        const headers = { ...(init?.headers as Record<string, string>), Authorization: authorization };
        return real(url, { ...init, headers, body }).then(async (reply) => {
          const json = await reply.clone().json();
          got = [reply.status, json.error];
          refresh = json.refresh_token ?? refresh;
          return reply;
        });
      }, () => oauth(configFor(profile)).signIn(memory()));
      return got;
    };
    expect(await exchange('spec', `Basic ${btoa('client-other:')}`)).toEqual([400, 'invalid_grant']);
    expect(await exchange('spec', `Basic ${btoa('client-spec:')}`)).toEqual([200, undefined]);
    const renew = async (headers: Record<string, string>, fields: Record<string, string>): Promise<unknown[]> => {
      const body = new URLSearchParams({ grant_type: 'refresh_token', refresh_token: refresh, ...fields });
      const reply = await fetch(`${origin}/p/spec/token`, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded', ...headers }, body });
      return [reply.status, (await reply.json()).error];
    };
    expect(await renew({}, { client_id: 'client-other' })).toEqual([400, 'invalid_grant']);
    expect(await renew({ Authorization: `Basic ${btoa('client-other:')}` }, {})).toEqual([400, 'invalid_grant']);
    expect(await renew({ Authorization: 'Bearer client-spec' }, {})).toEqual([401, 'invalid_client']);
    expect(await renew({}, { client_id: 'client-spec' })).toEqual([200, undefined]);
    // Google's requires a secret: the whole of it after the first colon, form-decoded, under `Basic` in any case, in
    // canonical Base64; Basic beside the form's credentials authenticates twice.
    expect(await exchange('google', `Basic ${btoa('client-google:wrong:fixture-secret')}`)).toEqual([401, 'invalid_client']);
    expect(await exchange('google', `Basic ${btoa('%63lient-google:fixture-secret')}A`)).toEqual([401, 'invalid_client']);
    expect(await exchange('google', `Basic ${btoa('client-google:fixture-secret').replace(/=+$/, '')}`)).toEqual([401, 'invalid_client']);
    expect(await exchange('google', `Basic ${btoa('client-google:%66ixture-secret')}`)).toEqual([200, undefined]);
    expect(await exchange('google', `basic ${btoa('client-google:fixture-secret')}`)).toEqual([200, undefined]);
    expect(await exchange('google', `Basic ${btoa('client-google:fixture-secret')}`, true)).toEqual([400, 'invalid_request']);
  });

  test('a token or revocation request is refused as RFC 6749 §5.2 and RFC 7009 §2.1 say', async () => {
    let tokens = { access_token: '', refresh_token: '' };
    // An exchange that names no client is refused before its code is spent: the same code then serves the client.
    const nameless: unknown[] = [];
    const signedIn = await withFetch((url, init, real) => {
      if (!url.endsWith('/token')) return undefined;
      const body = new URLSearchParams(String(init?.body));
      body.delete('client_id');
      return real(url, { ...init, body: body.toString() }).then(async (first) => {
        nameless.push([first.status, (await first.json()).error]);
        const reply = await real(url, init);
        tokens = await reply.clone().json();
        return reply;
      });
    }, () => oauth(configFor('spec')).signIn(memory()));
    expect(nameless).toEqual([[400, 'invalid_request']]);
    expect(signedIn.ok).toBe(true);
    // A request's status, error and challenge.
    const ask = async (path: string, body: string, headers: Record<string, string> = {}): Promise<unknown[]> => {
      const reply = await fetch(`${origin}/p/spec/${path}`, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded', ...headers }, body });
      const text = await reply.text();
      return [reply.status, text ? JSON.parse(text).error : undefined, reply.headers.get('www-authenticate')];
    };
    const live = async (): Promise<number> => (await fetch(`${origin}/p/spec/api/me`, { headers: { Authorization: `Bearer ${tokens.access_token}` } })).status;
    const refresh = `grant_type=refresh_token&refresh_token=${encodeURIComponent(tokens.refresh_token)}&client_id=client-spec`;
    expect(await ask('token', refresh, { 'Content-Type': 'application/json' })).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', `${refresh}&grant_type=refresh_token`)).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', 'client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', 'grant_type=refresh_token&client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', 'grant_type=authorization_code&redirect_uri=x&client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', refresh, { Authorization: 'Basic !' })).toEqual([401, 'invalid_client', 'Basic realm="fixture"']);
    // Twice, behind a parameter with no name too; a parameter with no value is not sent.
    expect(await ask('token', `=a&=b&${refresh}&client_id=client-other`)).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', 'grant_type=&client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    expect(await ask('token', 'grant_type=refresh_token&refresh_token=&client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    // A revocation by another client, or malformed, leaves the token live; the client's own ends it.
    const revoke = `token=${encodeURIComponent(tokens.access_token)}`;
    expect(await ask('revoke', `${revoke}&client_id=client-other`)).toEqual([400, 'invalid_grant', null]);
    expect(await ask('revoke', revoke, { Authorization: 'Basic !' })).toEqual([401, 'invalid_client', 'Basic realm="fixture"']);
    expect(await ask('revoke', `${revoke}&client_id=client-spec&client_id=client-spec`)).toEqual([400, 'invalid_request', null]);
    expect(await ask('revoke', 'client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    expect(await ask('revoke', 'token=&client_id=client-spec')).toEqual([400, 'invalid_request', null]);
    expect(await ask('revoke', `=a&=b&${revoke}&client_id=client-spec&client_id=client-other`)).toEqual([400, 'invalid_request', null]);
    expect(await ask('revoke', revoke)).toEqual([400, 'invalid_request', null]);
    expect(await live()).toBe(200);
    expect(await ask('revoke', `${revoke}&client_id=client-spec`)).toEqual([200, undefined, null]);
    expect(await live()).toBe(401);
  });
});

// ForgeRock's tree answered with the fixture's person (FR-1): the SSO token it ends in, "" when refused.
async function tree(password = 'lovelace'): Promise<string> {
  const at = `${origin}/p/forgerock/json/authenticate`;
  const node = await (await fetch(at, { method: 'POST', body: '{}' })).json() as { callbacks: { input: { value: string }[] }[] };
  node.callbacks[0].input[0].value = 'ada@example.com';
  node.callbacks[1].input[0].value = password;
  return ((await (await fetch(at, { method: 'POST', body: JSON.stringify(node) })).json()) as { tokenId?: string }).tokenId ?? '';
}

// An `authorize` that POSTs the authorization request with `sso` (FR-2) through the package's transport, as an app
// does, in the cookie or in a header of the cookie's name; `edit` alters where it was sent.
const withSession = (sso: string, edit: (location: string) => string = (l) => l, carry: 'cookie' | 'header' = 'cookie') =>
  async ({ endpoint, params, fetch: send }: OAuthAuthorization): Promise<string> => {
    const reply = await send(endpoint, {
      method: 'POST', redirect: 'manual',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded', ...(carry === 'cookie' ? { Cookie: `iPlanetDirectoryPro=${sso}` } : { iPlanetDirectoryPro: sso }) },
      body: new URLSearchParams({ ...params, csrf: sso, decision: 'allow' }).toString(),
    });
    return edit(reply.headers.get('location') ?? '');
  };

// An app that signs the person in with its own screens (LLP 1146 D8):
// ForgeRock's authentication tree, then the authorization request with its
// SSO token, which the app sends; the package keeps everything else.
describe('a first-party sign-in', () => {
  // No browser opens on this path
  const opened = { count: 0 };
  let browser: (...a: unknown[]) => Promise<string>;
  beforeEach(() => {
    const g = globalThis as Record<string, unknown>;
    browser = g.openAuthSession as typeof browser;
    opened.count = 0;
    g.openAuthSession = (...a: unknown[]) => { opened.count++; return browser(...a); };
  });
  afterEach(() => { (globalThis as Record<string, unknown>).openAuthSession = browser; });

  test('the app sends the authorization request; the exchange, refresh and sign-out are the package\'s', async () => {
    const p = oauth(configFor('forgerock')), store = memory();
    expect(p.claims(store)).toBeNull();
    expect(await p.session(store)).toEqual({ signedIn: false, subject: '', expiresAt: 0, scopes: [], error: '', ended: false });
    let asked: OAuthAuthorization | null = null;
    const sso = await tree(), send = withSession(sso);
    const r = await p.signIn(store, { authorize: (request) => { asked = request; return send(request); } });
    expect(r).toEqual({ ok: true, code: '', error: '', retryable: false, status: 0 });
    expect(opened.count).toBe(0);
    expect(asked!.endpoint).toBe(`${origin}/p/forgerock/authorize`);
    expect(asked!.url.startsWith(`${asked!.endpoint}?`)).toBe(true);
    expect(Object.keys(asked!.params).sort()).toEqual(['client_id', 'code_challenge', 'code_challenge_method', 'nonce', 'redirect_uri', 'response_type', 'scope', 'state']);
    expect(p.claims(store)).toMatchObject({ sub: 'user-1', name: 'Ada Lovelace' });
    // A session kept without an ID token has no claims
    const bare = memory();
    bare.set('forgerock.session', JSON.stringify({ ...JSON.parse(store.get('forgerock.session')!), idToken: undefined }));
    expect(p.claims(bare)).toBeNull();
    // The gateway's 403 for an expired access token refreshes (`expiredStatus`).
    await control({ action: 'expire-access', profile: 'forgerock' });
    expect(((await p.json(store, `${origin}/p/forgerock/api/me`)) as { name: string }).name).toBe('Ada Lovelace');
    expect(await counts('forgerock')).toMatchObject({ authenticate: 2, exchange: 1, refresh: 1 });
    expect((await p.signOut(store)).ok).toBe(true);
    expect(p.claims(store)).toBeNull();
  });

  test('configured as the docs say, with no auth.callback grant: callback set, and the token in a header', async () => {
    const g = globalThis as Record<string, unknown>, callback = g.authCallback;
    g.authCallback = (): string => { throw new Error('authCallback(): the grants name no auth.callback'); };
    try {
      const p = oauth({ ...configFor('forgerock'), callback: CALLBACK }), store = memory();
      expect((await p.session(store)).error).toBe('');
      expect(await p.signIn(store, { authorize: withSession(await tree(), undefined, 'header') })).toMatchObject({ ok: true });
      expect(opened.count).toBe(0);
    } finally {
      g.authCallback = callback;
    }
    expect(() => oauth({ ...configFor('forgerock'), callback: `${CALLBACK}?from=app` })).toThrow(TypeError);
    // What the host refuses as a session's callback: userinfo, and an http: or https: one with no host or a backslash
    for (const callback of ['https://u@app.example/cb', 'https:/app.example/cb', 'https://app.example\\cb', 'com.example.app://u@host/cb']) {
      expect(() => oauth({ ...configFor('forgerock'), callback })).toThrow(TypeError);
    }
    for (const callback of ['com.example.app:/cb', 'https://app.example/cb']) expect(() => oauth({ ...configFor('forgerock'), callback })).not.toThrow();
  });

  test("authorize's redirect is compared with the callback as the host compares one", async () => {
    const p = oauth({ ...configFor('forgerock'), callback: 'https://app.example/cb' }), sso = await tree();
    const signIn = (edit: (location: string) => string) => p.signIn(memory(), { authorize: withSession(sso, edit) });
    expect(await signIn((l) => l.replace('https://app.example', 'HTTPS://APP.Example'))).toMatchObject({ ok: true });
    expect(await signIn((l) => l.replace('https://app.example', 'https://u@app.example'))).toMatchObject({ ok: false, code: 'invalid_response' });
    expect(await signIn((l) => l.replace('/cb', '/CB'))).toMatchObject({ ok: false, code: 'invalid_response' });
    // What the host refuses: whitespace or a control character anywhere, and an empty authority
    for (const edit of [(l: string) => `${l}&extra=hello world`, (l: string) => `${l}&extra=\u0007`, (l: string) => l.replace('https://app.example', 'https://')]) {
      expect(await signIn(edit)).toMatchObject({ ok: false, code: 'invalid_response' });
    }
    // Case is folded as the host folds it, ASCII only: a Kelvin sign (U+212A) is no `k`
    const kelvin = oauth({ ...configFor('forgerock'), callback: 'https://k.example/cb' });
    const kelvinIn = (edit: (location: string) => string) => kelvin.signIn(memory(), { authorize: withSession(sso, edit) });
    expect(await kelvinIn((l) => l.replace('https://k.example', 'https://K.EXAMPLE'))).toMatchObject({ ok: true });
    expect(await kelvinIn((l) => l.replace('https://k.example', 'https://\u212a.example'))).toMatchObject({ ok: false, code: 'invalid_response' });
    // A callback the host could not read is refused at once: one with whitespace, a relative path and no authority, a
    // scheme with a Kelvin sign, or none
    for (const callback of ['https://app.example/c b', 'com.example.app:cb', '\u212a:/cb', '']) {
      expect(() => oauth({ ...configFor('forgerock'), callback })).toThrow(TypeError);
    }
  });

  test('what the authorization step answers is checked as any callback', async () => {
    const p = oauth(configFor('forgerock')), store = memory();
    // A wrong password leaves no session: the provider sends the person to its own sign-in page.
    expect(await p.signIn(store, { authorize: withSession(await tree('wrong')) })).toMatchObject({ ok: false, code: 'invalid_response' });
    const sso = await tree();
    expect(await p.signIn(store, { authorize: withSession(sso, (l) => l.replace(/state=[^&]+/, 'state=other')) })).toMatchObject({ ok: false, code: 'state_mismatch' });
    expect(await p.signIn(store, { authorize: withSession(sso, () => '') })).toMatchObject({ ok: false, code: 'invalid_response' });
    expect(await p.signIn(store, { authorize: () => Promise.reject(new OAuthError('access_denied', 'Lexus refused the sign-in.')) }))
      .toEqual({ ok: false, code: 'access_denied', error: 'Lexus refused the sign-in.', retryable: false, status: 0 });
    expect(await p.signIn(store, { authorize: () => Promise.reject(new Error('offline')) })).toMatchObject({ ok: false, code: 'network', retryable: true });
    // A refusal's status reaches the result, the token endpoint's too
    expect(await p.signIn(store, { authorize: () => Promise.reject(new OAuthError('server', 'Busy.', 429, true)) })).toMatchObject({ code: 'server', status: 429, retryable: true });
    await control({ action: 'fail-token', profile: 'forgerock', status: 503 });
    expect(await p.signIn(store, { authorize: withSession(sso) })).toMatchObject({ ok: false, code: 'server', status: 503, retryable: true });
    // A 2xx the package cannot use has no status of its own
    const unreadable = await withFetch((url, init) => (url.endsWith('/token') && init?.method === 'POST' ? Promise.resolve(new Response('not json', { status: 200 })) : undefined),
      () => p.signIn(store, { authorize: withSession(sso) }));
    expect(unreadable).toMatchObject({ ok: false, status: 0 });
    expect((await p.session(store)).signedIn).toBe(false);
    expect(await p.signIn(store, { authorize: withSession(sso) })).toMatchObject({ ok: true });
    expect(opened.count).toBe(0);
  });

  test('a 403 refreshes only when expiredStatus names it, and json throws unauthorized for one that stands', async () => {
    const p = oauth(config('forgerock')), store = memory();
    await p.signIn(store, { authorize: withSession(await tree()) });
    await control({ action: 'expire-access', profile: 'forgerock' });
    const refused = await p.json(store, `${origin}/p/forgerock/api/me`).then(() => null, (e: OAuthError) => e);
    expect(refused).toMatchObject({ code: 'server', status: 403 });
    expect((await counts('forgerock')).refresh ?? 0).toBe(0);
    const named = oauth(configFor('forgerock'));
    const stands = await named.json(store, `${origin}/p/forgerock/api/forbidden`).then(() => null, (e: OAuthError) => e);
    expect(stands).toMatchObject({ code: 'unauthorized', status: 403 });
    for (const expiredStatus of [[], [500], [401.5]]) expect(() => oauth({ ...configFor('forgerock'), expiredStatus })).toThrow(TypeError);
  });
});

describe('what an app hears and sends', () => {
  test('log hears what the session did, in order, with no token in it', async () => {
    const events: OAuthEvent[] = [];
    // Each wait ends as `log` hears its event
    const waiting: [number, () => void][] = [];
    const heard = (e: OAuthEvent): void => {
      events.push(e);
      for (const [n, done] of waiting.splice(0)) if (events.length >= n) done(); else waiting.push([n, done]);
    };
    const p = oauth({ ...configFor('forgerock'), log: heard }), store = memory();
    const refreshToken = (): string => (JSON.parse(store.get('forgerock.session') ?? '{}') as { refreshToken?: string }).refreshToken ?? '';
    // The sign-in's print waits until the refresh after it has happened: unless the package keeps events in order,
    // the refresh is heard first
    const subtle = crypto.subtle, digest = subtle.digest.bind(subtle);
    let release = (): void => undefined, holding = true;
    const held = new Promise<void>((done) => { release = done; });
    subtle.digest = (async (algorithm: AlgorithmIdentifier, data: BufferSource) => {
      if (holding && new TextDecoder().decode(data as Uint8Array) === refreshToken()) {
        holding = false;
        await held;
      }
      return digest(algorithm, data);
    }) as SubtleCrypto['digest'];
    // Released and restored whatever fails, so no later test waits on the held print
    try {
      const tokens = (): string[] => {
        const kept = JSON.parse(store.get('forgerock.session') ?? '{}') as Record<string, unknown>;
        return [kept.accessToken, kept.refreshToken, kept.idToken].filter((v): v is string => typeof v === 'string');
      };
      const seen: string[] = [];
      // The tokens each step left, read at once: the next step goes on while an event's print is still being made
      const step = (): void => { seen.push(...tokens()); };
      const until = async (n: number): Promise<void> => {
        if (events.length < n) await new Promise<void>((done) => waiting.push([n, done]));
      };
      // A print as the package documents it: the first six hex digits of the token's SHA-256
      const printOf = async (token: string): Promise<string> =>
        Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(token))).slice(0, 3), (b) => b.toString(16).padStart(2, '0')).join('');
      await p.signIn(store);
      step();
      const first = refreshToken();
      const api = `${origin}/p/forgerock/api/me`;
      await control({ action: 'expire-access', profile: 'forgerock' });
      await p.json(store, api);
      step();
      const second = refreshToken();
      release();
      await control({ action: 'fail-token', profile: 'forgerock', status: 503 });
      await control({ action: 'expire-access', profile: 'forgerock' });
      await p.json(store, api).catch(() => undefined);
      step();
      // A refresh that gets no reply
      await control({ action: 'expire-access', profile: 'forgerock' });
      await withFetch((url, init) => (refreshing(url, init) ? Promise.reject(new TypeError('offline')) : undefined), () => p.json(store, api).catch(() => undefined));
      step();
      await control({ action: 'error-token', profile: 'forgerock', error: 'invalid_grant' });
      await control({ action: 'expire-access', profile: 'forgerock' });
      await p.json(store, api).catch(() => undefined);
      step();
      await p.signIn(store);
      step();
      // A token the API refuses again after a refresh (`/api/forbidden` always answers 403).
      await p.fetch(store, `${origin}/p/forgerock/api/forbidden`);
      step();
      await p.signOut(store);
      await until(9);
      seen.push(...tokens());
      expect(events.map((e) => e.type)).toEqual(['signed-in', 'refreshed', 'refresh-failed', 'refresh-failed', 'ended', 'signed-in', 'refreshed', 'refused', 'signed-out']);
      const [signedIn, refreshed, failed, unanswered, ended, , , refused] = events as [
        Extract<OAuthEvent, { type: 'signed-in' }>, Extract<OAuthEvent, { type: 'refreshed' }>, Extract<OAuthEvent, { type: 'refresh-failed' }>,
        Extract<OAuthEvent, { type: 'refresh-failed' }>, Extract<OAuthEvent, { type: 'ended' }>, OAuthEvent, OAuthEvent, Extract<OAuthEvent, { type: 'refused' }>];
      expect(signedIn.token).toBe(await printOf(first));
      expect(refreshed).toMatchObject({ sent: signedIn.token, expiresIn: 3600 });
      expect(refreshed.got).toBe(await printOf(second));
      expect(refreshed.got).not.toBe(refreshed.sent);
      expect(failed).toMatchObject({ sent: refreshed.got, status: 503, code: 'server' });
      expect(unanswered).toMatchObject({ sent: refreshed.got, status: 0, code: 'network' });
      expect(ended).toMatchObject({ sent: refreshed.got, status: 400, code: 'invalid_grant' });
      expect(refused).toEqual({ type: 'refused', status: 403, url: `${origin}/p/forgerock/api/forbidden` });
      const said = JSON.stringify(events);
      for (const token of seen) expect(said).not.toContain(token);
    } finally {
      release();
      subtle.digest = digest;
    }
  });

  test("log hears a session's events in order across every provider made for its key", async () => {
    const events: string[] = [];
    let heard = (): void => undefined;
    const config = { ...configFor('forgerock'), log: (e: OAuthEvent): void => { events.push(e.type); heard(); } };
    const one = oauth(config), two = oauth(config), store = memory();
    const refreshToken = (): string => (JSON.parse(store.get('forgerock.session') ?? '{}') as { refreshToken?: string }).refreshToken ?? '';
    // The sign-in's print is held; each token's digests are counted
    const subtle = crypto.subtle, digest = subtle.digest.bind(subtle), asked = new Map<string, number>();
    let release = (): void => undefined, holding = true;
    const held = new Promise<void>((done) => { release = done; });
    subtle.digest = (async (algorithm: AlgorithmIdentifier, data: BufferSource) => {
      const text = new TextDecoder().decode(data as Uint8Array);
      asked.set(text, (asked.get(text) ?? 0) + 1);
      if (holding && text === refreshToken()) {
        holding = false;
        await held;
      }
      return digest(algorithm, data);
    }) as SubtleCrypto['digest'];
    try {
      await one.signIn(store);
      const first = refreshToken();
      await control({ action: 'expire-access', profile: 'forgerock' });
      await two.json(store, `${origin}/p/forgerock/api/me`);
      // A turn for anything the refresh's event would do: queued behind the sign-in's, it has not asked for the print
      // of the token it sent
      await new Promise((done) => setTimeout(done, 0));
      expect(asked.get(first)).toBe(1);
      const both = new Promise<void>((done) => { heard = () => { if (events.length === 2) done(); }; });
      release();
      await both;
      expect(events).toEqual(['signed-in', 'refreshed']);
    } finally {
      release();
      subtle.digest = digest;
    }
  });

  test('authorize sends with the package\'s transport', async () => {
    const sent: string[] = [];
    const p = oauth({ ...configFor('forgerock'), fetch: (url, init) => { sent.push(`${init.method ?? 'GET'} ${new URL(url).pathname}`); return fetch(url, init); } }), store = memory();
    expect((await p.signIn(store, { authorize: withSession(await tree()) })).ok).toBe(true);
    expect(sent).toEqual(['GET /p/forgerock/.well-known/openid-configuration', 'POST /p/forgerock/authorize', 'POST /p/forgerock/token']);
  });

  test('fetch carries every request the package makes', async () => {
    const sent: string[] = [];
    const p = oauth({ ...configFor('forgerock'), fetch: (url, init) => { sent.push(`${init.method ?? 'GET'} ${new URL(url).pathname}`); return fetch(url, init); } }), store = memory();
    await p.signIn(store);
    await control({ action: 'expire-access', profile: 'forgerock' });
    await p.json(store, `${origin}/p/forgerock/api/me`);
    await p.signOut(store);
    expect(sent).toEqual([
      'GET /p/forgerock/.well-known/openid-configuration', 'POST /p/forgerock/token',
      'GET /p/forgerock/api/me', 'POST /p/forgerock/token', 'GET /p/forgerock/api/me', 'POST /p/forgerock/revoke',
    ]);
  });

  test('a token an API refused right after a refresh refreshes again only once it has expired by the API\'s Date', async () => {
    const p = oauth(configFor('forgerock')), store = memory(), forbidden = `${origin}/p/forgerock/api/forbidden`;
    await p.signIn(store);
    expect((await p.fetch(store, forbidden)).status).toBe(403);
    expect((await counts('forgerock')).refresh).toBe(1);
    // Two reads the gateway keeps refusing: the second spends no refresh token
    expect((await p.fetch(store, forbidden)).status).toBe(403);
    expect((await counts('forgerock')).refresh).toBe(1);
    const { expiresAt } = await p.session(store);
    const refusedAt = (at: number) => (url: string) => (url === forbidden ? Promise.resolve(new Response(null, { status: 403, headers: { Date: new Date(at).toUTCString() } })) : undefined);
    await withFetch(refusedAt(expiresAt - 60_000), () => p.fetch(store, forbidden));
    expect((await counts('forgerock')).refresh).toBe(1);
    // Refused at its expiry, by the API's clock: refreshed
    await withFetch(refusedAt(expiresAt), () => p.fetch(store, forbidden));
    expect((await counts('forgerock')).refresh).toBe(2);
  });

  test("a refused token's record is the session's, which every provider made for its key reads", async () => {
    const a = oauth(configFor('forgerock')), b = oauth(configFor('forgerock')), store = memory(), forbidden = `${origin}/p/forgerock/api/forbidden`;
    await a.signIn(store);
    const before = (await counts('forgerock')).refresh ?? 0;
    for (const p of [a, b, a]) expect((await p.fetch(store, forbidden)).status).toBe(403);
    expect((await counts('forgerock')).refresh ?? 0).toBe(before + 1);
  });

  test('a refusal that lands after a newer refresh leaves that token its record', async () => {
    const p = oauth(configFor('forgerock')), store = memory(), forbidden = `${origin}/p/forgerock/api/forbidden`;
    await p.signIn(store);
    let calls = 0, release = (): void => undefined, retried = (): void => undefined;
    const held = new Promise<void>((done) => { release = done; }), retrying = new Promise<void>((done) => { retried = done; });
    // The API refuses every read, its second, the first read's retry, only once released
    const refuse = (url: string): Promise<Response> | undefined => {
      if (url !== forbidden) return undefined;
      if (++calls === 2) retried();
      return (calls === 2 ? held : Promise.resolve()).then(() => new Response(null, { status: 403, headers: { Date: new Date().toUTCString() } }));
    };
    await withFetch(refuse, async () => {
      const late = p.fetch(store, forbidden);
      await retrying;
      // Refused with the first read's new token, it refreshes again, and its own retry is refused: that token's record
      expect((await p.fetch(store, forbidden)).status).toBe(403);
      release();
      expect((await late).status).toBe(403);
      const before = (await counts('forgerock')).refresh ?? 0;
      expect((await p.fetch(store, forbidden)).status).toBe(403);
      expect((await counts('forgerock')).refresh ?? 0).toBe(before);
    });
  });

  test('a refresh answered 429 or 5xx keeps the session, whatever its body says', async () => {
    for (const status of [429, 503]) {
      const p = oauth(configFor('spec')), store = memory();
      await p.signIn(store);
      await control({ action: 'expire-access', profile: 'spec' });
      const e = await withFetch((url, init) => (refreshing(url, init) ? Promise.resolve(Response.json({ error: 'invalid_grant' }, { status })) : undefined),
        () => p.json(store, `${origin}/p/spec/api/me`).catch((x) => x));
      expect([e.code, e.status, e.retryable]).toEqual(['server', status, true]);
      expect((await p.session(store)).signedIn).toBe(true);
    }
  });

  test('a failed refresh tells its status only when it is 400 or over: a 503 cut off keeps it, an unusable 200 is 0', async () => {
    const events: OAuthEvent[] = [];
    let heard = (): void => undefined;
    const p = oauth({ ...configFor('spec'), log: (e) => { events.push(e); heard(); } }), store = memory();
    const failure = async (reply: () => Response): Promise<{ code: string; status: number; retryable: boolean }> => {
      await control({ action: 'expire-access', profile: 'spec' });
      const said = new Promise<void>((done) => { heard = () => { if (events.at(-1)?.type === 'refresh-failed') done(); }; });
      const e = await withFetch((url, init) => (refreshing(url, init) ? Promise.resolve(reply()) : undefined), () => p.json(store, `${origin}/p/spec/api/me`).catch((x) => x));
      await said;
      return e;
    };
    await p.signIn(store);
    const cut = (): Response => new Response(new ReadableStream({ start(c) { c.error(new TypeError('cut off')); } }), { status: 503 });
    expect(await failure(cut)).toMatchObject({ code: 'server', status: 503, retryable: true });
    expect(events.at(-1)).toMatchObject({ type: 'refresh-failed', status: 503, code: 'server' });
    expect(await failure(() => Response.json({ token_type: 'Bearer' }))).toMatchObject({ code: 'invalid_response' });
    expect(events.at(-1)).toMatchObject({ type: 'refresh-failed', status: 0, code: 'invalid_response' });
    expect((await p.session(store)).signedIn).toBe(true);
  });

  test('an issuer with fixed endpoints makes no discovery, and its ID tokens are still checked against it', async () => {
    const sent: string[] = [], b = `${origin}/p/forgerock`;
    const endpoints = { authorization: `${b}/authorize`, token: `${b}/token`, revocation: `${b}/revoke` };
    const transport = (url: string, init: RequestInit): Promise<Response> => { sent.push(`${init.method ?? 'GET'} ${new URL(url).pathname}`); return fetch(url, init); };
    const p = oauth({ ...configFor('forgerock'), endpoints, fetch: transport }), store = memory();
    expect((await p.signIn(store, { authorize: withSession(await tree()) })).ok).toBe(true);
    await p.signOut(store);
    expect(sent).toEqual(['POST /p/forgerock/authorize', 'POST /p/forgerock/token', 'POST /p/forgerock/revoke']);
    const other = oauth({ ...configFor('forgerock'), issuer: `${b}/other`, endpoints });
    expect(await other.signIn(memory(), { authorize: withSession(await tree()) })).toMatchObject({ ok: false, code: 'invalid_response' });
    // A callback's `iss` is checked against the issuer too: another server's is refused, the issuer's taken
    const named = async (iss: string) => withSession(await tree(), (l) => `${l}&iss=${encodeURIComponent(iss)}`);
    expect(await p.signIn(memory(), { authorize: await named('https://other.example') })).toMatchObject({ ok: false, code: 'issuer_mismatch' });
    expect(await p.signIn(memory(), { authorize: await named(configFor('forgerock').issuer!) })).toMatchObject({ ok: true });
  });

  test('fixed endpoints with no revocation revoke nothing, and a template issuer still runs discovery', async () => {
    const sent: string[] = [], b = `${origin}/p/forgerock`;
    const transport = (url: string, init: RequestInit): Promise<Response> => { sent.push(`${init.method ?? 'GET'} ${new URL(url).pathname}`); return fetch(url, init); };
    const p = oauth({ ...configFor('forgerock'), endpoints: { authorization: `${b}/authorize`, token: `${b}/token` }, fetch: transport }), store = memory();
    expect((await p.signIn(store, { authorize: withSession(await tree()) })).ok).toBe(true);
    expect((await p.signOut(store)).ok).toBe(true);
    expect(sent).toEqual(['POST /p/forgerock/authorize', 'POST /p/forgerock/token']);
    // A template issuer's endpoints, as its discovery document names them: discovery still runs
    const entra = configFor('entra'), doc = await (await fetch(`${entra.issuer}/.well-known/openid-configuration`)).json() as Record<string, string>;
    sent.length = 0;
    const t = oauth({ ...entra, endpoints: { authorization: doc.authorization_endpoint, token: doc.token_endpoint }, fetch: transport });
    expect((await t.signIn(memory())).ok).toBe(true);
    expect(sent[0]).toMatch(/^GET .*\/\.well-known\/openid-configuration$/);
  });

  test("a refusal of a token the session has since replaced is retried with the newer one, though that token's refusals refresh no more", async () => {
    const p = oauth(configFor('forgerock')), store = memory(), forbidden = `${origin}/p/forgerock/api/forbidden`;
    await p.signIn(store);
    // Refused right after its refresh: this token's refusals refresh no more
    expect((await p.fetch(store, forbidden)).status).toBe(403);
    const before = (await counts('forgerock')).refresh ?? 0;
    let release = (): void => undefined, sent = (): void => undefined;
    const held = new Promise<void>((done) => { release = done; }), going = new Promise<void>((done) => { sent = done; });
    const tokens: string[] = [];
    // The read with that token is refused once the person has signed in again; one with the new token is answered
    const answer = (url: string, init: RequestInit | undefined): Promise<Response> | undefined => {
      if (url !== forbidden) return undefined;
      tokens.push(new Headers(init?.headers).get('authorization') ?? '');
      if (tokens.length > 1) return Promise.resolve(new Response('{}', { status: 200 }));
      sent();
      return held.then(() => new Response(null, { status: 403, headers: { Date: new Date().toUTCString() } }));
    };
    await withFetch(answer, async () => {
      const read = p.fetch(store, forbidden);
      await going;
      await p.signIn(store);
      release();
      expect((await read).status).toBe(200);
    });
    expect(tokens).toHaveLength(2);
    expect(tokens[1]).not.toBe(tokens[0]);
    expect((await counts('forgerock')).refresh ?? 0).toBe(before);
  });

  test('a token whose expiry is unknown, refused right after its refresh, refreshes again a minute after that refusal', async () => {
    const p = oauth({ ...configFor('salesforce'), expiredStatus: [401, 403] }), store = memory(), forbidden = `${origin}/p/salesforce/api/forbidden`;
    await p.signIn(store);
    expect((await p.session(store)).expiresAt).toBe(0);
    const at = Date.UTC(2026, 9, 29, 12);
    const refusedAt = (when: number) => (url: string) => (url === forbidden ? Promise.resolve(new Response(null, { status: 403, headers: { Date: new Date(when).toUTCString() } })) : undefined);
    const refreshes = async (): Promise<number> => (await counts('salesforce')).refresh ?? 0;
    const before = await refreshes();
    await withFetch(refusedAt(at), () => p.fetch(store, forbidden));
    expect(await refreshes()).toBe(before + 1);
    await withFetch(refusedAt(at + 59_000), () => p.fetch(store, forbidden));
    expect(await refreshes()).toBe(before + 1);
    await withFetch(refusedAt(at + 60_000), () => p.fetch(store, forbidden));
    expect(await refreshes()).toBe(before + 2);
  });

  test('a token refused right after its refresh by an API whose Date the app cannot read refreshes once for each read', async () => {
    const p = oauth(configFor('forgerock')), store = memory(), forbidden = `${origin}/p/forgerock/api/forbidden`;
    await p.signIn(store);
    const undated = (url: string) => (url === forbidden ? Promise.resolve(new Response(null, { status: 403 })) : undefined);
    const refreshes = async (): Promise<number> => (await counts('forgerock')).refresh ?? 0;
    const before = await refreshes();
    await withFetch(undated, () => p.fetch(store, forbidden));
    expect(await refreshes()).toBe(before + 1);
    await withFetch(undated, () => p.fetch(store, forbidden));
    expect(await refreshes()).toBe(before + 2);
  });

  test('a token reply whose expires_in is 0 expires at its Date', async () => {
    const p = oauth(configFor('spec')), store = memory(), at = Math.floor(Date.now() / 1000) * 1000;
    await withFetch((url, init, real) => (url.endsWith('/token') && init?.method === 'POST'
      ? dated(real(url, init).then(async (r) => new Response(JSON.stringify({ ...await r.json() as object, expires_in: 0 }), { status: r.status })), at)
      : undefined), () => p.signIn(store));
    expect((await p.session(store)).expiresAt).toBe(at);
    // Refreshed with that lifetime and refused at once, at that same `Date`: the token has expired, so it refreshes again
    const forbidden = `${origin}/p/spec/api/forbidden`, before = (await counts('spec')).refresh ?? 0;
    await withFetch((url, init, real) => (url.endsWith('/token') && init?.method === 'POST'
      ? dated(real(url, init).then(async (r) => new Response(JSON.stringify({ ...await r.json() as object, expires_in: 0 }), { status: r.status })), at)
      : url === forbidden ? Promise.resolve(new Response(null, { status: 401, headers: { Date: new Date(at).toUTCString() } })) : undefined), async () => {
      await p.fetch(store, forbidden);
      await p.fetch(store, forbidden);
    });
    expect((await counts('spec')).refresh ?? 0).toBe(before + 2);
  });
});

describe('what the library asks of the host', () => {
  test('HTTP Basic sends the client secret in the Authorization header only, an empty one too', async () => {
    for (const secret of ['fixture-secret', '']) {
      const sent: { authorization: string; body: string }[] = [];
      const r = await withFetch((url, init) => {
        if (url.endsWith('/token')) sent.push({ authorization: new Headers(init?.headers).get('authorization') ?? '', body: String(init?.body) });
        return undefined;
      }, () => oauth({ ...configFor('spec'), clientSecret: secret, tokenAuth: 'basic' }).signIn(memory()));
      expect(r.ok).toBe(true);
      expect(sent.length).toBe(1);
      expect(sent[0].authorization).toBe(`Basic ${btoa(`client-spec:${secret}`)}`);
      expect(sent[0].body).not.toContain('client_secret');
      expect(sent[0].body).not.toContain('client_id');
    }
  });

  test('the code exchange, a refresh and revocation ask the host for a 60 s deadline', async () => {
    const deadlines: unknown[] = [], p = oauth(configFor('spec')), store = memory();
    const record = (url: string, init?: RequestInit): undefined => {
      if (url.endsWith('/token') || url.endsWith('/revoke')) deadlines.push((init as { exactTimeout?: number })?.exactTimeout);
      return undefined;
    };
    await withFetch(record, () => p.signIn(store));
    let refused = false;
    await withFetch((url, init) => (url.endsWith('/api/me') && !refused ? ((refused = true), Promise.resolve(new Response(null, { status: 401 }))) : record(url, init)),
      () => p.fetch(store, `${origin}/p/spec/api/me`));
    await withFetch(record, () => p.signOut(store));
    expect(deadlines).toEqual([60_000, 60_000, 60_000]);
  });

  test('a 401 that persists after the refresh is answered, after one refresh', async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const r = await withFetch((url) => (url.endsWith('/api/me') ? Promise.resolve(new Response(null, { status: 401 })) : undefined),
      () => p.fetch(store, `${origin}/p/spec/api/me`));
    expect(r.status).toBe(401);
    expect((await counts('spec')).refresh).toBe(1);
  });

  test("an API read the grants refuse is the app's to fix, not the network's", async () => {
    const p = oauth(configFor('spec')), store = memory();
    await p.signIn(store);
    const e = await withFetch((url) => (url.includes('/api/') ? Promise.reject(new TypeError("outside the app's grants (net.fetch)")) : undefined),
      () => p.fetch(store, `${origin}/p/spec/api/me`).catch((x) => x));
    expect([e.code, e.retryable]).toEqual(['config', false]);
    expect(e.message).toContain(`add \`net.fetch ${origin}\` to grants`);
  });
});

// What `index.d.ts` declares is what `index.ts` implements, as the type checker sees the two (`types.ts`).
test('the declared API is the one implemented', () => {
  const checked = Bun.spawnSync([`${import.meta.dir}/../../../node_modules/.bin/tsc`, '--noEmit', '--strict', '--allowImportingTsExtensions', '--module', 'preserve', 'test/types.ts'], { cwd: `${import.meta.dir}/..` });
  expect(checked.exitCode, `${checked.stdout}${checked.stderr}`).toBe(0);
}, 60_000);
