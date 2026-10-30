# OAuth 2.0 and OpenID Connect provider deviations: a catalogue for fake authorization servers

Research date: 2026-10-26. Scope: public clients (native and browser apps) that use the authorization code grant with PKCE and refresh tokens.

## How to read this catalogue

Each item has four parts:

- **Behaviour**: what the provider or spec does, in one or two sentences.
- **Fake server**: how a fake authorization server can emulate it, with a concrete HTTP shape.
- **Client must**: what a correct client library does.
- **Sources**: the URLs that support the item.

Item IDs are stable (for example `ENTRA-10` or `S25`). The ranked list at the end refers to them.

### Evidence labels

- **Observed** means the research saw the behaviour directly on 2026-10-26 with `curl` against the live endpoint. This covers discovery documents, CORS preflights, and token requests with fake credentials. No real user accounts were used.
- **UNVERIFIED** marks a detail for which no source was found. Most items with this label have a sourced core and one unsourced detail, such as an exact error body or an HTTP status. The suggested HTTP shape for that detail is a guess, and the item says so. An index of these items is at the end.
- **Search extract**: some Salesforce help pages (help.salesforce.com, developer.salesforce.com) returned HTTP 403 or needed JavaScript in the research tool. Those items use the search engine's extract of the official page and say "(search extract)". Pages on devcommunity.x.com also returned HTTP 403.
- Community posts by non-staff are labelled as such. Treat them as weaker evidence than provider documentation, staff posts, or library code.

### Contents

1. Providers: Google, Microsoft Entra ID, Apple, Auth0, Okta, OneLogin, Keycloak, AWS Cognito, GitHub, Slack, Salesforce, Facebook/Meta, X, Spotify, Dropbox, Strava, LinkedIn, Discord, Twitch, Reddit, Zoom, Atlassian, Notion, Fitbit, Xero, Box, Intuit, Withings, Ping.
2. Client library workarounds (`LIB-*`): which providers well-known libraries special-case, and why.
3. Spec-level edge cases (`S*`): RFC 6749, RFC 7636, RFC 8252, RFC 9207, RFC 9700, RFC 10017, RFC 7009, RFC 6750, RFC 9449, OpenID Connect Core, OIDC Discovery and RFC 8414.
4. The 20 behaviours most likely to break a naive client library.
5. Index of items with UNVERIFIED details.

# 1. Providers

## Google

### GOOG-1. Discovery: issuer matches, RFC 9207 advertised, no logout endpoint
- Behaviour: `https://accounts.google.com/.well-known/openid-configuration` returns `"issuer": "https://accounts.google.com"` (exact match, no trailing slash). The document advertises `"authorization_response_iss_parameter_supported": true`, `code_challenge_methods_supported: ["plain","S256"]`, a `revocation_endpoint` (`https://oauth2.googleapis.com/revoke`), and `token_endpoint_auth_methods_supported: ["client_secret_post","client_secret_basic"]` (no `none`). It has no `end_session_endpoint`. The token endpoint host (`oauth2.googleapis.com`) differs from the issuer host.
- Fake server: Serve the document above. Put the token and revocation endpoints on a different host from the issuer.
- Client must: Compare `issuer` to the configured issuer by exact string. Not assume that the token endpoint shares the issuer host. Not require `end_session_endpoint`. Not infer "public clients unsupported" from the missing `none` auth method alone.
- Sources: Observed live discovery document. Google docs show the same issuer: https://developers.google.com/identity/openid-connect/openid-connect

### GOOG-2. ID token `iss` has two allowed forms
- Behaviour: Google documents that `iss` is "Always `https://accounts.google.com` or `accounts.google.com` for Google ID tokens." The scheme-less form does not match the discovery `issuer`.
- Fake server: Issue ID tokens with `"iss": "accounts.google.com"` (no scheme) while discovery says `https://accounts.google.com`.
- Client must: Decide on a policy. A strict OIDC client rejects the token (OIDC Core 3.1.3.7 requires an exact match). A Google-compatible client accepts both values for this issuer only. The test asserts the chosen policy.
- Sources: https://developers.google.com/identity/openid-connect/openid-connect (section "Validating an ID token" and the `iss` claim table)

### GOOG-3. PKCE: `plain` is the default when the method is omitted
- Behaviour: Google supports `S256` and `plain`. "The value of the `code_challenge_method` defaults to `plain` if not present."
- Fake server: Accept `code_challenge` without `code_challenge_method`, and verify it as `plain` (`code_verifier == code_challenge`).
- Client must: Always send `code_challenge_method=S256` explicitly. A client that omits it and sends a hashed challenge gets `invalid_grant` at redemption.
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app

### GOOG-4. Token endpoint demands `client_secret` for Desktop and Web clients despite PKCE
- Behaviour: Google says desktop apps "are public clients and cannot keep secrets confidential", and its parameter table marks `client_secret` as optional. In practice, developers report HTTP 400 `{"error":"invalid_request","error_description":"client_secret is missing."}` when a Desktop client redeems a PKCE code without the secret. Google auto-generates the secret and it cannot be deleted. A quoted Google staff statement says "Google's Identity Platform as of today does not support public applications under the 'Web Application' profile. Flows always require a client_secret." iOS and Android client types have no secret: "The `client_secret` is not applicable to requests from clients registered as Android, iOS, or Chrome applications."
- Fake server: For a client registered as "desktop" or "web", return `400 Content-Type: application/json` with body `{"error":"invalid_request","error_description":"client_secret is missing."}` when `client_secret` is absent, even with a valid `code_verifier`. For a client registered as "ios", accept no secret.
- Client must: Allow an optional, non-confidential `client_secret` to be configured per provider and sent with `client_secret_post`. Report this error to the developer as a configuration error, not as a sign-out.
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app ; community reports (not staff) https://discuss.google.dev/t/desktop-oauth-pkce-exchange-returns-invalid-request-after-successful-loopback-callback-with-no-client-secret/390526 ; https://discuss.google.dev/t/authorization-code-flow-without-client-secret/168113 ; staff quote as reproduced in https://ktaka.blog.ccmp.jp/2025/07/oogle-oauth2-and-pkce-understanding.html (original thread: https://www.googlecloudcommunity.com/gc/Developer-Tools/Authorization-Code-Flow-without-client-secret/m-p/814109). The exact staff wording was not checked on the original thread page.

### GOOG-5. Redirect URIs: loopback with any port; custom schemes restricted by client type
- Behaviour: Desktop clients use `http://127.0.0.1:port` or `http://[::1]:port` with "a random available port". `localhost` is allowed but "may cause issues with client firewalls". Loopback is "DEPRECATED for Android, Chrome app and iOS OAuth client types". "Custom URI schemes are no longer supported on Android and Chrome apps." iOS clients use the reversed client ID as scheme (`com.googleusercontent.apps.123:/oauth2redirect`), and "the path should begin with a single slash". The out-of-band (copy and paste) method "is no longer supported".
- Fake server: For a "desktop" client, accept any port on `127.0.0.1` and `[::1]` against a registered portless loopback URI. For an "ios" client, accept only `com.googleusercontent.apps.<id>:/<path>` and reject loopback with `400` and an error page (`redirect_uri_mismatch`). Reject `urn:ietf:wg:oauth:2.0:oob`.
- Client must: Pick an ephemeral port, bind to the IP literal, and send the exact URI (with port) in both the authorization and token requests. Build `scheme:/path` (single slash) URIs for Google iOS clients.
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app

### GOOG-6. Authorization code contains `/`
- Behaviour: Google's documented code format is `code=4/P7q7W91a-oMsCeLvIaQm6bTrgtp7`. The `/` can appear literally or percent-encoded in the callback.
- Fake server: Issue codes that start with `4/` and contain `/`, `-`, and `_`. Return them unencoded in the `Location` query.
- Client must: Parse the callback with a real URL parser and form-encode `code` in the token request body. Not use naive string splitting on `/`.
- Sources: https://developers.google.com/identity/protocols/oauth2/web-server

### GOOG-7. Callback: errors in the query, `iss` parameter, extra `scope` parameter
- Behaviour: "All responses are returned in the query string". Errors look like `?error=access_denied`. The documented success example includes `state`, `code`, and `scope`. Discovery advertises RFC 9207 support (`authorization_response_iss_parameter_supported: true`), so callbacks should carry `iss`.
- Fake server: Redirect with `302 Location: <redirect_uri>?state=..&code=4/..&scope=openid%20email%20https://www.googleapis.com/auth/userinfo.email&iss=https%3A%2F%2Faccounts.google.com`. For denial, redirect with `?error=access_denied&state=..`.
- Client must: Ignore unknown callback parameters. Verify `state`. When discovery advertises RFC 9207, require `iss` and compare it to the issuer.
- Sources: https://developers.google.com/identity/openid-connect/openid-connect ; https://developers.google.com/identity/protocols/oauth2/web-server ; observed discovery document. The exact `iss` value in a live callback is UNVERIFIED (not observed in this research).

### GOOG-8. Refresh token only on first consent (web) unless `prompt=consent`
- Behaviour: For web server apps, "The `refresh_token` is only returned on the first authorization", and only with `access_type=offline`. To get a new one when a grant exists, "you must use the `prompt=consent` parameter". For installed apps, "refresh tokens are always returned".
- Fake server: Keep a per-user, per-client grant record. On the first code redemption with `access_type=offline`, include `refresh_token`. On later redemptions without `prompt=consent`, return `200` with `access_token`, `expires_in`, `scope`, `token_type`, `id_token`, and no `refresh_token`.
- Client must: Treat `refresh_token` as optional on code redemption. Keep an existing stored refresh token when a new sign-in for the same account returns none. Offer a `prompt=consent` path.
- Sources: https://developers.google.com/identity/protocols/oauth2/web-server ; https://developers.google.com/identity/protocols/oauth2/native-app ; https://authjs.dev/reference/core/providers_google

### GOOG-9. Refresh response omits `refresh_token`
- Behaviour: The documented refresh response contains only `access_token`, `expires_in`, `scope`, and `token_type`.
- Fake server: `200 application/json` `{"access_token":"ya29.x","expires_in":3920,"scope":"https://www.googleapis.com/auth/drive.metadata.readonly https://www.googleapis.com/auth/calendar.readonly","token_type":"Bearer"}`.
- Client must: Keep the old refresh token when the response omits one (RFC 6749 §6).
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app

### GOOG-10. Granular consent: granted scopes can be a subset
- Behaviour: "users may not grant your app access to all of them". The `scope` field lists the scopes actually granted.
- Fake server: Return a `scope` value with fewer scopes than requested.
- Client must: Read `scope` from the token response and expose granted scopes. Not assume the request was granted in full.
- Sources: https://developers.google.com/identity/protocols/oauth2/web-server

### GOOG-11. Refresh token invalidation reasons, including the 100-token limit
- Behaviour: A refresh token stops working when the user revokes access, when it "has not been used for six months", when the user changes the password and the token has Gmail scopes, when "the user account has exceeded a maximum number of granted (live) refresh tokens", when time-based access expires, or by admin policy (`admin_policy_enforced`). "There is currently a limit of 100 refresh tokens per Google Account per OAuth 2.0 client ID. If the limit is reached, creating a new refresh token automatically invalidates the oldest refresh token without warning." Projects in "Testing" status get refresh tokens that expire in 7 days (unless only name, email, and profile scopes are requested). Refresh tokens can be up to 512 bytes.
- Fake server: Maintain a FIFO of at most N (100, or smaller for tests) refresh tokens per user and client. On issuing token N+1, silently invalidate the oldest. Expire tokens after 7 days in a "testing" mode. Return `400 {"error":"invalid_grant","error_description":"Token has been expired or revoked."}` for invalidated tokens.
- Client must: Treat `invalid_grant` on refresh as terminal for that token and start interactive sign-in. Not create a new grant on every app launch (that rotates out tokens on other devices). Store tokens of at least 512 bytes.
- Sources: https://developers.google.com/identity/protocols/oauth2 (section "Refresh token expiration" and token sizes). Error description string: community reports https://discuss.google.dev/t/oauth-client-id-token-has-been-expired-or-revoked/96268 and https://nango.dev/blog/google-oauth-invalid-grant-token-has-been-expired-or-revoked

### GOOG-12. Time-based access adds `refresh_token_expires_in`
- Behaviour: `refresh_token_expires_in` "is only set when the user grants time-based access."
- Fake server: Include `"refresh_token_expires_in": 86400` in the code redemption response for some users, and expire the refresh token after that time with `invalid_grant`.
- Client must: Accept unknown extra numeric fields. Optionally schedule re-authentication from `refresh_token_expires_in`.
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app ; https://developers.google.com/identity/protocols/oauth2

### GOOG-13. DPoP binds only the refresh token; `token_type` stays `Bearer`
- Behaviour: Google supports optional DPoP proofs at the token endpoint. "The type of token returned. This value is always `Bearer`, even when DPoP is used." The page states "access tokens won't be DPoP bound" and API calls use `Authorization: Bearer`. Google returns `use_dpop_nonce` with a `DPoP-Nonce` header. A failed exchange occurs "if the `DPoP` header is missing when expected, invalid, or if the proof uses a different key than the one bound to the token. In these cases, the server returns a `400 Bad Request` error." RFC 9449 says DPoP-bound access tokens use `token_type` `DPoP`.
- Fake server: When the code redemption had a `DPoP` header, bind the refresh token to the JWK thumbprint. Respond with `token_type: "Bearer"`. On a refresh without a matching proof, return `400 {"error":"invalid_dpop_proof"}`. On the first proof without a nonce, return `400 {"error":"use_dpop_nonce"}` with header `DPoP-Nonce: <value>`.
- Client must: Not assume `token_type: DPoP` after sending a proof. Keep the DPoP key for the life of the refresh token. Retry once with the server nonce on `use_dpop_nonce`. The exact error code for a bad proof at Google is UNVERIFIED (`invalid_dpop_proof` is the RFC 9449 code).
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app ; RFC 9449 https://www.rfc-editor.org/rfc/rfc9449

### GOOG-14. ID token claims: `email_verified` example as string, `at_hash`, nonce
- Behaviour: Google's documented example ID token shows `"email_verified": "true"` (a string). `at_hash` "is always included" when the ID token is issued with an access token in the server flow. The ID token "is returned from a token refresh" (stated in claim descriptions). Keys are cached by HTTP cache headers.
- Fake server: Issue ID tokens with `email_verified` as the string `"true"` in one variant and boolean `true` in another. Include `at_hash`. Return a new `id_token` on refresh without `nonce`.
- Client must: Accept boolean and string forms of `email_verified`. Not require `nonce` in a refreshed ID token (OIDC Core 12.2). Refresh JWKS on unknown `kid`.
- Sources: https://developers.google.com/identity/openid-connect/openid-connect. Whether live Google ID tokens use a string `email_verified` is UNVERIFIED (only the doc example shows it).

### GOOG-15. Token endpoint and revocation: CORS enabled; `invalid_client` uses HTTP 401
- Behaviour: A CORS preflight to `https://oauth2.googleapis.com/token` and `/revoke` returns `200` with `access-control-allow-origin` echoing the `Origin`. A token request with an unknown client returns `HTTP/2 401` with `{"error":"invalid_client","error_description":"The OAuth client was not found."}`. Other token errors are documented as `400 Bad Request`.
- Fake server: Echo `Origin` in `Access-Control-Allow-Origin`. Return `401` for `invalid_client` and `400` for `invalid_grant`.
- Client must: Parse RFC 6749 §5.2 error JSON from both 400 and 401 responses. Not treat 401 at the token endpoint as "access token expired".
- Sources: Observed (preflight and token request on 2026-10-26). https://developers.google.com/identity/protocols/oauth2/native-app

### GOOG-16. Revocation (RFC 7009-like) revokes the whole grant
- Behaviour: `POST https://oauth2.googleapis.com/revoke` with form body `token=...`. "The token can be an access token or a refresh token. If the token is an access token and it has a corresponding refresh token, the refresh token will also be revoked." Success is `200`; errors are `400` with an error code. Revocation "removes all OAuth 2.0 scopes previously granted to a project" and "it might take some time before the revocation has full effect."
- Fake server: On revoking any token of a grant, invalidate all tokens of that grant after a configurable delay. Return `400 {"error":"invalid_token"}` for unknown tokens.
- Client must: Delete local tokens whether revocation succeeds or fails. Not expect revocation to be instant. The exact error body for unknown tokens is UNVERIFIED (docs say only "an error code").
- Sources: https://developers.google.com/identity/protocols/oauth2/native-app

### GOOG-17. Rate limiting and transient errors at the token endpoint
- Behaviour: UNVERIFIED. No Google documentation found that describes 429 or 5xx semantics for `oauth2.googleapis.com/token`.
- Fake server: (For general resilience tests only) return `503` with `Retry-After: 2` once, then succeed.
- Client must: Retry with backoff on 429 and 5xx and keep the refresh token.
- Sources: none found.

## Microsoft Entra ID

### ENTRA-1. Discovery for `common` and `organizations` has a `{tenantid}` placeholder issuer
- Behaviour: `https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration` and the `organizations` variant return `"issuer": "https://login.microsoftonline.com/{tenantid}/v2.0"` (a literal template). The v1 document (`/common/.well-known/openid-configuration`) returns `"issuer": "https://sts.windows.net/{tenantid}/"` (note trailing slash, different host). The `consumers` v2 document returns a concrete issuer, `https://login.microsoftonline.com/9188040d-6c67-4c5b-b112-36a304b66dad/v2.0`. ID tokens carry the user's actual tenant: "When a response returns from the `/common` endpoint, the issuer value in the token corresponds to the user's tenant." Microsoft says the app must validate "that the `issuer` property in the published metadata matches the `iss` claim in the token, in addition to the usual check that the `iss` claim in the token contains the tenant ID (`tid`) claim." Libraries that compare strictly fail (Gitea, InterSystems IRIS, Authlib reports). Auth.js and Convex Auth work around it with oauth4webapi's `_expectedIssuer`.
- Fake server: Serve discovery at `/common/v2.0/.well-known/openid-configuration` with `"issuer": "https://login.example/{tenantid}/v2.0"`. Issue ID tokens with `"iss": "https://login.example/<tid>/v2.0"` and `"tid": "<tid>"`. Variant: v1 metadata `https://sts.example/{tenantid}/`.
- Client must: Not fail discovery when `issuer` differs from the discovery base URL for this pattern. For multi-tenant authorities, substitute `tid` from the ID token into the template and then compare exactly. Not disable issuer validation. Use the tenant-specific authority for later requests for that user.
- Sources: Observed live discovery documents (common, organizations, consumers, v1 common). https://learn.microsoft.com/en-us/entra/identity-platform/howto-convert-app-to-be-multi-tenant ; https://learn.microsoft.com/en-us/answers/questions/38386/azure-app-registration-and-gitea-oauth2-issuer-pro ; https://community.intersystems.com/node/587613 ; https://github.com/get-convex/convex-auth/pull/305 (seen via search results and a mirror; not opened directly) ; https://discuss.streamlit.io/t/fix-for-st-login-microsoft-invalid-claim-iss-on-shared-multi-tenant-entra-app-no-msal-rewrite-needed/122249

### ENTRA-2. Discovery omits `code_challenge_methods_supported`, `revocation_endpoint`, and RFC 9207 metadata
- Behaviour: The live v2 `common` document has no `code_challenge_methods_supported`, no `revocation_endpoint`, and no `authorization_response_iss_parameter_supported`, although Entra supports PKCE with `plain` and `S256`. It has `end_session_endpoint` (`.../oauth2/v2.0/logout`) and `subject_types_supported: ["pairwise"]`. The userinfo endpoint is on a different host (`https://graph.microsoft.com/oidc/userinfo`).
- Fake server: Serve discovery without `code_challenge_methods_supported` and without `revocation_endpoint`, but enforce PKCE on SPA redirects.
- Client must: Send PKCE (S256) even when discovery does not advertise it. Not refuse to sign in. Treat revocation as unavailable (local sign-out plus `end_session_endpoint`). Not require `iss` in the callback.
- Sources: Observed live discovery document. PKCE methods: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow

### ENTRA-3. PKCE required for SPA redirects; public clients must not send secrets
- Behaviour: `code_challenge` is "required by the Microsoft identity platform for single page apps using the authorization code flow". "Public clients, which include native applications and single page apps, must not use secrets or certificates when redeeming an authorization code." Sending a secret from a public client gives `AADSTS700025: Client is public so neither 'client_assertion' nor 'client_secret' should be presented.` A confidential client without a secret gets `AADSTS7000218: The request body must contain the following parameter: 'client_assertion' or 'client_secret'.` Entra "prevents the use of client credentials in all flows in the presence of an `Origin` header".
- Fake server: Public client: reject a request that contains `client_secret` with `400 {"error":"invalid_client","error_description":"AADSTS700025: ...","error_codes":[700025]}`. Confidential client: reject a missing secret with `401 {"error":"invalid_client","error_codes":[7000218]}`. The v1 doc lists 401 for "the request is missing the client_secret parameter".
- Client must: Not send `client_secret` for public clients. Surface these errors as configuration errors.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes ; https://learn.microsoft.com/en-us/previous-versions/azure/active-directory/azuread-dev/v1-protocols-oauth-code. The HTTP status for AADSTS700025 is UNVERIFIED.

### ENTRA-4. SPA vs native redemption depends on the `Origin` header
- Behaviour: "the Microsoft identity platform returns an error if you attempt to use a `spa` redirect URI without an `Origin` header." A code issued to a non-SPA redirect and redeemed cross-origin fails with `invalid_request` "cross-origin token redemption is permitted only for the 'Single-Page Application' client-type" (AADSTS9002326). The reverse case (SPA code redeemed without `Origin`) gives AADSTS9002327 "Tokens issued for the 'Single-Page Application' client-type may only be redeemed via cross-origin requests." Hybrid apps hit this: an Ionic/Capacitor app sent `Origin: capacitor://localhost`. Server-side redemption of SPA refresh tokens also fails.
- Fake server: Tag each registered redirect URI as `spa` or `native`. At `/token`: if the code's redirect is `spa` and the request has no `Origin`, return `400 {"error":"invalid_request","error_description":"AADSTS9002327: Tokens issued for the 'Single-Page Application' client-type may only be redeemed via cross-origin requests.","error_codes":[9002327]}`. If the redirect is `native` and `Origin` is present, return `400 {"error":"invalid_request","error_description":"AADSTS9002326: Cross-origin token redemption is permitted only for the 'Single-Page Application' client-type.","error_codes":[9002326]}`. Apply the same rule to refresh tokens.
- Client must: In a WebView or browser runtime, know whether the platform adds `Origin` and use the matching registration type. Redeem SPA refresh tokens from the same browser context. Report these as configuration errors.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/answers/questions/1534740/cross-origin-token-redemption-is-permitted-only-fo ; https://learn.microsoft.com/en-us/answers/questions/1601797/aadsts9002327-tokens-issued-for-the-single-page-ap ; https://forum.ionicframework.com/t/capacitor-msal-azure-b2c-ios-not-working-aadsts9002326/240436

### ENTRA-5. Token endpoint CORS is `*`
- Behaviour: A preflight and an error response from `https://login.microsoftonline.com/common/oauth2/v2.0/token` return `Access-Control-Allow-Origin: *` and `Access-Control-Allow-Methods: POST, OPTIONS`. Docs say a non-`spa` redirect gives a CORS failure in the browser.
- Fake server: Return `Access-Control-Allow-Origin: *` on `/token` for SPA registrations, and omit it for non-SPA registrations to simulate the documented browser error.
- Client must: Not send credentials (cookies) with token requests (wildcard CORS forbids credentialed requests). Map a CORS network failure to a configuration error message.
- Sources: Observed preflight and error response on 2026-10-26. https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow

### ENTRA-6. Loopback: port ignored for `localhost` only; no IPv6; trailing slash added
- Behaviour: "the port component ... is ignored for the purposes of matching a localhost redirect URI." "This is *only* true for localhost redirect URIs." "The IPv6 loopback address (`[::1]`) isn't currently supported." Microsoft prefers `127.0.0.1`, but an `http://127.0.0.1` URI can be added only through the manifest. Redirect URIs without a path "are returned with a trailing slash" for `query` and `fragment` response modes (`http://localhost:7071` becomes `http://localhost:7071/`). Redirect URIs are case-sensitive. Query parameters in redirect URIs are not allowed for apps that sign in personal accounts. Mobile and desktop apps can use `http://localhost` or `https://login.microsoftonline.com/common/oauth2/nativeclient`.
- Fake server: Match `localhost` redirects ignoring the port. Reject `[::1]` with an AADSTS50011 error page. When redirecting to a pathless URI, append `/` before `?code=`.
- Client must: Accept a callback whose path is `/` when it registered no path. Use the exact URI it sent in the authorization request (not the received one) in the token request. Bind to `127.0.0.1` or `localhost` consistently with the registration, not `[::1]`.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/reply-url

### ENTRA-7. Token response: `ext_expires_in`, non-standard `scope`, `refresh_token` only with `offline_access`
- Behaviour: `refresh_token` is "Only provided if `offline_access` scope was requested." `id_token` is "Only provided if `openid` scope was requested." `scope` "is non-standard and, if omitted, the token is for the scopes requested". The doc samples show a percent-encoded `scope` (`"https%3A%2F%2Fgraph.microsoft.com%2Fmail.read"`). MSAL's response type has `ext_expires_in` ("How long the access token is valid (in seconds) if the server isn't responding"), `refresh_in`, `refresh_token_expires_in`, `foci`, `client_info`, and `suberror`. The `scope` in the token request "must all be from a single resource".
- Fake server: `200 application/json` `{"token_type":"Bearer","scope":"User.Read profile openid email","expires_in":4430,"ext_expires_in":4430,"access_token":"...","refresh_token":"...","id_token":"...","client_info":"eyJ1aWQiOi..."}`. Omit `refresh_token` when `offline_access` is not requested. Variant: omit `scope`.
- Client must: Request `offline_access` to get refresh tokens. Ignore unknown fields. Treat a missing `scope` as "same as requested". The live `scope` content and whether `client_info` appears without the `client_info=1` request parameter are UNVERIFIED.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://azuread.github.io/microsoft-authentication-library-for-js/ref/types/_azure_msal_common.ServerAuthorizationTokenResponse.html

### ENTRA-8. v1 endpoint: `expires_in` and `expires_on` as strings, `resource` field
- Behaviour: The v1 (`/oauth2/token`) documented responses have `"expires_in": "3600"`, `"expires_on": "1388444763"` (strings), `"resource": "https://service.contoso.com/"`, and a percent-encoded `scope`. The refresh table says the `token_type` "only supported value is **bearer**" while the example shows `"Bearer"`. The v1 `scope` parameter is ignored, and `resource` is used.
- Fake server: Return `{"token_type":"Bearer","expires_in":"3600","expires_on":"1460404526","resource":"https://api.example/","access_token":"...","refresh_token":"..."}`.
- Client must: Accept `expires_in` as a JSON number or a numeric string. Compare `token_type` case-insensitively.
- Sources: https://learn.microsoft.com/en-us/previous-versions/azure/active-directory/azuread-dev/v1-protocols-oauth-code

### ENTRA-9. Refresh: tokens replaced on every use, old ones not revoked
- Behaviour: "Refresh tokens replace themselves with a fresh token upon every use. The Microsoft identity platform doesn't revoke old refresh tokens when used to fetch new access tokens." Default lifetimes: 24 hours for SPAs, 90 days for other scenarios. A refresh token can get access tokens for other resources: "Refresh tokens are bound to a combination of user and client, but aren't tied to a resource or tenant."
- Fake server: Return a new `refresh_token` on every refresh and keep the old one valid. Variant for reuse-safe tests: old token still works after rotation.
- Client must: Replace the stored refresh token with the new one atomically. Not assume an old token is dead (a race where two refreshes run in parallel must not sign the user out).
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens

### ENTRA-10. SPA refresh tokens: fixed 24-hour lifetime, not sliding
- Behaviour: "For refresh tokens sent to a redirect URI registered as `spa`, the refresh token expires after 24 hours. Additional refresh tokens acquired using the initial refresh token carries over that expiration time." The error is AADSTS700084: "The refresh token was issued to a single page app (SPA), and therefore has a fixed, limited lifetime of {time}, which can't be extended."
- Fake server: Record `absolute_expiry = first_issue + 24h` on the token family. Copy it to every rotated token. After expiry return `400 {"error":"invalid_grant","error_description":"AADSTS700084: The refresh token was issued to a single page app (SPA), and therefore has a fixed, limited lifetime of 1.00:00:00, which can't be extended...","error_codes":[700084]}`. Use a short lifetime in tests.
- Client must: Treat this as a normal re-authentication (top-level redirect or popup), usually without user input. Not compute expiry from the latest rotation time.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens ; https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes

### ENTRA-11. Error format: `error_codes`, `trace_id`, `correlation_id`, `error_uri`; react to `error` only
- Behaviour: Token errors are JSON with `error`, `error_description` (prefixed `AADSTSnnnn:`), `error_codes` (array), `timestamp`, `trace_id`, `correlation_id`, and sometimes `error_uri`. Microsoft says for `error_description`: "Never use this field to react to an error in your code." A malformed refresh token gave `400 {"error":"invalid_grant","error_description":"AADSTS9002313: Invalid request. Request is malformed or invalid. ...","error_codes":[9002313],...}`.
- Fake server: Return this shape with HTTP 400.
- Client must: Branch on `error`. Optionally log `error_codes` and `correlation_id`.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes ; https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; observed error response on 2026-10-26

### ENTRA-12. `interaction_required` and `consent_required` from the token endpoint
- Behaviour: The token endpoint can return `interaction_required` ("Non-standard, as the OIDC specification calls for this code only on the `/authorize` endpoint") and `consent_required` (also non-standard). Conditional Access gives codes such as AADSTS50076 (MFA required) and AADSTS70043 (sign-in frequency).
- Fake server: On refresh, return `400 {"error":"interaction_required","error_description":"AADSTS50076: ...","error_codes":[50076],"suberror":"basic_action"}`.
- Client must: Treat `interaction_required` and `consent_required` on refresh as "interactive sign-in needed", not as a transient error. Keep account identity for `login_hint`. The `suberror` value shown is from MSAL's type list; live values are UNVERIFIED.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes

