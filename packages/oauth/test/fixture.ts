// A fake OAuth 2.0 / OpenID Connect authorization server for @exact/oauth's
// conformance cases (LLP 1146 D6). One Bun process serves every provider
// profile under `/p/<profile>`; a profile reproduces one provider's
// documented behaviour (PROVIDERS.md gives each one's source). The
// authorization endpoint approves at once, as a person already signed in
// would, and redirects to the callback. `POST /control` lets a test expire
// tokens, fail, cut off or hold the next token request, revoke a session, deny the
// next sign-in, set claims in the next ID token, and assert or await what it
// counted, in all or since a mark (`control`).
//
// Run: `bun fixture.ts` (PORT, default 4400). `exact test` starts it as the
// app's server when the app's `app.json` names it in `commands.server`.
import { createHash, randomBytes } from 'node:crypto';
import process from 'node:process';

export interface Profile {
  /** Discovery at `<issuer>/.well-known/openid-configuration`; false: none (endpoints are configured). */
  discovery: boolean;
  /** The issuer as the discovery document names it; `{base}` is the profile's base URL. */
  issuer: string;
  /** PKCE must be present and S256. */
  requirePkce: boolean;
  /** A refresh token is issued only when the authorization request carried this parameter or scope. */
  refreshTokenWhen: { param?: [string, string]; scope?: string } | 'always' | 'never';
  /** What a refresh does to the refresh token: a new one each time, the same one, or none in the reply. */
  rotation: 'rotate' | 'same' | 'omit';
  /** A spent refresh token used again revokes every token of its session (RFC 9700 §4.14.2). */
  reuseDetection: boolean;
  /** The token reply's body: JSON, or form-encoded unless the request says `Accept: application/json`. */
  replyFormat: 'json' | 'form-unless-accept-json';
  /** A token endpoint error comes with HTTP 200 instead of 400. */
  errorsWith200: boolean;
  /** `expires_in` as a number, a numeric string, or absent. */
  expiresIn: 'number' | 'string' | 'absent';
  /** `token_type` as written. */
  tokenType: string;
  /** `scope` in the token reply: space-separated, comma-separated, or absent. */
  scopeFormat: 'space' | 'comma' | 'absent';
  /** Issue an ID token when `openid` is asked. */
  idToken: boolean;
  /** Put RFC 9207's `iss` in the callback and say so in discovery. */
  issInCallback: boolean;
  /** Demand a client secret at the token endpoint. */
  requireSecret: boolean;
  /** Require `scope` on a refresh. */
  refreshNeedsScope: boolean;
  /** The ID token's `aud` as an array with a second audience, and `azp`. */
  audArray: boolean;
  /** Seconds an access token lasts. */
  accessSeconds: number;
  /** How a dead refresh token is refused: its status and `error`. */
  deadRefresh: { status: number; error: string };
  /** The ID token's `iss` as Google also spells it, `accounts.google.com` (GOOG-2). */
  issNoScheme: boolean;
  /** The token and revocation endpoints are on a second origin, the next port (GOOG-1). */
  tokenOnSecondOrigin: boolean;
  /** Scopes the authorization endpoint refuses with `invalid_scope`. */
  refusedScopes: string[];
  /** Seconds a spent refresh token is still accepted, issuing a new pair (a grace period). */
  graceSeconds: number;
  /** A spent refresh token stays usable until it expires, though a use still counts as reuse (ENTRA-9). */
  keepsSpent?: boolean;
  /** A refresh also invalidates the access token it replaces. */
  refreshRevokesAccess: boolean;
  /** The token and revocation endpoints answer a browser (CORS on the preflight, success and error). */
  tokenCors: boolean;
  /** Token replies let a page read `Date` (`Access-Control-Expose-Headers`); the providers observed expose nothing (ENTRA-5, GOOG-15). */
  exposeDate: boolean;
  /** The status of a revocation with an unknown token: 200 as RFC 7009 §2.2 says, or 400 `invalid_token` (GOOG-16). */
  revokeUnknown: 200 | 400;
}

const base: Profile = {
  discovery: true,
  issuer: '{base}',
  requirePkce: true,
  refreshTokenWhen: { scope: 'offline_access' },
  rotation: 'rotate',
  reuseDetection: false,
  replyFormat: 'json',
  errorsWith200: false,
  expiresIn: 'number',
  tokenType: 'Bearer',
  scopeFormat: 'space',
  idToken: true,
  issInCallback: false,
  requireSecret: false,
  refreshNeedsScope: false,
  audArray: false,
  accessSeconds: 3600,
  deadRefresh: { status: 400, error: 'invalid_grant' },
  issNoScheme: false,
  tokenOnSecondOrigin: false,
  refusedScopes: [],
  graceSeconds: 0,
  refreshRevokesAccess: false,
  tokenCors: true,
  exposeDate: false,
  revokeUnknown: 200,
};

