# Strings: MessageFormat 2 syntax (only `{name}` built, the rest refused), `lang`/`dir` from the resolved table, and the locale on `exactTime`

**Status:** Fixed: strict MF2 variable messages and escapes, CLDR table direction on every host, and exactTime.resolvedLocale; core checks and locale regressions pass, with unrelated host-suite failures recorded below.
**Systems:** Contract strings (`contract/cli/src/strings.rs`, `contract_types::strings`), plan strings, web/Apple/Linux hosts, runner time facts
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1060 (the 2026-09-27 ruling)

**Do:**
- Parse table values as MF2 simple messages. Implement `{name}` (and `{$name}` if MF2's variable spelling is adopted; say which in LLP 1060). Refuse at compile time, by name, `.match`/`.input`/`.local`, functions (`{$n :number}`), markup and any other construct, never passing them through as literal text. Keep MF2's escaping (`\{`).
- Set `lang` and `dir` from the resolved table: on the web, `document.documentElement.lang`/`dir` (and in the render server's document); on Apple, the accessibility language and the semantic content attribute; on Linux, whatever the painter's shaping and accessibility read. Right to left comes from the locale's script (CLDR likely-subtags for `ar`, `he`, `fa`, `ur`, …).
- Put the resolved table's locale on `exactTime` next to `locale`, so TypeScript sources stop copying the RFC 4647 lookup. Update the apps that copied it.
- Tests: an MF2 plural in a table is refused with its name; switching to `ar` sets `dir=rtl` on each host.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.


Implemented 2026-09-27:
- The shared parser in `plan/src/strings.rs` accepts `{name}` and MF2's
  `{$name}`, preserves MF2 text escapes, and refuses other constructs by
  name. The compiler diagnoses every table, including unused keys, with
  its file and key; plan decoding also rejects unsupported messages.
- `contract/lower/src/strings.rs` bakes ICU4X/CLDR script direction into
  `plan/tables/format.json`'s locale rows. Data-only tables retain their
  names and slot without baking unused text. No CLDR data ships in a host.
- `runner/src/runner/time.rs` exposes `resolvedLocale` alongside the viewer's
  `locale`, in the same commit as `t`. Document direction enters kernel
  inheritance transactionally and authored CSS direction takes precedence.
- Web batches and rendered HTML set `lang`/`dir`. Apple sets native content
  direction and accessibility language (including inline text); Linux
  updates its shaping catalog and invalidates the affected text caches.
- LLP 1060 records the admitted spelling and replaces its copied lookup
  example with direct table indexing. No in-repo app contains that lookup.
- Tests cover named MF2 refusals, escaping, data-only resolution, the default
  place selecting a non-base table, Arabic switching, CLDR explicit/likely
  scripts, CSS overrides, rendered HTML, native shaping, accessibility, and
  browser reload. The smoke fixture now uses the driver's actual default
  seed of 1. Two stale `set_place` test calls and a web motion inference
  error also had to be repaired to run verification.

Verification (from this worktree):
- Reproduction: `12 passed; 3 failed` before the parser/resolved-locale fix.
- Required build: passed. Required Rust tests: **1,750 passed, 0 failed,
  8 ignored** (aggregate). Required Clippy and fmt: passed.
- Caps: `All budgets within cap. 6 categories inspected.`
- Boot: `modules reachable before first pixel: 2`; `Allowed import paths only.`
- Focused final Rust host locale tests: **5 passed, 0 failed**. The full
  web/Apple/Linux/render Rust run passed every test except Apple's
  `abi::tests::fresh_preparation_reads_platform_secrets_and_defers_effects_until_commit`,
  whose platform Keychain write returned `User interaction is not allowed`.
- iOS XCTests: `Executed 53 tests, with 0 failures`, including Arabic
  native direction and accessibility language.
- macOS XCTests: `Executed 447 tests, with 1 failure`; the language test
  passed. The failure is the pre-existing
  `RasterLoaderTests.testTwentyGroupsOfDistinctReplacementsBoundSourceMaps`
  (`seen.count` 1, expected 240), already recorded in `QUEUE.md`.
- Linux smoke: `linux smoke: ok in 5.8 s`, including Arabic launch facts.
- Web smoke: `web smoke: 1 failure(s) in 57.6 s`. All app and locale
  assertions passed, including `web launch facts: defaults, repeated drive,
  overrides, translation, lang/dir, reload`. Its only failure is the known
  Chrome Keychain/password-encryption diagnostic already in `QUEUE.md`.

Nothing remains for a design ruling. The unrelated Keychain and raster-test
failures remain outside this ticket; no further fix loop was attempted for them.