### ENTRA-13. Expired or revoked refresh token: `invalid_grant` with several AADSTS codes
- Behaviour: AADSTS70008 (expired or revoked grant), AADSTS700082 (expired due to inactivity), AADSTS50173 (grant revoked, for example after a password reset), AADSTS70043 (sign-in frequency), AADSTS54005 (authorization code already redeemed). v1 says: "if you receive a response with the `interaction_required` or `invalid_grant` error codes, discard the refresh token and request a new authorization code." Revocation depends on credential type: a user password change revokes password-based tokens but non-password-based tokens "Stay alive".
- Fake server: `400 {"error":"invalid_grant","error_description":"AADSTS700082: The refresh token has expired due to inactivity...","error_codes":[700082]}`.
- Client must: On `invalid_grant`, discard the refresh token and start interactive sign-in (but see item ENTRA-14).
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes ; https://learn.microsoft.com/en-us/previous-versions/azure/active-directory/azuread-dev/v1-protocols-oauth-code ; https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens

### ENTRA-14. Throttling: 429 with `Retry-After`, 5xx without it, and loop detection as `invalid_grant`
- Behaviour: "If the server is having problems or if an application is requesting tokens too often Microsoft Entra ID will respond with `HTTP 429 (Too Many Requests)` and with `Retry-After` header". For `client_credentials`, `Retry-After: 60`. Entra "may respond with a `HTTP 5xx` error code with no `Retry-After` header." For user-facing calls, throttling arrives as `invalid_grant` with `AADSTS50196: The server terminated an operation because it encountered a loop while processing a request`. `temporarily_unavailable` means "Retry the request after a small delay." The v1 doc lists HTTP 500 as "Retry the request."
- Fake server: After K refreshes within T seconds, return `400 {"error":"invalid_grant","error_description":"AADSTS50196: The server terminated an operation because it encountered a loop while processing a request.","error_codes":[50196]}`. Separately return `429` with `Retry-After: 5`, and `503` with no `Retry-After`. Also `400 {"error":"temporarily_unavailable"}`.
- Client must: Retry 429, 5xx, and `temporarily_unavailable` with backoff and keep the refresh token. Cache access tokens and deduplicate concurrent refreshes so loop detection never fires. Be aware that AADSTS50196 is `invalid_grant` but does not mean the grant is dead.
- Sources: https://learn.microsoft.com/en-us/entra/msal/dotnet/advanced/client-and-server-throttling ; https://learn.microsoft.com/en-us/entra/identity-platform/reference-error-codes ; https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow

### ENTRA-15. ID token: tenant GUID in `iss`, MSA tenant, no `at_hash` from `/token`
- Behaviour: v2 `iss` ends in `/v2.0` and contains the tenant GUID. Personal Microsoft accounts use tenant `9188040d-6c67-4c5b-b112-36a304b66dad`. `at_hash` and `c_hash` are "not returned on ID tokens from the `/token` endpoint." `email` "isn't guaranteed to be correct and is mutable"; there is no `email_verified` claim in the list. `sub` is pairwise per application. The docs describe the refresh response `id_token` as "An unsigned JSON Web Token" and the sample header decodes to `{"typ":"JWT","alg":"none"}`.
- Fake server: Issue ID tokens without `at_hash` from `/token`. Use `iss` with the MSA tenant GUID for consumer accounts. Variant: `email` present and no `email_verified`.
- Client must: Not require `at_hash` in code-flow ID tokens. Use `tid` plus `oid` or `sub` as the user key, never `email`. Decide a policy on `alg: none` (reject is safe; OIDC Core 3.1.3.7 allows skipping signature checks only when the token came directly over TLS from the token endpoint). Whether live tokens from `/token` are signed is UNVERIFIED in this research.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/id-token-claims-reference ; https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow

### ENTRA-16. Access tokens for Microsoft APIs are opaque to the client
- Behaviour: "Tokens for Microsoft services can use a special format that will not validate as a JWT, and may also be encrypted for consumer (Microsoft account) users." "The client shouldn't validate access tokens."
- Fake server: Issue access tokens that are JWT-shaped but fail signature validation, and opaque strings for consumer accounts.
- Client must: Not parse or validate access tokens. Use `expires_in` for expiry.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/entra/identity-platform/claims-validation

### ENTRA-17. Callback: `response_mode` choices, errors in query or fragment, `session_state`
- Behaviour: `response_mode` can be `query`, `fragment`, or `form_post`. Microsoft recommends `form_post` "especially when using `http://localhost`". The callback error is `?error=access_denied&error_description=the+user+canceled+the+authentication`. v1 success callbacks include `session_state` and may include `admin_consent`. Authorization codes "expire after about 1 minute".
- Fake server: Return errors in the same location as the requested `response_mode` (fragment when `response_mode=fragment`). Add `session_state=<guid>` to success callbacks. Expire codes after 60 seconds with `invalid_grant`.
- Client must: Read errors from the location matching the response mode. Ignore extra parameters. Redeem the code immediately.
- Sources: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow ; https://learn.microsoft.com/en-us/previous-versions/azure/active-directory/azuread-dev/v1-protocols-oauth-code

### ENTRA-18. No RFC 7009 revocation; logout via `end_session_endpoint`
- Behaviour: Discovery has no `revocation_endpoint`. Refresh tokens are revoked by user or admin action, password changes, or single sign-out (per the revocation table), not by a client call.
- Fake server: Serve no revocation endpoint. Provide `/oauth2/v2.0/logout` that accepts `post_logout_redirect_uri`.
- Client must: Implement sign-out as local token deletion plus an optional browser visit to `end_session_endpoint`.
- Sources: Observed discovery document. https://learn.microsoft.com/en-us/entra/identity-platform/refresh-tokens

## Apple (Sign in with Apple)

### APPLE-1. Discovery: exact issuer, no PKCE metadata, no userinfo or logout
- Behaviour: `https://appleid.apple.com/.well-known/openid-configuration` returns `"issuer": "https://appleid.apple.com"`, `response_types_supported: ["code"]`, `response_modes_supported: ["query","fragment","form_post"]`, `scopes_supported: ["openid","email","name"]`, `token_endpoint_auth_methods_supported: ["client_secret_post"]`, a `revocation_endpoint`, and no `userinfo_endpoint`, `end_session_endpoint`, or `code_challenge_methods_supported`.
- Fake server: Serve that document.
- Client must: Not require `userinfo_endpoint` (read profile data from the ID token and callback). Not require logout support.
- Sources: Observed live discovery document.

### APPLE-2. Token endpoint requires a `client_secret` JWT; no public-client redemption
- Behaviour: `client_secret` is "A secret JSON Web Token, generated by the developer, that uses the Sign in with Apple private key". "Authorization code and refresh token validation requests require this parameter." The JWT is ES256-signed with `aud` `https://appleid.apple.com`, and "It's an error to request an expiration time more than `15777000` seconds (six months) in the future". Apple's REST docs do not mention PKCE. An expired or invalid secret gives `invalid_client` ("invalid client secret (expired token, malformed claims, or invalid signature)").
- Fake server: Reject `/auth/token` without `client_secret` or with a bad one: `400 application/json` `{"error":"invalid_client"}` (observed shape and status). Accept and ignore `code_verifier`.
- Client must: Support a backend-assisted code exchange for Apple (the app sends the code to its server). Not rely on PKCE alone. Whether Apple validates `code_verifier` is UNVERIFIED.
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/generate-and-validate-tokens ; https://developer.apple.com/documentation/signinwithapplerestapi/errorresponse ; client secret rules on https://developer.apple.com/documentation/accountorganizationaldatasharing/creating-a-client-secret (the page linked from the REST docs) ; observed `400 {"error":"invalid_client"}` on 2026-10-26.

### APPLE-3. Redirect URIs: HTTPS with a domain only; no loopback, IP, or custom scheme
- Behaviour: The redirect URI "must use the HTTPS protocol, include a domain name, and can't contain an IP address or `localhost`, and must not contain a fragment identifer (#)." For platforms with custom URL schemes (Android, old iOS), the app "must handle the data ... by storing it on their app server in the logic of their `redirect_uri` endpoint. You then redirect the custom URL scheme to give control back to the app."
- Fake server: Reject `http://127.0.0.1:*`, `http://localhost`, and `myapp://` redirect URIs at `/auth/authorize` with an error page. Accept only registered `https://` URIs.
- Client must: Support an HTTPS bounce page that forwards to the app (custom scheme or universal link). Not assume RFC 8252 loopback works.
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/generate-and-validate-tokens ; https://developer.apple.com/documentation/signinwithapple/incorporating-sign-in-with-apple-into-other-platforms ; https://developer.apple.com/documentation/signinwithapplerestapi/request-an-authorization-to-the-sign-in-with-apple-server. (the path ends with a period)

### APPLE-4. `response_mode=form_post` required when scopes are requested
- Behaviour: "If you requested any scopes, the value must be `form_post`." With `form_post`, "an HTTP POST request containing the results of the authorization is sent to the `redirectURI`" with `application/x-www-form-urlencoded` body. Requesting only `id_token` is unsupported; with `id_token`, `response_mode` must be `fragment` or `form_post`.
- Fake server: If `scope` is non-empty and `response_mode` is not `form_post`, return an error page (no redirect). Otherwise return an auto-submitting HTML form that POSTs `code`, `id_token`, `state`, and `user` to the redirect URI.
- Client must: Handle a POST callback (server or service worker); a custom-scheme or in-app browser callback cannot receive a POST body.
- Sources: https://developer.apple.com/documentation/signinwithapple/incorporating-sign-in-with-apple-into-other-platforms ; https://developer.apple.com/documentation/signinwithapplerestapi/request-an-authorization-to-the-sign-in-with-apple-server. (the path ends with a period)

### APPLE-5. `user` JSON only on the first authorization
- Behaviour: The callback carries `user`, "A JSON string containing the data requested in the `scope` property" in the form `{ "name": { "firstName": string, "lastName": string }, "email": string }`. "The API collects this information and shares it with your app the first time the user logs in". Later sign-ins return only the email in the ID token, not the name. "Immediately store it locally".
- Fake server: Include `user={"name":{"firstName":"Ada","lastName":"Lovelace"},"email":"x@privaterelay.appleid.com"}` in the form POST on the first authorization per user and client only. Omit it later.
- Client must: Pass `user` through to the app on first sign-in. Not treat its absence as an error.
- Sources: https://developer.apple.com/documentation/signinwithapple/authenticating-users-with-sign-in-with-apple ; https://developer.apple.com/documentation/signinwithapplerestapi/request-an-authorization-to-the-sign-in-with-apple-server. (the path ends with a period) ; https://developer.apple.com/documentation/signinwithapple/configuring-your-webpage-for-sign-in-with-apple

### APPLE-6. Callback error code is `user_cancelled_authorize`
- Behaviour: "The only error code that might be returned is `user_cancelled_authorize`. This error code is returned if the user clicks the cancel button during the web flow." The error comes with `state`. RFC 6749 would use `access_denied`.
- Fake server: POST `error=user_cancelled_authorize&state=<state>` to the redirect URI.
- Client must: Map unknown callback `error` values to a generic failure, and map `user_cancelled_authorize` to "cancelled".
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/request-an-authorization-to-the-sign-in-with-apple-server. (the path ends with a period) ; https://developer.apple.com/documentation/signinwithapple/configuring-your-webpage-for-sign-in-with-apple

### APPLE-7. Token response: `token_type` casing, refresh omits `refresh_token`
- Behaviour: The code exchange returns `access_token`, `token_type`, `expires_in`, `refresh_token`, `id_token`. The `TokenResponse` reference says `token_type` "is always `bearer`" while the examples show `"Bearer"`. On refresh, "The refresh token isn't returned when validating an existing refresh token." The refresh example returns `access_token`, `token_type`, `expires_in`, and `id_token`.
- Fake server: Code exchange: `{"access_token":"a","token_type":"bearer","expires_in":3600,"refresh_token":"r","id_token":"..."}`. Refresh: `{"access_token":"b","token_type":"Bearer","expires_in":3600,"id_token":"..."}`.
- Client must: Compare `token_type` case-insensitively. Keep the original refresh token forever (no rotation). Accept a new `id_token` on refresh.
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/tokenresponse ; https://developer.apple.com/documentation/signinwithapplerestapi/generate-and-validate-tokens

### APPLE-8. Refresh at most once a day or be throttled
- Behaviour: "You may verify the refresh token up to once a day ... Apple's servers may throttle your call if you attempt to verify a user's Apple Account more than once a day." "calling for a new identity token on every launch, or more frequently than once a day, can result in your request failing due to throttling."
- Fake server: Allow one refresh per refresh token per 24 hours. Return an error on the second. The throttle response status and body are UNVERIFIED; a fake could use `429` or `400 {"error":"invalid_request"}` as two variants.
- Client must: Not refresh on every launch. Cache the access token and ID token for their lifetime. Not sign the user out on a throttling error.
- Sources: https://developer.apple.com/documentation/signinwithapple/verifying-a-user

### APPLE-9. ID token: `email_verified` and `is_private_email` may be strings
- Behaviour: `email_verified` is "A string or Boolean value ... The value can either be a string (`"true"` or `"false"`) or a Boolean (`true` or `false`)." `is_private_email` is the same. `nonce` "is present only if you pass it in the authorization request." `nonce_supported` is a Boolean; "If this claim returns `true`, treat `nonce` as mandatory and fail the transaction". `email` may be empty for Work and School accounts. `iss` must be `https://appleid.apple.com`, and `aud` is the `client_id`.
- Fake server: Issue ID tokens with `"email_verified":"true"`, `"is_private_email":"true"`, `"nonce_supported":true`. Variant: omit `nonce` while `nonce_supported` is `false`.
- Client must: Parse `"true"`/`"false"` strings and booleans. Apply the `nonce_supported` rule. Handle an empty `email`.
- Sources: https://developer.apple.com/documentation/signinwithapplejs/authorizationi/id_token (claims table) ; https://developer.apple.com/documentation/signinwithapple/verifying-a-user

### APPLE-10. Authorization code lifetime and single use
- Behaviour: "The code is single-use only and valid for five minutes." Reuse gives `invalid_grant` ("invalid code (expired or previously used authorization code)").
- Fake server: Return `400 {"error":"invalid_grant"}` on the second redemption or after 5 minutes.
- Client must: Not retry a code redemption after a network timeout without user action (or accept that the retry fails with `invalid_grant`).
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/generate-and-validate-tokens ; https://developer.apple.com/documentation/signinwithapplerestapi/errorresponse

### APPLE-11. Revocation returns 200 for unknown tokens (and was observed returning 200 for a bad client secret)
- Behaviour: `POST https://appleid.apple.com/auth/revoke` with `client_id`, `client_secret`, `token`, and `token_type_hint`. "the `revoke` endpoint returns a `200` response code without a response body after the server invalidates the `token` value, or if the `token` value was previously invalidated." Errors use `400`. A request with an unknown `client_id` and a garbage `client_secret` returned `200` with `Content-Length: 0`.
- Fake server: Always return `200` with an empty body.
- Client must: Not parse a body from the revocation response. Not treat `200` as proof that the client secret is valid.
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/revoke-tokens ; observed on 2026-10-26.

### APPLE-12. No CORS on the token endpoint
- Behaviour: A CORS preflight to `https://appleid.apple.com/auth/token` returned `200` with no `Access-Control-Allow-Origin` header, and the POST response had none either.
- Fake server: Omit all `Access-Control-*` headers on `/auth/token`.
- Client must: Not attempt browser-side redemption for Apple. Route it through a backend.
- Sources: Observed on 2026-10-26.

### APPLE-13. Error responses use HTTP 400 with only `error`
- Behaviour: The documented error values are `invalid_request`, `invalid_client`, `invalid_grant`, `unauthorized_client`, `unsupported_grant_type`, and `invalid_scope`, with status `400 Bad Request`. The observed body had no `error_description`.
- Fake server: `400 application/json;charset=UTF-8` `{"error":"invalid_client"}`.
- Client must: Not require `error_description`. Treat `invalid_client` as a configuration error (for example, an expired client secret), not a sign-out.
- Sources: https://developer.apple.com/documentation/signinwithapplerestapi/errorresponse ; https://developer.apple.com/documentation/signinwithapplerestapi/generate-and-validate-tokens ; observed on 2026-10-26.

### APPLE-14. RFC 9207 `iss` callback parameter
- Behaviour: UNVERIFIED. Apple's discovery does not advertise `authorization_response_iss_parameter_supported`, and the documented callback parameters are `code`, `id_token`, `state`, `user`, and `error`.
- Fake server: Omit `iss` from callbacks.
- Client must: Not require `iss` unless discovery advertises it.
- Sources: Observed discovery document; https://developer.apple.com/documentation/signinwithapplerestapi/request-an-authorization-to-the-sign-in-with-apple-server. (the path ends with a period)

## Auth0

### AUTH0-1. Issuer has a trailing slash
- Behaviour: The discovery `issuer` and the ID token `iss` are `https://{tenant}.auth0.com/` with a trailing slash. Clients that build the issuer from the domain without the slash fail exact comparison. An Auth0 staff member calls the slash a deliberate design choice.
- Fake server: `GET /.well-known/openid-configuration` returns `200` with `{"issuer":"https://fake.example/","authorization_endpoint":"https://fake.example/authorize","token_endpoint":"https://fake.example/oauth/token",...}`. ID tokens carry `"iss":"https://fake.example/"`.
- Client must: Compare `iss` byte for byte with the discovery `issuer` (OIDC Discovery §4.3). It must not add or strip slashes. Configuration that a user types without the slash must be resolved through discovery.
- Sources: Observed `https://samples.auth0.com/.well-known/openid-configuration` (issuer `https://samples.auth0.com/`); https://auth0.com/docs/quickstart/backend/java-spring-security5/03-troubleshooting ; https://community.auth0.com/t/custom-domains-and-iss-claim-in-tokens/18672 ; https://support.spacelift.io/articles/2417947591-oidc-authentication-issues-with-auth0-and-trailing-slash-in-issuer-url

### AUTH0-2. Discovery omits `end_session_endpoint` on older tenants and omits RFC 9207 metadata
- Behaviour: `end_session_endpoint` appears only when "RP-Initiated Logout End Session Endpoint Discovery" is on. It is on by default only for tenants created on or after 14 November 2023. The observed `samples.auth0.com` document has no `end_session_endpoint` and no `authorization_response_iss_parameter_supported`. Both observed tenants advertise `code_challenge_methods_supported: ["S256","plain"]` and `revocation_endpoint: .../oauth/revoke`.
- Fake server: Serve discovery without `end_session_endpoint` and without `authorization_response_iss_parameter_supported`. Do not add `iss` to the authorization response.
- Client must: Treat a missing `end_session_endpoint` as "no RP-initiated logout"; clear local state and optionally call revocation. Must not require the RFC 9207 `iss` parameter when the metadata does not advertise it.
- Sources: https://auth0.com/docs/authenticate/login/logout/log-users-out-of-auth0 ; observed `https://samples.auth0.com/.well-known/openid-configuration` and `https://auth0.auth0.com/.well-known/openid-configuration`

### AUTH0-3. Refresh tokens require `offline_access` plus an API setting
- Behaviour: Auth0 issues a refresh token only when the authorize request includes `offline_access` and "Allow Offline Access" is enabled on the API. Native (public) clients do not send `client_secret`.
- Fake server: When `scope` lacks `offline_access`, return `200` from `/oauth/token` with `{"access_token":"...","id_token":"...","scope":"openid profile","expires_in":86400,"token_type":"Bearer"}` and no `refresh_token`.
- Client must: Handle a token response with no `refresh_token` as "no offline session". It must not crash or treat the session as refreshable.
- Sources: https://auth0.com/docs/secure/tokens/refresh-tokens/get-refresh-tokens ; https://auth0.com/docs/secure/tokens/refresh-tokens/refresh-token-rotation

### AUTH0-4. Refresh responses omit `refresh_token` when rotation is off
- Behaviour: The documented refresh response example has `access_token`, `expires_in`, `scope`, `id_token` (only when the original scope had `openid`) and `token_type: "Bearer"`, with no `refresh_token`. With rotation on, each exchange returns a new refresh token.
- Fake server: Non-rotating mode: `POST /oauth/token grant_type=refresh_token` returns `200 {"access_token":"...","expires_in":86400,"scope":"openid offline_access","id_token":"...","token_type":"Bearer"}`.
- Client must: Keep the old refresh token when the response has none (RFC 6749 §6). Keep the old ID token claims when `id_token` is absent.
- Sources: https://auth0.com/docs/secure/tokens/refresh-tokens/use-refresh-tokens ; https://auth0.com/docs/secure/tokens/refresh-tokens/refresh-token-rotation

### AUTH0-5. Rotation with family-wide reuse detection and an optional overlap window
- Behaviour: With rotation, the used refresh token is invalidated as soon as the new pair is issued. Reuse of an old token invalidates the whole family, including the newest token, and the user must sign in again. The `leeway` ("Rotation Overlap Period", seconds) is disabled by default. During the window only the immediately previous token can be reused, and a new rotating token is issued; reuse of the second-to-last token triggers breach detection.
- Fake server: Keep a family table. On reuse outside `leeway`, mark the family revoked and return `403 {"error":"invalid_grant","error_description":"Unknown or invalid refresh token."}` for every member. Within `leeway`, accept the previous token once more and issue another new token.
- Client must: Serialize refreshes across threads, tabs and processes. Persist the new refresh token atomically before using the new access token. Treat the reuse error as a terminal sign-out.
- Sources: https://auth0.com/docs/secure/tokens/refresh-tokens/refresh-token-rotation ; https://auth0.com/docs/secure/tokens/refresh-tokens/configure-refresh-token-rotation

### AUTH0-6. Invalid or expired refresh token returns HTTP 403, not 400
- Behaviour: Auth0 returns HTTP 403 with `invalid_grant` / `Unknown or invalid refresh token.` for expiry, reuse, revocation and limit eviction. The API reference also lists 401 ("refresh token is invalid") and 403 ("invalid or has expired") for the refresh endpoint.
- Fake server: `POST /oauth/token` returns `403`, `Content-Type: application/json`, body `{"error":"invalid_grant","error_description":"Unknown or invalid refresh token."}`.
- Client must: Classify by the `error` value, not by status. `invalid_grant` on 400, 401 or 403 means the grant is dead.
- Sources: https://support.auth0.com/center/s/article/troubleshoot-refresh-token-unknown-or-invalid-before-expiry-403-unknown-or-invalid-refresh-token ; https://auth0.com/docs/api/authentication/refresh-token/refresh-token

### AUTH0-7. 200 active refresh tokens per user per application
- Behaviour: When the limit is reached and a new refresh token is created, Auth0 revokes and deletes the oldest token. Revoked and expired tokens do not count.
- Fake server: Store at most N (configurable, 200 by default) refresh tokens per (user, client). On issue N+1, delete the oldest; it then fails as in item AUTH0-6.
- Client must: Not create a new grant on every app launch. Treat eviction like any `invalid_grant`.
- Sources: https://auth0.com/docs/secure/tokens/refresh-tokens

### AUTH0-8. Absolute and idle refresh token lifetimes; rotation does not extend the absolute lifetime
- Behaviour: Idle lifetime defaults to 30 days and renews on each exchange. Maximum lifetime is fixed at issuance (up to 1 year). With rotation, the default lifetime is 30 days and "does not extend when tokens are rotated".
- Fake server: Track `issued_at` of the family and `last_used_at`. Fail with item AUTH0-6's response when either limit passes.
- Client must: Expect forced re-authentication even for active users.
- Sources: https://auth0.com/docs/secure/tokens/refresh-tokens/configure-refresh-token-expiration ; https://auth0.com/docs/secure/tokens/refresh-tokens/configure-refresh-token-rotation

### AUTH0-9. Non-standard `error=unauthorized` at the callback
- Behaviour: Actions and Rules that deny login return `error=access_denied` with the custom reason in `error_description`. A legacy Rule that returns `UnauthorizedError` produces `error=unauthorized`, which is not an RFC 6749 §4.1.2.1 code.
- Fake server: `302 Location: myapp://callback?error=unauthorized&error_description=Your%20account%20is%20blocked&state=xyz`.
- Client must: Accept unknown `error` values at the callback, surface `error_description`, and still validate `state` first.
- Sources: https://support.auth0.com/center/s/article/troubleshooting-the-access-denied-error

### AUTH0-10. Revocation endpoint and grant deletion
- Behaviour: `/oauth/revoke` accepts calls without a secret when the client's `tokenEndpointAuthMethod` is `none`. With the tenant setting "Refresh Token Revocation Deletes Grant" on, revocation invalidates all tokens of the same grant; with it off, only that refresh token is revoked.
- Fake server: `POST /oauth/revoke` with `client_id` and `token` returns `200` with empty body. Optionally revoke every token sharing the grant.
- Client must: Send `client_id` in the body for public clients. Not rely on revocation status codes to learn whether the token existed (RFC 7009 §2.2).
- Sources: https://auth0.com/docs/secure/tokens/refresh-tokens/revoke-refresh-tokens ; observed `revocation_endpoint` in `https://samples.auth0.com/.well-known/openid-configuration`

### AUTH0-11. HTTP 429 with `x-ratelimit-*` headers
- Behaviour: Auth0 returns HTTP 429 when a rate limit is exceeded and sends `x-ratelimit-limit`, `x-ratelimit-remaining`, and `x-ratelimit-reset` (UNIX seconds). A community user reported a 500 "Global limit has been reached" on `/oauth/token`; staff could not reproduce it.
- Fake server: `429` with `x-ratelimit-limit: 30`, `x-ratelimit-remaining: 0`, `x-ratelimit-reset: <epoch+10>`. Body: UNVERIFIED (Auth0 docs show no body; a community post shows `{"statusCode":429,"error":"Too Many Requests","message":"Global limit has been reached","errorCode":"too_many_requests"}` from the Management API, not the token endpoint).
- Client must: Retry after `x-ratelimit-reset`; keep the session; never sign out on 429 or 5xx.
- Sources: https://auth0.com/docs/troubleshoot/customer-support/operational-policies/rate-limit-policy/rate-limit-use-cases ; https://community.auth0.com/t/global-limit-has-been-reached/77440 ; https://community.auth0.com/t/429-too-many-requests-post-oauth-token/46957

### AUTH0-12. Loopback redirect with any port (RFC 8252 §7.3) — UNVERIFIED
- Behaviour: An Auth0 Community feature request (thread 98409) reported that Auth0 rejects redirect URIs with a wildcard port. The thread is now removed, so the current behaviour is UNVERIFIED. Auth0 docs describe subdomain wildcards in Allowed Callback URLs and say nothing about loopback ports.
- Fake server: Exact string match on registered callbacks, including port; return an error page (no redirect) for `http://127.0.0.1:53124/cb` when only `http://127.0.0.1:3000/cb` is registered.
- Client must: Let the app pin a fixed loopback port when the provider does not honour §7.3, and show a clear error when the authorize page does not redirect.
- Sources: https://auth0.com/docs/get-started/applications/application-settings ; https://community.auth0.com/t/allow-wildcard-port-in-redirect-uri-as-per-rfc-8252/98409 (removed)

### AUTH0-13. CORS on `/oauth/token` — partly UNVERIFIED
- Behaviour: Auth0 has an "Allowed Origins (CORS)" list: "URLs that are allowed to make Cross-Origin Resource Sharing (CORS) requests to Auth0". "Allowed Web Origins" governs cross-origin authentication, device flow and `web_message`. The docs do not name the token endpoint explicitly, so the exact gating of `/oauth/token` is UNVERIFIED.
- Fake server: Reflect `Access-Control-Allow-Origin` only for origins on a configured list; omit it otherwise.
- Client must: Report a CORS failure as a configuration error, not as a sign-out.
- Sources: https://auth0.com/docs/get-started/applications/application-settings

## Okta

### OKTA-1. Two kinds of authorization server with different issuers and paths
- Behaviour: The org authorization server has issuer `https://{yourOktaDomain}` (no trailing slash) and endpoints under `/oauth2/v1/`. Custom servers have issuer `https://{yourOktaDomain}/oauth2/{authorizationServerId}` (e.g. `/oauth2/default`). Access tokens from the org server "aren't intended for validation or use by your own apps".
- Fake server: Two discovery documents: `/.well-known/openid-configuration` with `"issuer":"https://fake.example"` and `/oauth2/default/.well-known/openid-configuration` with `"issuer":"https://fake.example/oauth2/default"`.
- Client must: Derive the discovery URL from the configured issuer and compare `issuer` exactly. Never parse org-server access tokens.
- Sources: https://developer.okta.com/docs/concepts/auth-servers/ ; observed `https://okta.okta.com/.well-known/openid-configuration`

### OKTA-2. RFC 8414 discovery uses path insertion for custom servers
- Behaviour: The OAuth metadata URL for a custom server is `https://{domain}/.well-known/oauth-authorization-server/oauth2/{id}`, while the OIDC one is `https://{domain}/oauth2/{id}/.well-known/openid-configuration`.
- Fake server: Serve the same metadata at both URLs; return `404` at `https://fake.example/oauth2/default/.well-known/oauth-authorization-server`.
- Client must: When using RFC 8414 metadata, insert `/.well-known/oauth-authorization-server` between host and path (RFC 8414 §3.1); when using OIDC, append.
- Sources: https://developer.okta.com/docs/concepts/auth-servers/

### OKTA-3. PKCE S256 only; public clients use auth method `none`
- Behaviour: Discovery advertises `code_challenge_methods_supported: ["S256"]` and `token_endpoint_auth_methods_supported` includes `none`. The guide says the method "is always S256" and that a wrong app type makes Okta check for a secret.
- Fake server: Reject `code_challenge_method=plain` with `302 ...?error=invalid_request&state=...`. For a client registered as confidential, reject a token request without a secret with `401 {"error":"invalid_client"}`.
- Client must: Always send S256. Report `invalid_client` on a public client as a configuration error.
- Sources: https://developer.okta.com/docs/guides/implement-grant-type/authcodepkce/main/ ; observed `https://okta.okta.com/.well-known/openid-configuration`

### OKTA-4. Refresh tokens require `offline_access`
- Behaviour: "The `offline_access` scope must be requested as part of the code request". Without it, the token response has no `refresh_token`.
- Fake server: Omit `refresh_token` unless `offline_access` was in the authorize `scope`. Example code response: `{"access_token":"...","expires_in":3600,"id_token":"...","scope":"openid","token_type":"Bearer"}`.
- Client must: Add `offline_access` when it needs refresh, and handle a missing `refresh_token`.
- Sources: https://developer.okta.com/docs/guides/refresh-tokens/main/

