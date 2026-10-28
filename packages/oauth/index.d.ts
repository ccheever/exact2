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
}

export interface OAuthConfig {
  /** The issuer. Discovery reads `<issuer>/.well-known/openid-configuration`, then RFC 8414's document. */
  issuer?: string;
  /** Endpoints for a provider without discovery; with `issuer`, these override what discovery found. */
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
  /** The callback; the default is `authCallback()`. */
  callback?: string;
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
  /** Opens the provider's sign-in, then exchanges and keeps the tokens. Send it from a press. */
  signIn(store: OAuthStore): Promise<OAuthResult>;
  /** Forgets the session, then asks the provider to revoke it. */
  signOut(store: OAuthStore): Promise<OAuthResult>;
  /** `fetch` with the access token, to an origin in `apis`; refreshes once on 401. Answers the `Response`. */
  fetch(store: OAuthStore, url: string, init?: RequestInit): Promise<Response>;
  /** As `fetch`, then the parsed JSON body; any status that is not 2xx throws `OAuthError`. */
  json(store: OAuthStore, url: string, init?: RequestInit): Promise<unknown>;
}

/** A provider: one per issuer and client, made once at module level. */
export declare function oauth(config: OAuthConfig): OAuthProvider;
