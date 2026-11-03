# A sunset-only deploy is treated as already current

**Status:** Closed
**Systems:** `scripts/deploy.mjs` (`publishStream`, `changesAgainst`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** Setting, editing or clearing `deploy.sunset` for a stream, without also changing the plan or an asset, makes `publishStream` return `current` and write nothing. `--yes` does not publish the retirement card. Retiring a stream whose payload should stay fixed is the case the sunset card is for.

**Cause.** Under the stream lock, `changesAgainst` compares the plan card and the asset cards (`scripts/deploy.mjs`). The early return fires when that list is empty and the head is admissible. `deploy.sunset` is read after that return, when `streamHead` copies it onto the envelope.

**Reproduction.** Publish a signed head with no `sunset`. Add `deploy.sunset["<channel>/<compatibilityId>"]` with a message and run deploy again. The row's action is `current`, the origin receives no write, and the head has no retirement card. Removing a sunset that is already on the head does the same.

**Fix.** Include the effective sunset card in the comparison that decides `current`, and in the changes the table prints. A sunset that differs from the locked head allocates a sequence and swaps the head.

**Acceptance.** Adding, editing and removing the sunset card each publish a new sequence whose envelope carries the new card, with the plan and assets unchanged. A second deploy with the same sunset returns `current`.

**Independent verification (Codex).** At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `/tmp/exact-review-trust-js-repro.mjs` called exported `publishStream` with a valid signed sequence-7 head, an in-memory origin, unchanged payload cards, and a newly configured sunset. It returned `action: "current"` and performed zero writes. This exercises real signature verification and publishing logic, without baking an app or running the full deploy CLI. Relevant locations: `scripts/deploy.mjs:716-738`, `1125-1128`, `1138-1139`. No matching sunset issue was found in either older issue corpus.

**Resolution.** Deployment compares the effective advisory card as part of stream changes. Adding, editing or clearing it advances the signed sequence even when payloads are identical.

**Verification after the fix.** The exact new publishStream regression passed 18 assertions: add/edit/remove advanced sequences 8/9/10, signatures verified, and repeating each card performed no writes. This focused run extracted the test without the unrelated full-deploy build fixtures.