### OKTA-5. Refresh response can echo the same refresh token
- Behaviour: Whether Okta returns a new refresh token depends on the lifetime setting. In the documented refresh example the returned `refresh_token` equals the one sent.
- Fake server: Non-rotating mode returns `200 {"token_type":"Bearer","expires_in":3600,"access_token":"...","scope":"offline_access openid","refresh_token":"<same value>","id_token":"..."}`.
- Client must: Accept an unchanged refresh token without treating it as an error or a rotation.
- Sources: https://developer.okta.com/docs/guides/refresh-tokens/main/

### OKTA-6. Rotation with a 0–60 s grace period (default 30 s) and reuse detection
- Behaviour: SPAs rotate by default; native and web apps default to persistent tokens. The previous token stays valid for the grace period (default 30 s, range 0–60 s). Reuse after that makes Okta invalidate the most recently issued refresh token and all access tokens issued since the user authenticated. The expiry date stays the same across rotations.
- Fake server: Rotating mode with `grace=30`. Reuse inside the window returns a new pair. Reuse after the window revokes the family and returns `400 {"error":"invalid_grant","error_description":"The refresh token is invalid or expired."}` (status code: UNVERIFIED; the body is documented).
- Client must: Serialize refreshes; persist the newest token before use; treat `invalid_grant` as a terminal sign-out.
- Sources: https://developer.okta.com/docs/guides/refresh-tokens/main/ ; https://support.okta.com/help/s/article/okta-refresh-token-invalid-or-expired-error-error-invalid-grant-error-description-the-refresh-token-is-invalid-or-expired

### OKTA-7. Unused refresh tokens expire after seven days by default
- Behaviour: The default `refreshTokenLifetimeMinutes` is Unlimited, but "The refresh token lifetime does expire every seven days if it hasn't been used."
- Fake server: Idle expiry of 7 days; then item OKTA-6's error.
- Client must: Expect `invalid_grant` after long app inactivity and route to sign-in.
- Sources: https://developer.okta.com/docs/guides/refresh-tokens/main/

### OKTA-8. Native SSO refresh tokens die with the Okta session
- Behaviour: With the `device_sso` scope, the refresh token is tied to the Okta session; when that session ends, Okta invalidates the refresh token.
- Fake server: Invalidate the refresh token when a separate "session" is ended; return `invalid_grant`.
- Client must: Not assume refresh token validity is independent of browser sessions.
- Sources: https://support.okta.com/help/s/article/intermittent-invalid-grant-errors-on-mobile-devices-using-okta-native-single-sign-on

### OKTA-9. No any-port loopback; wildcards only in the lowest subdomain
- Behaviour: A 2022 Okta forum reply said "At this time, Okta does not support this RFC" (RFC 8252 any-port loopback). The reply is not labelled as staff. Okta allows an opt-in `*` only in the lowest-level subdomain of http/https redirect URIs, never in the path.
- Fake server: Exact match on scheme, host and port. Optional subdomain wildcard for http/https only.
- Client must: Allow a fixed port; report a clear error when the provider shows an error page instead of redirecting.
- Sources: https://devforum.okta.com/t/loopback-interface-redirection-ephemeral-ports-wildcard-ports-in-localhost-redirect-uri/22197 ; https://support.okta.com/help/s/article/Does-Okta-Support-Wildcard-Redirect-URIs

### OKTA-10. Callback error for unassigned users
- Behaviour: When a user is not assigned to the app, Okta redirects back with `error=access_denied` and `error_description=User is not assigned to the client application.` Apps that bounce straight back to login can loop.
- Fake server: `302 Location: myapp:/callback?error=access_denied&error_description=User+is+not+assigned+to+the+client+application.&state=xyz`.
- Client must: Stop and show the error; never auto-retry the authorize request on `access_denied`.
- Sources: https://devforum.okta.com/t/user-not-assigned-is-not-reprompted-to-reauthenitcate-on-log-out/12048 ; https://devforum.okta.com/t/unhandled-exception-when-user-is-not-assigned-to-application/9839

### OKTA-11. Revocation always returns 200; refresh revocation also kills access tokens
- Behaviour: `/v1/revoke` returns `200 OK` "to prevent any information leaks", also for invalid or already revoked tokens. Revoking a refresh token revokes the associated access token; revoking an access token does not revoke the refresh token. Discovery advertises `revocation_endpoint_auth_methods_supported` including `none`.
- Fake server: `POST /oauth2/v1/revoke` returns `200` with empty body for any token.
- Client must: Revoke the refresh token (not only the access token) on sign-out.
- Sources: https://developer.okta.com/docs/guides/revoke-tokens/main/ ; observed `https://okta.okta.com/.well-known/openid-configuration`

### OKTA-12. Logout requires `id_token_hint` and must run in the browser
- Behaviour: `/v1/logout` requires `id_token_hint` whose subject matches the current session; an expired but otherwise valid ID token is accepted. The token must come from the same authorization server. A backend POST does not fully end the session.
- Fake server: `GET /oauth2/v1/logout?id_token_hint=...&post_logout_redirect_uri=...&state=...` returns `302` to the registered URI; without `id_token_hint` return an HTML error page with `400`.
- Client must: Keep the last ID token (even when expired) for logout and open the logout URL in the browser.
- Sources: https://developer.okta.com/docs/api/openapi/okta-oauth/oauth/orgas/logoutwithpost ; https://developer.okta.com/docs/api/openapi/okta-oauth/oauth/customas/logoutcustomas

### OKTA-13. Rate limiting with `X-Rate-Limit-*` headers
- Behaviour: Okta returns HTTP 429 and sends `X-Rate-Limit-Limit`, `X-Rate-Limit-Remaining`, and `X-Rate-Limit-Reset` (UTC epoch seconds). For concurrent-limit errors, the first two are 0 and the reset is only a suggestion.
- Fake server: `429` with `X-Rate-Limit-Limit: 0`, `X-Rate-Limit-Remaining: 0`, `X-Rate-Limit-Reset: <epoch+5>`. Body: UNVERIFIED.
- Client must: Back off until the reset time; keep the session.
- Sources: https://developer.okta.com/docs/reference/rl-best-practices/

### OKTA-14. No RFC 9207 metadata; DPoP advertised
- Behaviour: The observed org discovery has no `authorization_response_iss_parameter_supported` and advertises `dpop_signing_alg_values_supported`.
- Fake server: Omit the RFC 9207 flag; include DPoP algorithms.
- Client must: Not require `iss` at the callback unless advertised. Ignore unknown discovery members.
- Sources: observed `https://okta.okta.com/.well-known/openid-configuration`

## OneLogin

### ONELOGIN-1. Issuer is a path URL; legacy `/oidc` and current `/oidc/2` coexist
- Behaviour: The current issuer is `https://{subdomain}.onelogin.com/oidc/2`. The legacy `https://openid-connect.onelogin.com/oidc` document is still served with a different issuer, no `end_session_endpoint`, and `RS256` only.
- Fake server: Serve `/oidc/2/.well-known/openid-configuration` with `"issuer":"https://fake.example/oidc/2"` and endpoints `/oidc/2/auth`, `/oidc/2/token`, `/oidc/2/token/revocation`, `/oidc/2/logout`.
- Client must: Use the path-bearing issuer exactly; do not normalise to the origin.
- Sources: observed `https://onelogin.onelogin.com/oidc/2/.well-known/openid-configuration` and `https://openid-connect.onelogin.com/oidc/.well-known/openid-configuration`

### ONELOGIN-2. `offline_access` is rejected; refresh tokens depend on an app setting
- Behaviour: The authorize docs say `offline_access` "will return an error if provided". `scopes_supported` lacks it. A `refresh_token` is "Returned only when a Refresh Token Timeout is set in the app settings."
- Fake server: Authorize with `scope=openid offline_access` returns `302 ...?error=invalid_scope&error_description=some+of+requested+scopes+are+not+whitelisted&state=...` (the exact error code for `offline_access` is not stated; `invalid_scope` is the documented code for unlisted scopes). Token responses include `refresh_token` only when a per-client flag is set.
- Client must: Not add `offline_access` unconditionally; make it configurable per provider.
- Sources: https://developers.onelogin.com/openid-connect/api/authorization-code ; https://developers.onelogin.com/openid-connect/api/authorization-code-grant ; observed discovery above

### ONELOGIN-3. Token response has no `scope`
- Behaviour: The documented code-exchange response contains `access_token`, `expires_in` (3600), `id_token`, `token_type` ("Always set to \"Bearer\"") and optionally `refresh_token`, with no `scope`.
- Fake server: `200 {"access_token":"...","expires_in":3600,"id_token":"...","token_type":"Bearer","refresh_token":"..."}`.
- Client must: Assume granted scope equals requested scope when `scope` is absent (RFC 6749 §5.1).
- Sources: https://developers.onelogin.com/openid-connect/api/authorization-code-grant

### ONELOGIN-4. Most token errors use `invalid_request`, not `invalid_grant`
- Behaviour: Locked, suspended or unauthorized users get `invalid_request` with descriptions "User is locked. Access is unauthorized", "User is suspended. Access is unauthorized", or "Access is unauthorized". An unknown `client_id` gives `invalid_request` / "Resource not found". A used or expired code gives `invalid_grant` / "grant request is invalid". The docs list 400 and 401 without mapping each error to a status.
- Fake server: Refresh for a suspended user returns `401 {"error":"invalid_request","error_description":"User is suspended. Access is unauthorized"}`.
- Client must: Not rely only on `invalid_grant` to detect a dead session. Treat a refresh `invalid_request` with a 401 as terminal after one retry; log the description.
- Sources: https://developers.onelogin.com/openid-connect/api/refresh-token ; https://developers.onelogin.com/openid-connect/api/authorization-code-grant

### ONELOGIN-5. Credentials in both header and body are an error
- Behaviour: "Sending credentials in both Authorization header and payload body will cause an error." Public PKCE clients send only `client_id` in the body.
- Fake server: If both `Authorization: Basic` and body `client_id`/`client_secret` are present, return `400 {"error":"invalid_request","error_description":"invalid authorization header value format"}` (the exact error for this case is UNVERIFIED).
- Client must: Use exactly one client authentication method.
- Sources: https://developers.onelogin.com/openid-connect/api/refresh-token

### ONELOGIN-6. Refresh returns a new refresh token; rotation and reuse rules UNVERIFIED
- Behaviour: The refresh response sample includes a new `refresh_token` ("The token that should be used to refresh the session again"). The docs do not say whether the old token stops working or whether reuse revokes a family: UNVERIFIED.
- Fake server: Return a new `refresh_token` on each refresh; offer a strict mode that invalidates the old one.
- Client must: Always store the newest refresh token.
- Sources: https://developers.onelogin.com/openid-connect/api/refresh-token

### ONELOGIN-7. PKCE S256 with auth method "None (PKCE)"
- Behaviour: Discovery advertises `code_challenge_methods_supported: ["S256"]`. The app must set Token Endpoint Authentication Method to "None (PKCE)"; `code_verifier` is then required. The iOS quickstart says that with PKCE "you can enter any URI you'd like", for example a custom scheme `com.testapp.oidc://callback`.
- Fake server: Accept custom-scheme redirect URIs; require `code_verifier` for `none` clients; reject `plain`.
- Client must: Always send S256 and `client_id`.
- Sources: https://developers.onelogin.com/openid-connect/api/authorization-code ; https://developers.onelogin.com/quickstart/authentication/native-ios-app ; observed discovery

### ONELOGIN-8. Redirect URI quirks
- Behaviour: Admins cannot save redirect URIs whose domain contains an underscore. Loopback any-port support is UNVERIFIED.
- Fake server: Exact matching; reject registration of `https://my_app.example/cb`.
- Client must: Nothing special beyond exact redirect URIs.
- Sources: https://support.onelogin.com/kb/4383003/onelogin-underscore-in-the-preceding-part-of-a-domain-url-can-t-be-saved-in-the-openid-connect-oidc-connector

### ONELOGIN-9. Revocation semantics and errors
- Behaviour: `/oidc/2/token/revocation` revokes only the given access token, but revoking a refresh token invalidates "all of the related tokens". An already revoked token can yield `400 {"error":"unsupported_token_type","error_description":"revocation of the presented token type is not supported"}`, unlike RFC 7009, which says invalid tokens get 200.
- Fake server: Second revocation of the same refresh token returns that 400 body.
- Client must: Treat any revocation error during sign-out as non-fatal and finish local sign-out.
- Sources: https://developers.onelogin.com/openid-connect/api/revoke-token

### ONELOGIN-10. Logout revokes tokens for every app in the SSO session
- Behaviour: `/oidc/2/logout` ends the OneLogin session and revokes all tokens issued under it, including those of other OIDC apps. `id_token_hint` is required when `post_logout_redirect_uri` is used; the URI must match exactly. `logout=false` revokes tokens without ending the session.
- Fake server: Logout invalidates all refresh tokens across clients that share the session ID.
- Client must: Expect `invalid_request`/`invalid_grant` on refresh after another app signs the user out.
- Sources: https://developers.onelogin.com/openid-connect/api/logout

### ONELOGIN-11. Rate limit headers; 429 status UNVERIFIED
- Behaviour: The developer docs state 5,000 requests per hour per IP and `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` (Unix time) headers. The docs' error table does not list 429; a 429 status for the OIDC token endpoint is UNVERIFIED (only a third-party guide states it).
- Fake server: Send the three headers; in throttle mode return `429`.
- Client must: Back off; keep the session.
- Sources: https://developers.onelogin.com/docs/ ; https://docs.datagrail.io/docs/integrations/api-documentation/onelogin

## Keycloak

### KC-1. Issuer depends on hostname configuration and path prefix
- Behaviour: The issuer is the realm URL, for example `https://sso.redhat.com/auth/realms/redhat-external` (with the legacy `/auth` prefix) or `https://host/realms/{realm}` in current docs. With `hostname-strict=false`, Keycloak builds URLs from `Host`/`X-Forwarded-Host`, so the issuer can differ between internal and external callers.
- Fake server: Build `issuer` and all endpoints from the request `Host` header; a second mode uses a fixed hostname. Offer a variant with `/auth/realms/{realm}`.
- Client must: Use the issuer from discovery as fetched through the same URL the user configured; fail clearly on mismatch.
- Sources: https://www.keycloak.org/server/hostname ; observed `https://sso.redhat.com/auth/realms/redhat-external/.well-known/openid-configuration` ; https://www.keycloak.org/securing-apps/oidc-layers

### KC-2. RFC 9207 `iss` at the callback (since 23.0.0), per-client opt-out
- Behaviour: Keycloak 23.0.0 adds `iss` to authorization responses by default and advertises `authorization_response_iss_parameter_supported: true`. Admins can disable it per client for older adapters.
- Fake server: `302 Location: myapp://cb?state=xyz&session_state=...&iss=https%3A%2F%2Ffake.example%2Frealms%2Fdemo&code=...`. Variant: metadata says `true` but `iss` missing (misconfigured client).
- Client must: When the flag is true, require `iss` and compare it exactly to the issuer; ignore extra callback parameters such as `session_state`.
- Sources: https://www.keycloak.org/docs/latest/upgrading/index.html (Migrating to 23.0.0) ; observed sso.redhat.com discovery

### KC-3. Extra token response fields: `refresh_expires_in`, `not-before-policy`, `session_state`
- Behaviour: Keycloak's `AccessTokenResponse` serialises `refresh_expires_in` (long), `not-before-policy` (int, hyphenated key) and `session_state`. `refresh_expires_in` is set only when the refresh token has an expiry, so offline tokens without expiry serialise `0`. The response always includes `scope`.
- Fake server: `200 {"access_token":"...","expires_in":300,"refresh_expires_in":1800,"refresh_token":"...","token_type":"Bearer","id_token":"...","not-before-policy":0,"session_state":"...","scope":"openid profile email"}`; offline variant with `"refresh_expires_in":0`.
- Client must: Ignore unknown fields; never treat `refresh_expires_in: 0` as "already expired".
- Sources: https://github.com/keycloak/keycloak/blob/main/core/src/main/java/org/keycloak/representations/AccessTokenResponse.java ; https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/TokenManager.java

### KC-4. Lowercase `token_type: "bearer"` in older versions
- Behaviour: Users reported `"token_type":"bearer"` in 2015 (KEYCLOAK-1855) and again in 2019; a Keycloak developer said clients should ignore case. Current source uses `"Bearer"`.
- Fake server: Legacy variant returns `"token_type":"bearer"`.
- Client must: Compare `token_type` case-insensitively (RFC 6749 §5.1, §7.1).
- Sources: https://lists.jboss.org/pipermail/keycloak-user/2015-September/003058.html ; https://lists.jboss.org/pipermail/keycloak-user/2019-October/019324.html ; https://github.com/keycloak/keycloak/blob/main/core/src/main/java/org/keycloak/util/TokenUtil.java

### KC-5. Refresh token lifetime follows SSO Session Idle/Max
- Behaviour: Regular refresh tokens are bound to the user session. "SSO Session Idle" invalidates the session after inactivity and resets on refresh; "SSO Session Max" is absolute. Offline tokens follow "Offline Session Idle" and optionally "Offline Session Max".
- Fake server: Short idle (e.g. 30 min) and max limits; after expiry return `400 {"error":"invalid_grant","error_description":"Session not active"}` or `"Token is not active"`.
- Client must: Refresh proactively; expect short-lived refresh tokens; use `offline_access` for long sessions.
- Sources: https://www.keycloak.org/docs/latest/server_admin/index.html ; https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/TokenManager.java

### KC-6. Optional rotation: "Revoke Refresh Token" and "Refresh Token Max Reuse"
- Behaviour: When "Revoke Refresh Token" is enabled, Keycloak issues a new refresh token on each refresh. A token older than the latest one fails with `invalid_grant` "Stale token". Use beyond the reuse count fails with "Maximum allowed refresh token reuse exceeded". With this option on, offline tokens are single-use too. A vendor blog says exceeding reuse invalidates the whole session; that is UNVERIFIED (the source path read here only rejects the request).
- Fake server: Track latest token ID and use count per client session; return `400 {"error":"invalid_grant","error_description":"Stale token"}`.
- Client must: Serialize refresh; persist new tokens before use.
- Sources: https://www.keycloak.org/docs/latest/server_admin/index.html ; https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/TokenManager.java ; https://skycloak.io/blog/keycloak-refresh-token-rotation-guide/

### KC-7. Refresh errors are HTTP 400 `invalid_grant` with varied descriptions
- Behaviour: `RefreshTokenGrantType` maps every `OAuthErrorException` to HTTP 400, except a certificate-binding failure, which is 401 `unauthorized_client`. Descriptions include "Session not active", "Offline session not active", "User disabled", "Invalid refresh token", "Stale token", "DPoP proof is missing".
- Fake server: `400 {"error":"invalid_grant","error_description":"Session not active"}`.
- Client must: Key on `error`, not `error_description`.
- Sources: https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/grants/RefreshTokenGrantType.java ; https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/TokenManager.java

### KC-8. PKCE optional by default; `plain` allowed
- Behaviour: Per-client "PKCE method" blank means PKCE is applied only if the client sends it; `S256` or `plain` enforce one method. Discovery advertises `["plain","S256"]`.
- Fake server: Default mode accepts a code request without `code_challenge`; enforcing mode rejects it with `302 ...?error=invalid_request&error_description=Missing+parameter%3A+code_challenge_method&state=...` (exact description UNVERIFIED).
- Client must: Always send S256 even when not required.
- Sources: https://www.keycloak.org/docs/latest/server_admin/index.html ; observed sso.redhat.com discovery

### KC-9. Loopback any port; exact case-sensitive matching; trailing wildcards
- Behaviour: Registering `http://127.0.0.1` "allows any port"; `localhost` is not recommended. Since 23.0.2, redirect URIs use exact, case-sensitive matching, including scheme and host. Trailing `*` wildcards are allowed (and a bare `*`, not for production).
- Fake server: Ignore the port only for `http://127.0.0.1` registrations; compare everything else exactly and case-sensitively.
- Client must: Use `127.0.0.1` with an ephemeral port; keep redirect URI casing identical between authorize and token requests.
- Sources: https://www.keycloak.org/securing-apps/oidc-layers ; https://www.keycloak.org/docs/latest/upgrading/index.html (Migrating to 23.0.2) ; https://www.keycloak.org/docs/latest/server_admin/index.html

### KC-10. CORS via "Web Origins"
- Behaviour: The client "Web Origins" list drives CORS on Keycloak endpoints; token endpoint errors are also sent through the CORS wrapper.
- Fake server: Add `Access-Control-Allow-Origin` only for listed origins, on success and error responses.
- Client must: Report missing CORS headers as configuration errors.
- Sources: https://www.keycloak.org/docs/latest/server_admin/index.html ; https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/grants/RefreshTokenGrantType.java

### KC-11. Logout: `redirect_uri` removed; confirmation page without `id_token_hint`
- Behaviour: From 18.0.0, logout follows RP-Initiated Logout; without a valid `id_token_hint`, a confirmation prompt appears. The legacy `redirect_uri` parameter and its compatibility switches were removed in a later major release (the section sits under the 26.0.0 notes). A newer release rejects `post_logout_redirect_uri` values that carry OIDC authorize parameters unless `allow-oidc-params-in-redirect-uris` is set.
- Fake server: `GET /realms/demo/protocol/openid-connect/logout?id_token_hint=...&post_logout_redirect_uri=...` redirects; without `id_token_hint`, return `200` HTML confirmation page; `redirect_uri=` alone yields an error page.
- Client must: Send `id_token_hint` and `post_logout_redirect_uri` (or `client_id`), never legacy `redirect_uri`.
- Sources: https://www.keycloak.org/docs/latest/upgrading/index.html

### KC-12. Revocation revokes consent for offline tokens
- Behaviour: `/protocol/openid-connect/revoke` supports access and refresh tokens; "When revoking a refresh token, the user consent for the corresponding client is also revoked."
- Fake server: Revoking a refresh token also drops the consent record, so the next authorize shows consent again.
- Client must: Expect a consent prompt after sign-out and re-login.
- Sources: https://www.keycloak.org/securing-apps/oidc-layers

### KC-13. Discovery may omit `none` from auth methods
- Behaviour: The observed sso.redhat.com discovery lists `token_endpoint_auth_methods_supported` without `none`, although Keycloak supports public clients.
- Fake server: Omit `none` from the list while accepting public clients.
- Client must: Not refuse to run when `none` is missing; use the registered client type.
- Sources: observed `https://sso.redhat.com/auth/realms/redhat-external/.well-known/openid-configuration` ; https://www.keycloak.org/docs/latest/server_admin/index.html (public clients for browser apps)

## AWS Cognito

### COGNITO-1. Issuer host differs from the authorize/token host; two issuer formats
- Behaviour: Discovery and JWKS live at `https://cognito-idp.{region}.amazonaws.com/{poolId}/.well-known/...`; authorize, token, userInfo and revoke live on the user pool domain (`{prefix}.auth.{region}.amazoncognito.com` or a custom domain). An "updated issuer" uses `https://issuer-cognito-idp.{region}.amazonaws.com/{poolId}`. AWS asks clients to ignore unknown discovery members and not hard-code their order.
- Fake server: Serve discovery at host A with `"issuer":"https://a.fake.example/pool_1"` and `"authorization_endpoint":"https://b.fake.example/oauth2/authorize"`, `"token_endpoint":"https://b.fake.example/oauth2/token"`.
- Client must: Not assume endpoints share the issuer's origin; tolerate extra metadata.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/federation-endpoints.html

### COGNITO-2. Refresh token always issued; `offline_access` ignored
- Behaviour: Cognito "doesn't implement the OpenID Connect `offline_access` scope model". Every authorization-code exchange returns a refresh token; requesting `offline_access` has no effect.
- Fake server: Always include `refresh_token` in code-exchange responses regardless of `scope`.
- Client must: Not depend on `offline_access` to get or suppress refresh tokens.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/token-endpoint.html

### COGNITO-3. Refresh omits `refresh_token` unless rotation is on; rotation keeps the original expiry
- Behaviour: Without rotation, refresh returns only `access_token`, `id_token`, `token_type`, `expires_in`, and the original refresh token stays valid. With rotation, a new refresh token is returned, valid only for the remaining original `RefreshTokenValidity`; a grace period of up to 60 s (`RetryGracePeriodSeconds`) allows retries. No refresh response includes `scope`.
- Fake server: Non-rotating: `200 {"access_token":"...","id_token":"...","token_type":"Bearer","expires_in":3600}`. Rotating: add `"refresh_token":"<new>"` and invalidate the old one after the grace period.
- Client must: Keep the old refresh token when none is returned; replace it when one is.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/token-endpoint.html ; https://docs.aws.amazon.com/cognito/latest/developerguide/amazon-cognito-user-pools-using-the-refresh-token.html

### COGNITO-4. Refresh token lifetime 60 min to 10 years, default 30 days
- Behaviour: Default expiry is 30 days after sign-in; configurable from 60 minutes to 10 years per app client.
- Fake server: Fixed absolute expiry per family; then `400 {"error":"invalid_grant"}`.
- Client must: Expect forced re-authentication at a fixed time after first sign-in.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/amazon-cognito-user-pools-using-the-refresh-token.html

### COGNITO-5. Token errors: bare `error` field, all 400; redirect mismatch is `unauthorized_client`
- Behaviour: The documented error map is `HTTP/1.1 400` with `{"error":"invalid_request|invalid_client|invalid_grant|unauthorized_client|unsupported_grant_type"}`. A revoked refresh token gives `invalid_grant`. A `redirect_uri` that does not match the authorize request gives `unauthorized_client` with `error_description` `invalid_redirect` (RFC 6749 §5.2 would use `invalid_grant`). A scope not allowed for the client yields `invalid_grant`; an unknown custom scope yields `invalid_scope`.
- Fake server: `400 {"error":"invalid_grant"}` (no description) for a revoked token; `400 {"error":"unauthorized_client","error_description":"invalid_redirect"}` for redirect mismatch.
- Client must: Handle a missing `error_description`; map `unauthorized_client` at the token endpoint to a configuration error, not a sign-out.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/token-endpoint.html

### COGNITO-6. PKCE S256 only; optional, not enforceable from the docs
- Behaviour: "Amazon Cognito authentication server supports only S256." A `code_challenge` without `code_challenge_method`, or a method other than `S256`, redirects with `error=invalid_request`. `code_verifier` is required only if a challenge was sent.
- Fake server: `302 Location: myapp://cb?error=invalid_request` (no `state` shown in the doc example) for `code_challenge_method=plain`.
- Client must: Always send `code_challenge_method=S256`; tolerate error redirects without `state` only by rejecting them (never process an unsolicited response).
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/authorization-endpoint.html ; https://docs.aws.amazon.com/cognito/latest/developerguide/using-pkce-in-authorization-code.html

### COGNITO-7. Redirect URIs: HTTPS except `http://localhost`; custom schemes allowed
- Behaviour: Callback URLs must be absolute, pre-registered and fragment-free; HTTPS is required except `http://localhost` "for testing purposes". App schemes such as `myapp://example` are supported. Whether `127.0.0.1` or arbitrary ports are accepted is UNVERIFIED.
- Fake server: Reject `http://` except `http://localhost`; exact match including port.
- Client must: Prefer custom schemes or claimed HTTPS links; do not rely on any-port loopback.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/user-pool-settings-client-apps.html ; https://docs.aws.amazon.com/cognito/latest/developerguide/authorization-endpoint.html

### COGNITO-8. Token endpoint CORS is `*`
- Behaviour: The token endpoint returns `Access-Control-Allow-Origin: *` and does not support custom CORS policies, so browser PKCE clients can call it cross-origin.
- Fake server: Always send `Access-Control-Allow-Origin: *` on `/oauth2/token`.
- Client must: Nothing special; useful as a positive control for browser tests.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/token-endpoint.html

### COGNITO-9. `email_verified` and `phone_number_verified` are strings in userInfo; booleans in ID tokens
- Behaviour: "Currently, Amazon Cognito returns the values for `email_verified` and `phone_number_verified` as strings" from `/oauth2/userInfo`, e.g. `"email_verified": "true"`. The ID token examples show `"email_verified": true`. Custom attributes are always strings, and in `identities` `primary` is `"true"` and `dateCreated` is a millisecond string. An Elastic user saw boolean in one response and string in the other, and parsing failed.
- Fake server: userInfo returns `{"sub":"...","email_verified":"true","phone_number_verified":"true"}`; ID token has `"email_verified":true`.
- Client must: Accept boolean or the strings `"true"`/`"false"`; never compare with `=== true` only.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/userinfo-endpoint.html ; https://docs.aws.amazon.com/cognito/latest/developerguide/amazon-cognito-user-pools-using-the-id-token.html ; https://discuss.elastic.co/t/openid-error-after-authenticating-against-aws-cognito/206018/12

### COGNITO-10. Access token has `client_id`, not `aud`; ID token `aud` is the client
- Behaviour: Access tokens carry `client_id` and `token_use: "access"`; `aud` appears only with RFC 8707 resource binding. ID tokens have `aud` = app client ID and `token_use: "id"`. Access and ID tokens are signed with different keys.
- Fake server: Issue access tokens without `aud`; sign ID and access tokens with different `kid`s.
- Client must: Not validate access tokens as ID tokens; check `token_use` if it inspects them.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/amazon-cognito-user-pools-using-the-access-token.html ; https://docs.aws.amazon.com/cognito/latest/developerguide/amazon-cognito-user-pools-using-the-id-token.html

### COGNITO-11. Revocation accepts only refresh tokens; 400 when the feature is off
- Behaviour: `/oauth2/revoke` revokes a refresh token and all access/ID tokens from it. A non-refresh token gives `400 unsupported_token_type`; a missing token or disabled feature gives `400 invalid_request`; bad credentials give `401 invalid_client`; an invalid or revoked token gives `200`. Success has an empty body. Public clients send `client_id` in the body.
- Fake server: `POST /oauth2/revoke token=<access token>&client_id=...` returns `400 {"error":"unsupported_token_type"}`.
- Client must: Revoke the refresh token, not the access token; treat revocation errors as non-fatal during sign-out.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/revocation-endpoint.html

