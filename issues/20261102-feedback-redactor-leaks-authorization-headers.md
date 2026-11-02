# Feedback redaction leaves Bearer and Basic credentials intact

**Status:** Closed
**Severity:** P1
**Systems:** feedback, credential redaction
**Author:** Codex
**Date:** 2026-11-02

**Problem.** A diary or consented attachment containing an HTTP Authorization
header can upload the complete credential. Both the preview and send path use
the same redactor, so the promised credential filter does not protect this
common format.

**Cause.** `scripts/feedback.mjs:65` matches only the first whitespace-delimited
word after an authentication field, and requires at least eight characters.
`Bearer` and `Basic` are shorter; an opaque token following them is untouched
unless it happens to match another provider-specific pattern.

**Reproduction.** Ran the exported `redact` at revision
`5b94c216a6fee8fca42026f5cb2e9a80063da03a` with these synthetic inputs. Both
were returned unchanged:

```text
Authorization: Bearer OpaqueTestCredential_7f9c3e0d11aabbccdd
Authorization: Basic dXNlcjpwYXNzd29yZA==
```

No real credential was used or sent. `pending()` and `attachments()` feed
redacted text into `send()`, which supplies no further credential filtering.

**Fix.** Redact the credentials of authentication headers explicitly, including
case variants, quoted JSON headers, and `Proxy-Authorization`. Test the final
request body with an injected fetch function so no external service is called.

## Resolution

Redact whole Authorization and Proxy-Authorization values, including Bearer and Basic credentials in text or JSON header forms, before diary and attachment upload.

## Verification

`bun test scripts/feedback.test.mjs` passes assertions against the actual posted payload.