/** The profiles, each named for the provider it imitates. Sources: PROVIDERS.md. */
export const PROFILES: Record<string, Profile> = {
  // The specifications as written: RFC 6749, 7636, 9207, OpenID Connect.
  spec: { ...base, issInCallback: true, exposeDate: true },
  // Google: a refresh token only with `access_type=offline` (GOOG-8); a
  // refresh keeps the same refresh token and leaves it out of the reply
  // (GOOG-9); the ID token's `iss` without its scheme (GOOG-2); a client
  // secret even for an installed app's client (GOOG-4).
  google: { ...base, refreshTokenWhen: { param: ['access_type', 'offline'] }, rotation: 'omit', requireSecret: true, issNoScheme: true, tokenOnSecondOrigin: true, issInCallback: true, revokeUnknown: 400 },
  // Microsoft Entra ID v2 `common`: discovery names `{tenantid}` (ENTRA-1);
  // `offline_access` brings a refresh token (ENTRA-7); refresh tokens rotate
  // and the old one is not revoked by the exchange (ENTRA-9).
  entra: { ...base, issuer: '{base}/{tenantid}/v2.0', keepsSpent: true },
  // Entra's v1 endpoint: `expires_in` as a string (ENTRA-8).
  'entra-v1': { ...base, issuer: '{base}/{tenantid}/v2.0', expiresIn: 'string', keepsSpent: true },
  // Auth0 with rotation: the issuer ends in a slash (AUTH0-1); a reused
  // refresh token revokes the family (AUTH0-5) and is refused with 403
  // `invalid_grant` (AUTH0-6).
  auth0: { ...base, issuer: '{base}/', reuseDetection: true, deadRefresh: { status: 403, error: 'invalid_grant' } },
  // Okta with rotation: the spent token still works for 30 s, after which
  // its reuse revokes the family (OKTA-6).
  okta: { ...base, reuseDetection: true, graceSeconds: 30 },
  // OneLogin: a path issuer (ONELOGIN-1); `offline_access` refused, and a
  // refresh token by app setting (ONELOGIN-2); no `scope` in the reply
  // (ONELOGIN-3); a dead refresh token answers 401 `invalid_request`
  // (ONELOGIN-4); a refresh returns a new refresh token (ONELOGIN-6).
  onelogin: { ...base, issuer: '{base}/oidc/2', refusedScopes: ['offline_access'], refreshTokenWhen: 'always', scopeFormat: 'absent', deadRefresh: { status: 401, error: 'invalid_request' } },
  // GitHub (an App's expiring user tokens): no discovery and no ID token
  // (GH-1); form-encoded replies unless asked for JSON (GH-2); errors under
  // HTTP 200 (GH-3) with its own codes (GH-4); comma-separated scopes and
  // `bearer` (GH-5, GH-6); single-use refresh tokens that also end the old
  // access token (GH-14).
  github: { ...base, discovery: false, requireSecret: true, replyFormat: 'form-unless-accept-json', errorsWith200: true, scopeFormat: 'comma', tokenType: 'bearer', idToken: false, refreshTokenWhen: 'always', reuseDetection: false, refreshRevokesAccess: true, deadRefresh: { status: 200, error: 'bad_refresh_token' }, tokenCors: false },
  // Salesforce: no `expires_in` (SF-1); a refresh reply without a refresh
  // token (SF-3).
  salesforce: { ...base, expiresIn: 'absent', refreshTokenWhen: 'always', rotation: 'omit' },
};

interface Grant { profile: string; client: string; redirect: string; challenge: string | null; nonce: string | null; offline: boolean; used: boolean; family: string }
interface Family { profile: string; client: string; scope: string; subject: string; revoked: boolean; refresh: string; spent: Map<string, number>; access: Map<string, number> }

