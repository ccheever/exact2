# A JSON null diary request throws instead of returning a validation error

**Status:** Closed
**Severity:** P3
**Systems:** feedback worker, request validation
**Author:** Codex
**Date:** 2026-11-02

**Problem.** `POST /diary` with the valid JSON body `null` throws an uncaught
TypeError. Malformed client data should receive a 400 response.

**Cause.** `scripts/feedback-worker.js:75` reads `body.attachments` after parsing
JSON without checking that `body` is a non-null object.

**Reproduction.** Invoked the exported worker locally with
`new Request('https://example.invalid/diary', { method: 'POST', body: 'null' })`.
It rejected with `TypeError: null is not an object (evaluating
'body.attachments')` before accessing any environment binding.

**Fix.** Validate the top-level object before reading fields. Cover null,
arrays, primitives, malformed JSON, and a valid diary in the request tests.

## Resolution

Require a non-null JSON object before reading diary fields. Arrays and JSON primitives receive HTTP 400.

## Verification

`bun test scripts/feedback.test.mjs` passes null, array and primitive requests along with valid diary submission.
