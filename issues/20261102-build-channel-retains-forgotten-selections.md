# Forgotten builds leave their selections in the build channel

**Status:** Closed
**Severity:** P2
**Systems:** Design tooling build channel (`scripts/tamber-builds.mjs`)
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

The channel caps remembered builds at 256 and removes a previous build when its stream registers another identity. Both cleanup paths omit the `picks` map. Every identity that reports a selection leaves its plan digest, site and serialized instance path retained for the life of the dev server, even after that identity is no longer remembered.

`picks` is created at `scripts/tamber-builds.mjs:169` and populated in the selected-report handler at line 534. The replacement and eviction paths at lines 415-421 clear the other per-build maps but never delete these selections. Long-lived development sessions or repeated automated drives therefore accumulate entries outside the intended build cap.

**Verification.** `/tmp/exact-review-build-picks.mjs` invokes the unchanged `buildChannel`. During construction only, a normal `Map` subclass records the allocated maps for inspection; it preserves all map behavior. The probe registers and selects 1,000 successive identities on one stream. The public build list contains one build, while the selection map contains all 1,000 entries, including `fixture-0`. No network, real app or heap-size estimate is involved.

**Fix.** Remove selections when a build is replaced or evicted, and decide explicitly whether re-registering an existing identity invalidates its prior selection. Keep temporary disconnection behavior consistent with the other remembered per-build state.

**Acceptance.** Replacing and evicting selected builds does not grow the selection map beyond the remembered builds. Selection movement for a currently registered build still works.

No matching ticket was found in this checkout or the older Exact issue directories.

## Resolution

Delete selections when a build is replaced or evicted. Re-registration starts with no old-document selection.

## Verification

`bun test scripts/tamber-builds.test.mjs` passes 1,000 replacements, 300 disconnected registrations, bounded selections and re-registration cleanup.
