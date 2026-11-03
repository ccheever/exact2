# Outline commands bypass the polled stream's queue caps

**Status:** Closed
**Severity:** P2
**Systems:** Design tooling build channel (`scripts/tamber-builds.mjs`)
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

A polled build accumulates every outline command while waiting to read them. The queue's advertised limits are 64 frames and 128 KiB, but the overflow branch only collapses live design frames. It leaves outline frames untouched. A slow build that continues polling can retain an increasing backlog of obsolete outlines and replay them before reaching the current selection.

`openPolled` coalesces pins, holds, navigation and inspect commands, but not outlines (`scripts/tamber-builds.mjs:317-343`). Its overflow handling removes only `tamber-*` frames. `outline` sends a complete sites array on each call (`scripts/tamber-builds.mjs:635-650`), so repeated selections can exceed both queue limits.

**Verification.** `/tmp/exact-review-polled-outlines.mjs` imports the unchanged `buildChannel`, registers one polled build, and sends 100 outlines of 2,000 sites each before polling. All 100 frames remain: their serialized poll arrays total 899,334 bytes, exceeding both declared caps. The probe uses the public channel methods, with no network or app UI. A completely idle stream still expires after five seconds; that does not enforce the queue bound during the interval or for a slow stream that keeps polling.

**Fix.** Coalesce outlines to the latest state and enforce a queue limit for every command class, preserving the protocol's explicitly permitted large copies frame. A subsequent clear must supersede earlier outlines.

**Acceptance.** Repeated outline changes and clears stay within the intended bound, and the next poll presents the newest outline state without replaying obsolete selections. Existing pin and live-batch coalescing remains correct.

No matching ticket was found in this checkout or the older Exact issue directories.

## Resolution

Coalesce outlines to their latest state. Recheck ordinary queue bounds after live-frame collapse and close an overfull stream for reconnect, preserving the separate bounded inspect-copy allowance.

## Verification

`bun test scripts/tamber-builds.test.mjs` passes repeated outlines, outline clear, oversized ordinary commands and existing copy-transfer cases.