// `--port <n>` (an app's `commands.server` passes it), else PORT, else 4400.
const flag = process.argv.indexOf('--port');
const port = Number(flag > 0 ? process.argv[flag + 1] : process.env.PORT ?? 4400);
let codes = new Map<string, Grant>();
let families = new Map<string, Family>();
let byRefresh = new Map<string, string>();
let byAccess = new Map<string, string>();
// What the next token request does, per profile: fail with a status, drop the connection, or wait.
let next: Record<string, { fail?: number; drop?: boolean; dropAfter?: boolean; hold?: boolean; error?: string; codes?: number[]; count?: number }> = {};
// Token requests held until the test releases them (`hold-token`, `release-token`), and the long-polls of `await`.
let held: (() => void)[] = [];
let awaiting: (() => void)[] = [];
let denyNext = new Set<string>();
let after: Record<string, boolean> = {};
// Profiles whose next token reply leaves out `token_type`.
let omitType = new Set<string>();
// Claims the next ID token a profile issues carries over its own (`id-claims`).
let idClaims: Record<string, Record<string, unknown>> = {};
let counts: Record<string, Record<string, number>> = {};
// Each profile's counts at its last `mark`, which an `expect` with `since` counts from.
let marks: Record<string, Record<string, number>> = {};
// The protocol's clock stands still from each reset: a grace period, an expiry, an ID token's times and a token
// reply's `Date` all read the same instant, however long a loaded machine takes between two requests. Only `await`'s
// bound for a hanging test reads the wall clock.
let clock = Date.now();
const now = (): number => clock;

const PEOPLE: Record<string, { sub: string; name: string; email: string }> = {
  default: { sub: 'user-1', name: 'Ada Lovelace', email: 'ada@example.com' },
};

function count(profile: string, what: string): void {
  (counts[profile] ??= {})[what] = (counts[profile][what] ?? 0) + 1;
  changed();
}

const b64url = (b: Buffer): string => b.toString('base64url');
const token = (prefix: string): string => `${prefix}.${b64url(randomBytes(18))}`;
// Each request's own origin, the port it arrived on (the fixture's or a test's), names the issuer and endpoints.
const baseOf = (origin: string, profile: string): string => `${origin}/p/${profile}`;
const issuerOf = (origin: string, profile: string): string => PROFILES[profile].issuer.replace('{base}', baseOf(origin, profile));

function idToken(origin: string, profile: string, client: string, sub: string, nonce: string | null): string {
  const p = PROFILES[profile], seconds = Math.floor(now() / 1000);
  const tid = '72f988bf-0000-0000-0000-000000000000';
  const claims: Record<string, unknown> = {
    iss: p.issNoScheme ? 'accounts.google.com' : issuerOf(origin, profile).replace('{tenantid}', tid),
    sub,
    aud: p.audArray ? [client, 'https://api.example.test'] : client,
    ...(p.audArray ? { azp: client } : {}),
    iat: seconds,
    exp: seconds + 3600,
    ...(nonce ? { nonce } : {}),
    ...(p.issuer.includes('{tenantid}') ? { tid } : {}),
    name: PEOPLE.default.name,
    ...idClaims[profile],
  };
  delete idClaims[profile];
  const header = { alg: 'RS256', typ: 'JWT', kid: 'fixture' };
  return `${b64url(Buffer.from(JSON.stringify(header)))}.${b64url(Buffer.from(JSON.stringify(claims)))}.${b64url(randomBytes(32))}`;
}

// The CORS headers of a token or revocation reply: none from a provider whose
// token endpoint does not answer a browser.
function tokenCors(p: Profile): Record<string, string> {
  if (!p.tokenCors) return {};
  return { 'Access-Control-Allow-Origin': '*', ...(p.exposeDate ? { 'Access-Control-Expose-Headers': 'Date' } : {}) };
}

function reply(p: Profile, req: Request, status: number, body: Record<string, unknown>): Response {
  const json = req.headers.get('accept')?.includes('application/json');
  const headers: Record<string, string> = { 'Cache-Control': 'no-store', Date: new Date(now()).toUTCString(), ...tokenCors(p) };
  // A client that failed to authenticate through the Authorization header is challenged in its scheme (RFC 6749 §5.2).
  if (status === 401 && body.error === 'invalid_client' && req.headers.has('authorization')) headers['WWW-Authenticate'] = 'Basic realm="fixture"';
  if (p.replyFormat === 'form-unless-accept-json' && !json) {
    const text = new URLSearchParams(Object.entries(body).map(([k, v]) => [k, String(v)])).toString();
    return new Response(text, { status, headers: { ...headers, 'Content-Type': 'application/x-www-form-urlencoded' } });
  }
  return Response.json(body, { status, headers });
}

function tokenError(p: Profile, req: Request, status: number, error: string, description: string): Response {
  return reply(p, req, p.errorsWith200 ? 200 : status, { error, error_description: description });
}

