// The auth fixture's source (LLP 1069.006 D6): the whole AT Protocol-style
// OAuth client, as a source writes it with `fetch`, `secret.keep`, Web
// Crypto (LLP 1069.005) and the two bindings, against the local fixture
// server (`fixture.mjs`) only. Discovery, the DPoP key and PAR happen when
// the app is prepared; the press calls `openAuthSession` at once (on the web
// its popup must open in the press's call stack); the answer checks `state`
// and `iss`, exchanges the code with the PKCE verifier and a DPoP proof,
// checks the account, and keeps the tokens. The transaction is single-use
// with an expiry, and never overwrites a signed-in session's key.
//
// Passkeys (LLP 1133.011 §10): each ceremony fetches its options from the
// fixture's relying party in the mutation that runs it (a challenge is
// single-use, D9), and posts the response back; the plan sees `{ok,
// message}` only, never a response, a challenge or a signature.
//
// Sign-in (LLP 1133.014 §10): a single-use nonce from the server when the
// screen shows, kept here; at a press `get` is called with nothing awaited
// before it (on the web the provider's button holds the popup's window for
// it), the credential posted to the server without its handle, and a fresh
// nonce fetched whatever the outcome. The plan sees the account's name only.
//
// "Check this build" (LLP 1133.028 §8.1): a challenge from the server,
// `attest` over it, and the server's verdict, which is all the plan sees;
// the token never leaves this file (D2).
import type { Answer, Store } from './app.contract.d.ts';
import { attest, resetAttestation } from '@exact/attestation';
import { credentials, PublicKeyCredential } from '@exact/passkeys';
import { preventSilentAccess } from '@exact/sign-in';

export const appId = 'com.exact.authfixture';
const ISSUER = 'http://127.0.0.1:4331';
const ORIGIN = 'https://auth-fixture.exact.test';
// The passkey relying party's server: the fixture server, unless the bake selects the fixture's public origin
// (LLP 1133.011 §13 question 4) with `EXACT_BACKEND_ORIGIN=https://exact-auth-fixture.expo.app`, whose host is
// then the RP ID. Sign-in and the attestation checks use the fixture server either way.
export const origin = 'http://127.0.0.1:4331';
export const grants = {
  'http://127.0.0.1:4331': ['net.fetch http://127.0.0.1:4331', 'auth.session http://127.0.0.1:4331',
    // The native callback: the client id's hostname reversed (AT Protocol).
    'auth.callback test.exact.auth-fixture:/oauth', 'auth.callback https://auth-fixture.exact.test/.exact/auth/callback',
    // The desktop's loopback callback, at any port (LLP 1133.005 D9).
    'auth.callback http://127.0.0.1/oauth/loopback',
    'secret.keep fixture.session', 'secret.keep fixture.pending', 'secret.keep fixture.dpop', 'secret.keep fixture.passkeys',
    'secret.keep fixture.pending-second', 'secret.keep fixture.dpop-second',
    // The fixture's relying party, and the web drive's (the page is a loopback one).
    'auth.passkey auth-fixture.exact.test', 'auth.passkey localhost', 'device.attestation',
    // The fixture's sign-in clients (LLP 1133.014 §13 question 6 names the real ones).
    'auth.apple com.exact.authfixture.web', 'auth.google fixture-web.apps.googleusercontent.com', 'auth.google fixture-ios.apps.googleusercontent.com'],
  'https://exact-auth-fixture.expo.app': ['net.fetch http://127.0.0.1:4331', 'auth.session http://127.0.0.1:4331',
    'net.fetch https://exact-auth-fixture.expo.app',
    'auth.callback test.exact.auth-fixture:/oauth', 'auth.callback https://auth-fixture.exact.test/.exact/auth/callback',
    // The desktop's loopback callback, at any port (LLP 1133.005 D9).
    'auth.callback http://127.0.0.1/oauth/loopback',
    'secret.keep fixture.session', 'secret.keep fixture.pending', 'secret.keep fixture.dpop', 'secret.keep fixture.passkeys',
    'secret.keep fixture.pending-second', 'secret.keep fixture.dpop-second',
    'auth.passkey exact-auth-fixture.expo.app', 'auth.passkey localhost', 'device.attestation',
    'auth.apple com.exact.authfixture.web', 'auth.google fixture-web.apps.googleusercontent.com', 'auth.google fixture-ios.apps.googleusercontent.com'],
};
// The Google Cloud project Play Integrity bills (LLP 1133.020 §3.2);
// unused on Apple.
const CLOUD_PROJECT = '000000000000';
// The RP ID: on the web the page's host, `localhost` for any loopback page; natively the relying party's
// host when it is public, else the app's origin's.
const rpId = () => {
  const page = /^https?:\/\/([^/:]+)/.exec(authCallback());
  if (page) return page[1] === '127.0.0.1' ? 'localhost' : page[1];
  return /^https:\/\//.test(origin) ? origin.slice('https://'.length) : 'auth-fixture.exact.test';
};
// One client id per application type (after review, item 3).
const CLIENT = {
  native: `${ORIGIN}/.exact/auth/client-metadata.native.json`,
  web: `${ORIGIN}/.exact/auth/client-metadata.web.json`,
};
const EXPECTED_DID = 'did:plc:fixture';
const P256 = { name: 'ECDSA', namedCurve: 'P-256' } as const;

