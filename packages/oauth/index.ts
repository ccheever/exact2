// @exact/oauth (LLP 1146 D5, D8, D9): an OAuth 2.0 and OpenID Connect client
// for `app.ts`. Authorization code with PKCE through `openAuthSession` or the
// app's own sign-in (`authorize`), the code exchange, refresh with rotation,
// authorized requests that refresh on 401 (`expiredStatus`), and sign-out.
// Every decision about what a provider's reply means is here, so an app
// writes none of them.
//
// Data code reads no clock (LLP 1027.000). The time a token was issued is the
// token response's `Date` header (RFC 9110 §6.6.1), and an expired access
// token is learned from the resource server's 401 (RFC 6750 §3.1), or the
// statuses `expiredStatus` names.
//
// A session's code exchange, refreshes and sign-out run one at a time
// (`exclusive`), so a rotated refresh token is spent once. Each is a POST,
// which runs to its end even when the answer that started it is let go, and
// what its reply's turn writes to the store stands (LLP 1146 D2).

/** What `session()` answers; `OAuthSession` in Contract. */
export interface OAuthSession {
  signedIn: boolean;
  /** The ID token's `sub`, or "" without one. */
  subject: string;
  /** When the access token expires, in epoch milliseconds by the provider's clock; 0 when unknown. */
  expiresAt: number;
  scopes: string[];
  /** Why the next sign-in cannot be prepared or the last session ended, for the person; "" otherwise. */
  error: string;
  /** The last session ended, by the provider or because the package could not go on with it: `error` says why; false for a sign-in that cannot be prepared. */
  ended: boolean;
}

/** What `signIn()` and `signOut()` answer; `OAuthResult` in Contract. */
export interface OAuthResult {
  ok: boolean;
  /** A stable code: "access_denied", "cancelled", "popup_blocked", "state_mismatch", "issuer_mismatch", "invalid_grant", "network", "server", "config", "invalid_response", or the provider's OAuth error code. */
  code: string;
  /** A sentence for the person; "" when ok. */
  error: string;
  /** Trying again later may succeed. */
  retryable: boolean;
  /** The HTTP status, 400 or over, of the reply that failed; 0 otherwise: no reply came, or the package refused a 2xx or 3xx it could not use. */
  status: number;
}

export interface OAuthConfig {
  /** The issuer. Discovery reads `<issuer>/.well-known/openid-configuration`, then RFC 8414's document. */
  issuer?: string;
  /** Endpoints for a provider without discovery. With an exact `issuer` (no `issuerTemplate`), `authorization` and `token` both given make no discovery: the ID token and a callback's `iss` are still checked against `issuer`, a callback without `iss` is taken, and sign-out revokes only at `revocation`. Otherwise they override what discovery found. */
  endpoints?: { authorization?: string; token?: string; revocation?: string };
  clientId: string;
  scopes: string[];
  /** The `secret.keep` name the session is kept under. */
  key: string;
  /** The origins that receive the access token, such as `https://api.example.com`. */
  apis: string[];
  /** Only for a provider whose app client types demand one; a secret in an app is not confidential (RFC 8252 §8.5). */
  clientSecret?: string;
  /** How `clientSecret` is sent (RFC 6749 §2.3.1): in the body ("post", the default) or as HTTP Basic ("basic"). */
  tokenAuth?: 'post' | 'basic';
  /** Extra authorization request parameters a provider documents (`access_type`, `prompt`, `audience`). */
  authorizationParams?: Record<string, string>;
  /** Send `scope` with each refresh, for a provider that requires it. */
  refreshScopes?: boolean;
  /** Other spellings of the issuer the ID token's `iss` may use (Google's `accounts.google.com`). */
  issuerAliases?: string[];
  /** Audiences an ID token may name beside `clientId`; one naming any other is refused (OIDC Core §3.1.3.7, step 3). */
  trustedAudiences?: string[];
  /** The discovery document's `issuer` names `{tenantid}`, filled from the ID token's `tid` (Entra's `common`). */
  issuerTemplate?: boolean;
  /** Token endpoint errors that also mean the session is over: a code, or a code with the status it comes with (OneLogin's `{ error: 'invalid_request', status: 401 }`, GitHub's `'bad_refresh_token'`). */
  endedErrors?: (string | { error: string; status: number })[];
  /** Retry a refresh once at once when no reply came, for a provider set to accept the previous refresh token for a while (Okta's grace period, Auth0's rotation overlap). */
  retryLostRefresh?: boolean;
  /** Read `scope` in a token reply as comma-separated (GitHub). */
  commaScopes?: boolean;
  /** Take a token reply without `token_type` as Bearer (LinkedIn, Notion). */
  bearerByDefault?: boolean;
  /** Callback errors that mean the person declined (Apple's `user_cancelled_authorize`). */
  cancelCodes?: string[];
  /** Ask the browser for a private session with no shared cookies, to choose another account. */
  ephemeral?: boolean;
  /** The callback; the default is `authCallback()`. With `signIn`'s `authorize`, the redirect URI the provider registered. */
  callback?: string;
  /** The statuses an API answers an expired access token with: `[401]` by default (RFC 6750 §3.1); `[401, 403]` for a gateway that answers 403. `fetch` and `json` refresh once on each. A token an API refused right after the refresh that brought it refreshes again only once it has expired by the API's `Date`, or, its expiry unknown, a minute after that refusal; where the app cannot read the API's `Date` (from another origin, a browser shows it only when the API exposes it), each read the API refuses refreshes once, as without this limit. */
  expiredStatus?: number[];
  /** The transport of every request the package makes; `fetch` by default. For a demo's server in the app, or a provider's cookies. */
  fetch?: (url: string, init: RequestInit) => Promise<Response>;
  /** Hears what the session did, for an app's diagnostics (`OAuthEvent`). */
  log?: (event: OAuthEvent) => void;
}

/**
 * What the session did, for `log`. It carries no token: `token`, `sent` and
 * `got` are refresh tokens' prints, the first six hex digits of their SHA-256
 * ("" for none). `refreshed`'s `expiresIn` is in seconds, 0 when unknown.
 * At `refresh-failed` the session stands; the event carries the reply's
 * status (400 or over, else 0) and OAuth error, or status 0 and `network`
 * when none came (`config` when the grants refused the request). At `ended`
 * the session ended, the provider's refusal or the package's. `refused`: an
 * API refused the access token again after a refresh.
 */
export type OAuthEvent =
  | { type: 'signed-in'; token: string }
  | { type: 'refreshed'; sent: string; got: string; expiresIn: number }
  | { type: 'refresh-failed'; sent: string; status: number; code: string }
  | { type: 'ended'; sent: string; status: number; code: string }
  | { type: 'refused'; status: number; url: string }
  | { type: 'signed-out' };