function issue(profile: string, req: Request, id: string, nonce: string | null, refresh: string | null): Response {
  const p = PROFILES[profile], family = families.get(id)!;
  const access = token('at');
  family.access.set(access, now() + p.accessSeconds * 1000);
  byAccess.set(access, id);
  const scopes = family.scope.split(' ').filter(Boolean);
  const body: Record<string, unknown> = { access_token: access, ...(omitType.delete(profile) ? {} : { token_type: p.tokenType }) };
  if (p.expiresIn === 'number') body.expires_in = p.accessSeconds;
  if (p.expiresIn === 'string') body.expires_in = String(p.accessSeconds);
  if (p.scopeFormat === 'space') body.scope = scopes.join(' ');
  if (p.scopeFormat === 'comma') body.scope = scopes.join(',');
  if (refresh) body.refresh_token = refresh;
  if (p.idToken && scopes.includes('openid')) body.id_token = idToken(new URL(req.url).origin, profile, family.client, family.subject, nonce);
  return reply(p, req, 200, body);
}

// A request's form, a parameter with no value left out, as if not sent (RFC 6749 §3.2).
const formOf = (text: string): URLSearchParams => new URLSearchParams([...new URLSearchParams(text)].filter(([, value]) => value !== ''));

// What a token or revocation request is refused as before its grant is read (RFC 6749 §5.2, RFC 7009 §2.1): a body
// that is not a form, a parameter sent twice (§3.2), a client that authenticates two ways (§2.3) or names itself
// nowhere (§4.1.3), as `invalid_request`, or not the way its profile requires, as `invalid_client`; else the client.
function checked(p: Profile, req: Request, form: URLSearchParams): { refused: [number, string, string] } | { client: string | null } {
  if (!/^application\/x-www-form-urlencoded\s*(;|$)/i.test(req.headers.get('content-type') ?? '')) return { refused: [400, 'invalid_request', 'the body is not a form'] };
  const repeated = [...new Set(form.keys())].find((name) => form.getAll(name).length > 1);
  if (repeated !== undefined) return { refused: [400, 'invalid_request', `${repeated || 'a parameter with no name'} is sent twice`] };
  if (!req.headers.has('authorization') && !form.has('client_id')) return { refused: [400, 'invalid_request', 'client_id is required'] };
  const sent = credentials(req, form);
  if (sent === 'twice') return { refused: [400, 'invalid_request', 'the client authenticates more than one way'] };
  if (!sent || p.requireSecret && sent.secret !== 'fixture-secret') return { refused: [401, 'invalid_client', 'client authentication failed'] };
  return { client: sent.id };
}

// The client a request names, and its secret (RFC 6749 §2.3.1): HTTP Basic credentials, the scheme in any case (RFC
// 7617 §2), canonical Base64 (RFC 4648 §4), split at the first colon and each part form-decoded; else the form's.
// Undefined when an Authorization header is not that; `twice` when the form authenticates too.
function credentials(req: Request, form: URLSearchParams): { id: string | null; secret: string | null } | 'twice' | undefined {
  const header = req.headers.get('authorization');
  if (header === null) return { id: form.get('client_id'), secret: form.get('client_secret') };
  const basic = /^basic +(\S+)$/i.exec(header)?.[1] ?? '', bytes = Buffer.from(basic, 'base64');
  if (!basic || bytes.toString('base64') !== basic) return undefined;
  const pair = bytes.toString(), colon = pair.indexOf(':'), decoded = (part: string): string => decodeURIComponent(part.replace(/\+/g, ' '));
  if (colon < 0) return undefined;
  let sent: { id: string; secret: string };
  try { sent = { id: decoded(pair.slice(0, colon)), secret: decoded(pair.slice(colon + 1)) }; } catch { return undefined; }
  return form.has('client_secret') || form.has('client_id') && form.get('client_id') !== sent.id ? 'twice' : sent;
}

async function tokenEndpoint(profile: string, req: Request): Promise<Response> {
  const response = await tokenEndpointInner(profile, req);
  const plan = after[profile];
  if (plan) {
    delete after[profile];
    // The token was spent; the reply never arrives.
    count(profile, 'dropped-after');
    return cutOff(PROFILES[profile]);
  }
  return response;
}

// A reply cut off: it arrives whole but its body stops short, so the client
// gets no token from it. A broken connection would not do: a browser may send
// the request again on a fresh one.
function cutOff(p: Profile): Response {
  return new Response('{"access_token":"', { status: 200, headers: { 'Content-Type': 'application/json', ...tokenCors(p) } });
}

