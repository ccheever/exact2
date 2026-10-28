// The fixture identity provider for drives and tests (LLP 1133.014 §9),
// outside the runner, as passkeys' `authenticator.mjs` is: given a
// `credential` ticket's `args`, it signs an ID token with a fixture key it
// keeps in a file the drive names, with the provider's real `iss`, the
// args' audience and nonce, a fixture `sub` and email, and wraps it in the
// provider's token shape: Google's natively is the JWT, Apple's
// `{authorization: {code, id_token, state}, user?}` with `user` on a first
// authorization. For the web (`token: "relay"`) it answers the fixture
// server's return route as the provider would (Apple's `form_post`,
// Google's redirect with a code the server exchanges at this module's token
// stub), takes the handle from the `303`'s `Location` and answers `{relay}`.
//
//   bun packages/sign-in/test/idp.mjs --keys <file> [--first] [--server <url>] '<ticket args JSON>'
//
// prints the `IdentityCredential`'s JSON. The fixture server trusts the
// key's public half (`jwks(keys)`) under the agent only.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';

export const ISSUERS = { apple: 'https://appleid.apple.com', google: 'https://accounts.google.com' };
const ES256 = { name: 'ECDSA', namedCurve: 'P-256' };
const b64 = bytes => Buffer.from(bytes).toString('base64url');
const json64 = v => b64(new TextEncoder().encode(JSON.stringify(v)));

/** The fixture's signing key, made on first use and kept in `keys`. */
async function key(keys) {
  if (existsSync(keys)) {
    const held = JSON.parse(readFileSync(keys, 'utf8'));
    if (held.idp) return held.idp;
  }
  const pair = await crypto.subtle.generateKey(ES256, true, ['sign', 'verify']);
  const idp = { kid: `fixture-${b64(crypto.getRandomValues(new Uint8Array(6)))}`, jwk: await crypto.subtle.exportKey('jwk', pair.privateKey) };
  const held = existsSync(keys) ? JSON.parse(readFileSync(keys, 'utf8')) : {};
  writeFileSync(keys, JSON.stringify({ ...held, idp }, null, 2));
  return idp;
}

/** The public keys a verifier trusts, as a JWKS. */
export async function jwks(keys) {
  const { kid, jwk } = await key(keys);
  const { d: _, key_ops: __, ...pub } = jwk;
  return { keys: [{ ...pub, kid, alg: 'ES256', use: 'sig' }] };
}

/** An ES256 JWT over `claims`. */
export async function sign(keys, claims) {
  const { kid, jwk } = await key(keys);
  const k = await crypto.subtle.importKey('jwk', jwk, ES256, false, ['sign']);
  const input = `${json64({ alg: 'ES256', kid, typ: 'JWT' })}.${json64(claims)}`;
  const sig = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, k, new TextEncoder().encode(input));
  return `${input}.${b64(new Uint8Array(sig))}`;
}

/** The ID token a provider would sign for `provider` (a ticket's provider entry). */
export async function idToken(keys, provider, audience = provider.audience) {
  const now = Math.floor(Date.now() / 1000);
  return sign(keys, {
    iss: ISSUERS[provider.provider], aud: audience, sub: `fixture-${provider.provider}-sub`,
    email: `fixture@${provider.provider}.example`, email_verified: true,
    iat: now, exp: now + 600, ...(provider.nonce ? { nonce: provider.nonce } : {}),
  });
}

/** `args`' credential: `{credential}` for a typed answer. `first` makes Apple's first authorization, with `user`. */
export async function respond(args, keys, { first = false, server } = {}) {
  const provider = args.identity?.providers?.[0];
  if (!provider) throw new Error('the ticket names no identity provider');
  const base = { type: 'identity', id: '', configURL: provider.configURL, isAutoSelected: false, handle: b64(crypto.getRandomValues(new Uint8Array(12))) };
  const user = first ? { email: 'fixture@apple.example', name: { firstName: 'Ada', lastName: 'Fixture' } } : undefined;
  if (provider.token === 'relay') return { credential: { ...base, token: { relay: await relay(keys, provider, user, server) } } };
  if (provider.provider === 'google') return { credential: { ...base, token: await idToken(keys, provider) } };
  const authorization = { code: `fixture-code-${b64(crypto.getRandomValues(new Uint8Array(9)))}`, id_token: await idToken(keys, provider), state: '' };
  return { credential: { ...base, token: { authorization, ...(user ? { user } : {}) }, apple: { user: `fixture-apple-sub`, realUserStatus: first ? 'likelyReal' : null } } };
}

/** The web: the provider's answer to the return route, and the handle its `303` carries. */
async function relay(keys, provider, user, server) {
  const back = new URL(provider.redirectURI);
  const origin = server ?? back.origin;
  const state = `fixture-state-${b64(crypto.getRandomValues(new Uint8Array(9)))}`;
  let response;
  if (provider.provider === 'apple') {
    const body = new URLSearchParams({ code: 'fixture-apple-code', id_token: await idToken(keys, provider), state, ...(user ? { user: JSON.stringify(user) } : {}) });
    response = await fetch(`${origin}${back.pathname}`, { method: 'POST', body, redirect: 'manual', headers: { 'content-type': 'application/x-www-form-urlencoded' } });
  } else {
    // Google's code, which the return route exchanges at the token stub with its secret.
    const code = await sign(keys, { fixtureCode: true, provider: 'google', aud: provider.audience, nonce: provider.nonce, sub: 'fixture-google-sub', email: 'fixture@google.example' });
    response = await fetch(`${origin}${back.pathname}?${new URLSearchParams({ code, state })}`, { redirect: 'manual' });
  }
  const location = response.headers.get('location');
  if (response.status !== 303 || !location) throw new Error(`the return route answered ${response.status}: ${await response.text()}`);
  const handle = new URL(location).searchParams.get('relay');
  if (!handle) throw new Error(`the return route's 303 carries no handle: ${location}`);
  return handle;
}

/** The token stub (Google's `https://oauth2.googleapis.com/token`): a fixture code for the ID token it stands for. */
export async function exchange(keys, code) {
  const [, payload] = String(code).split('.');
  const claims = JSON.parse(Buffer.from(payload ?? '', 'base64url').toString() || '{}');
  if (!claims.fixtureCode) return null;
  return idToken(keys, { provider: claims.provider, audience: claims.aud, nonce: claims.nonce });
}

if (import.meta.main) {
  const argv = process.argv.slice(2), flag = n => (argv.includes(n) ? argv[argv.indexOf(n) + 1] : undefined);
  const keys = flag('--keys'), json = argv.filter((a, i) => !a.startsWith('--') && !argv[i - 1]?.startsWith('--')).at(-1);
  if (!keys || !json) { console.error("usage: idp.mjs --keys <file> [--first] [--server <url>] '<ticket args JSON>'"); process.exit(64); }
  const out = await respond(JSON.parse(json), keys, { first: argv.includes('--first'), server: flag('--server') });
  console.log(JSON.stringify(out.credential));
}
