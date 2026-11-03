# `exact push` drops new assets and republishes deleted ones

**Status:** Closed
**Systems:** `scripts/push.mjs`
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** A push onto an existing stream builds the asset roster only from the previous head. A file added under `assets/` or `deck/` never enters the signed bundle. A file removed from the app is fetched back from the origin and published again. The update can be signed and accepted. Native clients then miss the new file and still receive the one the author deleted.

**Cause.** `carriedAssets` iterates `head.assets` and no other names (`scripts/push.mjs`). For an `assets/` or `deck/` card it reads the local file when `existsSync` is true; otherwise it `origin.get`s the head's blob. `push` installs that list as the bundle's assets and as the graph artifacts it classifies. `classifyArtifacts` checks the module interface against the cohort, not that the plan's asset names are the files in the app directory. A first publish, with no head, uses `bakedAssets` and does see the bake's rows.

**Reproduction.** Publish a stream whose head lists `assets/deleted.png` and not `assets/new.png`. Delete `assets/deleted.png`, add `assets/new.png`, and push. The outgoing roster is `assets/deleted.png`, and the bytes are the origin's old blob.

**Fix.** Build the application-owned roster from the files in the app now (`assets/` and `deck/`). Keep a head-only artifact, such as a shader the app does not store beside the plan, only where the push is documented to carry the head's copy. A deleted application file is a `removed` change.

**Acceptance.** The reproduction publishes `assets/new.png` and omits `assets/deleted.png`. A shader that exists only on the head is still carried. A push with no asset change still reports already current.

**Independent verification (Codex).** At commit `5b94c216a6fee8fca42026f5cb2e9a80063da03a`, `/tmp/exact-review-trust-js-repro.mjs` extracted and executed the actual `carriedAssets` function (`scripts/push.mjs:161-169`, following the repository's push-test approach). With only `assets/new.png` on disk and only `assets/deleted.png` on the prior head, the outgoing roster contained the deleted file alone, fetched from the origin. This proves roster construction; it does not run the complete push CLI or a native UI. Closed issues `20260904-entry-zero-mistakes-an-asset-removal.md` and `20260904-dev-loop-asset-deletion-is-a.md` concern client/dev consumption, not this publisher inventory.

**Resolution.** Push builds the app-owned asset roster from the current assets and deck directories. New files enter, deleted files leave, and retained head-only artifacts remain digest-verified.

**Verification after the fix.** A real temporary app tree regression passed additions, deletions and retained shader artifacts in the focused publisher batch.