### COGNITO-12. Non-standard `/logout` endpoint; discovery reportedly lacks `end_session_endpoint`
- Behaviour: Cognito's `/logout` takes `client_id` plus `logout_uri` (must be an "Allowed sign-out URL") or `redirect_uri` with authorize parameters; there is no `id_token_hint` or `post_logout_redirect_uri`. It does not sign users out of external IdPs. A forum user reports that Cognito does not return `end_session_endpoint` in discovery; AWS docs neither list nor deny it (partly UNVERIFIED).
- Fake server: Discovery without `end_session_endpoint`; `GET /logout?client_id=...&logout_uri=https%3A%2F%2Fapp.example%2Fbye` returns `302` to `logout_uri`.
- Client must: Allow a provider-specific logout URL builder; fall back to local sign-out plus revocation.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/logout-endpoint.html ; https://forum.inductiveautomation.com/t/idp-logout-functionality-not-working-as-expected/43802

### COGNITO-13. Callback errors in the query, including `server_error` and IdP timeouts
- Behaviour: Authorize errors redirect with query parameters such as `?error=invalid_request`, `?error=unauthorized_client`, `?error=invalid_scope`, `?error=server_error`, and IdP failures like `?error=invalid_request&error_description=Timeout+occurred+in+calling+IdP+token+endpoint`. The doc examples show no `state` on these error redirects. `prompt=none` without a session returns `?error=login_required&state=...`.
- Fake server: `302 Location: myapp://cb?error=server_error`.
- Client must: Treat `server_error` and `temporarily_unavailable` as retryable; reject any callback whose `state` does not match.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/authorization-endpoint.html

### COGNITO-14. Token refresh counts against the `UserAuthentication` quota
- Behaviour: Token refresh via the token endpoint counts toward the `UserAuthentication` category (default example 120 RPS per account and Region); code redemption has an extra allocation at the same rate. AWS says requests above the quota are throttled. The HTTP status and body the token endpoint returns when throttled are UNVERIFIED.
- Fake server: Throttle mode returns `429` with a JSON body (shape UNVERIFIED).
- Client must: Retry with backoff on 429/5xx; keep the session.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/limits.html

### COGNITO-15. Implicit-flow fragment uses lowercase `token_type=bearer`
- Behaviour: Implicit-grant redirects use `#access_token=...&token_type=bearer&expires_in=3600`, while the token endpoint returns `"Bearer"`.
- Fake server: Not needed for code+PKCE; include only if the suite covers implicit.
- Client must: Compare `token_type` case-insensitively.
- Sources: https://docs.aws.amazon.com/cognito/latest/developerguide/authorization-endpoint.html

## GitHub

GitHub has two kinds of app that sign in users: OAuth apps and GitHub Apps. Both use `https://github.com/login/oauth/authorize` and `https://github.com/login/oauth/access_token`.

### GH-1. No OIDC discovery and no ID token for user sign-in
- Behaviour: GitHub user sign-in is plain OAuth 2.0. GitHub publishes no discovery document at `https://github.com/.well-known/openid-configuration` (HTTP 404 when fetched on 2026-10-26). It issues no `id_token`. The client must call the REST API (`GET /user`) to find out who signed in.
- Fake server: `GET /.well-known/openid-configuration` returns `404`. Token responses never contain `id_token`. The client is configured with static endpoint URLs.
- Client must: support static endpoint configuration with no discovery and no ID token, and must not require `openid` or an `issuer`.
- Sources: https://github.com/.well-known/openid-configuration (observed 404); https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps

### GH-2. Token response is form-encoded unless `Accept: application/json` is sent
- Behaviour: By default the token endpoint returns `application/x-www-form-urlencoded`, for example `access_token=gho_...&scope=repo%2Cgist&token_type=bearer`. With `Accept: application/json` it returns JSON. With `Accept: application/xml` it returns XML.
- Fake server: `POST /login/oauth/access_token` without an `Accept` header returns `200`, `Content-Type: application/x-www-form-urlencoded; charset=utf-8`, body `access_token=gho_16C7e42F292c6912E7710c838347Ae178B4a&scope=repo%2Cgist&token_type=bearer`. With `Accept: application/json` it returns `{"access_token":"gho_...","scope":"repo,gist","token_type":"bearer"}`. With `Accept: application/xml` it returns `<OAuth><token_type>bearer</token_type><scope>repo,gist</scope><access_token>gho_...</access_token></OAuth>`.
- Client must: always send `Accept: application/json`. It should also parse a form-encoded body by `Content-Type`, because older code paths and proxies may still return it. Go's `golang.org/x/oauth2` parses `application/x-www-form-urlencoded` and `text/plain` token bodies with the comment "some endpoints return a query string".
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps; https://go.googlesource.com/oauth2/+/cfe200d5bb2f300f7e51d5259d993d035b3d11b3%5E%21/

### GH-3. Token endpoint errors come back with HTTP 200
- Behaviour: GitHub returns token-endpoint errors such as `incorrect_client_credentials` with HTTP status 200, not 400 as RFC 6749 §5.2 requires. Without the JSON `Accept` header the error is form-encoded too (`error=bad_verification_code&error_description=The+code+passed+is+incorrect+or+expired.`).
- Fake server: `200 OK`, `Content-Type: application/json; charset=utf-8`, body `{"error":"bad_verification_code","error_description":"The code passed is incorrect or expired.","error_uri":"/apps/managing-oauth-apps/troubleshooting-oauth-app-access-token-request-errors/#bad-verification-code"}`. A second variant returns the same error form-encoded. Note that `error_uri` is a relative path.
- Client must: check every token response body for an `error` member before it looks for `access_token`, whatever the status. It must not report "missing access_token" when the server actually gave an error code. It must not treat a relative `error_uri` as an absolute URL.
- Sources: https://github.com/golang/oauth2/issues/441 ("GitHub OAuth does not follow this spec exactly and can return an error response" with "HTTP code 200"); https://issues.apache.org/jira/browse/QPID-7151 ("some OAuth2 token endpoints (I'm looking at you GitHub) return status code 200 even if an error occurs"); https://github.com/OrienteerBAP/Orienteer/issues/774 (form-encoded error body); https://docs.github.com/en/apps/oauth-apps/maintaining-oauth-apps/troubleshooting-oauth-app-access-token-request-errors

### GH-4. Non-standard error codes at the token endpoint
- Behaviour: GitHub uses its own codes: `bad_verification_code` (bad or expired code, where RFC 6749 says `invalid_grant`), `incorrect_client_credentials` (RFC: `invalid_client`), `redirect_uri_mismatch`, `unverified_user_email`, and `bad_refresh_token` (bad or expired refresh token, RFC: `invalid_grant`).
- Fake server: return each of these `error` values in a `200` JSON body as in item GH-3. For refresh, `{"error":"bad_refresh_token","error_description":"The refresh token passed is incorrect or expired."}`. The `error_description` text for `bad_refresh_token` and the HTTP status for refresh errors are UNVERIFIED; the docs give only the code.
- Client must: map unknown error codes in a token response to a failure, not to success. It should treat `bad_refresh_token` like `invalid_grant` (the user must sign in again), and treat `bad_verification_code` as "restart authorization".
- Sources: https://docs.github.com/en/apps/oauth-apps/maintaining-oauth-apps/troubleshooting-oauth-app-access-token-request-errors; https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps (`bad_refresh_token`)

### GH-5. Comma-separated `scope` in the response; empty string for GitHub Apps
- Behaviour: Requests use space-delimited scopes, but the token response joins granted scopes with commas (`"scope":"repo,gist"`). For GitHub App user tokens, `scope` "will always be an empty string".
- Fake server: OAuth app variant returns `"scope":"repo,gist"`. GitHub App variant returns `"scope":""`.
- Client must: accept a comma delimiter as well as a space, or store `scope` as an opaque string. It must not read `"scope":""` as "no permissions" or as "same as requested" (RFC 6749 §5.1 lets an omitted scope mean "as requested", but an empty string is not omitted).
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps; https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens

### GH-6. Lowercase `token_type` `bearer`
- Behaviour: Every example returns `"token_type":"bearer"`, but API calls use `Authorization: Bearer ...`.
- Fake server: return `"token_type":"bearer"`.
- Client must: compare `token_type` without regard to case (RFC 6749 §5.1 says the value is case-insensitive) and send the scheme as `Bearer`.
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps

### GH-7. PKCE optional, S256 only, and `client_secret` still required
- Behaviour: Since 2025-07-14 GitHub accepts PKCE for OAuth apps and GitHub Apps. It does not require PKCE. Only `S256` is accepted (`plain` is rejected). GitHub does not separate public and confidential clients, so the token endpoint always requires `client_secret`, including for native apps and SPAs. Refresh also requires `client_secret` unless the token came from the device flow.
- Fake server: on `POST /login/oauth/access_token` without `client_secret`, return `200` with `{"error":"incorrect_client_credentials","error_description":"The client_id and/or client_secret passed are incorrect.", ...}`. The exact error for a missing secret is UNVERIFIED; the docs show this code for wrong credentials. Reject `code_challenge_method=plain` at `/authorize` (exact error UNVERIFIED). Accept requests with no PKCE at all.
- Client must: let the developer supply a client secret for GitHub even in a "public" app, or route the exchange through a backend. It must use `S256`. It must not assume that "PKCE accepted" means "secret not needed".
- Sources: https://github.blog/changelog/2025-07-14-pkce-support-for-oauth-and-github-app-authentication/; https://github.com/orgs/community/discussions/15752 (reply by @hpsin, quoted in a 2025-07-15 comment: "our implementation always requires the client secret", "native apps and SPA type apps need to include their client secret in the redemption"; the page shows no staff badge, but the author announces the feature for GitHub); https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens

### GH-8. No CORS on the token endpoint
- Behaviour: Browser apps cannot call `https://github.com/login/oauth/access_token` directly.
- Fake server: respond to `OPTIONS` and `POST` without `Access-Control-Allow-Origin`.
- Client must: report a clear "provider does not allow browser token exchange" error and support a backend or proxy exchange. It must not treat the opaque network error as a sign-out.
- Sources: https://github.com/orgs/community/discussions/15752 ("We still have to add support for CORS on the token endpoint unfortunately.", @hpsin, July 2025)

### GH-9. Loopback redirect with any port; `localhost` discouraged
- Behaviour: For loopback callbacks, "The `redirect_uri` does not need to match the port specified in the callback URL for the app." GitHub recommends `127.0.0.1` or `::1` over `localhost`.
- Fake server: register `http://127.0.0.1/path`. Accept `redirect_uri=http://127.0.0.1:53124/path`. Reject a different path.
- Client must: bind an ephemeral port on `127.0.0.1` and send that full URI in both the authorize and token requests (RFC 8252 §7.3).
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps

### GH-10. Redirect URI is optional and may match by subdirectory
- Behaviour: If `redirect_uri` is omitted, GitHub uses the first registered callback URL. OAuth apps may enable wildcard matching. Then the host (subdomains allowed) and port must match, and the path may be a subdirectory of the callback URL (`https://example.com/path/subdir/other` matches `https://example.com/path`). For GitHub Apps the `redirect_uri` "can't contain any additional parameters".
- Fake server: accept a missing `redirect_uri` and redirect to the first registered callback. In "wildcard" mode accept subdomains and sub-paths. In exact mode require a byte-exact match.
- Client must: always send `redirect_uri` explicitly, and send the same value at the token endpoint.
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps; https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app

### GH-11. Callback errors in the query, with `state`
- Behaviour: Authorization errors come back as query parameters: `?error=access_denied`, `?error=redirect_uri_mismatch`, `?error=application_suspended`, plus `error_description`, `error_uri` and `state`. `application_suspended` is not an RFC 6749 §4.1.2.1 code.
- Fake server: `302 Location: http://127.0.0.1:PORT/callback?error=application_suspended&error_description=Your+application+has+been+suspended.&error_uri=...&state=xyz`. The exact description text is UNVERIFIED.
- Client must: check `state` first, then surface unknown `error` values as a provider error.
- Sources: https://docs.github.com/en/apps/oauth-apps/maintaining-oauth-apps/troubleshooting-authorization-request-errors

### GH-12. GitHub-initiated callback with no `state` (GitHub App install flow)
- Behaviour: When a GitHub App has "Request user authorization (OAuth) during installation" set, GitHub starts the web flow itself after installation and redirects to the first callback URL with a `code`. "The `state` parameter is not returned when GitHub initiates the OAuth flow during app installation."
- Fake server: without any prior `/authorize` request from the client, send `302 Location: <first callback>?code=abc123`, with no `state` parameter.
- Client must: reject an unsolicited callback with no matching pending request (CSRF rule, RFC 6749 §10.12) with a clear error. It must not crash, and it must not redeem the code silently.
- Sources: https://docs.github.com/en/enterprise-server@3.2/developers/apps/building-github-apps/identifying-and-authorizing-users-for-github-apps (note quoted verbatim); https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app; https://www.github.com/supabase/auth-helpers/issues/584 (library hit "OAuth state parameter missing")

### GH-13. Expiring GitHub App user tokens: fixed `expires_in` and `refresh_token_expires_in`
- Behaviour: GitHub App user tokens expire after 8 hours (`expires_in: 28800`). The refresh token lasts 6 months (`refresh_token_expires_in: 15897600`). If the app opted out of expiring tokens, `expires_in`, `refresh_token` and `refresh_token_expires_in` are omitted and the token never expires. OAuth app tokens never carry `expires_in` and have no refresh token.
- Fake server: expiring variant returns `{"access_token":"ghu_...","expires_in":28800,"refresh_token":"ghr_...","refresh_token_expires_in":15897600,"scope":"","token_type":"bearer"}`. Non-expiring variant returns `{"access_token":"ghu_...","scope":"","token_type":"bearer"}`.
- Client must: treat a missing `expires_in` as "unknown or non-expiring" and fall back to a 401 from the resource server. It should keep `refresh_token_expires_in` if present, and must ignore unknown fields.
- Sources: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app; https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens

### GH-14. Single-use refresh tokens that also revoke the old access token
- Behaviour: "Once you use a refresh token, that refresh token and the old user access token will no longer work." Each refresh returns a new refresh token.
- Fake server: on `grant_type=refresh_token`, return a new `ghr_` token, and mark the old refresh token and old access token invalid at once. A second use of the old refresh token returns `200` with `{"error":"bad_refresh_token", ...}`. API calls with the old access token return `401`.
- Client must: persist the new refresh token atomically before using the new access token. It must serialize concurrent refreshes, and must stop using the old access token right away (not "until it expires").
- Sources: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens

### GH-15. Ten tokens per user/application/scope; older tokens revoked silently
- Behaviour: "There is a limit of ten tokens that are issued per user/application/scope combination." Past that, GitHub revokes one existing token: first an unused token created more than a minute ago, else the least recently used, else the oldest. A separate rate limit of ten tokens created per hour shows a re-authorization prompt; it does not revoke tokens.
- Fake server: keep a per-(user, client, scope) list. On the 11th issuance, revoke the oldest unused or least recently used token so that API calls with it return `401`.
- Client must: expect a token to be revoked server-side without warning, especially when the same user signs in on many devices or many test runs. A 401 must lead to refresh or re-authentication, not a crash.
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps; https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation

### GH-16. Automatic revocation (unused for a year, leaked to a public repo)
- Behaviour: GitHub revokes an OAuth token not used for one year. It also revokes a valid token pushed to a public repository.
- Fake server: a test hook that revokes a token out of band; then API calls return `401`.
- Client must: handle out-of-band revocation of a non-expiring token.
- Sources: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation

### GH-17. No RFC 7009 revocation endpoint; REST `DELETE` with the client secret instead
- Behaviour: Revocation uses `DELETE /applications/{client_id}/token` (one token) or `DELETE /applications/{client_id}/grant` (all tokens for the user) on `api.github.com`. Both need HTTP Basic auth with `client_id:client_secret`. They return `204`, or `422` for "Validation failed, or the endpoint has been spammed."
- Fake server: no `revocation_endpoint`. `DELETE /applications/{client_id}/token` with JSON body `{"access_token":"..."}` and Basic auth returns `204`.
- Client must: treat revocation as optional per provider. Sign-out must succeed locally when no RFC 7009 endpoint exists.
- Sources: https://docs.github.com/en/rest/apps/oauth-applications

### GH-18. Device flow polling errors (for comparison)
- Behaviour: Device flow polling returns `authorization_pending`, `slow_down` (adds 5 seconds to the interval and includes the new `interval`), `expired_token` (the text also says `token_expired`), `access_denied`, and `incorrect_device_code`. The device flow needs no `client_secret`. Device-flow submissions are limited to 50 per hour per application.
- Fake server: `200` with `{"error":"slow_down","error_description":"...","interval":10}`. The HTTP status is UNVERIFIED but follows item GH-3.
- Client must (if it supports the device flow): raise the interval on `slow_down`, and accept both `expired_token` and `token_expired`.
- Sources: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps

### GH-19. Token endpoint rate limiting or 5xx
- Behaviour: UNVERIFIED. No source found for a 429 or 5xx shape from `github.com/login/oauth/access_token`. The documented limits (items GH-15 and GH-18) show as an authorization prompt or as device-flow errors.

## Slack

Slack has two token endpoints. "Add to Slack" app installs use `oauth.v2.access`. Sign in with Slack (OIDC) uses `openid.connect.token`.

### SLACK-1. `ok:false` envelope instead of RFC 6749 errors, likely with HTTP 200
- Behaviour: Every Web API response, `oauth.v2.access` included, is a JSON object with a boolean `ok`. Failures carry `"ok": false` and a Slack-specific `error` string such as `invalid_code`, `invalid_refresh_token`, `bad_client_secret`, `invalid_grant_type`, `invalid_client_id`, or `bad_redirect_uri`. Slack's docs do not state the HTTP status. Slack's official Python SDK treats a response as successful only when the status is 200 and `ok` is true, and a third-party project observed `HTTP/2 200` with `{"ok":false,"error":"invalid_auth"}`.
- Fake server: `POST /api/oauth.v2.access` returns `200`, `Content-Type: application/json; charset=utf-8`, body `{"ok":false,"error":"invalid_code"}`. There is no `error_description`.
- Client must: for Slack, read `ok` first. Map `invalid_code` and `invalid_refresh_token` to the RFC equivalents (`invalid_grant`), and never treat a 200 as success by status alone.
- Sources: https://docs.slack.dev/apis/web-api/ ("a top-level boolean property `ok` that indicates success or failure"); https://docs.slack.dev/reference/methods/oauth.v2.access (error list, example `{"ok": false, "error": "invalid_client_id"}`); https://docs.slack.dev/tools/python-slack-sdk/reference/web/ (validate checks status 200 and `ok`); https://github.com/ut-issl/capsula/pull/1293 (observed HTTP 200 with `ok:false`). The HTTP status for `oauth.v2.access` errors in particular is not stated by Slack; treat it as observed, not documented.

### SLACK-2. Nested, non-standard success response with `token_type: "bot"`
- Behaviour: `oauth.v2.access` returns `ok`, a bot `access_token` (`xoxb-`), `"token_type":"bot"`, comma-separated `scope`, `bot_user_id`, `app_id`, `team`, `enterprise`, and an `authed_user` object with its own `access_token` (`xoxp-`), `scope` and `token_type` (`user`). App-level tokens use `"token_type":"app_level"`. When only user scopes are requested, the top-level `access_token` may be absent and the token lives only in `authed_user`. That last point is UNVERIFIED in the pages fetched.
- Fake server: `200` with `{"ok":true,"access_token":"xoxb-...","token_type":"bot","scope":"commands,incoming-webhook","bot_user_id":"U0KRQLJ9H","app_id":"A0KRD7HC3","team":{"name":"Example","id":"T9TK3CUKW"},"enterprise":null,"authed_user":{"id":"U1234","scope":"chat:write","access_token":"xoxp-...","token_type":"user"}}`.
- Client must: not reject `token_type` values other than `Bearer` in a generic way (or offer a provider hook). It must not assume the top-level fields are the user's token, and it must keep provider-specific extra fields.
- Sources: https://docs.slack.dev/authentication/installing-with-oauth; https://docs.slack.dev/authentication/using-token-rotation

### SLACK-3. Comma-separated scopes in requests and responses
- Behaviour: Slack's install flow takes `scope=incoming-webhook,commands` (commas) in the authorize URL and returns comma-separated `scope`. Sign in with Slack uses space-separated scopes (`openid profile email`).
- Fake server: `/oauth/v2/authorize` accepts commas, and returns `"scope":"commands,incoming-webhook"`.
- Client must: allow a provider-specific scope separator when building the request, and parse either separator in the response.
- Sources: https://docs.slack.dev/authentication/installing-with-oauth; https://docs.slack.dev/authentication/sign-in-with-slack

### SLACK-4. PKCE marks the app as a public client, irreversibly; required for custom schemes
- Behaviour: PKCE has been GA since 2026-03-30. With PKCE enabled, the code exchange on `oauth.v2.access` works without a client secret. "Enabling PKCE marks your app as a public client." This "is a one-way operation". PKCE parameters are "required when redirecting to a custom URI scheme". Desktop redirects (custom schemes and `localhost` for PKCE apps) cannot request bot scopes. The changelog does not say whether `plain` is accepted (UNVERIFIED). `openid.connect.token` is not mentioned (UNVERIFIED for PKCE).
- Fake server: PKCE-app mode accepts `POST /api/oauth.v2.access` with `client_id`, `code`, `code_verifier` and no secret. A custom-scheme redirect without `code_challenge` fails at authorize (error code UNVERIFIED).
- Client must: support a "no secret" exchange, and support a custom-scheme redirect with PKCE always on.
- Sources: https://docs.slack.dev/changelog/2026/03/30/pkce

### SLACK-5. Redirect URI: HTTPS only, subdirectory match, no fragment (non-PKCE apps)
- Behaviour: The `redirect_uri` "must use HTTPS" and "must match or be a subdirectory of a Redirect URL configured". A different port is a bad example. The same `redirect_uri` must be sent at authorize and at access, or Slack returns `bad_redirect_uri`. PKCE apps add custom schemes and `localhost` (item SLACK-4).
- Fake server: accept `https://example.com/path/subdir` when `https://example.com/path` is registered. Reject `http://` and other ports with `{"ok":false,"error":"bad_redirect_uri"}`.
- Client must: send an identical `redirect_uri` in both steps.
- Sources: https://docs.slack.dev/authentication/installing-with-oauth; https://docs.slack.dev/authentication/sign-in-with-slack

### SLACK-6. Opt-in token rotation: 12-hour tokens, single-use refresh tokens with a short grace period, cap of 2 active tokens
- Behaviour: With rotation on (always on for custom-scheme PKCE installs), access tokens carry `expires_in: 43200` and a `refresh_token` (`xoxe-1-...`). "Refresh tokens are designed to be used once." The used refresh token "is revoked after a short grace period" (length not stated). Slack enforces "a limit of 2 active tokens. If more than 2 tokens exist after the refresh, the oldest additional token will be revoked." Rotation "may not be turned off once it's turned on". PKCE-app refresh tokens expire after 30 days.
- Fake server: refresh returns `200` with `{"ok":true,"access_token":"xoxe.xoxb-1-...","token_type":"bot","expires_in":43200,"refresh_token":"xoxe-1-NEW","scope":"..."}`. Allow the old refresh token for a configurable grace period (for example 5 seconds; the real value is not documented). After that return `{"ok":false,"error":"invalid_refresh_token"}`. A third refresh revokes the oldest access token.
- Client must: store the new refresh token at once, and serialize refreshes. It must not refresh more often than needed (the 2-token cap means fast refreshes revoke tokens still in use), and must re-authenticate when a PKCE refresh token is older than 30 days.
- Sources: https://docs.slack.dev/authentication/using-token-rotation; https://docs.slack.dev/changelog/2026/03/30/pkce; https://docs.slack.dev/reference/methods/oauth.v2.access

### SLACK-7. Sign in with Slack discovery under-reports its capabilities
- Behaviour: `https://slack.com/.well-known/openid-configuration` has `issuer` `https://slack.com` (no trailing slash), token endpoint `https://slack.com/api/openid.connect.token`, `grant_types_supported: ["authorization_code"]` only (although the docs describe `grant_type=refresh_token` on that endpoint under token rotation), `response_modes_supported: ["query"]`, `claims_supported: ["sub","auth_time","iss"]` (although ID tokens contain `email`, `nonce`, `at_hash` and more), and no `revocation_endpoint` or `code_challenge_methods_supported`.
- Fake server: serve that exact document.
- Client must: not refuse refresh or PKCE because discovery does not list them, and not reject ID-token claims missing from `claims_supported`. Without `code_challenge_methods_supported` the client should still send S256 (RFC 9700 recommends PKCE in any case).
- Sources: https://slack.com/.well-known/openid-configuration (fetched 2026-10-26); https://docs.slack.dev/authentication/sign-in-with-slack

### SLACK-8. OIDC token response is wrapped in `ok`, with namespaced claims
- Behaviour: `openid.connect.token` returns `ok`, `access_token`, `"token_type":"Bearer"` and `id_token`. ID tokens carry URL-namespaced claims (`https://slack.com/team_id`, `https://slack.com/user_id`, ...). `email_verified` is a real boolean, and `date_email_verified` is a number. Nonce is supported and must be checked.
- Fake server: `200` with `{"ok":true,"access_token":"xoxp-...","token_type":"Bearer","id_token":"<JWT>"}`. The JWT is RS256 with `iss` `https://slack.com`, the expected `nonce`, `at_hash`, and the `https://slack.com/team_id` claim.
- Client must: ignore the extra `ok` member and keep unknown claims.
- Sources: https://docs.slack.dev/authentication/sign-in-with-slack

### SLACK-9. Revocation via `auth.revoke`, not RFC 7009
- Behaviour: Revocation is a Web API method, `auth.revoke`. The token goes in the `Authorization` header or as a POST `token` parameter. Success is `{"ok": true, "revoked": true}`. `test=1` does not actually revoke. Errors include `invalid_auth`, `token_expired`, `token_revoked`, `ratelimited`.
- Fake server: `POST /api/auth.revoke` with `Authorization: Bearer xoxp-...` returns `200` with `{"ok":true,"revoked":true}`.
- Client must: treat revocation as provider-specific and optional.
- Sources: https://docs.slack.dev/reference/methods/auth.revoke

### SLACK-10. HTTP 429 with `Retry-After`
- Behaviour: Slack returns `HTTP 429 Too Many Requests` with `Retry-After` (seconds) on all HTTP APIs. `oauth.v2.access` has "special rate limits" (600 per minute) and lists the `ratelimited` error.
- Fake server: `429`, `Retry-After: 30`, body `{"ok":false,"error":"ratelimited"}`. The body shape is UNVERIFIED (the rate-limit page does not show a body).
- Client must: retry after `Retry-After` and never clear the session on 429.
- Sources: https://docs.slack.dev/apis/web-api/rate-limits; https://docs.slack.dev/reference/methods/oauth.v2.access

### SLACK-11. Callback error shape
- Behaviour: UNVERIFIED. The Slack pages fetched describe only `code` and `state` on the callback. They do not document `error=access_denied` on the redirect.

## Salesforce

### SF-1. No `expires_in`; `issued_at` in milliseconds as a string, plus `signature`
- Behaviour: Token responses include `access_token`, `signature` (Base64 HMAC-SHA256 of `id` + `issued_at` with the client secret), `scope`, `id_token` (with `openid`), `instance_url`, `id` (identity URL), `token_type` and `issued_at` ("Time stamp of when the signature was created in milliseconds"). Developers report that `expires_in` is absent. The access-token lifetime follows the org's session timeout, which the client cannot see.
- Fake server: `200` with `{"access_token":"00D...!AQ...","signature":"<base64>","scope":"refresh_token openid api","id_token":"<JWT>","instance_url":"https://example.my.salesforce.com","id":"https://login.salesforce.com/id/00Dxx/005xx","token_type":"Bearer","issued_at":"1761500000000","refresh_token":"5Aep..."}`. No `expires_in`.
- Client must: treat `expires_in` as optional. With no expiry, it should refresh on a 401 from the resource server. It must not read `issued_at` as seconds, and must not crash on a string timestamp.
- Sources: https://help.salesforce.com/s/articleView?id=remoteaccess_oauth_web_server_flow.htm&language=en_US&type=5 (field definitions; page text seen through search extract); https://trailhead.salesforce.com/trailblazer-community/feed/0D54S00000A8VNmSAN (developer report: no `expires_in`; community, not staff); https://community.developer.atlassian.com/t/external-provider-as-salesforce-does-not-refresh-the-token-when-it-expires/63775 ("the expires_in property does not exist"). That `issued_at` is a JSON string, not a number, is UNVERIFIED from first-party text.

### SF-2. `instance_url` is the API base
- Behaviour: The token response tells the client which host to call ("A URL indicating the instance of the user's org").
- Fake server: return `instance_url` that differs from the token endpoint host.
- Client must: expose extra token-response fields to the app, and not drop unknown members.
- Sources: https://help.salesforce.com/s/articleView?id=remoteaccess_oauth_web_server_flow.htm&language=en_US&type=5 (search extract)

### SF-3. Refresh response omits `refresh_token` unless rotation is on
- Behaviour: The refresh-flow example response has `id`, `issued_at`, `instance_url`, `signature`, `access_token`, `token_type` and `scope`, with no `refresh_token`. Salesforce includes `refresh_token` "only when refresh token rotation is enabled". The example also has no `expires_in`, and `id_token` is absent.
- Fake server: on `grant_type=refresh_token`, return `200` with `{"access_token":"...","signature":"...","scope":"refresh_token openid api","instance_url":"https://example.my.salesforce.com","id":"https://login.salesforce.com/id/00Dxx/005xx","token_type":"Bearer","issued_at":"1761500360000"}`.
- Client must: keep the old refresh token when the response omits one (RFC 6749 §6), and keep the old ID token claims.
- Sources: https://help.salesforce.com/s/articleView?language=en_US&id=sf.remoteaccess_oauth_refresh_token_flow.htm&type=5 (search extract)