// What `await` waits for now holds: every waiting long-poll checks again.
function changed(): void {
  for (const wake of awaiting.splice(0)) wake();
}

async function tokenEndpointInner(profile: string, req: Request): Promise<Response> {
  const p = PROFILES[profile];
  const form = formOf(await req.text());
  const plan = next[profile];
  if (plan && (plan.count ?? 1) > 0) {
    plan.count = (plan.count ?? 1) - 1;
    if ((plan.count ?? 0) <= 0) delete next[profile];
    if (plan.hold) {
      count(profile, 'held');
      await new Promise<void>((release) => { held.push(release); changed(); });
    }
    if (plan.dropAfter) after[profile] = true;
    // A reply cut off before its body ends: the request was never processed, and the client sees no reply.
    if (plan.drop) {
      count(profile, 'dropped');
      return cutOff(p);
    }
    // A failure a browser can read, as the provider's other replies are: a
    // page sees the 429 or 503 itself, not a network error.
    if (plan.fail) { count(profile, 'failed'); return new Response('Service Unavailable', { status: plan.fail, headers: { 'Content-Type': 'text/plain', ...tokenCors(p) } }); }
    if (plan.error) return plan.codes ? reply(p, req, 400, { error: plan.error, error_description: `AADSTS${plan.codes[0]}: the fixture was told to answer ${plan.error}`, error_codes: plan.codes }) : tokenError(p, req, 400, plan.error, `the fixture was told to answer ${plan.error}`);
  }
  const request = checked(p, req, form);
  if ('refused' in request) return tokenError(p, req, ...request.refused);
  const client = request.client, grant = form.get('grant_type');
  // A parameter the grant requires (RFC 6749 §4.1.3, §6).
  const missing = (grant === 'authorization_code' ? ['code', 'redirect_uri'] : grant === 'refresh_token' ? ['refresh_token'] : grant ? [] : ['grant_type']).find((name) => !form.has(name));
  if (missing) return tokenError(p, req, 400, 'invalid_request', `${missing} is required`);
  if (grant === 'authorization_code') {
    count(profile, 'exchange');
    const code = form.get('code') ?? '', g = codes.get(code);
    if (!g || g.profile !== profile) return tokenError(p, req, 400, 'invalid_grant', 'unknown code');
    if (g.used) {
      families.get(g.family)!.revoked = true;
      return tokenError(p, req, 400, 'invalid_grant', 'code already used');
    }
    g.used = true;
    // A code, and a refresh token below, is redeemed only by the client it was issued to (RFC 6749 §4.1.3, §6).
    if (client !== g.client) return tokenError(p, req, 400, 'invalid_grant', 'client mismatch');
    if (form.get('redirect_uri') !== g.redirect) return tokenError(p, req, 400, 'invalid_grant', 'redirect_uri mismatch');
    if (g.challenge) {
      const verifier = form.get('code_verifier') ?? '';
      if (!/^[A-Za-z0-9\-._~]{43,128}$/.test(verifier)) return tokenError(p, req, 400, 'invalid_grant', 'bad code_verifier');
      if (b64url(createHash('sha256').update(verifier).digest()) !== g.challenge) return tokenError(p, req, 400, 'invalid_grant', 'PKCE verification failed');
    }
    const family = families.get(g.family)!;
    const refresh = g.offline ? token('rt') : null;
    if (refresh) { family.refresh = refresh; byRefresh.set(refresh, g.family); }
    return issue(profile, req, g.family, g.nonce, refresh);
  }
  if (grant === 'refresh_token') {
    count(profile, 'refresh');
    const sent = form.get('refresh_token') ?? '';
    const id = byRefresh.get(sent), family = id ? families.get(id) : undefined;
    const dead = (why: string): Response => tokenError(p, req, p.deadRefresh.status, p.deadRefresh.error, why);
    if (!family || family.profile !== profile) return dead('unknown refresh token');
    if (client !== family.client) return tokenError(p, req, 400, 'invalid_grant', 'client mismatch');
    if (family.revoked) return dead('refresh token revoked');
    const spentAt = family.spent.get(sent);
    // With no grace period, any reuse is reuse, whatever the millisecond.
    const inGrace = spentAt !== undefined && p.graceSeconds > 0 && now() - spentAt <= p.graceSeconds * 1000;
    if (spentAt !== undefined && !inGrace) {
      count(profile, 'reuse');
      if (p.reuseDetection) { family.revoked = true; family.access.clear(); }
      if (!p.keepsSpent) return dead('refresh token already used');
    }
    if (inGrace) count(profile, 'grace');
    if (p.refreshNeedsScope && !form.get('scope')) return tokenError(p, req, 400, 'invalid_request', 'scope is required');
    let refresh: string | null = null;
    if (p.refreshRevokesAccess) family.access.clear();
    if (p.rotation === 'rotate') {
      family.spent.set(sent, now());
      refresh = token('rt');
      family.refresh = refresh;
      byRefresh.set(refresh, id!);
    }
    if (p.rotation === 'same') refresh = sent;
    count(profile, 'refresh-ok');
    return issue(profile, req, id!, null, refresh);
  }
  return tokenError(p, req, 400, 'unsupported_grant_type', String(grant));
}

