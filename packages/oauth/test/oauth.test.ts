// @exact/oauth against the fixture's provider profiles, in Bun (LLP 1146
// D6): the protocol alone, with `openAuthSession` standing in for the
// browser (it follows the authorization URL to its redirect). What Exact adds
// (a let-go answer, the store asking readers again) is tested by the cases
// in an app (`cases.template.contract`).
import { afterAll, beforeAll, beforeEach, describe, expect, test } from 'bun:test';
import { handle, PROFILES } from './fixture.ts';
import { oauth, type OAuthConfig, type OAuthStore } from '../index.ts';

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
    expect(r).toEqual({ ok: true, code: '', error: '', retryable: false });
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
      expect((await oauth(configFor('spec')).session(memory())).error).toBe('Sign-in cannot be prepared: authCallback(): the grants name no auth.callback');
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
    expect((await p.session(store)).error).toBe('The provider asks you to sign in again.');
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

  test('a token request asks the host for a 60 s deadline', async () => {
    const deadlines: unknown[] = [];
    await withFetch((url, init) => { if (url.endsWith('/token')) deadlines.push((init as { exactTimeout?: number })?.exactTimeout); return undefined; },
      () => oauth(configFor('spec')).signIn(memory()));
    expect(deadlines).toEqual([60_000]);
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