### SF-4. Refresh token rotation (RTR) with reuse revocation and no grace period
- Behaviour: With RTR, each refresh returns a new refresh token and the old one is invalidated. Reuse of a rotated token invalidates the current refresh token and its access tokens. Salesforce made RTR default on new External Client Apps in July 2026, and made it a partner-app requirement by 2026-05-11. A third-party report says there is no grace window for concurrent refreshes.
- Fake server: RTR mode returns a new `refresh_token` on each refresh. Reuse of an old one returns `400` with `{"error":"invalid_grant","error_description":"expired access/refresh token"}` and revokes the whole family.
- Client must: serialize refreshes per session (one in flight), and persist the new token before use.
- Sources: https://help.salesforce.com/s/articleView?id=005316711&language=en_US&type=1 (RTR article; search extract); https://help.salesforce.com/s/articleView?id=005390146&language=en_US&type=1 (RTR default on new ECAs, July 2026; search extract); https://developer.salesforce.com/docs/platform/isvforce/guide/secure-code-ac-eca.html (definition and 2026-05-11 date; search extract, direct fetch 403); https://www.quotaguard.com/blog/salesforce-rtr-concurrency-safe-token-storage (third-party: no grace period); https://github.com/forcedotcom/SalesforceMobileSDK-Android/issues/2879 (Salesforce's own SDK failed to persist rotated tokens; fixed in 13.2.1)

### SF-5. Concurrent identical refresh requests are rejected
- Behaviour: Two identical refresh requests at the same time can fail with "token request is already being processed". Earlier this showed a misleading "expired authorization code" message. Salesforce changed this API error in a release note.
- Fake server: when a second refresh with the same token arrives while one is in flight, return `400` with `{"error":"invalid_grant","error_description":"token request is already being processed"}`. The exact `error` code is UNVERIFIED.
- Client must: dedupe in-flight refreshes, and not sign out on this error (retry after the first completes).
- Sources: https://help.salesforce.com/s/articleView?id=release-notes.rn_security_refresh_token_error.htm&language=en_US&release=252&type=5 (search extract; exact text not confirmed by direct fetch)

### SF-6. `invalid_grant` "expired access/refresh token" for revoked, rotated or expired refresh tokens
- Behaviour: The standard code is used, but one description covers expiry, revocation, rotation-out and the approval-limit eviction (item SF-7).
- Fake server: `400`, `Content-Type: application/json`, `{"error":"invalid_grant","error_description":"expired access/refresh token"}`.
- Client must: treat it as terminal (re-authenticate), not as transient.
- Sources: https://help.salesforce.com/s/articleView?id=sf.remoteaccess_oauth_flow_errors.htm&language=en_US&type=5 (search extract); https://www.nango.dev/blog/salesforce-oauth-refresh-token-invalid-grant (third-party)

### SF-7. Five approvals (refresh tokens) per user per app; the oldest is revoked
- Behaviour: Each connected app allows five unique approvals per user. A sixth approval revokes the oldest, and each OAuth 2.0 refresh token counts as an approval. Signing in on a sixth device or test run silently kills the first device's refresh token.
- Fake server: per (user, client) keep at most 5 refresh tokens. Issuing a 6th revokes the oldest, which then gets `invalid_grant` "expired access/refresh token".
- Client must: surface this as "signed out elsewhere" and re-authenticate.
- Sources: https://help.salesforce.com/s/articleView?language=en_US&id=sf.remoteaccess_request_manage.htm&type=5 (official; text seen only through search extract); https://developer.salesforce.com/docs/atlas.en-us.222.0.packagingGuide.meta/packagingGuide/remoteaccess_request_manage.htm (direct fetch 403); https://www.nango.dev/blog/salesforce-oauth-refresh-token-invalid-grant (third-party). Treat as partly verified.

### SF-8. Idle refresh-token expiry (30 days) and policy-driven expiry
- Behaviour: The default refresh token policy is "valid until revoked", but admins can set expiry. Salesforce's partner security guide says idle refresh tokens are invalidated after 30 days. A third-party blog dates enforcement from 2026-11-04 for production (2026-10-21 for sandboxes).
- Fake server: refresh tokens unused for N days return `400` with `invalid_grant` "expired access/refresh token".
- Client must: handle refresh-token expiry without `refresh_token_expires_in`.
- Sources: https://help.salesforce.com/s/articleView?language=en_US&id=xcloud.shr_manage_oauth_access_policies_for_a_connected_app_refresh_token_expiration_policy.htm&type=5 (search extract); https://developer.salesforce.com/docs/platform/isvforce/guide/secure-code-ac-eca.html (search extract); https://vantagepoint.io/blog/sf/salesforce-refresh-token-ttl-idle-integrations (third-party dates; UNVERIFIED from Salesforce)

### SF-9. Issuer depends on the login host (login, test, My Domain)
- Behaviour: `https://login.salesforce.com/.well-known/openid-configuration` has `issuer` `https://login.salesforce.com`. `https://test.salesforce.com/...` has `issuer` `https://test.salesforce.com`. Community answers say a My Domain or Experience Cloud site has its own issuer. The `iss` in an ID token follows the host the user signed in through.
- Fake server: serve several discovery documents with different issuers. Mint ID tokens whose `iss` is the host used at `/authorize`.
- Client must: validate `iss` against the discovery document it actually used (OIDC Discovery §4.3), and not hard-code one Salesforce issuer.
- Sources: https://login.salesforce.com/.well-known/openid-configuration and https://test.salesforce.com/.well-known/openid-configuration (fetched 2026-10-26); https://fusionauth.nodebb.com/community/forum/post/6338 (issuer mismatch report; community). The My Domain issuer value is UNVERIFIED first-party.

### SF-10. PKCE S256 only; secret optional by setting; PKCE enforceable
- Behaviour: Discovery lists `code_challenge_methods_supported: ["S256"]`. The app setting "Require Secret for Web Server Flow" can be cleared so the token request needs no `client_secret`. "Require Proof Key for Code Exchange (PKCE) Extension for Supported Authorization Flows" blocks flows without PKCE. Discovery also lists DPoP algorithms and mTLS aliases.
- Fake server: two modes. "Secret required" rejects a secretless exchange with `400 {"error":"invalid_client","error_description":"invalid client credentials"}` (exact description UNVERIFIED). "PKCE required" rejects a missing verifier.
- Client must: support both modes, and always send S256.
- Sources: https://login.salesforce.com/.well-known/openid-configuration; https://help.salesforce.com/s/articleView?language=en_US&id=sf.connected_app_create_api_integration.htm (search extract)

### SF-11. Custom-scheme callbacks allowed
- Behaviour: Mobile SDK docs say "the callback URL doesn't have to be a valid URL; it only has to match what the app expects", for example `sfdc://`.
- Fake server: accept a registered `myapp://oauth/done`, with exact matching.
- Client must: support custom schemes.
- Sources: https://developer.salesforce.com/docs/platform/mobile-sdk/guide/connected-apps-howto.html (search extract; direct fetch 403)

### SF-12. CORS on the token endpoint is off by default and allowlist-gated
- Behaviour: CORS is on by default only for `/.well-known/openid-configuration`, `/.well-known/auth-configuration` and `/services/oauth2/id/keys`. `/services/oauth2/token` supports CORS only on My Domain or Experience Cloud hosts, after an admin selects "Enable CORS for OAuth endpoints" and allowlists the origin. A non-allowlisted origin gets HTTP 403.
- Fake server: `POST /services/oauth2/token` with `Origin: https://app.example` not in the allowlist returns `403` with no `Access-Control-Allow-Origin`. Discovery responds with `Access-Control-Allow-Origin: *`.
- Client must: show a configuration error, not a sign-out, on a CORS failure.
- Sources: https://help.salesforce.com/s/articleView?language=en_US&id=sf.remoteaccess_oauth_endpoints_cors.htm&type=5 (search extract)

### SF-13. RFC 7009-style revocation with cascading effect
- Behaviour: `POST /services/oauth2/revoke` (form body `token=...`) returns 200 on success. Errors are 400 with `unsupported_token_type` or `invalid_token`. Revoking a refresh token also revokes its access tokens.
- Fake server: `200` with an empty body on success. On a bad token, `400` with `{"error":"invalid_token","error_description":"invalid token"}` (description UNVERIFIED).
- Client must: ignore 400 `invalid_token` on sign-out (RFC 7009 §2.2 says invalid tokens get 200, so Salesforce deviates here).
- Sources: https://help.salesforce.com/s/articleView?id=remoteaccess_revoke_token.htm&language=en_US&type=5 (search extract); https://developer.salesforce.com/blogs/developer-relations/2011/11/revoking-oauth-2-0-access-tokens-and-refresh-tokens

### SF-14. `refresh_token` scope needed to get a refresh token
- Behaviour: The connected app must include the `refresh_token` (or `offline_access`) scope, or no refresh token is issued.
- Fake server: omit `refresh_token` from the code-exchange response unless `refresh_token` or `offline_access` is in scope.
- Client must: tolerate a missing refresh token on the first exchange.
- Sources: https://community.auth0.com/t/refresh-token-missing-from-salesforce-social-login/78569 (community; summarized by search). First-party wording is UNVERIFIED.

## Facebook / Meta

### META-1. Discovery document is implicit-only, has no `token_endpoint`, and uses inconsistent hosts and trailing slashes
- Behaviour: `https://www.facebook.com/.well-known/openid-configuration/` (note the trailing slash) has `issuer` `https://www.facebook.com`, `authorization_endpoint` `https://facebook.com/dialog/oauth/` (no `www`, trailing slash), and `response_types_supported: ["id_token","token id_token"]`. There is no `token_endpoint`, no `code`, and no `code_challenge_methods_supported`. `subject_types_supported` is `["pairwise"]`. Yet the docs describe an OIDC code flow with PKCE at `graph.facebook.com/vXX.X/oauth/access_token`.
- Fake server: serve that document at the trailing-slash path. Redirect or 404 the path without the slash (behaviour of the no-slash path is UNVERIFIED).
- Client must: accept manual endpoint overrides when discovery is incomplete. It must compare `issuer` exactly (`https://www.facebook.com`) and not normalize hosts.
- Sources: https://www.facebook.com/.well-known/openid-configuration/ (fetched 2026-10-26); https://developers.facebook.com/documentation/facebook-login/guides/advanced/oidc-token.md

### META-2. Token exchange is a GET with versioned paths
- Behaviour: The manual flow exchanges the code with `GET https://graph.facebook.com/v26.0/oauth/access_token?client_id=...&redirect_uri=...&client_secret=...&code=...`. The path carries a Graph API version.
- Fake server: accept `GET /v26.0/oauth/access_token` with query parameters. Whether `POST` is accepted is UNVERIFIED (the docs show only GET).
- Client must: allow a provider-specific HTTP method, or fall back to GET for Meta.
- Sources: https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow

### META-3. PKCE with optional secret, and `plain` allowed
- Behaviour: In the OIDC code flow, "The client_secret parameter is now optional. If not included in your request, you must specify the code_verifier." Both `S256` (recommended) and `plain` are accepted. `nonce` is required with `scope=openid`.
- Fake server: `GET /v11.0/oauth/access_token?client_id=..&redirect_uri=..&code_verifier=..&code=..` returns `200` with `{"access_token":"EAA...","token_type":"bearer","expires_in":5183944,"id_token":"<JWT>"}`. The exact `token_type` casing here is UNVERIFIED; the long-lived token example shows `bearer`.
- Client must: use S256 and send `nonce`.
- Sources: https://developers.facebook.com/documentation/facebook-login/guides/advanced/oidc-token.md

### META-4. No refresh tokens; long-lived token exchange instead
- Behaviour: There is no `refresh_token`. A short-lived user token ("expire in hours") is swapped server-side for a long-lived one (`grant_type=fb_exchange_token`, needs `client_secret`, "generally lasts about 60 days"). An expired token cannot be exchanged; the user must sign in again. The SDKs refresh automatically if the app was used in the last 90 days.
- Fake server: the code exchange returns no `refresh_token`. `GET /oauth/access_token?grant_type=fb_exchange_token&client_id=..&client_secret=..&fb_exchange_token=..` returns `{"access_token":"...","token_type":"bearer","expires_in":5183944}`.
- Client must: work with no refresh token (session ends at expiry), and not loop on refresh.
- Sources: https://developers.facebook.com/docs/facebook-login/guides/access-tokens/get-long-lived

### META-5. Graph-style error object, not RFC 6749 errors
- Behaviour: Errors use `{"error":{"message":..., "type":"OAuthException", "code":190, "error_subcode":460, "error_user_title":..., "error_user_msg":..., "fbtrace_id":...}}`. `error` is an object, not a string. Code 190 means an expired or invalid token. Subcodes include 458 (app not installed), 460 (password changed), 463 (expired), and 467 (invalid). The docs do not state the HTTP status.
- Fake server: `400`, `Content-Type: application/json`, body `{"error":{"message":"Error validating access token: Session has expired.","type":"OAuthException","code":190,"error_subcode":463,"fbtrace_id":"AbC123"}}`. The status 400 and the message text are UNVERIFIED; the docs give the body shape only.
- Client must: handle a token-endpoint error where `error` is an object, without a type error. It should surface `message` and `code`, and map code 190 to re-authentication.
- Sources: https://developers.facebook.com/docs/graph-api/guides/error-handling

### META-6. Throttling and transient codes inside the same envelope
- Behaviour: Code 4 and code 17 are throttling (retry later). Codes 1 and 2 are temporary errors (retry). These come in the same `error` object.
- Fake server: `{"error":{"message":"(#4) Application request limit reached","type":"OAuthException","code":4,"fbtrace_id":"..."}}`. The HTTP status and message are UNVERIFIED.
- Client must: retry codes 1, 2, 4 and 17 with backoff, and not sign out.
- Sources: https://developers.facebook.com/docs/graph-api/guides/error-handling

### META-7. Callback error adds non-standard `error_reason`
- Behaviour: On cancel the redirect carries `error_reason=user_denied&error=access_denied&error_description=Permissions+error.`.
- Fake server: `302 Location: <redirect>?error_reason=user_denied&error=access_denied&error_description=Permissions+error.&state=xyz`. That `state` is echoed on errors is UNVERIFIED; the docs say `state` is returned unchanged.
- Client must: ignore unknown callback parameters.
- Sources: https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow

### META-8. Comma-or-space scopes
- Behaviour: `scope` is a "comma or space separated list".
- Fake server: accept both.
- Client must: none beyond normal behaviour, but a fake can test that the client's separator is accepted.
- Sources: https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow

### META-9. Strict redirect URI matching, with `state` as the only exception
- Behaviour: Strict Mode needs an exact match including query parameters, except `state`, whose value is ignored. Desktop apps must use `https://www.facebook.com/connect/login_success.html`. Custom schemes and loopback for the manual flow are not documented (UNVERIFIED).
- Fake server: exact-match `redirect_uri`, ignoring only the `state` value.
- Client must: not add extra query parameters to the redirect URI.
- Sources: https://developers.facebook.com/documentation/facebook-login/security; https://developers.facebook.com/blog/post/2017/12/18/strict-uri-matching/; https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow

### META-10. Revocation by deleting permissions
- Behaviour: There is no RFC 7009 endpoint. `DELETE /{user-id}/permissions` with a user or app token invalidates all of the user's tokens for the app and returns `true`.
- Fake server: `DELETE /v26.0/me/permissions?access_token=...` returns `200` with body `true`. Whether the body is a bare `true` or `{"success":true}` is UNVERIFIED; the docs say "a response of `true`".
- Client must: treat revocation as optional per provider.
- Sources: https://developers.facebook.com/docs/facebook-login/guides/permissions/request-revoke

### META-11. Limited Login (iOS) ID token: issuer and nonce checks; short-lived
- Behaviour: Limited Login returns an `AuthenticationToken` JWT (RS256). The app must check issuer, `aud` = app ID, `nonce`, signature against the JWKS, and `exp` ("Tokens are short-lived"). The page does not list the issuer value. Discovery says `https://www.facebook.com`.
- Fake server: mint an RS256 JWT with `iss` `https://www.facebook.com`, `aud` app ID, and `nonce`. A test variant uses a different `iss` (UNVERIFIED whether Meta ever does this).
- Client must: validate `iss`, `aud`, `nonce` and `exp`.
- Sources: https://developers.facebook.com/docs/facebook-login/limited-login/token/validating; https://www.facebook.com/.well-known/openid-configuration/

### META-12. Token introspection through `debug_token`
- Behaviour: Token validity is checked with `GET /debug_token?input_token=..&access_token=<app token>`. The response has `data.is_valid`, `data.expires_at` (Unix seconds) and `data.scopes`. This is not RFC 7662.
- Fake server: `{"data":{"app_id":"..","user_id":"..","is_valid":true,"expires_at":1767225600,"scopes":["email","public_profile"]}}`.
- Client must: none for core flows. It is noted for apps that need expiry when `expires_in` is missing.
- Sources: https://developers.facebook.com/docs/facebook-login/guides/advanced/manual-flow

## X (Twitter) OAuth 2.0

### X-1. PKCE mandatory, `plain` allowed; public clients without a secret
- Behaviour: The authorization code flow always uses PKCE (`S256` or `plain`). Native App and Single page App are public clients and send `client_id` in the body with no secret. Confidential clients (Web App, bot) use HTTP Basic and need not send `client_id` in the body.
- Fake server: `POST https://api.x.com/2/oauth2/token`, `Content-Type: application/x-www-form-urlencoded`, body `code=..&grant_type=authorization_code&client_id=..&redirect_uri=..&code_verifier=..`. Reject a missing `code_verifier`.
- Client must: send `client_id` in the body for public clients and always use S256.
- Sources: https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code; https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/user-access-token

### X-2. Authorization code valid for only 30 seconds
- Behaviour: "The auth_code has a time limit of 30 seconds." RFC 6749 §4.1.2 recommends at most 10 minutes; X is far shorter.
- Fake server: return `400 {"error":"invalid_request","error_description":"Value passed for the authorization code was invalid."}` when the code is redeemed after 30 seconds. The error body text is UNVERIFIED.
- Client must: redeem the code at once. It must not queue it behind other work (for example, waiting for app foregrounding on mobile).
- Sources: https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code

### X-3. Refresh token only with `offline.access` (spelled with a dot)
- Behaviour: Without the `offline.access` scope there is no refresh token, and the access token lasts 2 hours. The docs also once write `offline_access`.
- Fake server: omit `refresh_token` unless `offline.access` is in `scope`.
- Client must: let the developer configure the provider's offline scope name. It must not inject `offline_access` blindly.
- Sources: https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code; https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/user-access-token

### X-4. Token response: `token_type` "bearer", space-separated `scope`, `expires_in` 7200
- Behaviour: Example success: `{"token_type":"bearer","expires_in":7200,"access_token":"...","refresh_token":"...","scope":"tweet.read users.read follows.read ... offline.access"}`.
- Fake server: return exactly this shape.
- Client must: compare `token_type` without regard to case.
- Sources: https://docs.x.com/fundamentals/authentication/oauth-2-0/oauth-1-0a-token-exchange

### X-5. Single-use refresh tokens (about 6 months); reuse returns `invalid_request`, not `invalid_grant`
- Behaviour: Refresh tokens are "valid for about 6 months and single-use; each refresh returns a new one". Presenting a used refresh token returns 400 `{"error":"invalid_request","error_description":"Value passed for the token was invalid."}`. RFC 6749 §5.2 says this should be `invalid_grant`. Whether reuse also revokes the newer token (family revocation) is UNVERIFIED. The official TypeScript SDK replaces its whole stored token on refresh.
- Fake server: on refresh, issue a new refresh token and invalidate the old one. On reuse, `400`, `Content-Type: application/json`, body `{"error":"invalid_request","error_description":"Value passed for the token was invalid."}`.
- Client must: treat `invalid_request` on a refresh-token grant as terminal for that refresh token (re-authenticate). It must not keep retrying, and must serialize refreshes and persist the new token first.
- Sources: https://docs.x.com/fundamentals/authentication/oauth-2-0/oauth-1-0a-token-exchange; https://github.com/openclaw/openclaw/issues/183460 (live check of reuse response); https://github.com/xdevplatform/twitter-api-typescript-sdk/issues/54; https://github.com/xdevplatform/twitter-api-typescript-sdk/blob/main/src/OAuth2User.ts

### X-6. Exact-match redirect URIs
- Behaviour: "For OAuth 2.0, you will need to have exact match validation for your callback URL." Loopback-any-port and custom schemes are not documented (UNVERIFIED).
- Fake server: byte-exact match only. A different loopback port fails.
- Client must: register and use a fixed loopback port (or a custom scheme) for X.
- Sources: https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code; https://github.com/xdevplatform/twitter-api-typescript-sdk/blob/main/src/OAuth2User.ts

### X-7. `state` up to 500 characters
- Behaviour: `state` "can be up to 500 characters".
- Fake server: reject `state` longer than 500 characters (error shape UNVERIFIED).
- Client must: keep `state` short (for example, 43 base64url characters).
- Sources: https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code

### X-8. RFC 7009-style revocation at `/2/oauth2/revoke`
- Behaviour: `POST https://api.x.com/2/oauth2/revoke` with `token` and `client_id` (public client), or with Basic auth (confidential), revokes an access or refresh token. The response body is not documented (UNVERIFIED; a `{"revoked":true}` body was not confirmed).
- Fake server: `200` with `{"revoked":true}` as a guess, marked UNVERIFIED. Accept `token_type_hint`.
- Client must: ignore the body and treat 200 as success.
- Sources: https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/user-access-token; https://github.com/xdevplatform/twitter-api-typescript-sdk/blob/main/src/OAuth2User.ts

### X-9. Documented 429 and 503 on the token endpoint
- Behaviour: For the OAuth 1.0a to 2.0 exchange on the token endpoint, X documents `429 rate_limited` and `503 temporarily_unavailable` as well as 400 `invalid_request` and `invalid_grant`. Whether refresh grants return the same 429 and 503 is UNVERIFIED, but likely since the endpoint is shared.
- Fake server: `429` with `{"error":"rate_limited"}`, and `503` with `{"error":"temporarily_unavailable"}`.
- Client must: retry with backoff, and keep the refresh token (the refresh did not happen).
- Sources: https://docs.x.com/fundamentals/authentication/oauth-2-0/oauth-1-0a-token-exchange

### X-10. No OIDC
- Behaviour: X OAuth 2.0 issues no ID token. The docs fetched mention no discovery document. The authorize URL in the official SDK is `https://twitter.com/i/oauth2/authorize`, a different host from the token endpoint `api.x.com`.
- Fake server: no discovery. Authorize and token on different hosts.
- Client must: support static configuration and different hosts per endpoint.
- Sources: https://github.com/xdevplatform/twitter-api-typescript-sdk/blob/main/src/OAuth2User.ts; https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code. That no discovery document exists is UNVERIFIED (not fetched).

### X-11. Token endpoint CORS for SPAs
- Behaviour: UNVERIFIED. X classes "Single page App" as a public client, but no source confirms that `api.x.com/2/oauth2/token` sends CORS headers. A developer report mentions a CORS block only around the authorization window: https://lightrun.com/answers/twitterdev-twitter-api-typescript-sdk-twitter-api-oauth-20---cors--postmessage

### X-12. Conflicting FAQ text on expiry
- Behaviour: The X OAuth FAQ says "Access tokens are not explicitly expired." The OAuth 2.0 PKCE page says tokens last two hours without `offline.access`. The FAQ probably refers to OAuth 1.0a tokens; that reading is UNVERIFIED.
- Client must: trust `expires_in` when it is present.
- Sources: https://docs.x.com/fundamentals/authentication/faq; https://docs.x.com/resources/fundamentals/authentication/oauth-2-0/authorization-code

## Spotify

### SPOT-1. PKCE public client: no secret, `client_id` in the body on code exchange and refresh
- Behaviour: The PKCE flow is for apps "where the client secret can't be safely stored". The token request carries `client_id` and `code_verifier`, not `client_secret`. On refresh, `client_id` is "Only required for the PKCE extension", and the `Authorization: Basic` header is "Only required for the Authorization Code" (confidential) flow. `Content-Type: application/x-www-form-urlencoded` is required.
- Fake server: `POST /api/token`. Reject with `400 {"error":"invalid_client"}` when a PKCE-registered client sends a refresh without `client_id` in the body. Reject a body that is not form-encoded.
- Client must: For a public client, send `client_id` in the form body on every token request, including refresh. Do not depend on Basic auth.
- Sources: https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow ; https://developer.spotify.com/documentation/web-api/tutorials/refreshing-tokens

### SPOT-2. Refresh response may omit `refresh_token`
- Behaviour: "When refreshing an access token, the response might not include a new refresh token." and "When a refresh token is not returned, continue using the existing token." The documented example refresh response does contain `refresh_token` and uses `"token_type": "Bearer"`.
- Fake server: Refresh returns `200 {"access_token":"...","token_type":"Bearer","expires_in":3600,"scope":"user-read-email user-read-private"}` with no `refresh_token` on some calls, and with a new one on others.
- Client must: Replace the stored refresh token only when the response contains one. Keep the old one otherwise (RFC 6749 §6).
- Sources: https://developer.spotify.com/documentation/web-api/tutorials/refreshing-tokens ; https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow

### SPOT-3. Refresh tokens rotate and are single use in the PKCE flow (community reports only)
- Behaviour: Community forum posts report that a PKCE refresh token works once and a second use returns "Refresh token revoked". The posts are not by Spotify staff. Official docs say only that a new refresh token "might" be returned. Status: UNVERIFIED by an official source.
- Fake server: After a successful refresh that returns a new refresh token, a second use of the old token returns `400 {"error":"invalid_grant","error_description":"Refresh token revoked"}`.
- Client must: Store a new refresh token atomically before using it. Serialize refreshes so that two concurrent requests do not spend the same token.
- Sources: https://community.spotify.com/t5/Spotify-for-Developers/Refresh-token-revoked/m-p/5190755/highlight/true (community, non-staff)

### SPOT-4. Absolute 6-month refresh token lifetime; refreshing does not extend it
- Behaviour: From 2026-06-18 (new apps) and 2026-07-20 (existing apps), user refresh tokens (Authorization Code and PKCE) expire 6 months after the original user authorization. "Refreshing an access token does not reset or extend the refresh token's 6-month lifetime." An expired token gets "a `400 Bad Request` response with an `invalid_grant` error".
- Fake server: Record the grant time. When `now - grant_time > T`, refresh returns `400 {"error":"invalid_grant"}` even if the refresh token was rotated recently.
- Client must: Treat `invalid_grant` on refresh as terminal: clear the session and start interactive sign-in. Do not compute refresh-token expiry from the last refresh.
- Sources: https://developer.spotify.com/blog/2026-06-18-refresh-token-expiration

### SPOT-5. Redirect URI rules: no `localhost`, loopback IP literal with any port, HTTPS otherwise
- Behaviour: For apps created after 2025-04-09 (all clients by November 2025): exact match required; HTTPS unless loopback; "`localhost` is not allowed as redirect URI"; use `http://127.0.0.1:PORT` or `http://[::1]:PORT`; register the loopback URI "without any port number" and add the dynamic port in the authorization request; dynamic ports only for loopback IP literals.
- Fake server: Registered `http://127.0.0.1/callback`. Accept `redirect_uri=http://127.0.0.1:53124/callback` (any port). Reject `http://localhost:53124/callback` and `http://example.com/callback` with an error page (no redirect).
- Client must: Use `127.0.0.1` or `[::1]`, not `localhost`, for loopback redirects (RFC 8252 §7.3, §8.3). Send the identical `redirect_uri` to the token endpoint.
- Sources: https://developer.spotify.com/documentation/web-api/concepts/redirect_uri

### SPOT-6. Implicit grant deprecated
- Behaviour: The authorization concepts page lists "Implicit grant [Deprecated]" and links a migration guide.
- Fake server: Reject `response_type=token` with `error=unsupported_response_type` at the redirect.
- Client must: Use code + PKCE only.
- Sources: https://developer.spotify.com/documentation/web-api/concepts/authorization

## Dropbox

### DBX-1. Refresh tokens only with `token_access_type=offline`
- Behaviour: "include `token_access_type=offline` as a parameter on your Authorization URL" to get a refresh token. A forum thread shows a developer who got no refresh token because the authorize URL lacked this parameter.
- Fake server: When the authorize request lacks `token_access_type=offline`, the code exchange returns `200 {"access_token":"...","expires_in":14400,"token_type":"bearer","scope":"...","account_id":"...","uid":"..."}` with no `refresh_token`.
- Client must: Allow provider-specific extra authorization parameters. Handle a code-exchange response without `refresh_token` (session ends when the access token expires).
- Sources: https://docs.dropboxapi.com/dropbox-api/docs/oauth ; https://dropbox.tech/developers/using-oauth-2-0-with-offline-access ; https://www.dropboxforum.com/t5/Dropbox-API-Support-Feedback/oauth2-token-endpoint-not-returning-refresh-token/td-p/612640

### DBX-2. Refresh response omits `refresh_token`; refresh token is reusable and non-expiring
- Behaviour: The official example refresh response is `{"access_token": "<NEW ACCESS TOKEN>", "expires_in": "<EXPIRATION>", "token_type": "bearer"}`. "The refresh token can be repeatedly re-used and doesn't expire automatically (though it can be revoked on demand)." Note `token_type` is lowercase `bearer`. The code exchange returns extra fields `account_id` and `uid`. (The `"<EXPIRATION>"` placeholder is quoted in the blog example; whether the real value is a JSON string is UNVERIFIED. A forum example shows `14400`.)
- Fake server: Refresh returns `200 {"access_token":"sl.new","expires_in":14400,"token_type":"bearer"}` with no `refresh_token` and no `scope`. The same refresh token works indefinitely until revoked.
- Client must: Keep the old refresh token when the response omits it. Keep the previously granted scope when `scope` is omitted (RFC 6749 §5.1: scope may be omitted when identical). Compare `token_type` case-insensitively. Ignore unknown fields.
- Sources: https://dropbox.tech/developers/using-oauth-2-0-with-offline-access

### DBX-3. PKCE supports `plain` as well as `S256`; secret replaced by verifier
- Behaviour: Dropbox supports `S256` and `plain` (recommends `S256`); verifier length 43–128 characters. The official Python SDK sends `client_id` plus `code_verifier` and omits `client_secret` when PKCE is used. The code flow also allows omitting `redirect_uri` (code is shown on dropbox.com for the user to copy).
- Fake server: Accept `code_challenge_method=plain`. Token exchange with `grant_type=authorization_code&code=...&client_id=...&code_verifier=...` and no secret returns tokens.
- Client must: Always send `S256`. Do not send both Basic auth and body credentials (see library item LIB-1).
- Sources: https://docs.dropboxapi.com/dropbox-api/docs/oauth ; https://github.com/dropbox/dropbox-sdk-python/blob/main/dropbox/oauth.py

