# macOS smoke rejects its specified safe area

**Status:** Closed
**Resolution:** The smoke now treats the macOS full-size viewport and titlebar safe area according to the presenter contract.
**Systems:** Tooling, Apple host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1008 §9, LLP 1012

`node scripts/smoke.mjs macos` completes every functional step, then fails two
assertions in `scripts/smoke.mjs:400-429`. The fixture assumes every non-iOS
host reports zero safe-area insets and that a cover viewport equals the app
viewport plus those insets.

LLP 1008 §9 now specifies the opposite for macOS: `viewport-fit=cover` uses a
full-size-content window and reports the titlebar as
`safe-area-inset-top`. The observed values are a 420×860 app viewport, a
420×860 cover viewport, and top=32, so the smoke both double-counts the inset
and rejects the specified value.

Fix the host-specific oracle: iOS cover expands the safe-area viewport;
macOS cover is already the full window and may have a nonzero titlebar inset.
Keep assertions that content padding resolves from the reported environment.