/** The authorization request a sign-in sends, as `authorize` receives it. */
export interface OAuthAuthorization {
  /** The whole request: the endpoint with `params` in its query. */
  url: string;
  /** The authorization endpoint. */
  endpoint: string;
  /** `response_type`, `client_id`, `redirect_uri`, `scope`, `state`, the PKCE challenge, `nonce` with `openid`, and `authorizationParams`. */
  params: Record<string, string>;
  /** The package's transport (`OAuthConfig.fetch`, else `fetch`), to send it with. */
  fetch: (url: string, init: RequestInit) => Promise<Response>;
}

export interface OAuthSignIn {
  /** For an app that signs the person in with its own screens (LLP 1146 D8): sends the authorization request as the provider documents, and answers the URL it redirected to. Throw an `OAuthError` to say why it could not. */
  authorize?: (request: OAuthAuthorization) => Promise<string>;
}

/** A failed authorized request. A resource source that throws it keeps its last answer (LLP 1016 D4). */
export class OAuthError extends Error {
  code: string;
  status: number;
  retryable: boolean;
  /** The person must sign in again. */
  signedOut: boolean;
  constructor(code: string, message: string, status = 0, retryable = false, signedOut = false) {
    super(message);
    // An own property: a native host's hardened realm freezes
    // `Error.prototype`, whose `name` then refuses an assignment.
    Object.defineProperty(this, 'name', { value: 'OAuthError', writable: true, configurable: true });
    this.code = code;
    this.status = status;
    this.retryable = retryable;
    this.signedOut = signedOut;
  }
}

/** The store as an app's sources receive it. */
export interface OAuthStore {
  get(name: string): string | null;
  set(name: string, value: string): void;
  forget(name: string): void;
}

export interface OAuthProvider {
  /** The session the store keeps. While signed out it also readies the next sign-in, which may fetch the discovery document. */
  session(store: OAuthStore): Promise<OAuthSession>;
  /** Opens the provider's sign-in, then exchanges and keeps the tokens. Send it from a press. With `authorize`, the app's own sign-in sends the authorization request instead. */
  signIn(store: OAuthStore, options?: OAuthSignIn): Promise<OAuthResult>;
  /** Forgets the session, then asks the provider to revoke it. */
  signOut(store: OAuthStore): Promise<OAuthResult>;
  /** `fetch` with the access token, to an origin in `apis`; refreshes once on 401 (`expiredStatus`). Answers the `Response`. */
  fetch(store: OAuthStore, url: string, init?: RequestInit): Promise<Response>;
  /** As `fetch`, then the parsed JSON body; any status that is not 2xx throws `OAuthError`. */
  json(store: OAuthStore, url: string, init?: RequestInit): Promise<unknown>;
  /** The kept ID token's claims; `null` while signed out or with none. */
  claims(store: OAuthStore): Record<string, unknown> | null;
}

declare function openAuthSession(url: string, options: { callback: string; state: string; ephemeral?: boolean }): Promise<string>;
declare function authCallback(): string;

interface Metadata {
  issuer: string;
  authorization: string;
  token: string;
  revocation: string;
  /** RFC 9207 §3: the server puts `iss` in every authorization response. */
  issInCallback: boolean;
}

/** What `key` holds: a session, or why the last one ended. */
interface Kept {
  issuer?: string;
  /** The first ID token's claims, which a refreshed one must match (OIDC Core §12.2). */
  first?: Record<string, unknown>;
  accessToken?: string;
  refreshToken?: string;
  idToken?: string;
  expiresAt?: number;
  scopes?: string[];
  subject?: string;
  /** The client the session was signed in for. */
  client?: string;
  tokenEndpoint?: string;
  revocationEndpoint?: string;
  ended?: string;
}

interface Transaction {
  state: string;
  verifier: string;
  nonce: string;
  params: Record<string, string>;
  callback: string;
  meta: Metadata;
}

const SKEW_MS = 5 * 60 * 1000;
const TOKEN_TIMEOUT_MS = 60_000;
// OpenID Connect's codes for "sign in again" (Core §3.1.2.6); `invalid_grant`
// (RFC 6749 §5.2) is decided by `ends`.
const SIGN_IN_AGAIN = ['interaction_required', 'login_required', 'account_selection_required', 'consent_required'];
const CONFIG = ['invalid_client', 'unauthorized_client', 'unsupported_grant_type', 'invalid_scope', 'invalid_request'];
// Authorization request parameters the package sets and an app may not override.
const RESERVED = ['response_type', 'client_id', 'redirect_uri', 'scope', 'state', 'code_challenge', 'code_challenge_method', 'nonce', 'max_age'];

// Each session's work in flight, and the sessions sign-outs forgot but have
// not revoked, oldest first, by the key the session is kept under.
const sessionWork = new Map<string, Promise<unknown>>();
const unrevoked = new Map<string, Kept[]>();
// By session key, the access token an API last refused right after the refresh that brought it, and that refusal's
// `Date` (the API's clock, 0 without one)
const refusedFresh = new Map<string, { token: string; at: number }>();
// By session key, its events still to be heard, in the order they happened across every provider made for the key,
// though a print is made asynchronously
const logged = new Map<string, Promise<unknown>>();

