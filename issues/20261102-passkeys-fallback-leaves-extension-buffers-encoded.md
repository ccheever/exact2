# WebAuthn fallback leaves extension buffers encoded as strings

**Status:** Closed
**Severity:** P2
**Systems:** packages/passkeys, web, WebAuthn
**Author:** Codex
**Date:** 2026-11-02

At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `packages/passkeys/web/index.js:42-59` falls back when the browser lacks `parseCreationOptionsFromJSON` or `parseRequestOptionsFromJSON`. It decodes challenge, user ID and credential IDs, but leaves extension buffer values as base64url strings.

Requests using `extensions.prf.eval.first`, its optional second salt, `evalByCredential`, or `largeBlob.write` therefore fail TypeError before an authenticator runs. [WebAuthn's JSON conversion algorithm](https://www.w3.org/TR/webauthn-3/#sctn-parseRequestOptionsFromJSON) includes extension inputs; PRF salts and a large-blob write must become buffer sources.

`bun /tmp/exact-review-passkeys-browser.mjs` served the actual package locally and ran the pinned Chromium. Its native parser produced an ArrayBuffer for `prf.eval.first`. After disabling that parser to exercise the fallback, the actual package's `get` returned TypeError: first was not `(ArrayBuffer or ArrayBufferView)`. `/tmp/exact-review-web-repro.mjs` also confirmed PRF and largeBlob values remained strings. No app build, account access, or authenticator action was performed.

Decode all specified extension buffers in both fallback paths, including each per-credential PRF salt. Cover those paths with native JSON parser methods absent.

No duplicate found in the available exact2/exact issue corpora, including closed tickets.

**Resolution.** Decode PRF evaluation salts, per-credential salts, largeBlob.write and credBlob in the fallback JSON conversion for creation and request options, without mutating the caller’s objects.

**Verification after the fix.** The fallback Bun regression passed creation and request paths with native JSON parsers absent, asserting decoded bytes and unchanged input.
