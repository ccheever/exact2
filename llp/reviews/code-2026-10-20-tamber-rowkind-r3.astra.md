# Code review: LLP 1121 D3/D7, the design server's write-time row check, round 3 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `d05f9aec6c`.
- **Method:** one brief (sha256 `38cf8b90a8e87ad5abc218cc3b781f5652c7fc00e49568637ba2cee745f490f7`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a server write-path and compiler refactor, theme2's 14:19 ruling), then a narrow check of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (4 SHOULD-FIX).
- **Disposition:** taken ("Round 3").

Round 3: **changes needed — four SHOULD-FIX findings.**

- **SHOULD-FIX — SVG props can cause false refusals.** [scan.rs:135](contract/lower/src/theme/scan.rs:135) classifies attributes without their element context. `fePointLight x="pos"` enters the map as CSS `x`, but lowering renames it to the `lightX` prop. Changing `pos` from `8px` to a colour therefore fails the row check although the build accepts both versions. Compiler probes confirmed this distinction.

- **SHOULD-FIX — extracting mixed branches changes the compiler’s diagnostic.** [rows.rs:56](contract/lower/src/theme/rows.rs:56) loses the surrounding expression. For `-exact-press-scale=(flag ? "one" : other)`, with string-valued `other`, changing `one` from `1` to `0` makes the build report “computed value may name” at the expression. The checker instead reports the standalone token refusal at the literal. The new mixed-branch test covers padding, which lacks this numeric-row rule.

- **SHOULD-FIX — “passes before” is tested per leaf, not per authored row.** [rows.rs:220](contract/lower/src/theme/rows.rs:220) checks only extracted failing literals. With `padding=(flag ? "good" : "bad")`, where `bad` already names a colour, changing `good` from a length to a colour is refused despite that same row already failing before. This contradicts theme2’s Q1 rule; the existing baseline test covers only a plain literal.

- **SHOULD-FIX — mixed plain/conditional duplicates still get wrong columns.** [rows.rs:231](contract/lower/src/theme/rows.rs:231) cannot distinguish attribute-origin errors when the representative branch has identical attribute and literal spans. If conditional `box-shadow="0 0 2px ink"` is grouped first, removing `ink` relocates a matching plain occurrence’s error to its string. The build points to its attribute: **column 14 versus 25** in the probe. Existing duplicate tests use plain colours only.

The earlier fixes otherwise check out: unused styles and excluded contextual rows, bound-name substitution, isolated branch spans, package-path naming, thread-local restoration, and base-reference reach. I found no compiler regression in the binding extraction’s refusals, ordering, spans, or builder calls. Source-path capture, cache keys, removed names, lazy-before evaluation, Contract-edit skipping, and verifying-scrub wiring look consistent.

**Validation:** nine existing tests passed, including cleanup and the revised budget test, plus stdin compiler probes. File-writing server suites were not rerun. No files changed.
