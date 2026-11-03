# Bake asset verification repeatedly scans both complete asset lists

**Status:** Closed
**Severity:** P2
**Systems:** Build packaging (`scripts/app.mjs`, `verifyBakeFiles`)
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

`verifyBakeFiles` sorts the packaged and baked asset lists, then uses two linear `find` calls for every distinct asset name. Even identical lists require quadratic work. This synchronous verification runs while packaging a build, so apps with large static asset inventories pay the cost even when every file matches its receipt.

The repeated searches are in `scripts/app.mjs:855-856`. The lists are already sorted, so a merge comparison could find missing or changed cards in one pass; maps keyed by name would also avoid repeated scans.

**Verification.** `/tmp/exact-review-bake-asset-scaling.mjs` calls the actual exported function with matching receipts and distinct asset names. Three runs at 8,000 assets took 268, 267 and 315 ms; at 16,000 they took 885, 854 and 1,031 ms. These timings include sorting and comparison but no asset file reads or build. Smaller runs varied with JIT warmup; the quadratic searches are directly visible in the implementation.

**Fix.** Compare the inventories with a linear merge or indexed lookup while preserving the plan and asset digest/length checks and useful mismatch diagnostics.

**Acceptance.** Matching, changed, missing and extra assets produce the same results. Large matching inventories no longer perform a full-array search for each name.

No matching ticket was found in this checkout or the older Exact issue directories.

**Resolution.** Index both sorted inventories by asset name before comparing digest and byte count. Keep the existing first-card behavior and mismatch order, including missing and extra names.

**Verification after the fix.** The focused Bun regression checked 8,000 matching assets with fewer than 20 name reads per asset, then changed digest, changed length, missing, extra and changed-plan failures. The actual verifier took 4–11 ms for 8,000 assets and 8–16 ms for 16,000 in three local runs; no file reads or app build were included.