function authorize(profile: string, url: URL): Response {
  const p = PROFILES[profile], q = url.searchParams;
  count(profile, 'authorize');
  const redirect = q.get('redirect_uri') ?? '';
  if (!redirect) return new Response('redirect_uri is required', { status: 400 });
  const state = q.get('state');
  const back = (params: Record<string, string>): Response => {
    const target = new URL(redirect);
    for (const [k, v] of Object.entries(params)) target.searchParams.append(k, v);
    if (state !== null) target.searchParams.append('state', state);
    if (p.issInCallback) target.searchParams.append('iss', issuerOf(url.origin, profile));
    return new Response(null, { status: 302, headers: { Location: target.toString() } });
  };
  if (q.get('response_type') !== 'code') return back({ error: 'unsupported_response_type' });
  const client = q.get('client_id') ?? '';
  if (!client) return back({ error: 'invalid_request', error_description: 'client_id is required' });
  const challenge = q.get('code_challenge'), method = q.get('code_challenge_method');
  if (p.requirePkce && (!challenge || method !== 'S256')) return back({ error: 'invalid_request', error_description: 'PKCE with S256 is required' });
  if (denyNext.delete(profile)) return back({ error: 'access_denied', error_description: 'The person declined' });
  const scope = q.get('scope') ?? '';
  if (scope.split(' ').some((s) => p.refusedScopes.includes(s))) return back({ error: 'invalid_scope', error_description: 'some of requested scopes are not whitelisted' });
  const when = p.refreshTokenWhen;
  const offline = when === 'always' || (when !== 'never' && ((when.scope !== undefined && scope.split(' ').includes(when.scope)) || (when.param !== undefined && q.get(when.param[0]) === when.param[1])));
  const familyId = token('fam');
  families.set(familyId, { profile, client, scope, subject: PEOPLE.default.sub, revoked: false, refresh: '', spent: new Map(), access: new Map() });
  const code = token('code');
  codes.set(code, { profile, client, redirect, challenge, nonce: q.get('nonce'), offline, used: false, family: familyId });
  return back({ code });
}

function bearer(req: Request): Family | 'expired' | null {
  const auth = req.headers.get('authorization') ?? '';
  const m = /^Bearer (.+)$/i.exec(auth);
  if (!m) return null;
  const id = byAccess.get(m[1]), family = id ? families.get(id) : undefined;
  if (!family || family.revoked) return null;
  const until = family.access.get(m[1]);
  if (until === undefined) return null;
  return until < now() ? 'expired' : family;
}

function resource(profile: string, req: Request): Response {
  count(profile, 'api');
  const f = bearer(req);
  if (!f || f === 'expired') {
    return new Response(JSON.stringify({ error: 'invalid_token' }), {
      status: 401,
      headers: { 'Content-Type': 'application/json', 'WWW-Authenticate': `Bearer error="invalid_token"`, 'Access-Control-Allow-Origin': '*' },
    });
  }
  count(profile, 'api-ok');
  return Response.json({ sub: f.subject, name: PEOPLE.default.name, email: PEOPLE.default.email }, { headers: { 'Access-Control-Allow-Origin': '*' } });
}

