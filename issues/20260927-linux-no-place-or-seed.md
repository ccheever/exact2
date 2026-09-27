# The Linux host never reports locale, time zone or launch seed, so `t()` stays on the base table and the documented `Intl` idiom throws

**Status:** Fixed: Linux reports normalized locale, IANA zone and a secure launch seed and retains them on reload; usable runner defaults and one place-plus-seed commit are verified by Contract/Linux regressions and Linux smoke (broader host-suite failures noted below).
**Systems:** Linux host (`host/linux/src/app.rs`), runner time facts (`runner/src/runner/time.rs`, `runner/src/time.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1027.000.000 (the seed and place facts), LLP 1060

`host/linux/src/app.rs:287-296` calls only `set_time`. Nothing under `host/linux/src` calls `set_place` or `set_seed` (`runner/src/runner/time.rs:50,149`), so `locale` and `timeZone` stay `""` and `seed` stays `0`.

**Failures:**
- LLP 1027.000.000 tells TypeScript to write `new Intl.DateTimeFormat(time.locale, { timeZone: time.timeZone })`. With `""`/`""` that throws `RangeError` for the life of a Linux process.
- It also throws once on Apple and the web for any `exactTime` answer computed before `set_place` lands.
- `t()` always reads the base table on Linux, so the headless parity and smoke host cannot cover localization.
- The seed is always 0 on Linux, so ids a source mixes from it repeat across launches. That defeats the seed's purpose.

**Fix:**
- On Linux, derive locale from `LC_ALL`/`LC_MESSAGES`/`LANG` (normalize `en_US.UTF-8` to `en-US`) and the zone from the tz database. Draw a seed from `getrandom`.
- Give the pre-report values usable defaults (`en-US`/`UTC`), or document that sources must guard the empty strings.
- Set both facts in one runner call so launch commits once, not twice (the web and Apple currently commit twice, `host/web/src/host.rs:742-759`).

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Grok 4.7 xhigh, Astra max (design). Verification: confirmed by reading; `set_place`/`set_seed` have no caller on Linux.

Implementation and verification (2026-09-27, Astra):

- `host/linux/src/zone.rs` reads `LC_ALL`/`LC_MESSAGES`/`LANG`, normalizes
  POSIX locales, resolves `TZ`/the system IANA zone, and uses `getrandom`.
  `app.rs` reports place and seed together; the presenter restores its launch
  facts to replacement runners. Agent mode uses the drive's explicit facts.
- `runner/src/time.rs` defaults to `en-US`/`UTC`; `Runner::set_place` takes
  the optional seed, validates the whole fact and commits once. Web and Apple
  use that call. The obsolete separate seed setter is removed.
- Reproduced empty defaults in `contract/cli/tests/it/time.rs` (2 failures
  before the fix); its 3 tests now pass, including atomic commit and refusal.
  Added a regional-table regression in `strings.rs` for an unchanged default
  place; it failed before the equality fix and now passes.
- Linux zone tests: `3 passed; 0 failed`. Reload regression:
  `1 passed; 0 failed`. `linux smoke: ok in 94.1 s`, including fixed defaults,
  repeated drives, override facts and the translated `t()` table.
- Root build, test, Clippy and fmt pass: `1715 passed; 0 failed; 8 ignored`
  across 71 test binaries. Host Clippy also passes. Caps and boot checked
  with the final staged change.
- Web launch-facts assertions pass, but the full web smoke reports Chrome
  keychain/encryption and paint-metrics diagnostics. macOS XCTest ran twice:
  `Executed 443 tests, with 1 failure`; the unchanged raster test
  `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
  fails at line 157 (`seen.count` 1, expected 240). These are outside these
  tickets; no design ruling remains for the launch-facts fixes.
- Final web rerun: `web smoke: 1 failure(s) in 43.1 s`; all functional
  assertions, including launch facts, pass. Only Chrome's macOS keychain
  (`errSecInteractionNotAllowed`) and unavailable password encryption remain.
  Stopped after the third web attempt (first: missing offline dependency;
  fetched the locked dependency, then two completed functional sweeps).
