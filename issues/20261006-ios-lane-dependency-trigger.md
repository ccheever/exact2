# Trigger iOS verification for changes to shared runtime dependencies

**Status:** Open
**Systems:** async lane, UIKit, shared runtime
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** AGENTS.md app-runtime verification; scripts/async.mjs

`scripts/async.mjs:70` runs iOS XCTests and real-touch smoke only when the commit modifies `host/apple`. A change confined to kernel layout, runner transactions, plan/schema interpretation, motion, shared text code or an app can change UIKit behavior without running either iOS check.

The current `checks` function gates both `ios` and `ios-touch` on that one path. Root Cargo builds the Apple Rust library on this Mac; it does not prove the iOS archive/presenter or real gestures work. This makes common shared-runtime regressions wait until an unrelated Apple-file change to be detected.

Broaden the existing async trigger using the Apple host's real build inputs or a conservative dependency path set, and include relevant app changes. Keep this in the existing async lane; do not create a sixth blocking check.

Acceptance: representative commits confined to kernel, runner, a shared text/motion dependency and a touched iOS app schedule appropriate iOS build/tests and smoke. A docs-only commit can still skip them. The selector's coverage and exclusions are visible and tested.
