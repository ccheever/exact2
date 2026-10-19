# Code review: LLP 1121 stage 11 A1, package designs in the compiler, re-check 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `331f471132`.
- **Method:** one brief (sha256 `91e3ee182b2cc58f6aedbffd697f1c8105aa75b7d570104d002169f338ccf280`), blind to the other reviewer; the commit as amended after the previous round. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING).
- **Disposition:** taken: the checker records direct dial reads (Checked.reads); a test that `rgb(…)` and `#face` read no dial so named.

One remaining **BLOCKING** finding in `331f471132`:

- **[contract/lower/src/theme/packages.rs:288](contract/lower/src/theme/packages.rs:288)** — Direct dial detection still mistakes literal text for references. An app declaring `rgb: color = #123456` under `dials`, with `brand = rgb`, makes a package’s valid `reading.card = rgb(17 34 51)` fail with `tamber-unknown-token`. The fold reads a CSS function; `words()` reports a dial reference. Likewise, a dial named `face` incorrectly rejects literal `#face`. Record direct dial resolutions during folding, separately from dial influence. The 13 tests lack these collision cases.

No other blocker found in whole-token overrides, band mapping and bounds, `SOURCES` call ordering/thread isolation, resolved token/pair restrictions, containment, package discovery, missing-file watching, or D7 retention.

The known A2 limitation remains: the [server’s fold](contract/cli/src/tamber_params.rs:109) excludes package designs, so app/param references to package tokens are refused there.

Static review only; tests were not run because they require filesystem writes. Working tree unchanged.