async function control(req: Request): Promise<Response> {
  const body = await req.json() as { action: string; profile?: string; status?: number; error?: string; codes?: number[]; count?: number; most?: Record<string, number>; least?: Record<string, number>; since?: boolean; held?: number; claims?: Record<string, unknown> };
  const profile = body.profile ?? '';
  switch (body.action) {
    case 'reset':
      for (const release of held.splice(0)) release();
      codes = new Map(); families = new Map(); byRefresh = new Map(); byAccess = new Map(); next = {}; denyNext = new Set(); counts = {}; marks = {}; clock = Date.now(); after = {}; omitType = new Set(); idClaims = {};
      break;
    case 'expire-access':
      for (const f of families.values()) if (!profile || f.profile === profile) for (const k of f.access.keys()) f.access.set(k, 0);
      break;
    case 'revoke':
      for (const f of families.values()) if (!profile || f.profile === profile) { f.revoked = true; f.access.clear(); }
      break;
    case 'fail-token':
      next[profile] = { fail: body.status ?? 503, count: body.count ?? 1 };
      break;
    case 'drop-token':
      next[profile] = { drop: true, count: body.count ?? 1 };
      break;
    case 'drop-token-after':
      next[profile] = { dropAfter: true, count: body.count ?? 1 };
      break;
    case 'hold-token':
      next[profile] = { hold: true, count: body.count ?? 1 };
      break;
    case 'release-token':
      for (const release of held.splice(0)) release();
      break;
    // A long-poll: answers once `held` token requests are held and the
    // profile's counts reach `least`, or 408 with what there is after 50 s, a
    // bound only for a hanging test, under the driver's own 60 s
    // (docs/testing.md).
    case 'await': {
      const ready = (): boolean => held.length >= (body.held ?? 0)
        && Object.entries(body.least ?? {}).every(([k, n]) => (counts[profile]?.[k] ?? 0) >= n);
      const deadline = Date.now() + 50_000;
      while (!ready()) {
        if (Date.now() > deadline) return Response.json({ held: held.length, counts: counts[profile] ?? {} }, { status: 408 });
        await new Promise<void>((wake) => { awaiting.push(wake); setTimeout(wake, deadline - Date.now()); });
      }
      break;
    }
    case 'id-claims':
      idClaims[profile] = body.claims ?? {};
      break;
    case 'error-token':
      next[profile] = { error: body.error ?? 'invalid_grant', codes: body.codes, count: body.count ?? 1 };
      break;
    case 'deny-authorize':
      denyNext.add(profile);
      break;
    case 'omit-token-type':
      omitType.add(profile);
      break;
    case 'mark':
      marks[profile] = { ...counts[profile] };
      break;
    // A server-side assertion an authored test makes through `request`: what
    // the profile received, in all or with `since` since its last `mark`, is
    // at most `most` (a reused refresh token, say) and at least `least` (a
    // renewal that happened); 417 with the counts fails the step.
    case 'expect': {
      const all = counts[profile] ?? {}, mark = body.since ? marks[profile] ?? {} : {};
      const got = Object.fromEntries(Object.entries(all).map(([k, n]) => [k, n - (mark[k] ?? 0)]));
      const over = Object.entries(body.most ?? {}).filter(([k, n]) => (got[k] ?? 0) > n);
      const under = Object.entries(body.least ?? {}).filter(([k, n]) => (got[k] ?? 0) < n);
      const named = (list: [string, unknown][]) => Object.fromEntries(list.map(([k]) => [k, got[k] ?? 0]));
      if (over.length || under.length) return Response.json({ over: named(over), under: named(under), counts: got }, { status: 417 });
      break;
    }
    default:
      return new Response(`unknown action ${body.action}`, { status: 400 });
  }
  return Response.json({ ok: true });
}

// RFC 7009 §2.1: revoking a valid token of a grant ends the grant, the refresh
// token with an access token (GOOG-16). A token already spent or replaced
// ends nothing, so a sign-out that sends one leaves the session alive; the
// counts say which kind each revocation sent.
function revoke(profile: string, req: Request, form: URLSearchParams): Response {
  const p = PROFILES[profile], headers = tokenCors(p), sent = form.get('token') ?? '';
  const request = checked(p, req, form);
  if ('refused' in request) return tokenError(p, req, ...request.refused);
  if (!form.has('token')) return tokenError(p, req, 400, 'invalid_request', 'token is required');
  const id = byRefresh.get(sent) ?? byAccess.get(sent), family = id ? families.get(id) : undefined;
  // A token is revoked only for the client it was issued to (RFC 7009 §2.1).
  if (family && family.profile === profile && family.client !== request.client) return tokenError(p, req, 400, 'invalid_grant', 'the token was issued to another client');
  if (!family || family.profile !== profile) {
    count(profile, 'revoke-unknown');
    return p.revokeUnknown === 400 ? Response.json({ error: 'invalid_token' }, { status: 400, headers }) : new Response(null, { status: 200, headers });
  }
  const current = !family.revoked && (family.refresh === sent || family.access.has(sent));
  count(profile, current ? 'revoke-current' : 'revoke-stale');
  if (current) { family.revoked = true; family.access.clear(); }
  return new Response(null, { status: 200, headers });
}

