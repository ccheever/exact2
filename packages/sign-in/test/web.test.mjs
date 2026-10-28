// LLP 1133.014 §11 stage 1's hermetic proofs of the web cell: the providers'
// authorization URLs, the callback's answer, the button's window opened at
// the press before `press` is delivered and closed when no request claims
// it, the host glue adopting it, and the fixture server's return route.
//
//   bun test packages/sign-in/test/web.test.mjs
import { afterAll, beforeAll, expect, test } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import { answered, authorizationURL, implementation, roster } from '../web/index.js';
import { respond } from './idp.mjs';

const APPLE = 'https://appleid.apple.com', GOOGLE = 'https://accounts.google.com/gsi/fedcm.json';
const back = 'https://api.example/sign-in/return';

test("Apple's URL: the Services ID, the return route, form_post, the scope from fields, state and nonce", () => {
  const u = new URL(authorizationURL({ configURL: APPLE, clientId: 'com.example.web', params: { nonce: 'n1' }, web: { redirectURI: back } }, 's1'));
  expect(u.origin + u.pathname).toBe('https://appleid.apple.com/auth/authorize');
  expect(Object.fromEntries(u.searchParams)).toEqual({ client_id: 'com.example.web', redirect_uri: back, response_type: 'code id_token',
    response_mode: 'form_post', scope: 'name email', state: 's1', nonce: 'n1' });
  const none = new URL(authorizationURL({ configURL: APPLE, clientId: 'c', fields: [], web: { redirectURI: back } }, 's'));
  expect(none.searchParams.has('scope')).toBe(false);
});

test("Google's URL: the web client, the code flow, hints, and a code_challenge only when params carries one", () => {
  const p = { configURL: GOOGLE, clientId: 'w.apps.googleusercontent.com', loginHint: 'a@b.c', domainHint: 'b.c', params: { nonce: 'n2' }, web: { redirectURI: back } };
  const u = new URL(authorizationURL(p, 's2'));
  expect(u.origin + u.pathname).toBe('https://accounts.google.com/o/oauth2/v2/auth');
  expect(Object.fromEntries(u.searchParams)).toEqual({ client_id: 'w.apps.googleusercontent.com', redirect_uri: back, response_type: 'code',
    scope: 'openid email profile', state: 's2', nonce: 'n2', login_hint: 'a@b.c', hd: 'b.c' });
  const pkce = new URL(authorizationURL({ ...p, params: { nonce: 'n', code_challenge: 'abc' } }, 's'));
  expect(pkce.searchParams.get('code_challenge')).toBe('abc');
  expect(pkce.searchParams.get('code_challenge_method')).toBe('S256');
  expect(authorizationURL({ ...p, configURL: 'https://idp.example' }, 's')).toBe(null);
});

test('the callback answers {relay}, a cancellation NetworkError, any other error IdentityCredentialError', () => {
  const p = { configURL: GOOGLE };
  const ok = answered(p, 'https://app.example/.exact/auth/callback?state=s&relay=h1');
  expect(ok).toMatchObject({ type: 'identity', id: '', configURL: GOOGLE, token: { relay: 'h1' } });
  expect(typeof ok.handle).toBe('string');
  expect(answered(p, 'https://a/cb?state=s&error=access_denied').error).toBe('NetworkError');
  expect(answered({ configURL: APPLE }, 'https://a/cb?state=s&error=user_cancelled_authorize').error).toBe('NetworkError');
  expect(answered(p, 'https://a/cb?state=s&error=server_error').error).toBe('IdentityCredentialError');
});

test("the host session's statuses become D1's errors; the capabilities follow the grants", async () => {
  const statuses = { 499: ['NetworkError'], 409: ['NotAllowedError', 'busy'], 428: ['NotAllowedError', 'activation'] };
  for (const [status, [error, message]] of Object.entries(statuses)) {
    globalThis.location = { origin: 'https://app.example' };
    const impl = implementation({ grants: 'auth.google w', authSession: { open: async () => { throw Object.assign(new Error('x'), { status: Number(status) }); } } });
    const out = await impl.identity({ identity: { mode: 'active', providers: [{ configURL: GOOGLE, clientId: 'w', web: { redirectURI: back } }] } });
    expect(out.error).toBe(error);
    if (message) expect(out.message).toBe(message);
  }
  const caps = implementation({ grants: 'auth.google w\nnet.fetch https://api.example' }).getIdentityCapabilities();
  expect(caps.providers).toEqual({ apple: { active: false, passive: false, withPublicKey: false }, google: { active: true, passive: false, withPublicKey: false } });
});

/** A document with one element and a stub `window.open`. */
function page() {
  const opened = [];
  const el = () => ({ children: [], style: {}, attrs: {}, listeners: {}, append(...c) { this.children.push(...c); }, remove() {},
    setAttribute(k, v) { this.attrs[k] = v; }, addEventListener(t, f) { this.listeners[t] = f; } });
  const doc = { head: el(), createElement: el };
  const element = { ...el(), ownerDocument: doc };
  globalThis.window = { open: (url, name) => { const w = { url, name, closed: false, close() { this.closed = true; } }; opened.push(w); return w; } };
  globalThis.exact = {};
  return { element, opened };
}