export function oauth(config: OAuthConfig): OAuthProvider {
  if (!config.clientId) throw new TypeError('oauth: clientId is required');
  if (!config.key) throw new TypeError('oauth: key, the secret.keep name the session is kept under, is required');
  if (!config.issuer && !(config.endpoints?.authorization && config.endpoints?.token)) {
    throw new TypeError('oauth: give issuer (for discovery) or endpoints.authorization and endpoints.token');
  }
  if (!Array.isArray(config.apis)) throw new TypeError('oauth: apis lists the origins that receive the access token, such as ["https://api.example.com"]');
  for (const url of [config.issuer, config.endpoints?.authorization, config.endpoints?.token, config.endpoints?.revocation, ...config.apis]) {
    if (url) secure(url, 'oauth');
  }
  // The callback is compared whole with where the provider sends the person, its query and fragment the reply's
  if (config.callback !== undefined && !sessionCallback(config.callback)) {
    throw new TypeError('oauth: callback is the redirect URI, an absolute URL with no whitespace, userinfo, query or fragment, and for http: or https: a host and no backslash, as the host opens no other and the provider adds its own query');
  }
  const reserved = Object.keys(config.authorizationParams ?? {}).filter((k) => RESERVED.includes(k));
  if (reserved.length) throw new TypeError(`oauth: authorizationParams may not set ${reserved.join(', ')}; the package sets them, or does not check what they ask for (max_age)`);
  const expired = config.expiredStatus ?? [401];
  if (!expired.length || !expired.every((s) => Number.isInteger(s) && s >= 400 && s < 500)) {
    throw new TypeError('oauth: expiredStatus lists the 4xx statuses an API answers an expired access token with, such as [401, 403]');
  }
  const apis = config.apis.map(originOf);
  const note = (event: () => OAuthEvent | Promise<OAuthEvent>): void => {
    const log = config.log;
    if (log) logged.set(config.key, (logged.get(config.key) ?? Promise.resolve()).then(event).then(log).catch(() => undefined));
  };
  // Whether a token endpoint error ends the session (LLP 1146 §5, "Refresh").
  const ends = (reply: TokenReply): boolean => {
    if (reply.error === 'invalid_grant') return !throttled(reply);
    return SIGN_IN_AGAIN.includes(reply.error) || (config.endedErrors ?? []).some((e) =>
      typeof e === 'string' ? e === reply.error : e.error === reply.error && e.status === reply.status);
  };
  // The session's work, one at a time across every provider made for its
  // key: each caller waits in its own frame for the work in flight to end,
  // then runs its own there, so the requests it makes are its own answer's.
  const exclusive = async <T>(work: () => Promise<T>): Promise<T> => {
    for (let busy = sessionWork.get(config.key); busy; busy = sessionWork.get(config.key)) {
      await busy.then(() => undefined, () => undefined);
    }
    const run = work();
    sessionWork.set(config.key, run);
    try { return await run; } finally { if (sessionWork.get(config.key) === run) sessionWork.delete(config.key); }
  };
  let discovered: Promise<Metadata> | null = null;
  let prepared: Transaction | null = null;
  let preparing: Promise<Transaction> | null = null;
  // Why the last preparation failed, for `session().error`.
  let unprepared = '';

  const metadata = (): Promise<Metadata> => {
    discovered ??= discover(config).catch((e) => { discovered = null; throw e; });
    return discovered;
  };

  const read = (store: OAuthStore): Kept | null => {
    const text = store.get(config.key);
    if (!text) return null;
    try { return JSON.parse(text) as Kept; } catch { return null; }
  };

  // Each caller awaits the preparation in flight. One another answer started
  // is that answer's discovery, cancelled if that answer is let go: then this
  // caller starts it again itself.
  const prepare = async (): Promise<Transaction> => {
    for (let tries = 0; ; tries++) {
      if (prepared) return prepared;
      const mine = !preparing;
      preparing ??= prepareOnce();
      try { return await preparing; } catch (e) { if (mine || tries >= 2) throw e; }
    }
  };

  const prepareOnce = (): Promise<Transaction> => (async () => {
    const meta = await metadata();
    const state = random(32), verifier = random(32);
    const nonce = config.scopes.includes('openid') ? random(16) : '';
    const callback = config.callback ?? hostCallback();
    const params: Record<string, string> = {
      response_type: 'code',
      client_id: config.clientId,
      redirect_uri: callback,
      scope: config.scopes.join(' '),
      state,
      code_challenge: await challenge(verifier),
      code_challenge_method: 'S256',
      ...(nonce ? { nonce } : {}),
      ...config.authorizationParams,
    };
    prepared = { state, verifier, nonce, params, callback, meta };
    return prepared;
  })().then((tx) => { unprepared = ''; return tx; }, (e) => {
    // A network failure is an `OAuthError` here, so anything else is named as it is
    unprepared = e instanceof OAuthError ? e.message : `Sign-in cannot be prepared: ${(e as Error)?.message ?? String(e)}`;
    throw e;
  }).finally(() => { preparing = null; });

  const session = async (store: OAuthStore): Promise<OAuthSession> => {
    const kept = read(store);
    if (kept?.accessToken) {
      return { signedIn: true, subject: kept.subject ?? '', expiresAt: kept.expiresAt ?? 0, scopes: kept.scopes ?? [], error: '', ended: false };
    }
    // Ready the next sign-in now, so that the press opens the browser before
    // awaiting anything: a web popup must open in the press's task
    // (LLP 1069.006 D4), and discovery and the S256 digest are asynchronous.
    await prepare().catch(() => undefined);
    return { signedIn: false, subject: '', expiresAt: 0, scopes: [], error: kept?.ended ?? unprepared, ended: kept?.ended !== undefined };
  };

  // The app's own sign-in sends the authorization request (D8): no browser
  // opens, so it may await. What it answers must be the callback.
  const sentByApp = async (tx: Transaction | null, authorize: NonNullable<OAuthSignIn['authorize']>): Promise<[Transaction, string] | OAuthResult> => {
    try {
      tx ??= await prepare();
      prepared = null;
      const back = await authorize({ url: authorizationUrl(tx), endpoint: tx.meta.authorization, params: { ...tx.params }, fetch: transport(config) });
      if (typeof back === 'string' && isCallback(back, tx.callback)) return [tx, back];
      return result('invalid_response', 'Sign-in failed: the provider did not send the person back to the app.');
    } catch (e) {
      return failure(e);
    }
  };

  // The browser sends it. Prepared, `openAuthSession` comes before any
  // await; otherwise it is prepared first, which on the web may leave the
  // popup blocked (428).
  const openedInBrowser = async (tx: Transaction | null): Promise<[Transaction, string] | OAuthResult> => {
    let opened: Promise<string>;
    let readied: Promise<void> = Promise.resolve();
    try {
      const fallback = !tx;
      if (!tx) {
        tx = await prepare();
        prepared = null;
      }
      opened = openAuthSession(authorizationUrl(tx), { callback: tx.callback, state: tx.state, ephemeral: config.ephemeral });
      // Ready the next press, which then opens at once (a blocked popup's retry).
      if (fallback) readied = prepare().then(() => undefined, () => undefined);
    } catch (e) {
      return failure(e);
    }
    try {
      return [tx, await opened];
    } catch (e) {
      const status = (e as { status?: number }).status;
      if (status === 499) return result('cancelled', 'Sign-in was cancelled.');
      if (status === 428) {
        // The next press must open at once, so the next sign-in is ready first.
        await readied;
        return result('popup_blocked', 'The sign-in window was blocked. Tap Sign In again.', true);
      }
      if (status === 403 || status === 501) return result('config', `Sign-in cannot open here: ${(e as Error).message}`);
      return result('network', 'Sign-in could not finish. Try again.', true);
    }
  };

  const signIn = async (store: OAuthStore, options: OAuthSignIn = {}): Promise<OAuthResult> => {
    const ready = prepared;
    prepared = null;
    const reached = options.authorize ? await sentByApp(ready, options.authorize) : await openedInBrowser(ready);
    if (!Array.isArray(reached)) return reached;
    const [tx, back] = reached;
    const why = checkCallback(back, tx, config);
    if (why) return why;
    const code = callbackQuery(back).get('code')!;
    const answeredBy = callbackQuery(back).get('iss') ?? '';
    // Once sent, the exchange is a POST, which runs to its end even when
    // this sign-in is superseded, and keeps the tokens its code bought.
    return exclusive(async () => {
      let reply: TokenReply;
      try {
        reply = await tokenRequest(config, tx.meta.token, { grant_type: 'authorization_code', code, redirect_uri: tx.callback, code_verifier: tx.verifier });
      } catch (e) {
        return failure(e);
      }
      if (reply.error) return result(reply.error, describe(reply), retryable(reply, ends(reply)), reply.status);
      const kept = keep(reply, null, tx.meta, config);
      if (typeof kept === 'string') return result('invalid_response', `Sign-in failed: ${kept}.`, true);
      if (config.scopes.includes('openid') && !kept.idToken) return result('invalid_response', 'Sign-in failed: the provider sent no ID token.');
      if (kept.idToken) {
        const bad = checkIdToken(kept.idToken, tx.meta, config, tx.nonce, reply.date, null);
        if (bad) return result('invalid_response', `Sign-in failed: ${bad}.`);
        const c = claims(kept.idToken)!;
        // For a template issuer, the tenant the callback named is the one that
        // issued the token (RFC 9207). An exact issuer's callback and ID token
        // were each checked against the issuer, an ID token's alias included
        // (Google's `accounts.google.com`, GOOG-2).
        if (answeredBy && tx.meta.issuer.includes('{tenantid}') && c.iss !== answeredBy) return result('issuer_mismatch', 'Sign-in failed: the reply came from another server.');
        kept.subject = String(c.sub);
        kept.client = config.clientId;
        kept.first = { iss: c.iss, sub: c.sub, aud: c.aud, azp: c.azp, nonce: c.nonce };
        // Signed in again as the same account of the same issuer, for the same
        // client: a reply with no refresh token keeps the one held (Google
        // sends one only on first consent, GOOG-8). A subject is the issuer's
        // own (OIDC Core §8), and a refresh token the client's (RFC 6749 §6).
        const held = read(store);
        const same = held?.first?.iss === c.iss && held?.subject === kept.subject && held?.client === config.clientId;
        // The refresh token keeps the sign-in it came from: its refreshed ID
        // tokens are checked against that one's claims, its `nonce` included.
        if (!kept.refreshToken && held?.refreshToken && same) Object.assign(kept, { refreshToken: held.refreshToken, first: held.first });
      }
      store.set(config.key, JSON.stringify(kept));
      note(async () => ({ type: 'signed-in', token: await print(kept.refreshToken) }));
      return { ok: true, code: '', error: '', retryable: false, status: 0 };
    });
  };

  // Renews `seen`'s access token: one refresh of this session at a time, its
  // rotated refresh token kept before anyone uses the new access token (RFC
  // 9700 §4.14.2).
  const refresh = (store: OAuthStore, seen: Kept): Promise<Kept> => exclusive(async () => {
    const now = read(store);
    // Another caller refreshed, or the person signed out or in, meanwhile.
    if (!now?.accessToken) throw signedOutError(now);
    if (now.refreshToken !== seen.refreshToken || now.accessToken !== seen.accessToken) return now;
    const sent = now.refreshToken;
    if (!sent) {
      end(store, sent, 'The session has expired. Sign in again.', 0, 'invalid_grant');
      throw new OAuthError('invalid_grant', 'The session has expired. Sign in again.', 401, false, true);
    }
    // What the session keeps is enough: a refresh awaits no discovery,
    // which an answer may have started and may be let go mid-way.
    const endpoint = now.tokenEndpoint;
    if (!endpoint) throw new OAuthError('config', 'The session keeps no token endpoint. Sign in again.', 0, false, true);
    const body: Record<string, string> = { grant_type: 'refresh_token', refresh_token: sent };
    if (config.refreshScopes) body.scope = (now.scopes?.length ? now.scopes : config.scopes).join(' ');
    // No usable reply (none, or a 2xx whose body stops short): the server may
    // have spent the token. With `retryLostRefresh`, one immediate retry falls
    // inside the window such a provider gives (Okta's grace period, Auth0's
    // rotation overlap), from an answer still asking (a let-go one makes no
    // request); otherwise the session is kept and the call fails, retryable.
    const attempt = async (): Promise<TokenReply> => {
      const r = await tokenRequest(config, endpoint, body);
      if (r.error === 'invalid_response' && r.status >= 200 && r.status < 300) {
        throw new OAuthError('network', `${originOf(endpoint)}'s reply was cut off.`, r.status, true);
      }
      return r;
    };
    // A failure that keeps the session, as `log` hears it
    const failed = (status: number, code: string): void => note(async () => ({ type: 'refresh-failed', sent: await print(sent), status: failedStatus(status), code }));
    let reply: TokenReply | undefined;
    for (let tries = 0; !reply; tries++) {
      try {
        reply = await attempt();
      } catch (e) {
        if (tries === 0 && config.retryLostRefresh && e instanceof OAuthError && e.code === 'network') continue;
        failed(e instanceof OAuthError ? e.status : 0, e instanceof OAuthError ? e.code : 'network');
        throw e;
      }
    }
    if (reply.error) {
      if (ends(reply)) {
        const reason = SIGN_IN_AGAIN.includes(reply.error) ? 'The provider asks you to sign in again.' : 'The provider ended the session. Sign in again.';
        end(store, sent, reason, reply.status, reply.error);
        throw new OAuthError(reply.error, reason, reply.status, false, true);
      }
      failed(reply.status, reply.error);
      throw new OAuthError(reply.error, describe(reply), reply.status, retryable(reply, false));
    }
    // The person signed out, or in again, while the request was out: what is
    // stored now stands, and this reply is dropped.
    const after = read(store);
    if (after?.refreshToken !== sent) {
      if (!after?.accessToken) throw signedOutError(after);
      return after;
    }
    // An ID token the reply carries, an unchanged one too, is checked before
    // anything in the reply is kept, against the session's issuer and first
    // ID token (OpenID Connect Core §12.2): a reply that fails is not this
    // session's, so none of its tokens is used, and the session ends.
    const idToken = reply.fields.id_token;
    if (idToken !== undefined) {
      const meta: Metadata = { issuer: now.issuer ?? '', authorization: '', token: endpoint, revocation: '', issInCallback: false };
      const first = now.first ?? (now.idToken ? claims(now.idToken) : null);
      const bad = typeof idToken === 'string' && idToken ? checkIdToken(idToken, meta, config, '', reply.date, first) : 'the ID token is unreadable';
      if (bad) {
        const reason = `The session cannot continue: ${bad}. Sign in again.`;
        end(store, sent, reason, reply.status, 'invalid_response');
        throw new OAuthError('invalid_response', reason, reply.status, false, true);
      }
    }
    // A provider that rotates has spent `sent` by now: a new refresh token in
    // a 2xx reply is kept even when the rest of it is unusable.
    const rotated = typeof reply.fields.refresh_token === 'string' && reply.fields.refresh_token ? reply.fields.refresh_token : '';
    const next = keep(reply, now, null, config);
    if (typeof next === 'string') {
      if (rotated) store.set(config.key, JSON.stringify({ ...now, refreshToken: rotated } satisfies Kept));
      failed(reply.status, 'invalid_response');
      throw new OAuthError('invalid_response', `The provider's reply was unreadable: ${next}.`, reply.status, true);
    }
    store.set(config.key, JSON.stringify(next));
    note(async () => ({ type: 'refreshed', sent: await print(sent), got: await print(next.refreshToken), expiresIn: lifetime(reply) ?? 0 }));
    return next;
  });

  // Ends the session for good, only if the refresh token refused is still
  // the one kept: a newer sign-in or a sign-out already replaced it otherwise.
  // `status` and `code` are the refusal's, for `log`.
  const end = (store: OAuthStore, refused: string | undefined, reason: string, status: number, code: string): void => {
    if (read(store)?.refreshToken !== refused) return;
    store.set(config.key, JSON.stringify({ ended: reason } satisfies Kept));
    note(async () => ({ type: 'ended', sent: await print(refused), status: failedStatus(status), code }));
  };

  // Until an access token an API refused right after the refresh that brought it has expired by the API's `Date`, or,
  // its expiry unknown, a minute has passed, its refusals refresh no more: reads a gateway keeps refusing would
  // otherwise each refresh in turn, spending a rotating refresh token at every round trip.
  const dateOf = (response: Response): number => Date.parse(response.headers.get('date') ?? '') || 0;
  const refreshes = (response: Response, kept: Kept): boolean => {
    const refused = refusedFresh.get(config.key), date = dateOf(response);
    if (!refused || refused.token !== kept.accessToken || !date) return true;
    return kept.expiresAt ? date >= kept.expiresAt : date >= refused.at + 60_000;
  };

  const authorizedFetch = async (store: OAuthStore, url: string, init: RequestInit = {}): Promise<Response> => {
    secure(url, 'fetch');
    if (!apis.includes(originOf(url))) {
      throw new OAuthError('config', `oauth: ${originOf(url)} is not in apis, so it does not receive the access token; add it to apis if it should`);
    }
    let kept = read(store);
    if (!kept?.accessToken) throw signedOutError(kept);
    for (let attempt = 0; ; attempt++) {
      const headers = new Headers(init.headers);
      headers.set('Authorization', `Bearer ${kept.accessToken}`);
      let response: Response;
      try {
        response = await transport(config)(url, { ...init, headers, redirect: 'manual' });
      } catch (e) {
        throw grantOrNetwork(e, url);
      }
      if (!expired.includes(response.status)) return response;
      if (attempt > 0) {
        // Only while it is the session's token: one a later refresh replaced says nothing of the token in use
        if (read(store)?.accessToken === kept.accessToken) refusedFresh.set(config.key, { token: kept.accessToken ?? '', at: dateOf(response) });
        note(() => ({ type: 'refused', status: response.status, url }));
        return response;
      }
      // A refusal of a token the session has since replaced always goes to `refresh`, which takes the newer one
      if (read(store)?.accessToken === kept.accessToken && !refreshes(response, kept)) return response;
      kept = await refresh(store, kept);
    }
  };

  const json = async (store: OAuthStore, url: string, init?: RequestInit): Promise<unknown> => {
    const response = await authorizedFetch(store, url, init);
    if (!response.ok) {
      throw new OAuthError(expired.includes(response.status) ? 'unauthorized' : 'server', `${originOf(url)} answered ${response.status}.`, response.status, response.status === 429 || response.status >= 500);
    }
    try { return await response.json(); } catch {
      throw new OAuthError('invalid_response', `${originOf(url)}'s reply is not JSON.`, response.status, true);
    }
  };

  // After the session's work in flight, so it waits for a refresh: forgets the
  // session, then revokes its newest token. A token whose revoke gets no reply
  // (offline, or a superseded sign-out, whose request is refused) waits in
  // `unrevoked` for the next sign-out, which revokes every waiting token,
  // newest first, and stops at the first revoke with no reply.
  const signOut = (store: OAuthStore): Promise<OAuthResult> => exclusive(async () => {
    const kept = read(store), owed = unrevoked.get(config.key) ?? [];
    const token = (k: Kept): string | undefined => k.refreshToken ?? k.accessToken;
    if (kept && token(kept) && !owed.some(k => token(k) === token(kept))) owed.push(kept);
    unrevoked.set(config.key, owed);
    store.forget(config.key);
    note(() => ({ type: 'signed-out' }));
    while (owed.length) {
      const last = owed[owed.length - 1], endpoint = last.revocationEndpoint;
      // Best effort (RFC 7009 §2.2): the session is already gone here. No
      // reply, or 429 or 5xx, leaves the token to revoke later (§2.2.1).
      const done = (reply: TokenReply): boolean => reply.status !== 429 && reply.status < 500;
      if (endpoint && !await tokenRequest(config, endpoint, { token: token(last)!, token_type_hint: last.refreshToken ? 'refresh_token' : 'access_token' }).then(done, () => false)) break;
      owed.pop();
    }
    if (!owed.length) unrevoked.delete(config.key);
    return { ok: true, code: '', error: '', retryable: false, status: 0 };
  });

  const idClaims = (store: OAuthStore): Record<string, unknown> | null => {
    const kept = read(store);
    return kept?.accessToken && kept.idToken ? claims(kept.idToken) : null;
  };

  return { session, signIn, signOut, fetch: authorizedFetch, json, claims: idClaims };
}