function discovery(origin: string, profile: string): Response {
  const p = PROFILES[profile], b = p.issuer.startsWith('{base}/oidc') ? issuerOf(origin, profile) : baseOf(origin, profile);
  // The token endpoint's origin: the next port for a profile whose provider keeps it apart.
  const t = p.tokenOnSecondOrigin ? b.replace(/:(\d+)\//, (_, n) => `:${Number(n) + 1}/`) : b;
  if (!p.discovery) return new Response('not found', { status: 404 });
  return Response.json({
    issuer: issuerOf(origin, profile),
    authorization_endpoint: `${b}/authorize`,
    token_endpoint: `${t}/token`,
    userinfo_endpoint: `${b}/userinfo`,
    revocation_endpoint: `${t}/revoke`,
    response_types_supported: ['code'],
    grant_types_supported: ['authorization_code', 'refresh_token'],
    code_challenge_methods_supported: ['S256'],
    scopes_supported: ['openid', 'profile', 'email', 'offline_access'],
    ...(p.issInCallback ? { authorization_response_iss_parameter_supported: true } : {}),
  }, { headers: { 'Access-Control-Allow-Origin': '*' } });
}

export function handle(req: Request): Response | Promise<Response> {
  const url = new URL(req.url);
  if (req.method === 'OPTIONS') {
    // A token endpoint that does not answer a browser fails its preflight too (GH-8).
    const at = /^\/p\/([a-z0-9-]+)\/.*\/?(token|revoke)$/.exec(url.pathname);
    if (at && PROFILES[at[1]] && !PROFILES[at[1]].tokenCors) return new Response(null, { status: 204 });
    return new Response(null, { status: 204, headers: { 'Access-Control-Allow-Origin': '*', 'Access-Control-Allow-Headers': 'authorization, content-type, accept', 'Access-Control-Allow-Methods': 'GET, POST' } });
  }
  // A test's `request POST "/control"` goes under the first local URL its app
  // names, which may be a profile's issuer: the control answers under any path.
  if (url.pathname.endsWith('/control') && req.method === 'POST') return control(req);
  if (url.pathname === '/control/counts') return Response.json(counts);
  const m = /^\/p\/([a-z0-9-]+)(\/.*)?$/.exec(url.pathname);
  if (!m || !PROFILES[m[1]]) return new Response('not found', { status: 404 });
  const profile = m[1];
  // Everything after the profile, with Entra's tenant segment and Auth0's slash left out.
  const rest = (m[2] ?? '/').replace(/^\/(common|organizations|consumers)\/v2\.0/, '').replace(/^\/oidc\/2/, '');
  if (rest === '/.well-known/openid-configuration' || rest === '//.well-known/openid-configuration') return discovery(url.origin, profile);
  if (rest === '/authorize' && req.method === 'GET') return authorize(profile, url);
  if (rest === '/token' && req.method === 'POST') return tokenEndpoint(profile, req);
  if ((rest === '/userinfo' || rest === '/api/me') && req.method === 'GET') return resource(profile, req);
  // An API that refuses a valid token, and one that redirects (the library follows no redirect).
  if (rest === '/api/forbidden' && req.method === 'GET') return new Response('forbidden', { status: 403, headers: { 'Access-Control-Allow-Origin': '*' } });
  if (rest === '/api/moved' && req.method === 'GET') return new Response(null, { status: 302, headers: { Location: `${url.origin}/p/${profile}/api/me`, 'Access-Control-Allow-Origin': '*' } });
  if (rest === '/revoke' && req.method === 'POST') {
    count(profile, 'revoke');
    const plan = next[profile];
    if (plan?.fail) { delete next[profile]; return new Response('unavailable', { status: plan.fail, headers: tokenCors(PROFILES[profile]) }); }
    return req.text().then((text) => revoke(profile, req, formOf(text)));
  }
  return new Response('not found', { status: 404 });
}

if (import.meta.main) {
  // The second origin serves the same handler: a profile's token endpoint
  // there answers as on the first (GOOG-1's oauth2.googleapis.com).
  // No idle timeout: a held token request or an `await` waits as long as its case holds it, where Bun cuts a request
  // its handler has not answered after 10 s.
  for (const p of [port, port + 1]) Bun.serve({ port: p, hostname: '127.0.0.1', idleTimeout: 0, fetch: handle });
  console.log(`oauth fixture on http://127.0.0.1:${port} and http://127.0.0.1:${port + 1}`);
}
