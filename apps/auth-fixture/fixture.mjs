#!/usr/bin/env bun
// A local OAuth authorization and resource server for the auth fixture
// (LLP 1069.006 D7: sign-in under the agent reaches fixture servers only).
// The AT Protocol's profile, small: PAR required, PKCE S256, DPoP-bound
// tokens (ES256 proofs verified with WebCrypto, `jti` single-use, a nonce
// the client must retry with, `jkt` bound at PAR and checked at the token
// and resource servers, `ath` on the resource), `iss` in the callback. Two
// registered clients, one per application type (after review, item 3): the
// native one's redirect is the client id's hostname reversed, the web one's
// the app's `/.exact/auth/callback` (any loopback port in development).
//
//   bun apps/auth-fixture/fixture.mjs [--port 4331]
//   GET /oauth/authorize?…&auto=1   the browser's page approves and redirects;
//                                   with Accept: application/json it answers
//                                   {location} instead, for a drive's `type @t`
//   GET /fixture/log                 what the server checked, for a smoke
import { createServer } from 'node:http';

const args = process.argv.slice(2);
const port = Number(args[args.indexOf('--port') + 1] || 4331) || 4331;
export const ISSUER = `http://127.0.0.1:${port}`;
const ORIGIN = 'https://auth-fixture.exact.test';
const CLIENTS = {
  [`${ORIGIN}/.exact/auth/client-metadata.native.json`]: uri => uri === 'test.exact.auth-fixture:/oauth',
  [`${ORIGIN}/.exact/auth/client-metadata.web.json`]: uri => uri === `${ORIGIN}/.exact/auth/callback`
    || /^http:\/\/(127\.0\.0\.1|localhost):\d+\/\.exact\/auth\/callback$/.test(uri),
};
const DID = 'did:plc:fixture', HANDLE = 'fixture.test';
const requests = new Map(), codes = new Map(), tokens = new Map(), jtis = new Set();
const nonce = () => crypto.randomUUID();
let authNonce = nonce(), resourceNonce = nonce(), coop = false, auto = false;
const log = { par: 0, authorize: 0, token: 0, resource: 0, dpopVerified: 0, nonceRetries: 0, pkce: 0, refused: [] };

const b64 = s => Buffer.from(s, 'base64url');
const sha64 = text => Buffer.from(Bun.CryptoHasher.hash('sha256', text)).toString('base64url');
const thumbprint = jwk => sha64(JSON.stringify({ crv: jwk.crv, kty: jwk.kty, x: jwk.x, y: jwk.y }));

// A DPoP proof (RFC 9449 §4.3): the header's JWK verifies the ES256
// signature; `htm`/`htu` match; `jti` is new; the server's nonce is there.
async function dpop(req, url, expectedNonce, ath) {
  const header = req.headers.dpop;
  if (!header) throw ['invalid_dpop_proof', 'no DPoP header'];
  const [h, p, s] = header.split('.');
  const head = JSON.parse(b64(h)), body = JSON.parse(b64(p));
  if (head.typ !== 'dpop+jwt' || head.alg !== 'ES256' || head.jwk?.crv !== 'P-256' || head.jwk.d) throw ['invalid_dpop_proof', 'header'];
  const key = await crypto.subtle.importKey('jwk', { kty: 'EC', crv: 'P-256', x: head.jwk.x, y: head.jwk.y }, { name: 'ECDSA', namedCurve: 'P-256' }, false, ['verify']);
  const ok = await crypto.subtle.verify({ name: 'ECDSA', hash: 'SHA-256' }, key, b64(s), new TextEncoder().encode(`${h}.${p}`));
  if (!ok) throw ['invalid_dpop_proof', 'signature'];
  if (body.htm !== req.method || body.htu !== url) throw ['invalid_dpop_proof', `htm/htu ${body.htm} ${body.htu}`];
  if (typeof body.iat !== 'number' || typeof body.jti !== 'string') throw ['invalid_dpop_proof', 'iat/jti'];
  if (jtis.has(body.jti)) throw ['invalid_dpop_proof', 'jti replayed'];
  if (body.nonce !== expectedNonce) { log.nonceRetries++; throw ['use_dpop_nonce', 'nonce']; }
  if (ath !== undefined && body.ath !== ath) throw ['invalid_dpop_proof', 'ath'];
  jtis.add(body.jti);
  log.dpopVerified++;
  return thumbprint(head.jwk);
}