test("the button opens the popup's window in its own click handler, leaving press to the page, and closes one no request claims", async () => {
  const { element, opened } = page();
  let pressed = 0;
  const events = { load() {}, press() { pressed++; } };
  const View = roster['exact-sign-in-button'].view;
  const button = new View({ implementation: { agent: false }, element, props: { provider: 'google' }, events });
  button.button.listeners.click();
  expect(pressed).toBe(0); // the click bubbles to the element, whose press the page delivers (rt.js onPress)
  expect(opened.length).toBe(1);
  expect(opened[0].url).toBe('about:blank');
  expect(globalThis.exact.authWaiting.window).toBe(opened[0]);
  await new Promise(r => setTimeout(r, 2100));
  expect(opened[0].closed).toBe(true);
  expect(globalThis.exact.authWaiting).toBe(null);
});

test('under the agent the button opens nothing', () => {
  const { element, opened } = page();
  const View = roster['exact-sign-in-button'].view;
  const button = new View({ implementation: { agent: true }, element, props: { provider: 'apple' }, events: { load() {} } });
  button.button.listeners.click();
  expect(opened).toEqual([]);
});

test('the host glue navigates the waiting window instead of opening one, and a request from another control opens its own', async () => {
  const { opened } = page();
  globalThis.navigator = { userActivation: { isActive: false } };
  globalThis.location = { origin: 'https://app.example' };
  globalThis.addEventListener = () => {}; globalThis.removeEventListener = () => {};
  const { arm } = await import(resolve(import.meta.dir, '../../../host/web/auth-glue.js'));
  const waiting = { window: { closed: false, location: { href: 'about:blank' }, close() {} }, claimed: false };
  globalThis.exact.authWaiting = waiting;
  const calls = [];
  const env = { agent: false, log() {}, active: () => true, deliver() {},
    call: req => { calls.push(req); return req.op === 'arm' ? { present: { url: 'https://accounts.google.com/o/oauth2/v2/auth?x=1', callback: 'https://app.example/.exact/auth/callback' } } : {}; } };
  arm({ ticket: 1 }, env);
  expect(calls[0].popup).toBe(true); // the waiting window stands for the activation
  expect(waiting.claimed).toBe(true);
  expect(waiting.window.location.href).toBe('https://accounts.google.com/o/oauth2/v2/auth?x=1');
  expect(opened).toEqual([]);
  // No window waiting and no activation: the glue reports the blocked popup (428), opening nothing.
  calls.length = 0;
  const env2 = { ...env, call: req => { calls.push(req); return req.op === 'arm' ? (req.popup === false ? { settled: true } : { present: {} }) : {}; } };
  arm({ ticket: 2 }, env2);
  expect(calls[0].popup).toBe(false);
  expect(opened).toEqual([]);
});

// ---------------------------------------------------------------- the fixture server's return route
let server, dir, keys;
const ISSUER = 'http://127.0.0.1:4339';
const post = (path, body) => fetch(`${ISSUER}${path}`, { method: 'POST', body: JSON.stringify(body) }).then(r => r.json());
beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'sign-in-web-test-')); keys = join(dir, 'keys.json');
  server = spawn(process.execPath, [resolve(import.meta.dir, '../../../apps/auth-fixture/fixture.mjs'), '--port', '4339', '--substitute', '--idp-keys', keys], { stdio: 'ignore' });
  for (let i = 0; i < 100; i++) { if (await fetch(`${ISSUER}/fixture/log`).then(r => r.ok, () => false)) break; await new Promise(r => setTimeout(r, 50)); }
  await post('/fixture/sign-in', { callback: 'https://app.example/.exact/auth/callback' });
});
afterAll(() => { server?.kill(); rmSync(dir, { recursive: true, force: true }); });

const ticket = (provider, nonce) => ({ identity: { providers: [{ provider, configURL: provider === 'apple' ? APPLE : GOOGLE, nonce,
  audience: provider === 'apple' ? 'com.exact.authfixture.web' : 'fixture-web.apps.googleusercontent.com', token: 'relay', redirectURI: `${ISSUER}/sign-in/return` }] } });

test('the return route answers only its configured callback, and its handle is single-use', async () => {
  for (const provider of ['apple', 'google']) {
    const { nonce, client } = await post('/sign-in/nonce', {});
    const { credential } = await respond(ticket(provider, nonce), keys, { first: true });
    const { handle: _, ...posted } = credential;
    expect(await post('/sign-in/identity', { credential: posted, client, nonce })).toMatchObject({ ok: true, provider });
    const again = await post('/sign-in/nonce', { client });
    expect(await post('/sign-in/identity', { credential: posted, client, nonce: again.nonce })).toEqual({ ok: false, error: 'handle' });
  }
  // A `redirect_uri`-like parameter in the request changes nothing: the 303 goes to the configured callback.
  const r = await fetch(`${ISSUER}/sign-in/return?state=s&error=access_denied&callback=https://evil.example/`, { redirect: 'manual' });
  expect(r.status).toBe(303);
  expect(r.headers.get('location')).toBe('https://app.example/.exact/auth/callback?state=s&error=access_denied');
});

test("a handle is refused to a client whose nonce differs", async () => {
  const mine = await post('/sign-in/nonce', {}), theirs = await post('/sign-in/nonce', {});
  const { credential } = await respond(ticket('google', mine.nonce), keys);
  expect(await post('/sign-in/identity', { credential, client: theirs.client, nonce: theirs.nonce })).toEqual({ ok: false, error: 'nonce' });
});