const encoder = new TextEncoder();
function b64url(bytes: ArrayBuffer | Uint8Array): string {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let text = '';
  for (let i = 0; i < view.length; i++) text += String.fromCharCode(view[i]);
  return btoa(text).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}
const json64 = (value: unknown) => b64url(encoder.encode(JSON.stringify(value)));
const random64 = (n: number) => b64url(crypto.getRandomValues(new Uint8Array(n)));
const sha64 = async (text: string) => b64url(await crypto.subtle.digest('SHA-256', encoder.encode(text)));
const form = (fields: Record<string, string>) =>
  Object.entries(fields).map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(v)}`).join('&');

// An ES256 DPoP proof (RFC 9449): the public JWK in the header; `jti`,
// `htm`, `htu`, `iat` from the app's clock, the server's nonce, and `ath`
// on a resource request.
async function proof(pair: CryptoKeyPair, htm: string, htu: string, nowMs: number, nonce?: string, ath?: string) {
  const { kty, crv, x, y } = await crypto.subtle.exportKey('jwk', pair.publicKey);
  const input = `${json64({ typ: 'dpop+jwt', alg: 'ES256', jwk: { kty, crv, x, y } })}.${json64({
    jti: crypto.randomUUID(), htm, htu, iat: Math.floor(nowMs / 1000), ...(nonce ? { nonce } : {}), ...(ath ? { ath } : {}),
  })}`;
  const signature = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, pair.privateKey, encoder.encode(input));
  return `${input}.${b64url(signature)}`;
}

// A request with a DPoP proof; a `use_dpop_nonce` answer retries once with
// the server's nonce (each server's nonce kept apart: it is asked per call).
async function dpopFetch(pair: CryptoKeyPair, url: string, init: { method: string; headers?: Record<string, string>; body?: string }, nowMs: number, ath?: string) {
  let nonce: string | undefined;
  for (let attempt = 0; attempt < 2; attempt++) {
    const headers = { ...(init.headers ?? {}), DPoP: await proof(pair, init.method, url, nowMs, nonce, ath) };
    const response = await fetch(url, { ...init, headers });
    if (response.status === 400 || response.status === 401) {
      const body = await response.text();
      const next = response.headers.get('dpop-nonce');
      if (attempt === 0 && next && body.includes('use_dpop_nonce')) { nonce = next; continue; }
      throw new Error(`${url}: ${response.status}`);
    }
    return response;
  }
  throw new Error(`${url}: no nonce accepted`);
}

interface Pending {
  state: string; verifier: string; issuer: string; clientId: string; callback: string;
  tokenEndpoint: string; authorize: string; requestUri: string; expectedDid: string; expires: number;
}

const session = (store: Store) => {
  const kept = store.get('fixture.session');
  if (!kept) return { signedIn: false, handle: '' };
  return { signedIn: true, handle: String(JSON.parse(kept).handle) };
};

// A desktop's loopback callback on a port the client chose, as Codex's
// ChatGPT sign-in uses 1455 (LLP 1133.005 D9). A second sign-in waits beside
// the first on its own port, with its own transaction and key.
const LOOPBACK_CALLBACK = 'http://127.0.0.1:4332/oauth/loopback';
const SECOND_CALLBACK = 'http://127.0.0.1:4333/oauth/loopback';
const keys = (second: boolean) => second
  ? { pending: 'fixture.pending-second', dpop: 'fixture.dpop-second' }
  : { pending: 'fixture.pending', dpop: 'fixture.dpop' };

async function prepareSignIn(store: Store, nowMs: number, loopback = false, second = false) {
  if (store.get('fixture.session')) return { ok: false, message: 'Already signed in' };
  const meta = await (await fetch(`${ISSUER}/.well-known/oauth-authorization-server`)).json();
  if (meta.issuer !== ISSUER) return { ok: false, message: 'The server names another issuer' };
  const pair = await crypto.subtle.generateKey(P256, false, ['sign', 'verify']) as CryptoKeyPair;
  await store.keepKey(keys(second).dpop, pair);
  const state = random64(32), verifier = random64(32);
  const callback = second ? SECOND_CALLBACK : loopback ? LOOPBACK_CALLBACK : authCallback();
  const clientId = callback.endsWith('/.exact/auth/callback') && /^https?:\/\//.test(callback) ? CLIENT.web : CLIENT.native;
  const par = await dpopFetch(pair, meta.pushed_authorization_request_endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: form({
      client_id: clientId, redirect_uri: callback, state, response_type: 'code', scope: 'atproto',
      code_challenge: await sha64(verifier), code_challenge_method: 'S256', login_hint: 'fixture.test',
    }),
  }, nowMs);
  if (!par.ok) return { ok: false, message: `PAR failed (${par.status})` };
  const { request_uri: requestUri } = await par.json();
  const pending: Pending = {
    state, verifier, issuer: meta.issuer, clientId, callback, tokenEndpoint: meta.token_endpoint,
    authorize: meta.authorization_endpoint, requestUri, expectedDid: EXPECTED_DID, expires: nowMs + 5 * 60_000,
  };
  store.set(keys(second).pending, JSON.stringify(pending));
  return { ok: true, message: 'Ready to sign in' };
}

async function signIn(store: Store, nowMs: number, second = false) {
  const key = keys(second);
  const kept = store.get(key.pending);
  if (!kept) return { ok: false, message: 'Prepare first' };
  const pending = JSON.parse(kept) as Pending;
  if (nowMs > pending.expires) { store.forget(key.pending); return { ok: false, message: 'Expired: prepare again' }; }
  const url = `${pending.authorize}?${form({ client_id: pending.clientId, request_uri: pending.requestUri })}`;
  let back: string;
  try {
    back = await openAuthSession(url, { callback: pending.callback, state: pending.state });
  } catch (e: any) {
    return { ok: false, message: e.status === 499 ? 'Sign-in cancelled' : `Sign-in failed (${e.status ?? 'error'})` };
  }
  // Single use: the transaction is spent whatever the callback says.
  store.forget(key.pending);
  const query = back.slice(back.indexOf('?') + 1).split('#')[0].split('&').map(p => p.split('=').map(decodeURIComponent));
  const one = (name: string) => { const all = query.filter(([k]) => k === name); return all.length === 1 ? all[0][1] : null; };
  if (one('error')) return { ok: false, message: `The server refused: ${one('error')}` };
  if (one('state') !== pending.state) return { ok: false, message: 'The callback is not this sign-in' };
  if (one('iss') !== pending.issuer) return { ok: false, message: 'The callback names another issuer' };
  const code = one('code');
  if (!code) return { ok: false, message: 'The callback has no code' };
  const pair = await store.key(key.dpop);
  if (!pair) return { ok: false, message: 'The DPoP key is gone: prepare again' };
  const token = await dpopFetch(pair, pending.tokenEndpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/x-www-form-urlencoded' },
    body: form({ grant_type: 'authorization_code', code, code_verifier: pending.verifier, redirect_uri: pending.callback, client_id: pending.clientId }),
  }, nowMs);
  if (!token.ok) return { ok: false, message: `The code exchange failed (${token.status})` };
  const tokens = await token.json();
  if (tokens.token_type !== 'DPoP' || tokens.sub !== pending.expectedDid) return { ok: false, message: 'The token is for another account' };
  const who = await dpopFetch(pair, `${ISSUER}/xrpc/whoami`, {
    method: 'GET', headers: { authorization: `DPoP ${tokens.access_token}` },
  }, nowMs, await sha64(tokens.access_token));
  if (!who.ok) return { ok: false, message: `The resource refused the token (${who.status})` };
  const { handle, did } = await who.json();
  if (did !== pending.expectedDid) return { ok: false, message: 'The resource names another account' };
  // The tokens stay in the store: the plan sees `{ok, message}` only.
  store.set('fixture.session', JSON.stringify({ handle, did, access: tokens.access_token, refresh: tokens.refresh_token }));
  return { ok: true, message: `Signed in as ${handle}` };
}

async function post(server: string, path: string, body: unknown) {
  const response = await fetch(`${server}${path}`, {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  });
  return response.json();
}
const relyingParty = (path: string, body: unknown) => post(origin, path, body);

// The relying party keeps nothing: each passkey it registered is a record it signed, which the app keeps
// (`fixture.passkeys`) and sends back. Dropping a record is the server forgetting that passkey.
interface Kept { id: string; record: string }
const kept = (store: Store): Kept[] => JSON.parse(store.get('fixture.passkeys') ?? '[]');
const keep = (store: Store, list: Kept[]) => store.set('fixture.passkeys', JSON.stringify(list));
const records = (store: Store) => kept(store).map(k => k.record);

// What a hand-built app shows for a ceremony's failure: nothing for a
// cancel or a request a newer one replaced, the line for a passkey the
// account already has, else the platform's message.
function failed(e: any) {
  const code = e?.code ?? e?.name;
  if (code === 'InvalidStateError') return { ok: false, message: 'You already have a passkey for this account' };
  if (code === 'NotAllowedError' || code === 'AbortError') return { ok: false, message: '' };
  return { ok: false, message: `Passkeys are unavailable (${code ?? 'error'})` };
}

// The server's list after a sign-in, so the credential manager hides a
// passkey the server forgot (D11).
async function signedInAs(store: Store, done: { userId: string; handle: string }, credentialId: string) {
  store.set('fixture.session', JSON.stringify({ handle: done.handle, did: done.userId, passkey: credentialId }));
  const { ids } = await relyingParty('/passkey/accepted', { userId: done.userId, records: records(store) });
  await PublicKeyCredential.signalAllAcceptedCredentials({ rpId: rpId(), userId: done.userId, allAcceptedCredentialIds: ids });
  return { ok: true, message: `Signed in as ${done.handle}` };
}

async function createPasskey(store: Store, username: string) {
  const session = store.get('fixture.session');
  const name = session ? String(JSON.parse(session).handle) : username.trim() || 'fixture.test';
  const { publicKey } = await relyingParty('/passkey/register/options', { username: name, rpId: rpId(), records: records(store) });
  let response;
  try { response = await credentials.create({ publicKey }); } catch (e) { return failed(e); }
  const done = await relyingParty('/passkey/register', { response });
  if (!done.ok) return { ok: false, message: `The server refused the passkey (${done.error})` };
  keep(store, [...kept(store), { id: done.credentialId, record: done.record }]);
  return signedInAs(store, done, done.credentialId);
}

// Conditional requests in a row that ended in an error other than AbortError
// (a newer ceremony replacing one). A platform that refuses every one would
// otherwise have the sign-in screen ask again as fast as it answers.
let refusedSuggestions = 0;

async function passkeySignIn(store: Store, mediation?: 'conditional') {
  const { publicKey } = await relyingParty('/passkey/sign-in/options', { rpId: rpId() });
  let response;
  try { response = await credentials.get({ publicKey, mediation }); } catch (e) {
    const code = (e as any)?.code ?? (e as any)?.name;
    refusedSuggestions = mediation === 'conditional' && code !== 'AbortError' ? refusedSuggestions + 1 : 0;
    if (refusedSuggestions >= 3) {
      const why = (e as any)?.message ? `: ${(e as any).message}` : '';
      return { ok: false, message: `Passkey suggestions are off (${code ?? 'error'}${why})` };
    }
    return failed(e);
  }
  refusedSuggestions = 0;
  const list = kept(store), at = list.findIndex(k => k.id === response.id);
  const done = await relyingParty('/passkey/sign-in', { response, record: at < 0 ? null : list[at].record });
  if (done.error === 'unknown-credential') {
    // A passkey whose registration the server never kept: the credential manager forgets it.
    if (at >= 0) keep(store, list.filter((_, i) => i !== at));
    await PublicKeyCredential.signalUnknownCredential({ rpId: rpId(), credentialId: response.id });
    return { ok: false, message: 'That passkey is not for an account here' };
  }
  if (!done.ok) return { ok: false, message: `The server refused the sign-in (${done.error})` };
  // The record with the sign count this assertion carried.
  keep(store, list.map((k, i) => (i === at ? { id: k.id, record: done.record } : k)));
  return signedInAs(store, done, response.id);
}

// The server forgets the passkey this session signed in with (drive 9's signal): its next use is unknown.
function forgetPasskey(store: Store) {
  const session = store.get('fixture.session');
  const id = session ? JSON.parse(session).passkey : undefined;
  if (typeof id !== 'string') return { ok: false, message: 'Not signed in with a passkey' };
  keep(store, kept(store).filter(k => k.id !== id));
  return { ok: true, message: 'The server forgot this passkey' };
}

const errorName = (e: unknown) => String((e as any)?.name ?? (e as any)?.code ?? 'Error');

// One attestation (or, with a key the server holds, an assertion) over a
// fresh challenge, and what the server concluded. A `NetworkError` keeps
// the key, so the same challenge is tried once more (D3).
async function proveOnce(account: string | undefined) {
  const { challenge } = await post(ISSUER, '/attest/challenge', {});
  const ask = () => attest({ clientData: challenge, account, android: { cloudProjectNumber: CLOUD_PROJECT } });
  let proof, failure = '';
  try { proof = await ask(); } catch (e) {
    if (errorName(e) !== 'NetworkError') return { ok: false, unknown: false, message: `Failed: ${errorName(e)}` };
    failure = `Failed: NetworkError; retried. `;
    try { proof = await ask(); } catch (again) { return { ok: false, unknown: false, message: `${failure}Failed: ${errorName(again)}` }; }
  }
  const verdict = await post(ISSUER, '/attest/verify', { ...proof, clientData: challenge });
  return { ok: verdict.ok === true, unknown: verdict.error === 'unknown-key', message: failure + String(verdict.message) };
}

// On "unknown key" the key is forgotten here too and a new one attested.
async function checkBuild(store: Store) {
  const kept = store.get('fixture.session');
  const account = kept ? String(JSON.parse(kept).handle) : undefined;
  const first = await proveOnce(account);
  if (!first.unknown) return { ok: first.ok, message: first.message };
  await resetAttestation({ account });
  const again = await proveOnce(account);
  return { ok: again.ok, message: `Unknown key; reset. ${again.message}` };
}

const PROVIDERS = {
  apple: { configURL: 'https://appleid.apple.com', clientId: 'com.exact.authfixture.web' },
  google: { configURL: 'https://accounts.google.com/gsi/fedcm.json', clientId: 'fixture-web.apps.googleusercontent.com',
    apple: { clientId: 'fixture-ios.apps.googleusercontent.com' } },
} as const;
const onWeb = () => /^https?:\/\//.test(authCallback());
// The nonce for the next request, and the client it was issued to.
let ready: { nonce: string; client: string } | null = null;

async function identityNonce() {
  ready = await post(ISSUER, '/sign-in/nonce', { client: ready?.client });
  return { ok: true, message: '' };
}

const provider = (name: 'apple' | 'google', nonce: string) => ({
  ...PROVIDERS[name], params: { nonce }, ...(onWeb() ? { web: { redirectURI: `${ISSUER}/sign-in/return` } } : {}),
});

// What a hand-built app shows for a sign-in's failure: nothing for a cancel or
// a request a newer one replaced, else the reason.
const signInFailed = (e: unknown) => {
  const code = errorName(e);
  return ['NetworkError', 'NotAllowedError', 'AbortError'].includes(code) ? { ok: false, message: '' } : { ok: false, message: `Sign-in failed (${code})` };
};

/** The server's account for a credential: the handle stays here. */
async function signedInWith(store: Store, credential: any, asked: { nonce: string; client: string }) {
  const { handle: _handle, ...posted } = credential;
  const done = await post(ISSUER, '/sign-in/identity', { credential: posted, client: asked.client, nonce: asked.nonce });
  if (!done.ok) return { ok: false, message: `The server refused the sign-in (${done.error})` };
  const name = done.name || done.email;
  store.set('fixture.session', JSON.stringify({ handle: name, provider: done.provider, sub: done.sub }));
  return { ok: true, message: `Signed in as ${name}` };
}

async function signInWith(store: Store, name: 'apple' | 'google') {
  const asked = ready;
  if (!asked) return { ok: false, message: 'Not ready: try again' };
  ready = null;
  try {
    let credential;
    try { credential = await credentials.get({ identity: { mode: 'active', providers: [provider(name, asked.nonce)] } }); }
    catch (e) { return signInFailed(e); }
    return await signedInWith(store, credential, asked);
  } finally { await identityNonce(); }
}

// A passkey or the target's passive provider, in one sheet where it offers one.
// Its nonce is its own, so a press while it is up still has the one kept for it.
async function offerSignIn(store: Store) {
  const asked: { nonce: string; client: string } = await post(ISSUER, '/sign-in/nonce', { client: ready?.client });
  const { publicKey } = await relyingParty('/passkey/sign-in/options', { rpId: rpId() });
  let credential: any;
  try {
    credential = await credentials.get({ publicKey, identity: { providers: [provider('apple', asked.nonce), provider('google', asked.nonce)] } });
  } catch (e) {
    const code = errorName(e);
    return code === 'NotSupportedError' ? { ok: false, message: 'No sign-in sheet here' } : signInFailed(e);
  }
  if (credential.type === 'identity') return await signedInWith(store, credential, asked);
  const list = kept(store), at = list.findIndex(k => k.id === credential.id);
  const done = await relyingParty('/passkey/sign-in', { response: credential, record: at < 0 ? null : list[at].record });
  if (!done.ok) return { ok: false, message: `The server refused the sign-in (${done.error})` };
  keep(store, list.map((k, i) => (i === at ? { id: k.id, record: done.record } : k)));
  return signedInAs(store, done, credential.id);
}

export const answer: Answer = (source, args, store) => {
  switch (source) {
    case 'currentSession': return session(store) as never;
    case 'prepareSignIn': return prepareSignIn(store, args[0] as number) as never;
    case 'prepareLoopbackSignIn': return prepareSignIn(store, args[0] as number, true) as never;
    case 'signIn': return signIn(store, args[0] as number) as never;
    case 'prepareSecondSignIn': return prepareSignIn(store, args[0] as number, true, true) as never;
    case 'signInSecond': return signIn(store, args[0] as number, true) as never;
    case 'createPasskey': return createPasskey(store, args[0] as string) as never;
    case 'signInWithPasskey': return passkeySignIn(store) as never;
    case 'suggestPasskey': return passkeySignIn(store, 'conditional') as never;
    case 'forgetPasskey': return forgetPasskey(store) as never;
    case 'passkeyIdle': return { ok: true, message: '' } as never;
    case 'checkBuild': return checkBuild(store) as never;
    case 'identityNonce': return identityNonce() as never;
    case 'signInWith': return signInWith(store, args[0] as 'apple' | 'google') as never;
    case 'offerSignIn': return offerSignIn(store) as never;
    case 'signOut': return (async () => {
      // The next sign-in may not happen without UI (LLP 1133.014 D1).
      await preventSilentAccess();
      for (const key of ['fixture.session', 'fixture.pending', 'fixture.dpop', 'fixture.pending-second', 'fixture.dpop-second']) store.forget(key);
      return { ok: true, message: 'Signed out' };
    })() as never;
    default: throw new Error(`unknown source ${source}`);
  }
};