### DBX-4. Invalid refresh token error
- Behaviour: Forum threads show `invalid_grant` with `error_description` "refresh token is malformed" when an authorization code or other value is sent as `refresh_token`.
- Fake server: `400 {"error":"invalid_grant","error_description":"refresh token is malformed"}`.
- Client must: Map `invalid_grant` to "re-authentication required".
- Sources: https://www.dropboxforum.com/discussions/101000042/error-invalid-grant-error-description-refresh-token-is-malformed/712728 ; https://community.dropbox.com/en/discussion/669304/refresh-token-is-malformed

### DBX-5. Revocation is not RFC 7009: bearer-authenticated RPC call
- Behaviour: Revocation uses `POST https://api.dropboxapi.com/2/auth/token/revoke` with the access token in `Authorization: Bearer`, and it revokes "an access token (and corresponding refresh token, if any)". There is no `token` form parameter as RFC 7009 defines. Whether an empty body or `null` JSON body is required is UNVERIFIED (official reference page did not render).
- Fake server: `POST /2/auth/token/revoke` with `Authorization: Bearer <access_token>` returns `200` with body `null`; the paired refresh token then fails with `invalid_grant`. A form `token=` parameter is ignored.
- Client must: Support a provider-specific revocation adapter. Revoke with a valid access token (refresh first if needed). Do not assume RFC 7009 shape is available.
- Sources: https://dropbox.tech/developers/using-oauth-2-0-with-offline-access ; https://www.dropboxforum.com/t5/API-support/Revoke-access-token/m-p/207070

## Strava

### STRAVA-1. No PKCE; `client_secret` required on code exchange and refresh
- Behaviour: The authentication docs do not mention PKCE and mark `client_secret` required for the token exchange and refresh. Strava cannot be used as a pure public client.
- Fake server: Token requests without `client_secret` return `400 {"message":"Bad Request","errors":[{"resource":"Application","field":"client_secret","code":"invalid"}]}`. (The exact error body for a missing secret is UNVERIFIED; the body shape is taken from the refresh error in item STRAVA-4.)
- Client must: Allow a configured secret or a backend token proxy for such providers. Do not send PKCE parameters if the provider rejects unknown parameters (Strava behaviour with unknown parameters is UNVERIFIED).
- Sources: https://developers.strava.com/docs/authentication/

### STRAVA-2. `expires_at` absolute timestamp alongside `expires_in`; extra `athlete` object
- Behaviour: Code exchange returns `"token_type": "Bearer"`, `"expires_at": 1568775134`, `"expires_in": 21600`, `refresh_token`, `access_token`, an `athlete` object and space-delimited `scope`. The refresh response omits `athlete` and `scope`.
- Fake server: Code exchange `200 {"token_type":"Bearer","expires_at":1568775134,"expires_in":21600,"refresh_token":"e5n...","access_token":"a4b...","athlete":{"id":1},"scope":"activity:read activity:write"}`. Refresh `200 {"token_type":"Bearer","access_token":"a9b...","expires_at":1568775134,"expires_in":20566,"refresh_token":"b5c..."}`.
- Client must: Compute expiry from `expires_in` relative to the local request time. Ignore unknown fields. Keep the previous scope when the refresh response omits `scope`.
- Sources: https://developers.strava.com/docs/authentication/

### STRAVA-3. Refresh returns the same access token if it has more than one hour left; old refresh token invalidated immediately when a new one is issued
- Behaviour: "The refresh token may or may not be the same refresh token used to make the request." "Once a new refresh token is returned, the older refresh token is invalidated immediately." Refresh returns the existing access token if it expires in more than one hour, and a new one otherwise. Access tokens last six hours. No reuse grace period is documented.
- Fake server: If `access_token_exp - now > 3600`, return the same `access_token` with a smaller `expires_in` (e.g. `20566`). When rotating, return a new refresh token and reject the old one at once with `400 {"message":"Bad Request","errors":[{"resource":"RefreshToken","field":"refresh_token","code":"invalid"}]}`.
- Client must: Accept a refresh response whose access token equals the old one. Persist the new refresh token before any other use. Serialize refreshes per account.
- Sources: https://developers.strava.com/docs/authentication/

### STRAVA-4. Non-RFC error body on the token endpoint
- Behaviour: A failed refresh returns HTTP 400 with `{"message":"Bad Request","errors":[{"resource":"RefreshToken","field":"refresh_token","code":"invalid"}]}`. There is no `error` member.
- Fake server: As above.
- Client must: Treat a 4xx token response without an RFC 6749 `error` field as an error. Classify a 400 on refresh as "re-authentication required" only by explicit mapping; do not crash on a missing `error` key.
- Sources: https://groups.google.com/g/strava-api/c/5PzF-Jf9KWU

### STRAVA-5. Callback returns the granted `scope`, which can be narrower than requested
- Behaviour: "If access is accepted, `code` and `scope` parameters will be included in the query string." "A user can opt out of the scopes requested by the application." Request scopes may be comma-delimited (`activity:read_all,activity:write`); the token response scope is space-delimited. `approval_prompt=force|auto` replaces `prompt`.
- Fake server: Redirect `GET /callback?state=xyz&code=abc&scope=read,activity:read`. Token response `scope` contains only the granted subset.
- Client must: Ignore extra callback parameters. Read the granted scope from the token response, not the request. Handle comma and space delimiters.
- Sources: https://developers.strava.com/docs/authentication/

### STRAVA-6. Mobile: app-to-app authorization URL and custom scheme callback; loopback hosts allowed
- Behaviour: iOS apps should first try `strava://oauth/mobile/authorize?...` and fall back to the web URL; the `redirect_uri` uses a registered custom scheme such as `YourApp://www.yourapp.com/en-US`. The `redirect_uri` must be within the app's registered callback domain; `localhost` and `127.0.0.1` are allowed.
- Fake server: Accept any `redirect_uri` whose host equals the registered callback domain (prefix/domain match, not exact match), including custom schemes.
- Client must: Support a provider-specific authorization endpoint override and custom-scheme redirects.
- Sources: https://developers.strava.com/docs/authentication/

### STRAVA-7. Revocation: new RFC 7009-style endpoint (2026) with confidential client auth; old deauthorize endpoint revokes everything
- Behaviour: From 2026-06-01, `POST https://www.strava.com/oauth/revoke` with HTTP Basic `client_id:client_secret`, `token` (access or refresh) and optional `token_type_hint`. "Success: HTTP 200 with an empty body." "Revocation returns 200 whether or not the token was found." Revoking either token revokes its pair. Errors: `400` (token missing), `401` (bad Authorization header), `503` ("safe to retry"). The legacy `POST /oauth/deauthorize` "will invalidate all refresh tokens and access tokens that the application has for the athlete" and is supported only until 2027-06-01.
- Fake server: Implement both endpoints with the statuses above; return `503` intermittently on `/oauth/revoke`.
- Client must: Retry revocation on `503`. Treat `200` as success even for unknown tokens. Expect that revoking the access token also kills the refresh token.
- Sources: https://developers.strava.com/docs/authentication/

## LinkedIn

### LI-1. Standard 3-legged flow requires `client_secret`; PKCE only on a separate native endpoint enabled per app
- Behaviour: `/oauth/v2/accessToken` marks `client_secret` required. Native PKCE uses `GET https://www.linkedin.com/oauth/native-pkce/authorization` with required `state`, `code_challenge`, and `code_challenge_method` "should always be: `S256`". The token request sends `client_id` and `code_verifier` without a secret. PKCE must be enabled by "your point of contact at LinkedIn". Redirects for native PKCE are loopback only: `http://127.0.0.1:{port}` or `http://[::1]:{port}`, on "a random port".
- Fake server: Two authorization endpoints. `/oauth/v2/authorization` rejects missing secret at token time with `400 invalid_request "A required parameter "client_secret" is missing"`. `/oauth/native-pkce/authorization` accepts only loopback IP redirects on any port.
- Client must: Allow a per-provider authorization endpoint distinct from discovery. Use loopback IP literals.
- Sources: https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow ; https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow-native

### LI-2. Token response without `token_type`; `refresh_token_expires_in` extra field
- Behaviour: The documented 3-legged response example is `{"access_token":"AQUv...","expires_in":5184000,"scope":"r_basicprofile"}` (no `token_type`). With programmatic refresh tokens, the response adds `refresh_token` and `refresh_token_expires_in`. Whether the live server omits `token_type` is UNVERIFIED (docs example only).
- Fake server: `200 {"access_token":"AQX...","expires_in":86400,"refresh_token":"AQW...","refresh_token_expires_in":525600,"scope":"r_basicprofile"}` with no `token_type`.
- Client must: Default a missing `token_type` to `Bearer` if the client library chooses to be lenient (RFC 6749 §5.1 makes it REQUIRED; strict libraries such as oauth4webapi reject it, see item LIB-4). Ignore or store `refresh_token_expires_in`.
- Sources: https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow ; https://learn.microsoft.com/en-us/linkedin/shared/authentication/programmatic-refresh-tokens

### LI-3. Refresh tokens only for approved partners; fixed (non-sliding) 365-day lifetime; refresh response repeats the same refresh token
- Behaviour: "Programmatic refresh tokens are available for a limited set of partners." Refresh tokens last a year; on refresh "the lifespan ... of the refresh token remains the same as specified in the initial OAuth flow (365 days)". The refresh response example returns the same `refresh_token` string with a smaller `refresh_token_expires_in` (`439200` vs `525600`). Other apps "refresh" by repeating the authorization flow, which skips consent while the member is logged in.
- Fake server: Refresh returns the identical refresh token and a decreasing `refresh_token_expires_in`. After the absolute deadline, refresh fails (see item LI-4).
- Client must: Handle a refresh response that returns the same refresh token. Not assume sliding expiry. Support silent re-authorization (interactive flow that completes without UI) when no refresh token exists.
- Sources: https://learn.microsoft.com/en-us/linkedin/shared/authentication/programmatic-refresh-tokens ; https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow

### LI-4. Expired or revoked refresh token returns `invalid_request`, not `invalid_grant`; bad code returns 401
- Behaviour: Refresh error table: `400 invalid_request "The provided authorization grant or refresh token is invalid, expired or revoked"`. Code exchange: `401 invalid_request "Unable to retrieve access token: authorization code not found"`, and `400 invalid_redirect_uri "... appid/redirect uri/code verifier does not match authorization code. Or authorization code expired ..."` (a non-RFC error code that also covers PKCE verifier mismatch and expired codes).
- Fake server: Refresh with a dead token returns `400 {"error":"invalid_request","error_description":"The provided authorization grant or refresh token is invalid, expired or revoked"}`. Code exchange with an unknown code returns `401 {"error":"invalid_request",...}`.
- Client must: Not rely only on `error == "invalid_grant"` to detect a dead refresh token. Not treat a 401 from the token endpoint as "invalid client credentials" only.
- Sources: https://learn.microsoft.com/en-us/linkedin/shared/authentication/programmatic-refresh-tokens ; https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow

### LI-5. Non-standard callback error codes; redirect URI query parameters ignored
- Behaviour: Callback errors are `user_cancelled_login` and `user_cancelled_authorize` (not RFC 6749 `access_denied`), plus `error_description` and `state`. Redirect URL query parameters are ignored when matching ("`...callback?id=1` will be `...callback`"); fragments are invalid. Authorization codes live 30 minutes. A new token with a different scope invalidates all earlier access tokens.
- Fake server: Cancel returns `302 Location: <redirect_uri>?error=user_cancelled_authorize&error_description=...&state=xyz`.
- Client must: Treat any unknown `error` value at the callback as a failure (user cancellation should be distinguishable but not required). Validate `state` before reading `error`.
- Sources: https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow

## Discord

### DISCORD-1. Callback includes RFC 9207 `iss`; discovery advertises it
- Behaviour: "Every authorization response that Discord redirects to your `redirect_uri` includes an `iss` parameter". Example: `?code=...&state=...&iss=https%3A%2F%2Fdiscord.com`. Discovery returns `"issuer": "https://discord.com"` (no trailing slash) and `"authorization_response_iss_parameter_supported": true`, `"code_challenge_methods_supported": ["S256"]`. Discovery lists no `revocation_endpoint` or `token_endpoint_auth_methods_supported` (observed 2026-10-26).
- Fake server: Append `iss=https://discord.com` to success and error redirects; serve the discovery document above.
- Client must: When metadata says `authorization_response_iss_parameter_supported: true`, require `iss` and compare it exactly with the issuer (RFC 9207 §2.4). Fall back to a configured revocation URL when discovery lacks one.
- Sources: https://docs.discord.com/developers/topics/oauth2 ; https://discord.com/.well-known/openid-configuration

### DISCORD-2. Token endpoint and revocation accept only form bodies; standard flow needs a secret; public clients via "Public Client" setting
- Behaviour: Token and revocation URLs accept only `application/x-www-form-urlencoded`; "JSON content is not permitted and will return an error." Token requests "require either HTTP Basic authentication or `client_id` and `client_secret` supplied in the form data body." The Social SDK docs say to enable "Public Client" in the Developer Portal for apps with no backend and exchange the code with the code verifier.
- Fake server: `Content-Type: application/json` body returns `400`. Revocation at `POST /api/oauth2/token/revoke` with `token` and optional `token_type_hint` revokes "any active access or refresh tokens associated with that authorization".
- Client must: Always form-encode. Expect that revoking one token kills the whole grant.
- Sources: https://docs.discord.com/developers/topics/oauth2 ; https://discord.com/developers/docs/social-sdk/authentication.html

### DISCORD-3. Response example
- Behaviour: Example `{"access_token":"6qrZ...","token_type":"Bearer","expires_in":604800,"refresh_token":"D43f...","scope":"identify"}`. Refresh rotation is not documented (UNVERIFIED either way).
- Fake server: As above; refresh returns a new refresh token.
- Client must: Replace the refresh token when a new one arrives.
- Sources: https://docs.discord.com/developers/topics/oauth2

## Twitch

### TWITCH-1. `scope` returned as a JSON array; lowercase `token_type`
- Behaviour: Authorization code and refresh responses show `"scope": ["channel:moderate","chat:edit","chat:read"]` and `"token_type": "bearer"`. RFC 6749 §3.3/§5.1 define `scope` as a space-delimited string. Auth.js rewrites the array to a space-joined string in a Twitch-specific `conform` hook. oauth4webapi throws `'"response" body "scope" property must be a string'` for non-string scope.
- Fake server: `200 {"access_token":"...","expires_in":14124,"refresh_token":"...","scope":["chat:read","chat:edit"],"token_type":"bearer"}`.
- Client must: Accept `scope` as an array or string (normalize), and compare `token_type` case-insensitively.
- Sources: https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/ ; https://dev.twitch.tv/docs/authentication/refresh-tokens/ ; https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/providers/twitch.ts ; https://github.com/panva/oauth4webapi/blob/main/src/index.ts

### TWITCH-2. Error body uses `status`/`message`; `error` holds the HTTP reason phrase; 400 or 401 for a dead refresh token
- Behaviour: Failed refresh: `400 {"error":"Bad Request","status":400,"message":"Invalid refresh token"}`. "A refresh request can fail with HTTP status code 401 Unauthorized if the refresh token is no longer valid." Auth.js maps a body without a string `error` to `invalid_request` using `message`.
- Fake server: Return either `400` with the body above, or `401` with `{"status":401,"message":"invalid refresh token"}` (the 401 body text is UNVERIFIED).
- Client must: Treat both 400 and 401 from the refresh endpoint as "refresh token dead" when the body says so. Do not treat `error: "Bad Request"` as an OAuth error code.
- Sources: https://dev.twitch.tv/docs/authentication/refresh-tokens/ ; https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/providers/twitch.ts

### TWITCH-3. Public clients: no secret, device flow only, refresh tokens expire after 30 days and are single use
- Behaviour: `client_secret` on refresh "is not required if your application's client type was set to _public_". Public client refresh tokens "will expire 30 days after they are generated" (refresh page) and the device-flow section says refresh tokens are one-time use and expire after 30 days of inactivity. Public clients can use only the device code flow. Discovery advertises `code_challenge_methods_supported: ["S256"]` and `token_endpoint_auth_methods_supported: ["client_secret_post"]` only. A password change or app disconnect invalidates refresh tokens.
- Fake server: Public client refresh returns a new refresh token; reuse of the old one returns `400 {"status":400,"message":"Invalid refresh token"}`.
- Client must: Rotate tokens atomically; handle a fixed 30-day horizon.
- Sources: https://dev.twitch.tv/docs/authentication/refresh-tokens/ ; https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/ ; https://id.twitch.tv/oauth2/.well-known/openid-configuration

### TWITCH-4. Revocation returns 400 for an invalid token (RFC 7009 §2.2 says 200)
- Behaviour: `POST https://id.twitch.tv/oauth2/revoke` with form `client_id` and `token`. Success: `200` with no body. Invalid token: `400 {"status":400,"message":"Invalid token"}`. Unknown client: `404 {"status":404,"message":"client does not exist"}`.
- Fake server: As above.
- Client must: Treat revocation `400 Invalid token` as "already revoked" during sign-out; never block local sign-out on revocation failure.
- Sources: https://dev.twitch.tv/docs/authentication/revoke-tokens/

### TWITCH-5. Discovery issuer has a path (`https://id.twitch.tv/oauth2`)
- Behaviour: Observed discovery at `https://id.twitch.tv/oauth2/.well-known/openid-configuration` returns `"issuer":"https://id.twitch.tv/oauth2"`.
- Fake server: Serve discovery under a path-bearing issuer.
- Client must: Build the discovery URL by appending `/.well-known/openid-configuration` to the full issuer including path (OIDC Discovery §4), and compare the issuer exactly.
- Sources: https://id.twitch.tv/oauth2/.well-known/openid-configuration

## Reddit

### REDDIT-1. Token errors returned with HTTP 200
- Behaviour: prawcore (the official Python Reddit API wrapper core) checks `if "error" in payload:` after a successful HTTP response, with the comment `# Why are these OKAY responses?`.
- Fake server: Code exchange with a bad code returns `200 Content-Type: application/json {"error":"invalid_grant"}`.
- Client must: Treat a 2xx token response that contains `error` or lacks `access_token` as an error.
- Sources: https://github.com/praw-dev/prawcore/blob/main/prawcore/auth.py

### REDDIT-2. Installed apps use Basic auth with an empty password; refresh tokens only with `duration=permanent`
- Behaviour: "The "password" for non-confidential clients (installed apps) is an empty string." Refresh tokens are issued only with `duration=permanent`. The documented refresh response example contains no `refresh_token`. PKCE is not mentioned. Token `scope` may use commas ("Commas are supported too"). Go's x/oauth2 comment: "Reddit only accepts client secret in the Authorization header".
- Fake server: Token endpoint requires `Authorization: Basic base64("<client_id>:")`; body credentials are rejected with `401`. Refresh returns `{"access_token":"...","token_type":"bearer","expires_in":3600,"scope":"identity"}`.
- Client must: Support Basic auth with an empty secret for public clients (do not skip the header when the secret is empty). Keep the old refresh token when omitted.
- Sources: https://github.com/reddit-archive/reddit/wiki/OAuth2 ; https://github.com/golang/oauth2/blob/master/internal/token.go

### REDDIT-3. Revocation returns 204, also for invalid tokens
- Behaviour: `POST https://www.reddit.com/api/v1/revoke_token` with `token` and optional `token_type_hint`, Basic-authenticated; "this request will return a success (204) response even if the passed in `token` was never valid."
- Fake server: Always `204 No Content`.
- Client must: Accept any 2xx as success.
- Sources: https://github.com/reddit-archive/reddit/wiki/OAuth2

## Zoom

### ZOOM-1. PKCE defaults to `plain` when method omitted; public clients must use S256; loopback ignores port
- Behaviour: Supported methods `S256` and `plain`; omitted method means `plain`; "Public clients must use S256." The PKCE token request has `grant_type`, `code`, `client_id`, `redirect_uri`, `code_verifier` with no Authorization header. Loopback redirects apply only to PKCE or native clients; Zoom ignores only the port; "Do not use localhost."
- Fake server: Authorization request with `code_challenge` but no `code_challenge_method` treats it as `plain`; the S256 verifier then fails with `400 {"reason":"Invalid request : Code verifier is invalid","error":"invalid_request"}` (the exact body text is UNVERIFIED).
- Client must: Always send `code_challenge_method=S256` explicitly (RFC 7636 §4.3 also defaults to plain).
- Sources: https://developers.zoom.us/docs/integrations/oauth/

### ZOOM-2. Extra `api_url` field; lowercase `bearer`; non-standard revocation response
- Behaviour: Example response `{"access_token":"<JWT>","token_type":"bearer","refresh_token":"<JWT>","expires_in":3600,"scope":"user:read:user","api_url":"https://api.zoom.us"}`. Revocation `POST https://zoom.us/oauth/revoke` with `token` and Basic auth returns `{"status": "success"}`.
- Fake server: As above.
- Client must: Ignore unknown fields and accept any 2xx revocation body.
- Sources: https://developers.zoom.us/docs/integrations/oauth/

### ZOOM-3. Refresh token rotation with conflicting staff statements; 90-day lifetime; no reuse tolerance
- Behaviour: Docs: each refresh returns a `refresh_token`; use the latest one; refresh tokens last 90 days. A Zoom staff member (Gianni, 2024-03-19) wrote: "A refresh token can be used to fetch a new access token and refresh token again and again until it expires after 90 days", invalidation happens on expiry, uninstall, or reauthorization, and "we don't use token tolerance mechanism for OAuth apps now". Other forum posts describe used tokens being invalidated. Developers report `invalid_grant` "Invalid Token!" on failure. Exact current rotation behaviour: UNVERIFIED (sources conflict).
- Fake server: Two variants: (a) rotate and keep the old token valid until its 90-day expiry; (b) rotate and invalidate the old token with `400 {"reason":"Invalid Token!","error":"invalid_grant"}` (JSON shape UNVERIFIED; only the code and message are reported).
- Client must: Always store the newest refresh token; on a network timeout during refresh, retry with the same old token (variant a succeeds; variant b exposes the lost-response problem).
- Sources: https://developers.zoom.us/docs/integrations/oauth/ ; https://devforum.zoom.us/t/zoom-refresh-token-invalidated-when-there-is-a-timeout-and-app-left-in-unusable-state/105680 ; https://devforum.zoom.us/t/issues-with-refresh-token-expiration-invalidation-in-account-level-oauth-app/115946

## Atlassian (Jira/Confluence Cloud 3LO)

### ATL-1. Rotating refresh tokens with 10-minute reuse interval; `invalid_grant` with HTTP 403
- Behaviour: Refresh tokens need `offline_access` in the scope. "You receive a new refresh token as well and the refresh token you used for the request is disabled." Inactivity expiry 90 days (each new token resets it). "Reuse interval (leeway)" 10 minutes: "breach detection features don't apply when exchanging a refresh token multiple times" within it. A dead token returns "`403 Forbidden` with `{"error": "invalid_grant", "error_description": "Unknown or invalid refresh token."}`" Causes include a password change, expiry, or not storing the rotated token. The authorization URL requires `audience=api.atlassian.com` and `prompt=consent`.
- Fake server: Refresh rotates. Reuse of an old token within 10 minutes returns tokens; after 10 minutes returns `403 {"error":"invalid_grant","error_description":"Unknown or invalid refresh token."}`.
- Client must: Classify `invalid_grant` by error code, not by HTTP status (403 here, 400 per RFC 6749 §5.2). Persist rotated tokens.
- Sources: https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/

### ATL-2. No PKCE for public clients in Cloud (as of 2024 forum posts)
- Behaviour: The 3LO docs require `client_secret` and do not mention PKCE. Forum user ibuchanan (2024-05-21): "Currently, Atlassian only supports the authorization code flow." Another user reports PKCE behind a flag. Feature request ECO-283 asks for PKCE. Staff status of the posters is not shown on the fetched page, and current status in 2026 is UNVERIFIED.
- Fake server: Token request without `client_secret` returns `401 {"error":"access_denied","error_description":"Unauthorized"}` (body text UNVERIFIED).
- Client must: Support a backend-held secret or proxy for such providers.
- Sources: https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/ ; https://community.developer.atlassian.com/t/oauth-2-0-with-proof-key-for-code-exchange-pkce/80173 ; https://jira.atlassian.com/browse/ECO-283

## Notion

### NOTION-1. JSON request body with Basic auth; no `token_type`/`expires_in` documented; rotating refresh; conditional `redirect_uri`
- Behaviour: Token requests use `Authorization: Basic base64(CLIENT_ID:CLIENT_SECRET)` and `Content-Type: application/json` with body `{"grant_type":"authorization_code","code":"...","redirect_uri":"..."}`. The response field table lists `access_token`, `refresh_token`, `bot_id`, `duplicated_template_id`, `owner`, `workspace_icon`, `workspace_id`, `workspace_name`; no `token_type` or `expires_in`. "Refreshing an access token will generate a new access token and a new refresh token." `redirect_uri` is required if it was in the authorization URL or if several are registered; "Otherwise, it is not allowed." No PKCE. Whether Notion also accepts form-encoded bodies is UNVERIFIED.
- Fake server: `POST /v1/oauth/token` accepts only JSON; returns `200 {"access_token":"ntn_...","refresh_token":"nrt_...","bot_id":"...","workspace_id":"...","owner":{"type":"user"}}`; rejects an unexpected `redirect_uri` with `400 {"error":"invalid_request"}`.
- Client must: Allow a per-provider request encoder. Handle a missing `expires_in` (unknown lifetime: refresh on 401). Handle a missing `token_type`.
- Sources: https://developers.notion.com/docs/authorization

## Fitbit (legacy Web API, shuts down 2026-10-30)

### FITBIT-1. Single-use refresh tokens with a two-minute idempotent window; `errors` array instead of `error`
- Behaviour: "The refresh token can only be used once, as a new refresh token is returned with the new access token." "identical refresh token requests within a two-minute period" receive the same response. Response includes `user_id`. Errors use an `errors` array with `errorType` (e.g. `expired_token`). Client-type apps pass `client_id` in the body instead of the Authorization header. requests-oauthlib ships a Fitbit compliance fix that copies `errors[0].errorType` into `error`.
- Fake server: Refresh rotates; a repeat of the same request within 120 s returns the identical body; later reuse returns `400 {"errors":[{"errorType":"invalid_grant","message":"Refresh token invalid: ..."}],"success":false}` (message text UNVERIFIED).
- Client must: Treat responses with `errors[]` as errors. Retrying a timed-out refresh within the grace window is safe.
- Sources: https://dev.fitbit.com/build/reference/web-api/authorization/refresh-token/ ; https://github.com/requests/requests-oauthlib/blob/master/requests_oauthlib/compliance_fixes/fitbit.py

## Xero

### XERO-1. Rotating refresh tokens with a 30-minute grace period; 60-day lifetime
- Behaviour: Xero's developer blog: refresh tokens last 60 days; "Once used, a refresh token expires within 30 minutes"; access tokens expire in 30 minutes.
- Fake server: Rotate on refresh; accept the previous refresh token for 30 minutes, then return `400 {"error":"invalid_grant"}`.
- Client must: Persist the new token; a retry with the old token inside the grace window is safe.
- Sources: https://devblog.xero.com/xero-api-token-management-best-practices-and-troubleshooting-01853f4244a9

## Box

### BOX-1. Single-use refresh tokens, 60-day lifetime
- Behaviour: A refresh token "can be used to obtain a new Access Token and Refresh Token only once"; "A Refresh Token is valid for 60 days"; "a new Refresh Token is returned with the new Access Token." Error body for reuse: UNVERIFIED.
- Fake server: Rotate; reject reuse with `400 {"error":"invalid_grant","error_description":"Invalid refresh token"}` (text UNVERIFIED).
- Client must: Serialize refreshes and persist atomically.
- Sources: https://developer.box.com/guides/authentication/tokens/refresh/

## Intuit (QuickBooks Online)

### INTUIT-1. `x_refresh_token_expires_in` extra field; previous refresh token valid for 24 hours after rotation
- Behaviour: Intuit's official JS SDK README: "always use the refresh token returned in the most recent token_endpoint response. Your previous refresh tokens expire 24 hours after you receive a new one." Example responses contain `"token_type":"bearer"`, `"x_refresh_token_expires_in":8726400`, and one README example shows `"x_refresh_token_expires_in": "8726400"` as a string.
- Fake server: `200 {"id_token":"...","expires_in":3600,"token_type":"bearer","x_refresh_token_expires_in":8726400,"refresh_token":"...","access_token":"..."}`; old refresh token accepted for 24 h after rotation.
- Client must: Parse numeric fields that may be strings; ignore unknown fields.
- Sources: https://github.com/intuit/oauth-jsclient/blob/master/README.md

## Withings

### WITHINGS-1. Token response wrapped in `{"status":..., "body":{...}}`; `action=requesttoken` parameter
- Behaviour: Home Assistant's Withings integration documents that "Withings API returns habitual token data under json key "body"" with a numeric `status`, and copies `body` to the root. Both code exchange and refresh send `action=requesttoken`. Whether errors use HTTP 200 with non-zero `status` is UNVERIFIED (official reference did not render).
- Fake server: `200 {"status":0,"body":{"access_token":"...","expires_in":10800,"token_type":"Bearer","scope":"user.info","refresh_token":"...","userid":"123"}}`; errors as `200 {"status":503,"error":"Invalid params"}` (UNVERIFIED).
- Client must: Fail clearly (no crash) when `access_token` is missing at the root; offer a response-transform hook.
- Sources: https://github.com/home-assistant/core/blob/dev/homeassistant/components/withings/application_credentials.py

## Ping (PingFederate/PingOne, via AppAuth issue)

### PING-1. `id_token: null` in refresh responses
- Behaviour: An AppAuth-Android issue reports a Ping refresh response `{"access_token":"...","refresh_token":"...","id_token":null,"scope":"api:read-data api:write-data","expires_in":3600,"token_type":"bearer"}`. AppAuth-Android read JSON `null` as the string `"null"` and failed with "Unable to parse ID Token".
- Fake server: Refresh response with explicit `null` members (`"id_token": null`, optionally `"scope": null`).
- Client must: Treat JSON `null` members as absent. Keep the previous ID token claims when a refresh omits `id_token` (OIDC Core §12.2 makes it optional).
- Sources: https://github.com/openid/AppAuth-Android/issues/1055

