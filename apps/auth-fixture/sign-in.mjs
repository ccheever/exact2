// The auth fixture's sign-in routes (LLP 1133.014 §10), beside its OAuth
// server and passkey relying party in `fixture.mjs`: what an app's own
// server does with a provider's ID token, as the recipe
// (`docs/recipes/sign-in.md`) gives it.
//
//   POST /sign-in/nonce     {}                          → {nonce, client}  single-use, five minutes, bound to `client`
//   GET  /sign-in/return    Google's redirect: code, state
//   POST /sign-in/return    Apple's form_post: code, id_token, state, user?
//                           → 303 to the configured page callback with `state` and `relay=<handle>` (or `error`)
//   POST /sign-in/identity  {credential, client}         → {ok, provider, sub, name, email} | {ok: false, error}
//   POST /fixture/sign-in   {callback}                    a drive's: the page callback the return route answers
//
// The identity route redeems a web handle once, or takes a native token,
// then checks the ID token by D2: its ES256 or RS256 signature against the
// provider's keys (under `--substitute`, the fixture provider's key from
// `--idp-keys`), `iss`, `aud` among the audiences the bake prints, `exp`,
// and `nonce` equal to the one issued to this client. Apple's first `user`
// is kept by `sub`, and its `code` recorded where a real server would
// exchange it for the refresh token it revokes at deletion.
import { exchange, jwks as fixtureJwks, ISSUERS } from '../../packages/sign-in/test/idp.mjs';

// The fixture's clients (§13 question 6 names the real ones Charlie creates).
export const AUDIENCES = {
  apple: ['com.exact.authfixture', 'com.exact.authfixture.web'],
  google: ['fixture-web.apps.googleusercontent.com', 'fixture-ios.apps.googleusercontent.com'],
};
const PUBLISHED = { apple: 'https://appleid.apple.com/auth/keys', google: 'https://www.googleapis.com/oauth2/v3/certs' };
const TTL = 5 * 60_000;
const b64 = bytes => Buffer.from(bytes).toString('base64url');
const random = n => b64(crypto.getRandomValues(new Uint8Array(n)));

