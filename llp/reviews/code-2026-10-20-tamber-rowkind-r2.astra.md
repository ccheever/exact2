# Code review: LLP 1121 D3/D7, the design server's write-time row check, round 2 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `720f2f322b`.
- **Method:** one brief (sha256 `e0c8345e40c8782d66aa8b4ce125c2eb1d7e3f32b057f172d49f290f41565dbf`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a server write-path and compiler refactor, theme2's 14:19 ruling), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING, SHOULD-FIX findings).
- **Disposition:** taken ("Round 2").

Round 2: **changes needed**.

- **BLOCKING — shared `let` expressions can cause false refusals.** [rows.rs:51](contract/lower/src/theme/rows.rs:51) resolves bound identifiers into standalone literals, which strengthens validation beyond the build. Repro: a child component with `derive preferred = "ink"`, `derive chosen = (preferred != "" ? preferred : "#000000")`, and `color=chosen`. Removing `ink`, while retaining another token, still compiles; the extracted `"ink"` leaf instead produces `tamber-unknown-token`. Compiler probes confirmed both outcomes. Preserve the compiler’s expression-checking behavior.

- **SHOULD-FIX — conditional refusals point to the wrong column.** [rows.rs:55](contract/lower/src/theme/rows.rs:55) preserves the attribute span when promoting a branch to a standalone literal. Removing `ink` from `box-shadow=(flag ? "0 0 2px ink" : "none")` therefore reports the attribute’s column; the build reports the failing string’s column. In the probe: **12 versus 31**. The relocation at [rows.rs:209](contract/lower/src/theme/rows.rs:209) cannot recover that lost context. Preserve whether each occurrence was a branch.

Round-1 verification: unused-style false refusals are removed; font-dependent, shorthand and motion coverage was narrowed and documented in QUEUE. Plain duplicate-span relocation, package-path naming, and lowerer thread-local restoration check out. The base-name reach test is present, and the revised budget exercises 102 distinct rows with parsing/folding timed. The tests still omit the two expression shapes above.

No compiler behavior regression found in the binding extraction: refusal order/spans and axes → tokens → stacks builder order are preserved. Source-ID/path capture, removed-name handling, lazy-before judgment, cache-key checking, Contract-edit skipping, and the verifying-scrub hook look consistent. The protocol documentation now explains refusal and pre-existing failures.

Read-only validation: **9 existing tests passed**, including cleanup and budget tests, plus stdin compiler probes. Server integration suites were not run because they create files. No files changed.
