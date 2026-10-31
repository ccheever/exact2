// @exact/oauth's public API (LLP 1146 D5), for the type checker and for
// reading: `index.ts` implements it, and `test/types.ts`, given to the type
// checker by a case in `test/oauth.test.ts`, confirms the two agree. The
// recipe is docs/recipes/oauth.md.

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
  /** How `clientSecret` is sent (RFC 6749 §2.3.1): in the body ("post", the default) or as HTTP Basic ("basic"), an empty one included. */
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
export declare class OAuthError extends Error {
  code: string;
  status: number;
  retryable: boolean;
  /** The person must sign in again. */
  signedOut: boolean;
  constructor(code: string, message: string, status?: number, retryable?: boolean, signedOut?: boolean);
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

/** A provider: one per issuer and client, made once at module level. */
export declare function oauth(config: OAuthConfig): OAuthProvider;