export function signIn(log, { substitute = false, idpKeys = null } = {}) {
  const nonces = new Map(), handles = new Map(), accounts = new Map(), names = new Map();
  let callback = 'https://auth-fixture.exact.test/.exact/auth/callback';
  log.signIn = { nonces: 0, returned: 0, redeemed: 0, verified: 0, codes: 0, refused: [] };
  const refuse = (error) => { log.signIn.refused.push(error); return { ok: false, error }; };

  async function keysOf(provider) {
    if (substitute && idpKeys) return (await fixtureJwks(idpKeys)).keys;
    return (await (await fetch(PUBLISHED[provider])).json()).keys;
  }

  /** D2's checks of an ID token from `provider`; the claims, or why it is refused. */
  async function verify(provider, token, nonce) {
    const [h, p, s] = String(token ?? '').split('.');
    if (!s) return { error: 'malformed' };
    const head = JSON.parse(Buffer.from(h, 'base64url').toString()), claims = JSON.parse(Buffer.from(p, 'base64url').toString());
    const jwk = (await keysOf(provider)).find(k => k.kid === head.kid);
    if (!jwk) return { error: 'unknown key' };
    const alg = head.alg === 'ES256' ? [{ name: 'ECDSA', namedCurve: 'P-256' }, { name: 'ECDSA', hash: 'SHA-256' }]
      : head.alg === 'RS256' ? [{ name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, { name: 'RSASSA-PKCS1-v1_5' }] : null;
    if (!alg) return { error: `alg ${head.alg}` };
    const key = await crypto.subtle.importKey('jwk', jwk, alg[0], false, ['verify']);
    if (!(await crypto.subtle.verify(alg[1], key, Buffer.from(s, 'base64url'), new TextEncoder().encode(`${h}.${p}`)))) return { error: 'signature' };
    const issuers = provider === 'google' ? [ISSUERS.google, 'accounts.google.com'] : [ISSUERS.apple];
    if (!issuers.includes(claims.iss)) return { error: 'iss' };
    if (!AUDIENCES[provider].includes(claims.aud)) return { error: 'aud' };
    if (!(claims.exp * 1000 > Date.now())) return { error: 'exp' };
    // Google's claim is the raw nonce; Apple's the raw nonce too until stage 1 records its web form (D1).
    if (claims.nonce !== nonce) return { error: 'nonce' };
    return { claims };
  }

  const providerOf = config => (config === 'https://appleid.apple.com' ? 'apple' : config === 'https://accounts.google.com/gsi/fedcm.json' ? 'google' : null);

  return {
    reset() { nonces.clear(); handles.clear(); accounts.clear(); names.clear(); Object.assign(log.signIn, { nonces: 0, returned: 0, redeemed: 0, verified: 0, codes: 0, refused: [] }); },
    /** The return route (D3 step 3): GET for Google's redirect, POST for Apple's form_post. */
    async back(method, query, form) {
      const f = method === 'POST' ? form : query;
      const answer = params => `${callback}?${new URLSearchParams(params)}`;
      if (!f.state) return { status: 400, body: 'no state' };
      if (f.error) return { status: 303, location: answer({ state: f.state, error: f.error }) };
      let kept;
      if (method === 'POST') {
        if (!f.id_token || !f.code) return { status: 400, body: 'no id_token or code' };
        kept = { provider: 'apple', token: f.id_token, code: f.code, user: f.user ? JSON.parse(f.user) : null };
      } else {
        // Google's code, exchanged with the server's secret at the token endpoint (here the fixture's stub).
        const token = substitute && idpKeys ? await exchange(idpKeys, f.code) : null;
        if (!token) return { status: 400, body: 'the code exchange failed' };
        kept = { provider: 'google', token };
      }
      const handle = random(16);
      handles.set(handle, { ...kept, expires: Date.now() + TTL });
      log.signIn.returned++;
      return { status: 303, location: answer({ state: f.state, relay: handle }) };
    },
    async route(path, body) {
      if (path === '/fixture/sign-in') { callback = String(body.callback ?? callback); return { ok: true, callback }; }
      if (path === '/sign-in/nonce') {
        const nonce = random(24), client = String(body.client || random(12));
        nonces.set(nonce, { client, expires: Date.now() + TTL });
        log.signIn.nonces++;
        return { nonce, client };
      }
      if (path !== '/sign-in/identity') return null;
      const { credential, client, nonce } = body;
      const issued = nonces.get(nonce);
      nonces.delete(nonce); // single use, whatever follows
      if (!issued || issued.expires < Date.now() || issued.client !== client) return refuse('nonce');
      const provider = providerOf(credential?.configURL);
      if (!provider) return refuse('provider');
      let token = credential.token, user = null, code = null;
      if (token && typeof token === 'object' && 'relay' in token) {
        const kept = handles.get(token.relay);
        handles.delete(token.relay);
        if (!kept || kept.expires < Date.now() || kept.provider !== provider) return refuse('handle');
        log.signIn.redeemed++;
        ({ token, user, code } = kept);
      } else if (provider === 'apple') {
        ({ id_token: token, code } = token?.authorization ?? {});
        user = credential.token?.user ?? null;
      }
      const checked = await verify(provider, token, nonce);
      if (checked.error) return refuse(checked.error);
      const { sub, email } = checked.claims;
      if (code) log.signIn.codes++; // where a real server exchanges Apple's code for the refresh token it revokes (D2)
      // The name arrives once (D2): kept from the first authorization.
      const key = `${provider}:${sub}`;
      if (user?.name && !names.has(key)) names.set(key, [user.name.firstName, user.name.lastName].filter(Boolean).join(' '));
      accounts.set(key, { email });
      log.signIn.verified++;
      return { ok: true, provider, sub, email, name: names.get(key) ?? '' };
    },
  };
}
