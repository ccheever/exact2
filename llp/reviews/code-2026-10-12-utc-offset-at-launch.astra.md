# Code review: the date at launch (time.utcOffset read 0 at boot), 2026-10-12 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `bf8b5bdc6821e74041af246b65a2ece4ed6d2449e4952aa957ba6e22cbc231ec`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The cap (set_time moved to abi/preferences.rs), the unwind leak (a Drop guard) and the remembered invalid date (only valid dates kept) fixed in round 1's commit.

---

1. **MAJOR — Required cap check fails.** [host/apple/src/abi.rs:1310](/Users/admin/x2p2/apps/exact2-d4/host/apple/src/abi.rs:1310): HEAD grows this file beyond the limit. `bun scripts/caps.mjs` exits 1, counting **1,512 lines against 1,500**. Extract some methods before landing.

2. **MAJOR — `launching` leaks its date on unwind.** [runner/src/time.rs:52](/Users/admin/x2p2/apps/exact2-d4/runner/src/time.rs:52): if `boot()` panics, restoration never executes. After `catch_unwind`, an ordinary boot on that thread inherits the failed launch’s date; catching an inner launch’s panic also corrupts the outer scope. Restore through a `Drop` guard. The test’s “outside `launching`” assertion actually calls `launching(None)`, masking this class of leak.

3. **MINOR — Rejected time overwrites the remembered valid date.** [host/apple/src/abi.rs:1311](/Users/admin/x2p2/apps/exact2-d4/host/apple/src/abi.rs:1311): `set_time` caches its input before validation. After a valid date, `set_time(NaN, 0)` leaves the live runner unchanged but poisons the cache. The next candidate filters that invalid fact to `None`, boots at zero, and can reject matching kept answers again. Preserve the last valid date when refusing input.

The normal path looks correct: construction installs the date before settlement, `exactTime` bypasses baked/carried answers, and dependent identity checks therefore see the real offset. Both bridge boot paths are covered. Normal nesting restores correctly; the frozen agent clock and drive-zone calculation are preserved. Discarding the pre-boot “not booted” batch does not change session state.

Remaining gaps are unchanged: Linux and Wasm web supply time after boot; render, direct Rust/content-region boots, and unprimed entry points still start at zero. Apple’s `seed` also remains zero until `set_place`; locale/timeZone initially retain their defaults.

**Validation:** 66 selected tests passed using existing compiled binaries, including the new test. No fresh build or Swift/UI run was performed. The added test does not directly exercise persisted-answer admission.

NOT READY