// --- the callback (RFC 6749 §4.1.2, RFC 9207 §2.4) -------------------------

function checkCallback(back: string, tx: Transaction, config: OAuthConfig): OAuthResult | null {
  const q = callbackQuery(back);
  const states = q.getAll('state');
  if (states.length !== 1 || states[0] !== tx.state) return result('state_mismatch', 'Sign-in failed: the reply was for another sign-in.');
  // The issuer before anything the reply says is trusted (RFC 9207 §2.4):
  // equal to the issuer, or, for a template, the template with one tenant in it.
  const iss = q.getAll('iss'), issuer = tx.meta.issuer;
  const named = (v: string): boolean => (issuer.includes('{tenantid}') ? templateMatches(issuer, v) : v === issuer);
  if (issuer && (iss.length > 1 || (iss.length === 1 && !named(iss[0])) || (iss.length === 0 && tx.meta.issInCallback))) {
    return result('issuer_mismatch', 'Sign-in failed: the reply came from another server.');
  }
  const errors = q.getAll('error');
  if (errors.length) {
    if (errors.length > 1 || q.has('code')) return result('invalid_response', 'Sign-in failed: the reply was malformed.');
    const error = errors[0];
    if (error === 'access_denied' || config.cancelCodes?.includes(error)) return result('access_denied', 'Sign-in was declined.');
    return result(error, `Sign-in failed (${error}).`);
  }
  const codes = q.getAll('code');
  if (codes.length !== 1 || !codes[0]) return result('invalid_response', 'Sign-in failed: the reply had no code.');
  return null;
}

