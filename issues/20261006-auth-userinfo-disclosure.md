# Redact URL credentials from auth summaries and grant refusals

**Status:** Open
**Systems:** Runner auth, Auth agent substitute, Native and web hosts
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** LLP 1069.006 D2 and D7, runner/src/auth.rs

The auth URL parser retains the entire authority, including userinfo. Both `Session::summary` and `Url::origin` render it as `scheme://authority`. The first exposes it in the supposedly filtered pending summary; the second exposes it in grant-refusal text.

Verified through `Session::from_json`, `summary` and `check_grants`:
- `https://user:review-secret@example.com/authorize?code=hidden` is accepted as a session body.
- The summary contains `"origin":"https://user:review-secret@example.com"`.
- With the ordinary `auth.session https://example.com` grant, the refusal contains that same credential-bearing URL.
- `https://user:review-secret@login.example.com/authorize` is admitted by `auth.session https://*.example.com`, so this also reaches the held-session path.

The query filtering correctly hides `code`, but credentials in the authority bypass that filtering. Refusing a request must not disclose those bytes either.

Reject userinfo at the auth request boundary, or normalize an origin that excludes it before matching and displaying. Ensure rejection diagnostics are themselves redacted. Apply the same policy on every host, through the shared module.

Acceptance: malformed or credential-bearing authorization URLs cannot expose username/password bytes through pending state, refusals or logs. Exact-origin and wildcard matching use the normalized host/origin, and existing accepted auth flows and callback redaction still pass.