# 2. Client library workarounds

### LIB-1. golang.org/x/oauth2: `expires_in` as string, form-encoded responses, errors with HTTP 200, auth-style probing, refresh token retention, `token_type` normalization
- Behaviour: `internal/token.go`: `ExpiresIn expirationTime \`json:"expires_in"\` // at least PayPal returns string, while most return number` (parsed through `json.Number`, clamped to `MaxInt32`). Responses with `Content-Type: application/x-www-form-urlencoded` or `text/plain` are parsed as query strings ("some endpoints return a query string"). "according to spec, servers should respond status 400 in error case ... but some unorthodox servers respond 200 in error case" so any `error` member fails the request. Missing `access_token` is an error. Auth-style probing: try Basic header first, then body params, with the comment "Reddit only accepts client secret in the Authorization header; Dropbox accepts either it in URL param or Auth header, but not both; Google only accepts URL param (not spec compliant?), not Auth header; Stripe only accepts client secret in Auth header with Bearer method, not Basic". "Don't overwrite `RefreshToken` with an empty value if this was a token refreshing request." `Token.Type()` maps `bearer` (any case) to `Bearer`. Default expiry skew `defaultExpiryDelta = 10 * time.Second`.
- Fake server: Variants for each: `"expires_in":"3600"`; `Content-Type: text/plain` with `access_token=...&token_type=bearer`; `200 {"error":"invalid_grant"}`; reject requests that carry both Basic auth and body secret.
- Client must: Match each of these behaviours.
- Sources: https://github.com/golang/oauth2/blob/master/internal/token.go ; https://github.com/golang/oauth2/blob/master/token.go

### LIB-2. requests-oauthlib compliance fixes (providers special-cased)
- Behaviour: `compliance_fixes/` contains: Facebook (form-encoded body with `Content-Type: text/plain` and status 200, `expires` instead of `expires_in`, no `token_type`); Fitbit (`errors` list instead of `error`); eBay (`token_type` values "Application Access Token" / "User Access Token" rewritten to `Bearer`); Mailchimp (`scope: null` removed; `expires_in: 0` replaced by 3600); Weibo and Douban (missing `token_type`, token in query string); Plentymarkets (camelCase keys); Slack and Instagram (token sent as a `token`/`access_token` parameter on resource requests).
- Fake server: One fake per quirk: `expires_in: 0`, `scope: null`, missing `token_type`, non-standard `token_type`, camelCase keys.
- Client must: Treat `expires_in: 0` or `null` as "unknown", not "already expired" (judgement call; Mailchimp fix substitutes 3600). Treat `scope: null` as absent.
- Sources: https://github.com/requests/requests-oauthlib/tree/master/requests_oauthlib/compliance_fixes

### LIB-3. Authlib compliance hooks
- Behaviour: Authlib documents `access_token_response`, `refresh_token_response`, and `protected_request` hooks because "Some services even return with the wrong Content Type". Its example fixes Stack Overflow: non-JSON response, `expires` instead of `expires_in`, missing `token_type`.
- Fake server: Form-encoded body with `expires=86400` and no `token_type`.
- Client must: Provide a response-transform hook per provider.
- Sources: https://github.com/authlib/authlib/blob/main/docs/oauth2/client/http/index.rst

### LIB-4. oauth4webapi (strict reference behaviour)
- Behaviour: Token endpoint processing requires HTTP 200 (any other status is parsed for an OAuth error body), requires string `access_token` and `token_type`, lowercases `token_type`, accepts `expires_in` as a number or a numeric string (`parseFloat`), rejects non-string `scope`, and rejects `token_type` values other than `bearer`/`dpop` unless a custom handler is registered. A 200 response with `{"error":...}` fails as an invalid response (missing `access_token`), not as an OAuth error.
- Fake server: Use as the strict baseline: Twitch array scope, LinkedIn missing `token_type`, and Reddit 200 errors all fail it.
- Client must: Decide explicitly for each lenient case; document which quirks are tolerated.
- Sources: https://github.com/panva/oauth4webapi/blob/main/src/index.ts

### LIB-5. openid-client: Entra ID `{tenantid}` issuer and Azure AD B2C issuer mismatch
- Behaviour: In `discovery()`, when the discovered `issuer` differs from the discovery URL, openid-client accepts it only if the origin is `https://login.microsoftonline.com` (Entra) or the host ends with `.b2clogin.com` (B2C). For Entra it validates ID tokens against `server.issuer.replace('{tenantid}', tid)` using the token's `tid` claim.
- Fake server: Multi-tenant discovery at `/common/v2.0/.well-known/openid-configuration` returns `"issuer":"https://login.example/{tenantid}/v2.0"`; ID tokens carry `iss` with the real tenant and a `tid` claim. B2C-style: discovery issuer differs from the discovery host.
- Client must: Not relax issuer checks globally; apply a narrowly scoped template substitution.
- Sources: https://github.com/panva/openid-client/blob/main/src/index.ts

### LIB-6. Auth.js (NextAuth) provider overrides
- Behaviour: Twitch `conform` joins array `scope` and maps `message` errors to `invalid_request`. WeChat `conform` adds missing `token_type: "bearer"`. Microsoft Entra ID: rewrites `{tenantid}` in discovered issuer, then after the code exchange checks `responseJson.error` itself ("These providers return errors in the response body") and re-runs discovery for the `tid` from the ID token. Apple: parses the `user` form parameter (JSON string) that Apple sends only on first consent, because Apple has no userinfo endpoint. FusionAuth: strips `WWW-Authenticate` from non-401 token responses. Strava and LinkedIn: forced `token_endpoint_auth_method: "client_secret_post"`. Reddit, Atlassian, LinkedIn, LINE: `checks: ["state"]` only (no PKCE); Salesforce `checks: ["pkce","state","nonce"]`; Slack `checks: ["nonce"]`. Atlassian requires `audience: "api.atlassian.com"`. For providers without discovery, Auth.js uses a placeholder issuer `https://authjs.dev` ("TODO: review fallback issuer").
- Fake server: Use these as a list of providers needing per-provider adapters.
- Client must: Provide per-provider configuration for client authentication method, enabled checks, extra authorization parameters, and response transforms.
- Sources: https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/providers/twitch.ts ; https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/providers/wechat.ts ; https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/providers/microsoft-entra-id.ts ; https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/lib/actions/callback/oauth/callback.ts ; https://github.com/nextauthjs/next-auth/blob/main/packages/core/src/providers/fusionauth.ts ; https://github.com/nextauthjs/next-auth/tree/main/packages/core/src/providers

### LIB-7. AppAuth issues
- Behaviour: AppAuth-Android #300: Entra v2 `common` token endpoint returned no `token_type` when only an ID token was requested, which broke `TokenResponse` parsing (reporter patched it locally). AppAuth-Android #233: refresh crash `JSONException: field "token_type" not found` (fixed in 0.6.1 via #228). AppAuth-Android #1055: `"id_token": null` parsed as the string `"null"` (Ping).
- Fake server: Responses without `token_type`; responses with JSON `null` members.
- Client must: Tolerate a missing `token_type` when no access token is meant to be used, or fail with a clear error; treat `null` as absent.
- Sources: https://github.com/openid/AppAuth-Android/issues/300 ; https://github.com/openid/AppAuth-Android/issues/233 ; https://github.com/openid/AppAuth-Android/issues/1055

# 3. Spec-level edge cases

Each item below is a behaviour that a fully conformant server may exhibit, or a check that the specs require of the client. Quotes are from the cited section.

## RFC 6749 (OAuth 2.0 Framework)

### S1. Error response at the redirect URI (§4.1.2.1)
- Behaviour: The server redirects back with `error` (REQUIRED, one of `invalid_request`, `unauthorized_client`, `access_denied`, `unsupported_response_type`, `invalid_scope`, `server_error`, `temporarily_unavailable`), optional `error_description` and `error_uri`, and `state` if the client sent one. If the redirect URI or client ID is invalid, the server "MUST NOT automatically redirect the user-agent to the invalid redirection URI" (the user sees an error page and the client never gets a callback).
- Fake server: `302 Location: com.example.app:/cb?error=access_denied&error_description=User%20denied&state=xyz`. Variants: `error=temporarily_unavailable`, `error=server_error`, no `error_description`, an unknown error code such as `error=interaction_required` (OIDC) or a provider-specific code, and a "dead end" variant that never redirects.
- Client must: Check `state` before anything else, then surface `error` as a typed failure (cancel vs failure vs retryable). Treat `temporarily_unavailable` and `server_error` as retryable by the user. Accept unknown error codes. Time out or let the user cancel when no callback arrives.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.2.1

### S2. Unrecognized parameters in the callback and token response (§4.1.2, §5.1)
- Behaviour: "The client MUST ignore unrecognized response parameters" (§4.1.2) and "The client MUST ignore unrecognized value names in the response" (§5.1). The server likewise "MUST ignore unrecognized request parameters" (§3.1, §3.2).
- Fake server: Callback `?code=abc&state=xyz&session_state=...&iss=...&foo=bar`; token response with extra members such as `"expires_on":"1700000000"`, `"ext_expires_in":3599`, `"refresh_token_expires_in":1209600`, `"id_token":...`, `"user":{...}`, `"x_custom":null`.
- Client must: Not fail on extra fields or on extra fields with unexpected types. Keep them available to the app if needed.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.2 , https://www.rfc-editor.org/rfc/rfc6749.html#section-5.1 , https://www.rfc-editor.org/rfc/rfc6749.html#section-3.2

### S3. Token request requirements (§3.2, §4.1.3)
- Behaviour: The token request MUST be a POST with `application/x-www-form-urlencoded` UTF-8 body. `redirect_uri` is "REQUIRED, if the 'redirect_uri' parameter was included in the authorization request", and the values "MUST be identical". `client_id` is "REQUIRED, if the client is not authenticating with the authorization server."
- Fake server: Return `400 {"error":"invalid_grant"}` if `redirect_uri` differs by a single byte (trailing slash, case, port) from the authorization request, or `400 {"error":"invalid_request"}` if `client_id` is missing for a public client. Reject JSON bodies with `400 invalid_request`.
- Client must: Send the byte-identical `redirect_uri` it used in the authorization request, send `client_id` for public clients, and use form encoding.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-3.2 , https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.3

### S4. Authorization code is single-use and short-lived (§4.1.2, §10.5)
- Behaviour: "A maximum authorization code lifetime of 10 minutes is RECOMMENDED." "The client MUST NOT use the authorization code more than once." On a repeated exchange the server "SHOULD attempt to revoke all access tokens already granted based on the compromised authorization code".
- Fake server: First redemption `200` with tokens; second redemption with the same code `400 {"error":"invalid_grant"}` and revoke the tokens from the first redemption (later refresh gets `invalid_grant`). Also expire codes after a short time (for example 60 s) to test slow clients.
- Client must: Never retry a code exchange automatically after it may have reached the server (for example after a timeout on a non-idempotent POST) without accepting that the retry can fail and can revoke the first result. Deduplicate callback delivery (a deep link delivered twice must not redeem twice).
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.2 , https://www.rfc-editor.org/rfc/rfc6749.html#section-10.5

### S5. Successful token response shape (§5.1)
- Behaviour: `access_token` and `token_type` are REQUIRED. `expires_in` is only RECOMMENDED (may be absent). `refresh_token` is OPTIONAL. `scope` is "OPTIONAL, if identical to the scope requested by the client; otherwise, REQUIRED." `token_type` "Value is case insensitive." The body uses `application/json` and the server MUST send `Cache-Control: no-store` and `Pragma: no-cache`.
- Fake server: Variants: `{"access_token":"a","token_type":"bearer"}` (no `expires_in`, no `scope`, lowercase type); `{"access_token":"a","token_type":"Bearer","expires_in":3600,"scope":"openid"}` where granted scope is narrower than requested; `Content-Type: application/json; charset=utf-8`.
- Client must: Compare `token_type` case-insensitively. Treat missing `expires_in` as "unknown lifetime" (not as "expired" and not as "never expires" without a policy; a common approach is to refresh on a 401 from the resource server). When `scope` is absent, assume the requested scope; when present, store the granted scope and expose that the grant is narrower.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-5.1

### S6. Token error response (§5.2)
- Behaviour: Errors use HTTP 400 "unless specified otherwise", except that `invalid_client` MAY use 401 and MUST use 401 with `WWW-Authenticate` when the client authenticated with the `Authorization` header. Codes: `invalid_request`, `invalid_client`, `invalid_grant`, `unauthorized_client`, `unsupported_grant_type`, `invalid_scope`, with optional `error_description` and `error_uri`. An expired, revoked, or mismatched code or refresh token is `invalid_grant`.
- Fake server: `400 {"error":"invalid_grant","error_description":"Token is expired"}`; `401 WWW-Authenticate: Basic realm="x"` with `{"error":"invalid_client"}`.
- Client must: Treat `invalid_grant` on refresh as "the grant is gone, sign the user out or re-authenticate". Do not treat `invalid_client`, `invalid_request`, `unauthorized_client` or 5xx/429 the same way without thought: those usually indicate configuration bugs or transient failures. Parse the error body regardless of 400 vs 401.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-5.2

### S7. Refresh semantics (§6)
- Behaviour: The refresh request scope "MUST NOT include any scope not originally granted" and "if omitted is treated as equal to the scope originally granted". "The authorization server MAY issue a new refresh token, in which case the client MUST discard the old refresh token and replace it with the new refresh token." The server "MAY revoke the old refresh token after issuing a new refresh token".
- Fake server: Three modes: (a) no `refresh_token` in the refresh response; (b) new `refresh_token` every time, old one immediately invalid; (c) new `refresh_token`, old one still valid. Also a variant where the refresh response omits `scope` or returns a narrower `scope`.
- Client must: Keep the old refresh token when the response omits `refresh_token`. Replace it atomically (persist before use) when a new one arrives. Serialize concurrent refreshes so two requests never present the same rotating token. Keep the previously stored scope when the refresh response omits `scope`.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-6

### S8. Unknown token type (§7.1)
- Behaviour: "The client MUST NOT use an access token if it does not understand the token type."
- Fake server: Return `"token_type":"DPoP"` or `"token_type":"mac"` to a client that did not request DPoP.
- Client must: Reject (or surface) unknown token types instead of sending them as `Bearer`.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-7.1 , https://www.rfc-editor.org/rfc/rfc9449.html#section-5

### S9. Refresh tokens bound to the client; rotation (§10.4)
- Behaviour: The server "MUST maintain the binding between a refresh token and the client to whom it was issued" and may use "refresh token rotation in which a new refresh token is issued with every access token refresh response."
- Fake server: A refresh token issued to client A, presented by client B, returns `400 {"error":"invalid_grant"}`.
- Client must: Not share refresh tokens across client IDs (for example between a debug and a release build with different client IDs).
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-10.4

### S10. CSRF and `state` (§10.12, §4.1.2)
- Behaviour: The CSRF binding value "MUST contain a non-guessable value", and the client "SHOULD utilize the 'state' request parameter". The server returns `state` exactly as received ("The exact value received from the client"), including when it returns an error.
- Fake server: Variants: correct `state`; missing `state`; `state` from another pending request; `state` with characters that need percent-decoding (for example `+`, `%2B`, `=`); a callback that arrives with no pending request at all.
- Client must: Reject a callback whose `state` does not match a pending request, including error callbacks. Compare after a single URL decode. Handle more than one pending request or reject the second start.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-10.12 , https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.2

### S11. Scope syntax (§3.3)
- Behaviour: Scope is "a list of space-delimited, case-sensitive strings".
- Fake server: Return `"scope":"read write"` (spec), and a deviant `"scope":"read,write"` for provider emulation (see provider sections).
- Client must: Split on spaces per spec; if the library supports deviant providers, make the delimiter configurable rather than guessing.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-3.3

### S12. Client credentials encoding in HTTP Basic (§2.3.1)
- Behaviour: The client ID and secret are form-urlencoded before Base64 in HTTP Basic: "the encoded value is used as the username" and "the client password is encoded using the same algorithm". Servers "MUST support the HTTP Basic authentication scheme" for clients with a password.
- Fake server: A client secret containing `+`, `/`, `:` or `%`: accept only the form-encoded form in Basic (spec), or only the raw form (a common deviation; UNVERIFIED which named providers do this).
- Client must: For public clients, send `client_id` in the body and no Basic header. For confidential clients, encode per §2.3.1 or offer `client_secret_post`.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-2.3.1

### S13. Redirect URI fragment and registration (§3.1.2, §3.1.2.2)
- Behaviour: "The endpoint URI MUST NOT include a fragment component." Servers "MUST require the following clients to register their redirection endpoint", including public clients.
- Fake server: Reject an authorization request whose `redirect_uri` is not registered with a non-redirecting error page (see S1).
- Client must: Not put a fragment in `redirect_uri`. Report a clear configuration error when the browser never returns.
- Sources: https://www.rfc-editor.org/rfc/rfc6749.html#section-3.1.2 , https://www.rfc-editor.org/rfc/rfc6749.html#section-3.1.2.2

## RFC 7636 (PKCE)

### S14. Verifier format and S256 (§4.1, §4.2)
- Behaviour: The verifier uses unreserved characters with "a minimum length of 43 characters and a maximum length of 128 characters". "If the client is capable of using 'S256', it MUST use 'S256'." The challenge is BASE64URL without padding.
- Fake server: Reject `code_verifier` shorter than 43 or longer than 128 characters or with characters outside `[A-Za-z0-9-._~]` with `400 {"error":"invalid_request"}` (or `invalid_grant`). Reject a challenge with `=` padding.
- Client must: Generate 43 to 128 unreserved characters from a CSPRNG and send `code_challenge_method=S256`.
- Sources: https://www.rfc-editor.org/rfc/rfc7636.html#section-4.1 , https://www.rfc-editor.org/rfc/rfc7636.html#section-4.2

### S15. Default method is `plain` (§4.3)
- Behaviour: `code_challenge_method` "defaults to 'plain' if not present in the request."
- Fake server: If the client omits `code_challenge_method`, treat the challenge as plain; the S256 verifier then fails at the token endpoint with `400 {"error":"invalid_grant"}`.
- Client must: Always send `code_challenge_method=S256` explicitly.
- Sources: https://www.rfc-editor.org/rfc/rfc7636.html#section-4.3

### S16. PKCE required, missing challenge (§4.4.1) and mismatch (§4.6)
- Behaviour: A server that requires PKCE returns `invalid_request` at the authorization endpoint when `code_challenge` is missing. A verifier mismatch returns `invalid_grant` at the token endpoint.
- Fake server: Callback `?error=invalid_request&error_description=code_challenge%20required&state=...`; token endpoint `400 {"error":"invalid_grant","error_description":"PKCE verification failed"}`.
- Client must: Keep the verifier with the pending request keyed by `state` so that a second concurrent flow cannot overwrite it.
- Sources: https://www.rfc-editor.org/rfc/rfc7636.html#section-4.4.1 , https://www.rfc-editor.org/rfc/rfc7636.html#section-4.6

### S17. No downgrade to `plain` (§7.2)
- Behaviour: "Clients MUST NOT downgrade to 'plain' after trying the 'S256' method."
- Fake server: Return `?error=invalid_request&error_description=transform%20algorithm%20not%20supported` for S256.
- Client must: Fail; never retry with `plain`.
- Sources: https://www.rfc-editor.org/rfc/rfc7636.html#section-7.2

## RFC 8252 (Native apps)

### S18. Private-use URI schemes (§7.1)
- Behaviour: Apps "MUST use a URI scheme based on a domain name under their control, expressed in reverse order", and "only a single slash ('/') appears after the scheme component" (for example `com.example.app:/oauth2redirect`).
- Fake server: Accept `com.example.app:/cb` and redirect to it with `302`. A provider-emulation variant rejects schemes without a dot (see provider sections).
- Client must: Parse callbacks for both `scheme:/path` and `scheme://host/path` forms because providers and OSes differ.
- Sources: https://www.rfc-editor.org/rfc/rfc8252.html#section-7.1

### S19. Loopback redirect with any port (§7.3, §8.3)
- Behaviour: "The authorization server MUST allow any port to be specified at the time of the request for loopback IP redirect URIs". `localhost` "is NOT RECOMMENDED"; use `127.0.0.1` or `[::1]` with `http`.
- Fake server: Spec mode: registered `http://127.0.0.1/cb` matches `http://127.0.0.1:54321/cb`. Deviant modes: exact match only (port must be registered); accepts `localhost` but not `127.0.0.1`, or the reverse.
- Client must: Bind the listener before building the URL, use the actual bound port in both the authorization and token requests, and allow choosing `127.0.0.1` vs `localhost` per provider.
- Sources: https://www.rfc-editor.org/rfc/rfc8252.html#section-7.3 , https://www.rfc-editor.org/rfc/rfc8252.html#section-8.3 , https://www.rfc-editor.org/rfc/rfc9700.html#section-4.1.3

### S20. Native apps are public clients; PKCE mandatory (§6, §8.4, §8.5)
- Behaviour: "Public native app clients MUST implement the Proof Key for Code Exchange". "native apps are classified as public clients". Secrets in apps "should not be treated as confidential secrets".
- Fake server: Spec mode: accept `client_id` + `code_verifier` without a secret. Deviant mode: `400 {"error":"invalid_request","error_description":"client_secret is missing."}` (Google-style; see provider section).
- Client must: Support optional `client_secret` per provider configuration without assuming it is secret.
- Sources: https://www.rfc-editor.org/rfc/rfc8252.html#section-6 , https://www.rfc-editor.org/rfc/rfc8252.html#section-8.4 , https://www.rfc-editor.org/rfc/rfc8252.html#section-8.5

### S21. External user-agent only (§8.12)
- Behaviour: "native apps MUST NOT use embedded user-agents". Some providers block embedded web views (see Google section).
- Fake server: Return `403` or an error page when the `User-Agent` looks like a web view (provider emulation only).
- Client must: Use `ASWebAuthenticationSession`, Custom Tabs or the system browser.
- Sources: https://www.rfc-editor.org/rfc/rfc8252.html#section-8.12

## RFC 9207 (Issuer identification)

### S22. `iss` in the authorization response, including errors (§2, §2.4, §3)
- Behaviour: A supporting server includes `iss` in every authorization response, including error responses, and advertises `authorization_response_iss_parameter_supported: true`. The client compares with "simple string comparison" and "MUST reject" a mismatch. A client must reject responses without `iss` from servers that advertise support. If the metadata is absent the default is `false`. If an ID token is returned from the authorization endpoint, `iss` "MUST always be identical to the `iss` claim in the ID Token."
- Fake server: Variants: `?code=c&state=s&iss=https%3A%2F%2Fas.example` (correct); `iss` of another issuer (mix-up); `iss` missing while metadata says `true`; `iss` present while metadata omits the flag; `iss` with a trailing slash difference.
- Client must: URL-decode once and compare byte-for-byte with the discovered issuer. Reject mismatches; do not "normalize" trailing slashes.
- Sources: https://www.rfc-editor.org/rfc/rfc9207.html#section-2 , https://www.rfc-editor.org/rfc/rfc9207.html#section-2.4 , https://www.rfc-editor.org/rfc/rfc9207.html#section-3

## RFC 9700 (Security BCP)

### S23. PKCE downgrade protection on the server (§2.1.1, §4.8.2)
- Behaviour: "Authorization servers MUST support PKCE", and a token request "containing a `code_verifier` is rejected" if the authorization request had no `code_challenge`.
- Fake server: Authorization request without `code_challenge`, token request with `code_verifier`: `400 {"error":"invalid_grant"}`.
- Client must: Always send the challenge; do not send a verifier for a flow that started without one.
- Sources: https://www.rfc-editor.org/rfc/rfc9700.html#section-2.1.1 , https://www.rfc-editor.org/rfc/rfc9700.html#section-4.8.2

### S24. Mix-up defence for multi-AS clients (§2.1, §4.4.2)
- Behaviour: Clients using more than one AS must defend against mix-up; the preferred defence is the RFC 9207 `iss` parameter ("the client MUST abort the interaction" on mismatch). Distinct redirect URIs per AS are a fallback that "SHOULD therefore only be used if other options are not available".
- Fake server: Two fake ASes; AS-B returns a callback for a flow started at AS-A.
- Client must: Store the expected issuer with the pending request and check `iss` (or use per-AS redirect URIs).
- Sources: https://www.rfc-editor.org/rfc/rfc9700.html#section-2.1 , https://www.rfc-editor.org/rfc/rfc9700.html#section-4.4.2

### S25. Refresh tokens for public clients: rotation or sender-constraining (§2.2.2, §4.14)
- Behaviour: "Refresh tokens for public clients MUST be sender-constrained or use refresh token rotation". With rotation "The previous refresh token is invalidated, but information about the relationship is retained", and on replay the server "cannot determine which party submitted the invalid refresh token" so it "will revoke the active refresh token". "Refresh tokens SHOULD expire if the client has been inactive for some time." Servers "MAY revoke refresh tokens automatically in case of a security event" such as a password change or logout.
- Fake server: Rotation with family revocation: RT1 → (AT2, RT2); presenting RT1 again → `400 {"error":"invalid_grant"}` and RT2 is also revoked. Idle expiry: RT unused for N seconds → `invalid_grant`.
- Client must: Serialize refreshes across threads, processes, app extensions and browser tabs; persist the new RT before using the new AT; on `invalid_grant` after a lost response, treat the session as ended. Never "retry with the old RT" after a timeout without accepting family revocation.
- Sources: https://www.rfc-editor.org/rfc/rfc9700.html#section-2.2.2 , https://www.rfc-editor.org/rfc/rfc9700.html#section-4.14

### S26. Exact redirect URI matching, localhost port exception (§4.1.3)
- Behaviour: "the authorization server MUST ensure that the two URIs are equal"; "The only exception is native apps using a `localhost` URI" (port may vary).
- Fake server: Reject `com.example.app:/cb/` when `com.example.app:/cb` is registered.
- Client must: Use the registered string exactly.
- Sources: https://www.rfc-editor.org/rfc/rfc9700.html#section-4.1.3

### S27. Implicit grant discouraged; 303 redirects (§2.1.2, §4.12)
- Behaviour: Clients "SHOULD NOT use the implicit grant". The AS "MUST NOT use the HTTP 307 status code for redirection" when the request may contain credentials and "SHOULD use HTTP status code 303".
- Fake server: Use `303 See Other` for the callback redirect in one variant and `302` in another.
- Client must: Accept any 3xx in a loopback listener and use the code flow only.
- Sources: https://www.rfc-editor.org/rfc/rfc9700.html#section-2.1.2 , https://www.rfc-editor.org/rfc/rfc9700.html#section-4.12

### S28. CSRF via PKCE instead of `state` (§2.1, §4.7.1)
- Behaviour: Clients "MAY rely on the CSRF protection provided by PKCE", but "Clients MUST ensure that the authorization server supports PKCE" first.
- Fake server: A server that ignores `code_challenge` silently (accepts any or no verifier) and advertises no `code_challenge_methods_supported`.
- Client must: Keep using `state` unless PKCE support is confirmed; per RFC 8414 §2, a missing `code_challenge_methods_supported` means "the authorization server does not support PKCE".
- Sources: https://www.rfc-editor.org/rfc/rfc9700.html#section-4.7.1 , https://www.rfc-editor.org/rfc/rfc8414.html#section-2

## RFC 10017 (OAuth 2.0 for Browser-Based Applications, formerly draft-ietf-oauth-browser-based-apps)

### S29. Browser clients: PKCE enforced, rotation or sender-constraining, capped lifetime (§6.3.2.1, §6.3.2.3)
- Behaviour: Authorization servers "MUST support and enforce PKCE" for browser clients. They "MUST either rotate refresh tokens on each use OR use sender-constrained refresh tokens", "MUST either set a maximum lifetime on refresh tokens" (or an equivalent), and with rotation "MUST NOT extend the lifetime of the new refresh token". Browser clients make a cross-origin POST to the token endpoint, so the token endpoint must answer CORS preflights.
- Fake server: Rotating RTs whose absolute expiry is fixed from first issue (for example 24 h, like Entra SPA). Token endpoint responds to `OPTIONS` with `Access-Control-Allow-Origin: https://app.example` and a variant that omits CORS headers entirely.
- Client must: Expect forced re-authentication at a fixed time even with continuous refreshes. Report a CORS failure as a configuration error, not as sign-out (a CORS failure is indistinguishable from a network error in `fetch`).
- Sources: https://www.rfc-editor.org/info/rfc10017 , https://datatracker.ietf.org/doc/html/draft-ietf-oauth-browser-based-apps (sections 4, 6.3.2.1, 6.3.2.3 in draft -27)

## RFC 7009 (Token revocation)

### S30. Revocation request and response (§2.1, §2.2, §2.2.1, §2.3)
- Behaviour: POST form with `token` and optional `token_type_hint`; public clients send `client_id`. Revoking a refresh token SHOULD also invalidate access tokens of the same grant; revoking an access token MAY revoke the refresh token. The server returns HTTP 200 even "if the client submitted an invalid token" and "The content of the response body is ignored by the client." `unsupported_token_type` is an error. On 503 "the client must assume the token still exists and may retry after a reasonable delay", optionally with `Retry-After`. CORS on the endpoint is only MAY (§2.3).
- Fake server: `200` with empty body for unknown tokens; `400 {"error":"unsupported_token_type"}` for access tokens; `503 Retry-After: 5`; no CORS headers.
- Client must: Treat revocation as best-effort: clear local tokens regardless, retry 503 in the background, and not require a JSON body. Expect access tokens to keep working until expiry when only the access token could not be revoked.
- Sources: https://www.rfc-editor.org/rfc/rfc7009.html#section-2.1 , https://www.rfc-editor.org/rfc/rfc7009.html#section-2.2 , https://www.rfc-editor.org/rfc/rfc7009.html#section-2.2.1 , https://www.rfc-editor.org/rfc/rfc7009.html#section-2.3

## RFC 6750 and RFC 9449 (using the token)

### S31. Expired access token at the resource server (RFC 6750 §3, §3.1)
- Behaviour: Expired tokens get `401` with `WWW-Authenticate: Bearer error="invalid_token", error_description="The access token expired"`. `insufficient_scope` is `403`.
- Fake server: Resource endpoint that returns this before `expires_in` has elapsed (server-side revocation or clock skew).
- Client must: Refresh once on `401 invalid_token` even if the local clock says the token is valid; do not refresh on `403 insufficient_scope`.
- Sources: https://www.rfc-editor.org/rfc/rfc6750.html#section-3 , https://www.rfc-editor.org/rfc/rfc6750.html#section-3.1