function authorizationUrl(tx: Transaction): string {
  return tx.meta.authorization + (tx.meta.authorization.includes('?') ? '&' : '?') + form(tx.params);
}

// Whether `url` is `callback` with a query or fragment, compared as the host compares a callback (runner/src/auth.rs
// `accept`): the scheme and host without ASCII case, no userinfo, the path exactly
function isCallback(url: string, callback: string): boolean {
  const got = callbackParts(url), want = callbackParts(callback);
  if (!got || !want || got.authority?.includes('@')) return false;
  return got.scheme === want.scheme && got.authority === want.authority && got.path === want.path;
}

// Whether the host takes `callback` as a session's (runner/src/auth.rs `Session::from_json`): read as it reads a callback,
// with no query, fragment or userinfo, and for http: or https: a host and no backslash
function sessionCallback(callback: string): boolean {
  const parts = callbackParts(callback);
  if (!parts || /[?#]/.test(callback) || parts.authority?.includes('@')) return false;
  return (parts.scheme !== 'http' && parts.scheme !== 'https') || (parts.authority !== undefined && !callback.includes('\\'));
}

// ASCII letters lowercased and nothing else, as Rust's `to_ascii_lowercase`
const asciiLower = (text: string): string => text.replace(/[A-Z]+/g, (letters) => letters.toLowerCase());

// What Rust's `char::is_whitespace` and `char::is_control` match: Unicode's White_Space and the C0 and C1 controls
const WHITESPACE_OR_CONTROL = /[\u0000-\u0020\u007f-\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/;

// A URL's scheme and authority, their ASCII letters lowercased, and its path, read as the host reads a callback
// (runner/src/auth.rs `Url::parse`), or null for one the host refuses: a scheme that is not RFC 3986's, whitespace or a control character anywhere, an
// empty authority, or, without an authority, a path that is not absolute
function callbackParts(url: string): { scheme: string; authority?: string; path: string } | null {
  const colon = url.indexOf(':'), given = url.slice(0, colon);
  if (colon < 0 || !/^[a-z][a-z0-9+.-]*$/i.test(given) || WHITESPACE_OR_CONTROL.test(url)) return null;
  const scheme = asciiLower(given);
  const rest = url.slice(colon + 1).split('#')[0].split('?')[0];
  if (!rest.startsWith('//')) return rest.startsWith('/') ? { scheme, path: rest } : null;
  const after = rest.slice(2), slash = after.indexOf('/'), end = slash < 0 ? after.length : slash;
  return end ? { scheme, authority: asciiLower(after.slice(0, end)), path: after.slice(end) } : null;
}

// --- discovery (OIDC Discovery 1.0 §4, RFC 8414 §3) -----------------------

async function discover(config: OAuthConfig): Promise<Metadata> {
  const fixed = config.endpoints ?? {};
  // Fixed endpoints need no discovery; an exact issuer beside them is still what the ID token and the callback's `iss` are
  // checked against. A template issuer's `{tenantid}` form is the discovery document's, so it still discovers.
  if (!config.issuer || (fixed.authorization && fixed.token && !config.issuerTemplate)) {
    return { issuer: config.issuer ?? '', authorization: fixed.authorization!, token: fixed.token!, revocation: fixed.revocation ?? '', issInCallback: false };
  }
  const issuer = config.issuer;
  const origin = originOf(issuer), path = issuer.replace(/^[a-z][a-z0-9+.-]*:\/\/[^/?#]+/i, '').replace(/\/$/, '');
  let doc: Record<string, unknown> | null = null;
  // OpenID Connect's location, then RFC 8414's, which puts the path last.
  for (const url of [`${issuer.replace(/\/$/, '')}/.well-known/openid-configuration`, `${origin}/.well-known/oauth-authorization-server${path}`]) {
    let response: Response;
    try {
      response = await transport(config)(url, { headers: { Accept: 'application/json' } });
    } catch (e) {
      throw grantOrNetwork(e, url);
    }
    if (response.status === 404) continue;
    if (!response.ok) throw new OAuthError('server', `${origin} answered ${response.status} to discovery.`, response.status, response.status === 429 || response.status >= 500);
    try { doc = await response.json() as Record<string, unknown>; } catch {
      throw new OAuthError('invalid_response', `${origin}'s discovery document is not JSON.`, response.status, true);
    }
    break;
  }
  if (!doc) throw new OAuthError('config', `${issuer} publishes no discovery document; give endpoints instead.`);
  const named = String(doc.issuer ?? '');
  // The document's issuer is the one asked for, exactly (OIDC Discovery §4.3).
  if (named !== issuer && !(config.issuerTemplate && templateMatches(named, issuer))) {
    throw new OAuthError('config', `${issuer}'s discovery document names another issuer, ${named}.`);
  }
  const text = (k: string): string => (typeof doc![k] === 'string' ? doc![k] as string : '');
  const meta: Metadata = {
    issuer: named,
    authorization: fixed.authorization ?? text('authorization_endpoint'),
    token: fixed.token ?? text('token_endpoint'),
    revocation: fixed.revocation ?? text('revocation_endpoint'),
    issInCallback: doc.authorization_response_iss_parameter_supported === true,
  };
  if (!meta.authorization || !meta.token) throw new OAuthError('config', `${issuer}'s discovery document names no authorization or token endpoint; check the issuer, or give endpoints instead.`);
  for (const url of [meta.authorization, meta.token, meta.revocation]) {
    if (url && !/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) throw new OAuthError('config', `${issuer}'s discovery document names an endpoint that is not an absolute URL, ${url}; give endpoints instead.`);
    if (url) secure(url, 'discovery');
  }
  return meta;
}

// An issuer with `{tenantid}` matches a configured one whose segment there
// is one tenant name (Entra's `common`, `organizations`, `consumers`).
function templateMatches(template: string, issuer: string): boolean {
  if (!template.includes('{tenantid}')) return false;
  const [before, after] = template.split('{tenantid}');
  const tenant = issuer.slice(before.length, issuer.length - after.length);
  return issuer.startsWith(before) && issuer.endsWith(after) && /^[\w.-]+$/.test(tenant);
}

// --- the token endpoint (RFC 6749 §3.2, §5) --------------------------------

interface TokenReply {
  status: number;
  /** The reply's `Date`, in epoch milliseconds; 0 without one. */
  date: number;
  fields: Record<string, unknown>;
  /** The OAuth error code; "server" for a 429 or 5xx, whatever its body says; "invalid_response" for an unreadable reply; "" on success. */
  error: string;
}

async function tokenRequest(config: OAuthConfig, endpoint: string, body: Record<string, string>): Promise<TokenReply> {
  const headers: Record<string, string> = { 'Content-Type': 'application/x-www-form-urlencoded', Accept: 'application/json' };
  const fields = { ...body };
  if (config.tokenAuth === 'basic' && config.clientSecret !== undefined) {
    headers.Authorization = `Basic ${btoa(`${formEncode(config.clientId)}:${formEncode(config.clientSecret)}`)}`;
  } else {
    fields.client_id = config.clientId;
    if (config.clientSecret) fields.client_secret = config.clientSecret;
  }
  let response: Response;
  try {
    // A deadline: a token request may outlive its answer (LLP 1146 D2), and
    // the session's other work waits for it.
    response = await transport(config)(endpoint, { method: 'POST', headers, body: form(fields), redirect: 'error', exactTimeout: TOKEN_TIMEOUT_MS } as RequestInit);
  } catch (e) {
    throw grantOrNetwork(e, endpoint, true);
  }
  // A 429 or 5xx is the server's failure whatever its body says, as an OAuth error response is a 400 or 401 (RFC 6749
  // §5.2); otherwise an `error` member fails the call under any status, 200 included (GitHub).
  const unavailable = response.status === 429 || response.status >= 500;
  let text: string;
  try {
    text = await response.text();
  } catch {
    // Any other reply cut off is as good as none
    if (!unavailable) throw new OAuthError('network', `${originOf(endpoint)}'s reply was cut off.`, 0, true);
    text = '';
  }
  const date = Date.parse(response.headers.get('date') ?? '') || 0;
  const parsed = parseBody(text, response.headers.get('content-type') ?? '');
  const error = unavailable ? 'server'
    : typeof parsed?.error === 'string' && parsed.error ? parsed.error
      : !response.ok || !parsed ? 'invalid_response' : '';
  return { status: response.status, date, fields: parsed ?? {}, error };
}

// JSON, or form-encoded for a server that answers so (GitHub without `Accept`).
function parseBody(text: string, type: string): Record<string, unknown> | null {
  if (/x-www-form-urlencoded/i.test(type)) {
    const out: Record<string, unknown> = {};
    let repeated = false;
    new URLSearchParams(text).forEach((v, k) => { if (k in out) repeated = true; out[k] = v; });
    return Object.keys(out).length && !repeated ? out : null;
  }
  try {
    const v = JSON.parse(text);
    return v && typeof v === 'object' && !Array.isArray(v) ? v : null;
  } catch { return null; }
}

// What to keep from a token reply (RFC 6749 §5.1, §6), or why it is unusable.
function keep(reply: TokenReply, before: Kept | null, meta: Metadata | null, config: OAuthConfig): Kept | string {
  const f = reply.fields;
  const accessToken = typeof f.access_token === 'string' ? f.access_token : '';
  if (!accessToken) return 'no access token';
  // `token_type` is required and case-insensitive (§5.1); some providers omit it (`bearerByDefault`).
  const tokenType = typeof f.token_type === 'string' ? f.token_type : config.bearerByDefault ? 'Bearer' : '';
  if (!tokenType) return 'no token type';
  if (tokenType.toLowerCase() !== 'bearer') return `token type ${tokenType} is not Bearer`;
  // Without `expires_in` or a `Date` the expiry is unknown, and the server's 401 says when.
  const seconds = lifetime(reply);
  const expiresAt = seconds !== undefined && reply.date ? reply.date + seconds * 1000 : 0;
  // A scope list is space-separated (§3.3); its absence keeps what was held, or what was asked for.
  const scopes = typeof f.scope === 'string' ? f.scope.split(config.commaScopes ? /[\s,]+/ : ' ').filter(Boolean) : before?.scopes ?? (before ? [] : config.scopes);
  const text = (v: unknown): string | undefined => (typeof v === 'string' && v ? v : undefined);
  return {
    issuer: before?.issuer ?? meta?.issuer ?? '',
    first: before?.first,
    accessToken,
    // No new refresh token: the old one stands (§6).
    refreshToken: text(f.refresh_token) ?? before?.refreshToken,
    idToken: text(f.id_token) ?? before?.idToken,
    expiresAt,
    scopes,
    subject: before?.subject ?? '',
    client: before?.client,
    tokenEndpoint: before?.tokenEndpoint ?? meta?.token,
    revocationEndpoint: before?.revocationEndpoint ?? meta?.revocation,
  };
}

// A token reply's `expires_in` in seconds, undefined when absent or
// unreadable. It may come as a string of digits (Entra v1).
function lifetime(reply: TokenReply): number | undefined {
  const raw = reply.fields.expires_in;
  return typeof raw === 'number' && Number.isInteger(raw) && raw >= 0 ? raw
    : typeof raw === 'string' && /^\d+$/.test(raw) ? Number(raw) : undefined;
}

// --- the ID token (OpenID Connect Core §3.1.3.7, §12.2) --------------------

// Checks no JWT signature: the token came straight from the token endpoint
// over HTTPS (`secure`), which step 6 accepts in its place. `now` is the
// token reply's `Date`.
// `first` is the session's first ID token's claims, for a refreshed one.
function checkIdToken(token: string, meta: Metadata, config: OAuthConfig, nonce: string, now: number, first: Record<string, unknown> | null): string {
  const c = claims(token);
  if (!c) return 'the ID token is unreadable';
  if (typeof c.iss !== 'string' || typeof c.sub !== 'string' || typeof c.exp !== 'number' || typeof c.iat !== 'number') return 'the ID token lacks a required claim';
  if (typeof c.aud !== 'string' && !(Array.isArray(c.aud) && c.aud.every((a) => typeof a === 'string'))) return 'the ID token lacks a required claim';
  const issuer = meta.issuer.includes('{tenantid}') ? meta.issuer.replace('{tenantid}', String(c.tid ?? '')) : meta.issuer;
  if (meta.issuer && c.iss !== issuer && !config.issuerAliases?.includes(c.iss)) return 'the ID token is from another issuer';
  const aud = Array.isArray(c.aud) ? c.aud as string[] : [c.aud as string];
  if (!aud.includes(config.clientId)) return 'the ID token is for another app';
  if (aud.some((a) => a !== config.clientId && !config.trustedAudiences?.includes(a))) return 'the ID token names an audience the app does not trust';
  if ((aud.length > 1 || c.azp !== undefined) && c.azp !== config.clientId) return 'the ID token is for another app';
  if (now && c.exp * 1000 + SKEW_MS < now) return 'the ID token has expired';
  if (first) {
    // A refreshed ID token is the same person's, for the same app (§12.2).
    if (c.iss !== first.iss || c.sub !== first.sub || !sameAudiences(c.aud, first.aud) || c.azp !== first.azp) return 'the ID token is for another account';
    if (c.nonce !== undefined && c.nonce !== first.nonce) return 'the ID token is for another sign-in';
  } else if (nonce && c.nonce !== nonce) {
    return 'the ID token is for another sign-in';
  }
  return '';
}

// Audiences as a set: a string is a list of one (OpenID Connect Core §2).
function sameAudiences(a: unknown, b: unknown): boolean {
  const set = (v: unknown): string[] => (Array.isArray(v) ? v.map(String) : [String(v ?? '')]).sort();
  return JSON.stringify(set(a)) === JSON.stringify(set(b));
}

function claims(token: string): Record<string, unknown> | null {
  const part = token.split('.')[1];
  if (!part) return null;
  try {
    const v = JSON.parse(new TextDecoder().decode(base64urlDecode(part)));
    return v && typeof v === 'object' && !Array.isArray(v) ? v : null;
  } catch { return null; }
}

// --- results and errors ----------------------------------------------------

function result(code: string, error: string, retry = false, status = 0): OAuthResult {
  return { ok: false, code, error, retryable: retry, status: failedStatus(status) };
}

// A reply's status as `OAuthResult` and `log` give it: 400 or over, else 0
const failedStatus = (status: number): number => (status >= 400 ? status : 0);

function failure(e: unknown): OAuthResult {
  if (e instanceof OAuthError) return result(e.code, e.message, e.retryable, e.status);
  return result('network', 'Sign-in could not finish. Try again.', true);
}

function retryable(reply: TokenReply, ended: boolean): boolean {
  if (ended || CONFIG.includes(reply.error) || SIGN_IN_AGAIN.includes(reply.error)) return false;
  return reply.error === 'server' || reply.error === 'invalid_response' || reply.error === 'temporarily_unavailable' || throttled(reply) || reply.status === 429 || reply.status >= 500;
}

// Entra's throttling of a person comes as `invalid_grant` with AADSTS50196
// in `error_codes` (PROVIDERS.md ENTRA-14): a pause, not the session's end.
function throttled(reply: TokenReply): boolean {
  const codes = reply.fields.error_codes;
  return reply.error === 'invalid_grant' && Array.isArray(codes) && codes.includes(50196);
}

function describe(reply: TokenReply): string {
  if (reply.error === 'server' || reply.error === 'temporarily_unavailable' || throttled(reply)) return `The provider answered ${reply.status}. Try again later.`;
  if (reply.error === 'invalid_response') return 'The provider\'s reply was unreadable.';
  if (CONFIG.includes(reply.error)) return `The app's sign-in settings were refused (${reply.error}).`;
  return `The provider refused (${reply.error}).`;
}

function signedOutError(kept: Kept | null): OAuthError {
  return new OAuthError('signed_out', kept?.ended ?? 'Not signed in.', 0, false, true);
}

// A fetch the grants refuse says which grant line to add.
// A token endpoint's failure names the browser's case too: a provider whose
// token endpoint refuses cross-origin requests fails there the same way, and
// needs a server to exchange the code (GH-8).
function grantOrNetwork(e: unknown, url: string, token = false): OAuthError {
  const message = String((e as Error)?.message ?? e);
  if (/grant/i.test(message)) return new OAuthError('config', `oauth: ${message}; add \`net.fetch ${originOf(url)}\` to grants`);
  const browser = token ? ' In a browser, a token endpoint that refuses cross-origin requests fails this way too; such a provider needs a server to exchange the code.' : '';
  return new OAuthError('network', `${originOf(url)} did not answer.${browser}`, 0, true);
}

// HTTPS, or a loopback fixture (RFC 8414 §2; OIDC Core §3.1.3.7 step 6).
// The host's callback: one the grants do not name is the app's to fix, not
// the network's.
function hostCallback(): string {
  try {
    return authCallback();
  } catch (e) {
    throw new OAuthError('config', `Sign-in cannot be prepared: ${(e as Error)?.message ?? String(e)}`);
  }
}

function secure(url: string, what: string): void {
  const origin = originOf(url);
  if (origin.startsWith('https://')) return;
  if (/^http:\/\/(127\.0\.0\.1|localhost|\[::1\])(:\d+)?$/.test(origin)) return;
  throw new OAuthError('config', `${what}: ${origin} is not HTTPS; tokens go only over HTTPS (a loopback server excepted)`);
}

// --- encoding --------------------------------------------------------------

function callbackQuery(url: string): URLSearchParams {
  const hash = url.indexOf('#'), query = url.indexOf('?');
  const q = query >= 0 && (hash < 0 || query < hash) ? url.slice(query + 1, hash > query ? hash : undefined) : '';
  return new URLSearchParams(q);
}

// The origin as the web compares origins (RFC 6454 §4, the URL Standard's
// serialization): its host in ASCII, a padded or default port normalized. A
// scheme with no origin keeps its scheme and host, for the refusal to name.
function originOf(url: string): string {
  let parsed: URL;
  try { parsed = new URL(url); } catch { throw new TypeError(`oauth: ${url} is not an absolute URL`); }
  return parsed.origin === 'null' ? `${parsed.protocol}//${parsed.host}` : parsed.origin;
}

function formEncode(s: string): string {
  return encodeURIComponent(s).replace(/%20/g, '+');
}

function form(fields: Record<string, string>): string {
  return Object.entries(fields).map(([k, v]) => `${formEncode(k)}=${formEncode(v)}`).join('&');
}

function base64url(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

function base64urlDecode(s: string): Uint8Array {
  const b = s.replace(/-/g, '+').replace(/_/g, '/');
  const text = atob(b + '='.repeat((4 - (b.length % 4)) % 4));
  return Uint8Array.from(text, (c) => c.charCodeAt(0));
}

function random(n: number): string {
  return base64url(crypto.getRandomValues(new Uint8Array(n)));
}

// The package's transport, read at each request, so a test's `fetch` stands in.
function transport(config: OAuthConfig): (url: string, init: RequestInit) => Promise<Response> {
  return config.fetch ?? ((url, init) => fetch(url, init));
}

// A refresh token's print for `log`: the first six hex digits of its SHA-256, "" for none.
async function print(token: string | undefined): Promise<string> {
  if (!token) return '';
  return Array.from((await sha256(token)).slice(0, 3), (b) => b.toString(16).padStart(2, '0')).join('');
}

async function challenge(verifier: string): Promise<string> {
  return base64url(await sha256(verifier));
}

async function sha256(text: string): Promise<Uint8Array> {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text)));
}