const cors = {
  'access-control-allow-origin': '*',
  'access-control-allow-headers': 'content-type, dpop, authorization',
  'access-control-expose-headers': 'dpop-nonce',
  'access-control-allow-methods': 'GET, POST',
};
function send(res, status, body, headers = {}) {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store', ...cors, ...headers });
  res.end(JSON.stringify(body));
}
const refuse = (res, status, error, why, extra = {}) => { log.refused.push(`${error}: ${why}`); send(res, status, { error, error_description: why }, extra); };

async function readForm(req) {
  let text = '';
  for await (const chunk of req) text += chunk;
  return Object.fromEntries(new URLSearchParams(text));
}

export const server = createServer(async (req, res) => {
  const url = new URL(req.url, ISSUER);
  const at = `${ISSUER}${url.pathname}`;
  try {
    if (req.method === 'OPTIONS') { res.writeHead(204, cors); res.end(); return; }
    if (url.pathname === '/.well-known/oauth-authorization-server') return send(res, 200, {
      issuer: ISSUER, authorization_endpoint: `${ISSUER}/oauth/authorize`, token_endpoint: `${ISSUER}/oauth/token`,
      pushed_authorization_request_endpoint: `${ISSUER}/oauth/par`, require_pushed_authorization_requests: true,
      response_types_supported: ['code'], grant_types_supported: ['authorization_code', 'refresh_token'],
      code_challenge_methods_supported: ['S256'], dpop_signing_alg_values_supported: ['ES256'],
      token_endpoint_auth_methods_supported: ['none'], authorization_response_iss_parameter_supported: true,
      scopes_supported: ['atproto'],
    });
    if (url.pathname === '/oauth/par' && req.method === 'POST') {
      let jkt;
      try { jkt = await dpop(req, at, authNonce); }
      catch ([error, why]) { return refuse(res, 400, error, why, error === 'use_dpop_nonce' ? { 'dpop-nonce': authNonce } : {}); }
      const f = await readForm(req);
      const client = CLIENTS[f.client_id];
      if (!client) return refuse(res, 400, 'invalid_client', f.client_id);
      if (!client(f.redirect_uri)) return refuse(res, 400, 'invalid_request', `redirect_uri ${f.redirect_uri}`);
      if (f.code_challenge_method !== 'S256' || !/^[A-Za-z0-9_-]{43}$/.test(f.code_challenge ?? '')) return refuse(res, 400, 'invalid_request', 'PKCE S256');
      if (!f.state || f.response_type !== 'code' || !(f.scope ?? '').split(' ').includes('atproto')) return refuse(res, 400, 'invalid_request', 'state/response_type/scope');
      const requestUri = `urn:ietf:params:oauth:request_uri:req-${crypto.randomUUID()}`;
      requests.set(requestUri, { ...f, jkt, expires: Date.now() + 5 * 60_000 });
      log.par++;
      return send(res, 201, { request_uri: requestUri, expires_in: 300 }, { 'dpop-nonce': authNonce });
    }
    if (url.pathname === '/oauth/authorize' && req.method === 'GET') {
      const pushed = requests.get(url.searchParams.get('request_uri'));
      if (!pushed || pushed.client_id !== url.searchParams.get('client_id') || pushed.expires < Date.now()) return refuse(res, 400, 'invalid_request', 'request_uri');
      if (!auto && url.searchParams.get('approve') !== '1' && !(req.headers.accept ?? '').includes('application/json')) {
        // A person's page: one button that approves (the manual run).
        // `/fixture/coop?on=1`: the page severs its opener, as a provider's
        // Cross-Origin-Opener-Policy does (the callback returns by channel).
        res.writeHead(200, { 'content-type': 'text/html', 'cache-control': 'no-store', ...(coop ? { 'cross-origin-opener-policy': 'same-origin' } : {}) });
        res.end(`<!doctype html><meta charset="utf-8"><title>Fixture sign-in</title><body style="font:17px system-ui;margin:2em"><h1>Fixture sign-in</h1><p>Sign in to <b>${HANDLE}</b> for the auth fixture?</p><a id="approve" href="${url.pathname}${url.search}&approve=1" style="padding:12px 18px;background:#2f6fd6;color:white;border-radius:8px;text-decoration:none">Approve</a></body>`);
        return;
      }
      requests.delete(url.searchParams.get('request_uri'));
      const code = `code-${crypto.randomUUID()}`;
      codes.set(code, { ...pushed, expires: Date.now() + 60_000 });
      log.authorize++;
      const location = `${pushed.redirect_uri}?${new URLSearchParams({ code, state: pushed.state, iss: ISSUER })}`;
      if ((req.headers.accept ?? '').includes('application/json')) return send(res, 200, { location });
      res.writeHead(302, { location, 'cache-control': 'no-store' }); res.end(); return;
    }
    if (url.pathname === '/oauth/token' && req.method === 'POST') {
      let jkt;
      try { jkt = await dpop(req, at, authNonce); }
      catch ([error, why]) { return refuse(res, 400, error, why, error === 'use_dpop_nonce' ? { 'dpop-nonce': authNonce } : {}); }
      const f = await readForm(req);
      const grant = codes.get(f.code);
      codes.delete(f.code); // single use, whatever follows
      if (f.grant_type !== 'authorization_code' || !grant || grant.expires < Date.now()) return refuse(res, 400, 'invalid_grant', 'code');
      if (grant.jkt !== jkt) return refuse(res, 400, 'invalid_dpop_proof', 'key not the one bound at PAR');
      if (grant.client_id !== f.client_id || grant.redirect_uri !== f.redirect_uri) return refuse(res, 400, 'invalid_grant', 'client or redirect_uri');
      if (sha64(f.code_verifier ?? '') !== grant.code_challenge) return refuse(res, 400, 'invalid_grant', 'PKCE verifier');
      log.pkce++;
      const access = `at-${crypto.randomUUID()}`;
      tokens.set(access, { jkt, sub: DID });
      log.token++;
      return send(res, 200, { access_token: access, token_type: 'DPoP', expires_in: 3600, refresh_token: `rt-${crypto.randomUUID()}`, scope: 'atproto', sub: DID }, { 'dpop-nonce': authNonce });
    }
    if (url.pathname === '/xrpc/whoami' && req.method === 'GET') {
      const [scheme, access] = (req.headers.authorization ?? '').split(' ');
      const held = tokens.get(access);
      if (scheme !== 'DPoP' || !held) return refuse(res, 401, 'invalid_token', 'no DPoP token');
      let jkt;
      try { jkt = await dpop(req, at, resourceNonce, sha64(access)); }
      catch ([error, why]) { return refuse(res, 401, error, why, error === 'use_dpop_nonce' ? { 'dpop-nonce': resourceNonce } : {}); }
      if (jkt !== held.jkt) return refuse(res, 401, 'invalid_token', 'key not the token\'s');
      log.resource++;
      return send(res, 200, { did: held.sub, handle: HANDLE });
    }
    if (url.pathname === '/fixture/log') return send(res, 200, log);
    // `/fixture/auto?on=1`: approve without a page (a real session driven
    // with no hands on the sheet, `EXACT_DEVICE=real`).
    if (url.pathname === '/fixture/auto') { auto = url.searchParams.get('on') === '1'; return send(res, 200, { auto }); }
    if (url.pathname === '/fixture/coop') { coop = url.searchParams.get('on') === '1'; return send(res, 200, { coop }); }
    if (url.pathname === '/fixture/reset') {
      requests.clear(); codes.clear(); tokens.clear(); jtis.clear(); authNonce = nonce(); resourceNonce = nonce(); coop = false; auto = false;
      Object.assign(log, { par: 0, authorize: 0, token: 0, resource: 0, dpopVerified: 0, nonceRetries: 0, pkce: 0, refused: [] });
      return send(res, 200, { ok: true });
    }
    send(res, 404, { error: 'not_found' });
  } catch (error) {
    refuse(res, 500, 'server_error', String(error?.message ?? error));
  }
});

if (import.meta.main) {
  server.listen(port, '127.0.0.1', () => console.log(`auth fixture server on ${ISSUER}`));
}