### S32. DPoP nonce challenge and token type (RFC 9449 §5, §8)
- Behaviour: The AS may answer a token request with `400` `{"error":"use_dpop_nonce"}` and a `DPoP-Nonce` header; "the client is expected to retry its token request." A DPoP-bound response MUST carry `"token_type":"DPoP"`; for public clients "the refresh token MUST be bound to the respective public key". If the type is not `DPoP` and binding matters, "The client MUST discard the response".
- Fake server: First token request → `400 DPoP-Nonce: n1` `{"error":"use_dpop_nonce"}`; second with nonce → `200 {"token_type":"DPoP",...}`. Variant returns `Bearer` despite a DPoP proof.
- Client must: Retry once with the nonce (this is not an auth failure), keep the DPoP key for the life of the refresh token, and check `token_type`.
- Sources: https://www.rfc-editor.org/rfc/rfc9449.html#section-5 , https://www.rfc-editor.org/rfc/rfc9449.html#section-8

## OpenID Connect Core 1.0

### S33. ID token validation (§2, §3.1.3.7)
- Behaviour: `iss` "MUST exactly match"; `aud` "MAY be a single case-sensitive string" or an array that MUST contain the `client_id`; reject if it "contains additional audiences not trusted by the Client"; `azp`, if present, "MUST contain the OAuth 2.0 Client ID of this party"; `exp` must be in the future, with "some small leeway, usually no more than a few minutes, to account for clock skew"; `iat` "can be used to reject tokens that were issued too far away from the current time"; `alg` "SHOULD be the default of RS256".
- Fake server: Variants: `aud` as string; `aud` as `["client","https://api.example"]` with `azp`; `iat` 5 minutes in the future; `exp` 30 s in the past; ES256 or PS256 signature; a `kid` not in the cached JWKS (key rotation).
- Client must: Accept both `aud` shapes, apply bounded skew, refetch JWKS once on an unknown `kid`, and support the algorithms the discovery document lists.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#IDToken , https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation

### S34. Nonce (§2, §3.1.2.1, §3.1.3.7)
- Behaviour: If a nonce was sent, "a nonce Claim MUST be present" and "Clients MUST verify that the nonce Claim Value is equal to the value of the nonce parameter". Nonce values need "Sufficient entropy".
- Fake server: Omit `nonce`; return a different `nonce`; echo a nonce that the provider hashed (provider emulation, see Apple/Google sections if sourced there).
- Client must: Reject missing or mismatched nonce when one was sent.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#IDToken , https://openid.net/specs/openid-connect-core-1_0.html#AuthRequest , https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation

### S35. `at_hash` optional in the code flow (§3.1.3.6)
- Behaviour: In the code flow `at_hash` is "OPTIONAL".
- Fake server: Omit `at_hash` in one variant; include a correct one in another.
- Client must: Not require `at_hash`; validate it only when present.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#CodeIDToken

### S36. `email_verified` is a boolean (§5.1)
- Behaviour: `email_verified` is type boolean: "True if the End-User's e-mail address has been verified; otherwise false."
- Fake server: Spec `true`; deviant `"true"` (string) for provider emulation (see provider sections).
- Client must: Accept the boolean; if tolerant of providers, accept `"true"`/`"false"` strings explicitly and nothing else.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#StandardClaims

### S37. Offline access (§11)
- Behaviour: "When offline access is requested, a prompt parameter value of consent MUST be used", and the OP "MUST ignore the offline_access request unless the Client is using a response_type value" that returns a code.
- Fake server: Without `prompt=consent`, silently ignore `offline_access` and return no `refresh_token`.
- Client must: Treat "no refresh token" as a normal outcome and report it, not crash.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#OfflineAccess

### S38. ID token in refresh responses (§12.2)
- Behaviour: The refresh response "might not contain an id_token". If present, `iss`, `sub` and `aud` "MUST be the same" as the original, `iat` is new, `auth_time` represents the original authentication, and it "SHOULD NOT have a nonce Claim" (if present, the same value as the original).
- Fake server: Variants: no `id_token`; `id_token` with no `nonce`; `id_token` with the original nonce; deviant `id_token` with a different `sub` (must be rejected).
- Client must: Keep the previous ID token claims when none is returned. Not require a nonce on refreshed ID tokens. Reject a refreshed ID token whose `iss`/`sub`/`aud` changed.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#RefreshTokenResponse

### S39. Refresh error response (§12.3)
- Behaviour: "If the Refresh Request is invalid or unauthorized, the Authorization Server returns the Token Error Response" as in RFC 6749 §5.2.
- Fake server: `400 {"error":"invalid_grant"}`.
- Client must: Same as S6.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#RefreshErrorResponse

### S40. Error responses in the query for the code flow (§3.1.2.6)
- Behaviour: For the code flow "the error response parameters are added to the query component of the Redirection URI" unless another response mode is requested.
- Fake server: Default: query. Variant with `response_mode=fragment` or `form_post` (OAuth 2.0 Form Post Response Mode: the AS returns an HTML page that auto-POSTs `application/x-www-form-urlencoded` to the redirect URI).
- Client must: Read parameters from the query by default, from the fragment when requested, and accept a POST body on loopback/web callbacks when using `form_post`.
- Sources: https://openid.net/specs/openid-connect-core-1_0.html#AuthError , https://openid.net/specs/oauth-v2-form-post-response-mode-1_0.html

## Discovery (OIDC Discovery 1.0 and RFC 8414)

### S41. Discovery URL construction and trailing slash (OIDC Discovery §4, §4.1; RFC 8414 §3, §5)
- Behaviour: OIDC appends `/.well-known/openid-configuration` to the issuer, and "any terminating / MUST be removed before appending". RFC 8414 instead inserts `/.well-known/oauth-authorization-server` between host and path (`https://example.com/issuer1` → `/.well-known/oauth-authorization-server/issuer1`).
- Fake server: Issuer with a path (`https://as.example/tenant1`) and with a trailing slash (`https://as.example/`), serving only one of the two well-known locations.
- Client must: Build both forms correctly; strip one trailing slash only when building the OIDC URL, not when comparing issuers.
- Sources: https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderConfigurationRequest , https://www.rfc-editor.org/rfc/rfc8414.html#section-3 , https://www.rfc-editor.org/rfc/rfc8414.html#section-5

### S42. Issuer validation (OIDC Discovery §4.3; RFC 8414 §3.3)
- Behaviour: "The issuer value returned MUST be identical to the Issuer URL" used to build the request, and "MUST also be identical to the iss Claim value in ID Tokens". In RFC 8414, "If these values are not identical, the data contained in the response MUST NOT be used."
- Fake server: Variants: `issuer` with an extra trailing slash; `issuer` containing a `{tenantid}` placeholder (Entra-style); `issuer` of a different host.
- Client must: Compare exactly and fail loudly, with an opt-in escape hatch for documented provider deviations (multi-tenant issuer templates).
- Sources: https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderConfigurationValidation , https://www.rfc-editor.org/rfc/rfc8414.html#section-3.3

### S43. Required metadata and PKCE advertisement (OIDC Discovery §3; RFC 8414 §2)
- Behaviour: `issuer`, `authorization_endpoint`, `token_endpoint` (unless implicit-only), `jwks_uri`, `response_types_supported`, `subject_types_supported`, `id_token_signing_alg_values_supported` are required in OIDC. RFC 8414 adds `code_challenge_methods_supported` ("If omitted, the authorization server does not support PKCE") and optional `revocation_endpoint`.
- Fake server: Discovery document that omits `code_challenge_methods_supported` while supporting PKCE (a common real-world gap; provider sections note examples), and one that omits `revocation_endpoint`.
- Client must: Decide a policy: send PKCE anyway (harmless to servers that ignore unknown params, per RFC 6749 §3.1) but not rely on it for CSRF unless advertised (S28).
- Sources: https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderMetadata , https://www.rfc-editor.org/rfc/rfc8414.html#section-2

## Transport-level behaviours (not in a single RFC)

### S44. Rate limiting and transient errors on the token endpoint
- Behaviour: RFC 6749 does not define 429 or 5xx semantics for the token endpoint; RFC 7009 §2.2.1 defines 503 + `Retry-After` for revocation only, and RFC 6749 §4.1.2.1 defines `temporarily_unavailable` and `server_error` only for the authorization endpoint. Providers return 429 and 5xx in practice (see provider sections).
- Fake server: `429 Retry-After: 2` with JSON or HTML body; `502`/`503`/`504` with an HTML body; a TCP reset; a 30 s stall.
- Client must: Treat these as transient: keep the refresh token, back off, and never sign the user out. Only `invalid_grant` (and possibly provider-documented equivalents) ends the session.
- Sources: https://www.rfc-editor.org/rfc/rfc7009.html#section-2.2.1 , https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.2.1 , https://www.rfc-editor.org/rfc/rfc6749.html#section-5.2

# 4. The 20 behaviours most likely to break a naive client library

The order uses three factors: how many major providers show the behaviour, how often a client meets it in normal use, and how bad the failure is. Signing out a user who has a valid session, or never signing them in, counts as worse than a cosmetic error. Tags list the providers with a sourced item. The IDs point to the items above.

1. **Rotating refresh tokens with reuse detection, hit by concurrent or retried refreshes.** Two tabs, threads, processes or app extensions refresh at the same time. Or a refresh response is lost and the client retries with the old token. The server then sees reuse and may revoke the whole token family. The client must serialize refreshes, write the new refresh token to storage before it uses the new access token, and know each provider's grace window.
   Providers: Auth0 (AUTH0-5, no leeway by default), Okta (OKTA-6, 30 s grace), Salesforce (SF-4 no grace, SF-5), X (X-5), GitHub Apps (GH-14), Slack (SLACK-6), Strava (STRAVA-3), Keycloak (KC-6), Atlassian (ATL-1, 10 min leeway), Box (BOX-1), Twitch public clients (TWITCH-3), Cognito with rotation (COGNITO-3, up to 60 s grace), Xero (XERO-1, 30 min), Fitbit (FITBIT-1, 2 min), Zoom (ZOOM-3, conflicting reports), Spotify (SPOT-3, community reports only). Spec: S7, S25.

2. **The refresh response omits `refresh_token`.** RFC 6749 §6 lets the server leave it out, and the client must then keep the old one. A client that overwrites storage with `undefined` signs the user out at the next refresh.
   Providers: Google (GOOG-9), Apple (APPLE-7), Auth0 without rotation (AUTH0-4), Cognito without rotation (COGNITO-3), Salesforce without rotation (SF-3), Spotify (SPOT-2), Dropbox (DBX-2), Reddit (REDDIT-2). Library precedent: golang.org/x/oauth2 (LIB-1). Spec: S7.

3. **Transient failures that a client reads as "signed out".** Examples: 429 and 5xx from the token endpoint, a network error caused by CORS, and errors that use a terminal-looking code for throttling. The client must keep the refresh token and back off. Only a confirmed dead grant ends the session.
   Providers: Entra (ENTRA-14: 429 with `Retry-After`, 5xx without it, and throttling sent as `invalid_grant` AADSTS50196), Salesforce (SF-5: concurrent refresh rejected with "token request is already being processed", exact `error` code UNVERIFIED), Apple (APPLE-8: refresh more than once a day is throttled), Auth0 (AUTH0-11), Okta (OKTA-13), Slack (SLACK-10), X (X-9: 429 and 503), Meta (META-6: codes 1, 2, 4, 17), Cognito (COGNITO-14), Strava revocation 503 (STRAVA-7). Spec: S44, S30 (RFC 7009 503).

4. **Terminal refresh errors in non-standard shapes or status codes.** The client must find the "grant is dead" signal from the `error` value and a per-provider map. It must not rely on HTTP 400 plus `invalid_grant` alone.
   Providers: Auth0 403 `invalid_grant` (AUTH0-6), Atlassian 403 (ATL-1), Twitch 400 or 401 with `status`/`message` (TWITCH-2), X `invalid_request` (X-5), LinkedIn `invalid_request` (LI-4), OneLogin `invalid_request` with 401 (ONELOGIN-4), GitHub `bad_refresh_token` with HTTP 200 (GH-4), Slack `invalid_refresh_token` inside `ok:false` (SLACK-1), Strava `errors[]` with no `error` member (STRAVA-4), Meta object-valued `error` with code 190 (META-5), Fitbit `errors[]` (FITBIT-1), Entra `interaction_required` on refresh (ENTRA-12). Spec: S6, S39.

5. **Token errors returned with HTTP 200, or in a non-RFC envelope.** A client that checks only the status reports "missing access_token" or, worse, stores garbage.
   Providers: GitHub (GH-3), Slack `ok:false` (SLACK-1), Reddit (REDDIT-1), Withings `status`/`body` wrapper (WITHINGS-1, error shape UNVERIFIED). Library precedent: golang.org/x/oauth2 and Auth.js check for `error` on 200 (LIB-1, LIB-6).

6. **A "public" client still has to send a client secret, or must not send one.** Some providers require a secret even with PKCE. Others reject a secret from a public client. The client needs a per-provider, non-confidential `client_secret` option, a backend exchange option, and a clear configuration error.
   Providers that require a secret: Google Desktop and Web clients (GOOG-4), GitHub (GH-7), Apple, which needs a signed JWT (APPLE-2), Strava (STRAVA-1), LinkedIn standard flow (LI-1), Atlassian (ATL-2), Discord standard flow (DISCORD-2), Reddit, which uses Basic auth with an empty password (REDDIT-2), Salesforce when "Require Secret" is set (SF-10). Provider that rejects a secret: Entra AADSTS700025 (ENTRA-3). Spec: S20.

7. **Issuer values that fail an exact comparison.** Discovery `issuer` or ID token `iss` does not match what a naive client builds. The client must compare exactly and allow only narrow, documented exceptions.
   Providers: Entra `{tenantid}` placeholder for `common` and `organizations`, and the v1 `sts.windows.net` issuer with a trailing slash (ENTRA-1), Auth0 trailing slash (AUTH0-1), Google scheme-less `accounts.google.com` in `iss` (GOOG-2), Keycloak issuer that follows the `Host` header and `/auth` prefix (KC-1), Salesforce issuer per login host (SF-9), Okta org vs custom server (OKTA-1), OneLogin path issuer (ONELOGIN-1), Twitch path issuer (TWITCH-5). Library precedent: openid-client and Auth.js Entra workarounds (LIB-5, LIB-6). Spec: S41, S42, S33.

8. **No refresh token unless a provider-specific parameter, scope or setting is used.** Some providers give one only on first consent. The client must treat a missing refresh token as normal, and it must not inject `offline_access` blindly.
   Providers: Google first consent only, with `access_type=offline` and `prompt=consent` (GOOG-8), Entra `offline_access` (ENTRA-7), Auth0 `offline_access` plus an API setting (AUTH0-3), Okta `offline_access` (OKTA-4), X `offline.access` with a dot (X-3), Dropbox `token_access_type=offline` (DBX-1), Reddit `duration=permanent` (REDDIT-2), Salesforce `refresh_token` scope (SF-14, first-party wording UNVERIFIED), OneLogin rejects `offline_access` (ONELOGIN-2), Cognito ignores it and always issues one (COGNITO-2), Meta has no refresh tokens (META-4), LinkedIn partners only (LI-3). Spec: S37.

9. **`expires_in` missing, a string, or zero.** A client may compute `NaN` expiry, treat the token as already expired, or treat it as never expiring.
   Providers: Salesforce has no `expires_in` and sends `issued_at` in milliseconds (SF-1), GitHub OAuth apps and non-expiring GitHub App tokens (GH-13), Notion (NOTION-1), Entra v1 sends `expires_in` and `expires_on` as strings (ENTRA-8), Intuit string example (INTUIT-1), Keycloak `refresh_expires_in: 0` for offline tokens (KC-3). Library precedent: PayPal strings, Mailchimp `0` (LIB-1, LIB-2), and oauth4webapi accepts numeric strings (LIB-4). Spec: S5.

10. **Loopback redirect rules differ from RFC 8252 §7.3.** Port matching, `localhost` vs `127.0.0.1`, and `[::1]` support all differ by provider.
    Providers: Entra ignores the port only for `localhost`, has no `[::1]`, and adds `/` to a pathless URI (ENTRA-6). Spotify forbids `localhost` and allows any port on IP literals (SPOT-5). Okta (OKTA-9), X (X-6) and Auth0 (AUTH0-12, UNVERIFIED) need exact ports. Cognito allows only `http://localhost` (COGNITO-7). Apple allows no loopback at all (APPLE-3). Google (GOOG-5), GitHub (GH-9), Keycloak (KC-9), Zoom (ZOOM-1) and LinkedIn native PKCE (LI-1) allow any port. Spec: S19, S26.

11. **`scope` in an unexpected format.** Formats include commas, a JSON array, an empty string, an omitted field, percent-encoding, or `null`.
    Providers: GitHub commas, and `""` for GitHub Apps (GH-5), Slack commas (SLACK-3), Strava commas in the callback and request (STRAVA-5), Meta commas or spaces (META-8), Twitch array (TWITCH-1), OneLogin omitted (ONELOGIN-3), Dropbox and Cognito omitted on refresh (DBX-2, COGNITO-3), Entra omitted or percent-encoded in the docs (ENTRA-7, ENTRA-8), Google narrower than requested (GOOG-10). Library precedent: Mailchimp `scope: null` (LIB-2) and Ping `null` members (PING-1). Spec: S5, S11.

12. **`token_type` lowercase, missing, or not `Bearer`.** A case-sensitive check or a required-field check fails.
    Providers: lowercase `bearer` at GitHub (GH-6), Apple (APPLE-7), Dropbox (DBX-2), Twitch (TWITCH-1), Zoom (ZOOM-2), X (X-4), Intuit (INTUIT-1), Reddit (REDDIT-2), Keycloak legacy (KC-4) and Cognito implicit (COGNITO-15). Missing at LinkedIn (LI-2, docs example only) and Notion (NOTION-1), and Entra when only an ID token is requested (LIB-7). Non-Bearer: Slack `bot` and `user` (SLACK-2), and Google keeps `Bearer` even with DPoP (GOOG-13). Library precedent: eBay and WeChat fixes (LIB-2, LIB-6). Spec: S5, S8.

13. **Silent eviction of older refresh tokens by per-user limits.** A fresh sign-in on one device or test run revokes the token on another. The client must not create a new grant on every launch, and it must treat eviction as a normal `invalid_grant`.
    Providers: Google 100 per account per client (GOOG-11), Auth0 200 (AUTH0-7), GitHub 10 per user, app and scope (GH-15), Salesforce 5 approvals (SF-7, partly verified), Slack 2 active tokens with rotation (SLACK-6).

14. **Absolute or idle refresh-token lifetimes that rotation does not extend.** Active users are still forced to sign in again at a fixed time. The client must not compute expiry from the last rotation.
    Providers: Entra SPA 24 hours, fixed (ENTRA-10), Spotify 6 months from grant (SPOT-4), Auth0 rotation lifetime and idle lifetime (AUTH0-8), Cognito (COGNITO-3, COGNITO-4), LinkedIn 365 days (LI-3), Twitch public 30 days (TWITCH-3), Slack PKCE 30 days (SLACK-6), GitHub Apps 6 months (GH-13). Idle expiry: Google 6 months (GOOG-11), Okta 7 days (OKTA-7), Keycloak SSO Session Idle (KC-5), Salesforce 30 days (SF-8), Atlassian 90 days (ATL-1). Spec: S25, S29.

15. **Non-standard callback errors, and callbacks with no matching `state`.** The client must check `state` first and reject unsolicited callbacks. It must map unknown `error` values to a failure, and recognise provider-specific "cancelled" codes.
    Providers: Apple `user_cancelled_authorize` (APPLE-6), LinkedIn `user_cancelled_login` and `user_cancelled_authorize` (LI-5), Auth0 `unauthorized` (AUTH0-9), GitHub `application_suspended` (GH-11), Meta extra `error_reason` (META-7), Okta `access_denied` for unassigned users, which loops if retried (OKTA-10), Cognito error redirects without `state` (COGNITO-6, COGNITO-13), GitHub App install flow sends `code` with no `state` and no prior request (GH-12). Spec: S1, S10.

16. **Missing or incomplete discovery, and endpoints on other hosts.** A client that requires discovery, or that checks capabilities against it, fails.
    Providers: GitHub, no discovery and no ID token (GH-1), X, no OIDC (X-10, absence of discovery UNVERIFIED), Meta, implicit-only discovery with no `token_endpoint` (META-1). Entra omits `code_challenge_methods_supported` and `revocation_endpoint` but enforces PKCE (ENTRA-2). Slack under-reports (SLACK-7). Apple has no userinfo or logout (APPLE-1). Auth0 older tenants have no `end_session_endpoint` (AUTH0-2). Keycloak omits `none` (KC-13). Endpoints on hosts other than the issuer: Google (GOOG-1), Cognito (COGNITO-1), X (X-10). Spec: S43, S28.

17. **Browser token redemption blocked by CORS or `Origin` rules.** Browser clients see an opaque network error, and hybrid web views break.
    Providers: Entra requires `Origin` for `spa` redirects and rejects it for native ones (ENTRA-4, ENTRA-5). There is no CORS at GitHub (GH-8) or Apple (APPLE-12). Salesforce uses an allowlist and returns 403 otherwise (SF-12). Auth0 (AUTH0-13, partly UNVERIFIED) and Keycloak (KC-10) use allowlists. CORS is open at Google (GOOG-15), Cognito (COGNITO-8) and Entra SPA (ENTRA-5). Spec: S29.

18. **ID token claim variance.** The client must accept both forms or document a strict policy.
    Variants: `email_verified` as the string `"true"` at Apple (APPLE-9), Cognito userinfo (COGNITO-9) and Google's docs example (GOOG-14). No `at_hash` from Entra `/token` (ENTRA-15). An unsigned refresh ID token in Entra's docs (ENTRA-15, live behaviour UNVERIFIED). An `id_token` that is omitted (AUTH0-4, SF-3) or `null` (PING-1) on refresh. A refreshed ID token, which Google returns (GOOG-14), should not carry `nonce` (S38). Apple's `nonce_supported` rule (APPLE-9). Spec: S33 to S36, S38.

19. **Non-JSON token responses or requests.** Examples: a form-encoded response, a GET exchange, a JSON-only request body, or a wrapped body.
    Providers: GitHub form-encoded unless `Accept: application/json` is sent (GH-2), Meta GET exchange on a versioned path (META-2), Notion JSON request body (NOTION-1), Withings `body` wrapper (WITHINGS-1). Library precedent: Facebook `text/plain` and Stack Overflow (LIB-2, LIB-3), and golang.org/x/oauth2 parses `text/plain` (LIB-1). Spec: S3, S5.

20. **Short-lived, single-use authorization codes, and a callback delivered twice or by POST.** A client that delays redemption, redeems twice, or cannot receive a POST callback fails at sign-in.
    Providers: X 30-second codes (X-2), Entra about 1 minute (ENTRA-17), Apple 5 minutes and single use (APPLE-10), Apple `form_post` required when scopes are requested, plus a `user` JSON value that comes only once (APPLE-4, APPLE-5), Entra recommends `form_post` for `localhost` (ENTRA-17). Spec: S4, S40.

Next in line, outside the top 20:

- Revocation that is not RFC 7009. Examples: GitHub `DELETE` (GH-17), Dropbox bearer RPC (DBX-5), Slack `auth.revoke` (SLACK-9), Meta `DELETE /permissions` (META-10), no endpoint at Entra (ENTRA-18), and 400 for invalid tokens at Twitch, OneLogin and Salesforce (TWITCH-4, ONELOGIN-9, SF-13).
- RFC 9207 `iss`. Google, Keycloak 23+ and Discord advertise it (GOOG-1, KC-2, DISCORD-1), and Entra, Auth0, Okta and Apple do not (ENTRA-2, AUTH0-2, OKTA-14, APPLE-14).
- PKCE `plain` as the default when the method is omitted, at Google and Zoom (GOOG-3, ZOOM-1, S15).

# 5. Index of items with UNVERIFIED details

These items have no source for the behaviour as a whole. The fake server shapes in them are suggestions for resilience tests only:

- GOOG-17 (Google token endpoint 429 and 5xx), GH-19 (GitHub token endpoint 429 and 5xx), SLACK-11 (Slack callback error shape), X-11 (X token endpoint CORS), APPLE-14 (Apple RFC 9207 `iss`), AUTH0-12 (Auth0 loopback any port; the source thread was removed).
- SPOT-3 (Spotify single-use refresh tokens) has only non-staff community reports.
- ZOOM-3 (Zoom rotation) has conflicting staff statements.

The items below have a sourced core and at least one UNVERIFIED detail. The item text names the detail.

- GOOG-7: Callback: errors in the query, `iss` parameter, extra `scope` parameter
- GOOG-13: DPoP binds only the refresh token; `token_type` stays `Bearer`
- GOOG-14: ID token claims: `email_verified` example as string, `at_hash`, nonce
- GOOG-16: Revocation (RFC 7009-like) revokes the whole grant
- GOOG-17: Rate limiting and transient errors at the token endpoint
- ENTRA-3: PKCE required for SPA redirects; public clients must not send secrets
- ENTRA-7: Token response: `ext_expires_in`, non-standard `scope`, `refresh_token` only with `offline_access`
- ENTRA-12: `interaction_required` and `consent_required` from the token endpoint
- ENTRA-15: ID token: tenant GUID in `iss`, MSA tenant, no `at_hash` from `/token`
- APPLE-2: Token endpoint requires a `client_secret` JWT; no public-client redemption
- APPLE-8: Refresh at most once a day or be throttled
- APPLE-14: RFC 9207 `iss` callback parameter
- AUTH0-11: HTTP 429 with `x-ratelimit-*` headers
- AUTH0-12: Loopback redirect with any port (RFC 8252 §7.3) — UNVERIFIED
- AUTH0-13: CORS on `/oauth/token` — partly UNVERIFIED
- OKTA-6: Rotation with a 0–60 s grace period (default 30 s) and reuse detection
- OKTA-13: Rate limiting with `X-Rate-Limit-*` headers
- ONELOGIN-5: Credentials in both header and body are an error
- ONELOGIN-6: Refresh returns a new refresh token; rotation and reuse rules UNVERIFIED
- ONELOGIN-8: Redirect URI quirks
- ONELOGIN-11: Rate limit headers; 429 status UNVERIFIED
- KC-6: Optional rotation: "Revoke Refresh Token" and "Refresh Token Max Reuse"
- KC-8: PKCE optional by default; `plain` allowed
- COGNITO-7: Redirect URIs: HTTPS except `http://localhost`; custom schemes allowed
- COGNITO-12: Non-standard `/logout` endpoint; discovery reportedly lacks `end_session_endpoint`
- COGNITO-14: Token refresh counts against the `UserAuthentication` quota
- GH-4: Non-standard error codes at the token endpoint
- GH-7: PKCE optional, S256 only, and `client_secret` still required
- GH-11: Callback errors in the query, with `state`
- GH-18: Device flow polling errors (for comparison)
- GH-19: Token endpoint rate limiting or 5xx
- SLACK-2: Nested, non-standard success response with `token_type: "bot"`
- SLACK-4: PKCE marks the app as a public client, irreversibly; required for custom schemes
- SLACK-10: HTTP 429 with `Retry-After`
- SLACK-11: Callback error shape
- SF-1: No `expires_in`; `issued_at` in milliseconds as a string, plus `signature`
- SF-5: Concurrent identical refresh requests are rejected
- SF-8: Idle refresh-token expiry (30 days) and policy-driven expiry
- SF-9: Issuer depends on the login host (login, test, My Domain)
- SF-10: PKCE S256 only; secret optional by setting; PKCE enforceable
- SF-13: RFC 7009-style revocation with cascading effect
- SF-14: `refresh_token` scope needed to get a refresh token
- META-1: Discovery document is implicit-only, has no `token_endpoint`, and uses inconsistent hosts and trailing slashes
- META-2: Token exchange is a GET with versioned paths
- META-3: PKCE with optional secret, and `plain` allowed
- META-5: Graph-style error object, not RFC 6749 errors
- META-6: Throttling and transient codes inside the same envelope
- META-7: Callback error adds non-standard `error_reason`
- META-9: Strict redirect URI matching, with `state` as the only exception
- META-10: Revocation by deleting permissions
- META-11: Limited Login (iOS) ID token: issuer and nonce checks; short-lived
- X-2: Authorization code valid for only 30 seconds
- X-5: Single-use refresh tokens (about 6 months); reuse returns `invalid_request`, not `invalid_grant`
- X-6: Exact-match redirect URIs
- X-7: `state` up to 500 characters
- X-8: RFC 7009-style revocation at `/2/oauth2/revoke`
- X-9: Documented 429 and 503 on the token endpoint
- X-10: No OIDC
- X-11: Token endpoint CORS for SPAs
- X-12: Conflicting FAQ text on expiry
- SPOT-3: Refresh tokens rotate and are single use in the PKCE flow (community reports only)
- DBX-2: Refresh response omits `refresh_token`; refresh token is reusable and non-expiring
- DBX-5: Revocation is not RFC 7009: bearer-authenticated RPC call
- STRAVA-1: No PKCE; `client_secret` required on code exchange and refresh
- LI-2: Token response without `token_type`; `refresh_token_expires_in` extra field
- DISCORD-3: Response example
- TWITCH-2: Error body uses `status`/`message`; `error` holds the HTTP reason phrase; 400 or 401 for a dead refresh token
- ZOOM-1: PKCE defaults to `plain` when method omitted; public clients must use S256; loopback ignores port
- ZOOM-3: Refresh token rotation with conflicting staff statements; 90-day lifetime; no reuse tolerance
- ATL-2: No PKCE for public clients in Cloud (as of 2024 forum posts)
- NOTION-1: JSON request body with Basic auth; no `token_type`/`expires_in` documented; rotating refresh; conditional `redirect_uri`
- FITBIT-1: Single-use refresh tokens with a two-minute idempotent window; `errors` array instead of `error`
- BOX-1: Single-use refresh tokens, 60-day lifetime
- WITHINGS-1: Token response wrapped in `{"status":..., "body":{...}}`; `action=requesttoken` parameter
- S12: Client credentials encoding in HTTP Basic (which named providers accept only raw, non-form-encoded credentials)
