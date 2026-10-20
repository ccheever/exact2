# Code review: LLP 1121 stage 12 B2, expressions over base names per host column, round 1 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `d05d20fbb1`.
- **Method:** one brief (sha256 `0cc4912f79bbbf7f36118133830467c22f1b044a471a769558d9c484d8d84e79`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a kernel and compiler change, theme2's 00:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** request changes (1 BLOCKING, 5 SHOULD-FIX, 1 NIT).
- **Disposition:** each finding taken in the next amend (the commit message's "Round 1").

codex
- **BLOCKING — Accepted residuals can fail on native hosts.** [contract/lower/src/scale.rs:57](contract/lower/src/scale.rs:57) conflates evaluation failure with “not a residual.” `min(space.md + 1px, …, space.md + 17px)` exceeds the kernel’s 32-value limit. With `state chosen = "pad"` and `padding=chosen`, compilation succeeds but runtime folding fails. Literal use refuses it without identifying a column. Validate residual parseability before admitting the token, and preserve column-specific errors.

- **SHOULD-FIX — Fractional range boundaries falsely reject valid values.** [params.rs:365](contract/lower/src/params.rs:365) compares widened f32 results against f64 bounds. Reproduced: `calc(space.md * 0.1)` with `range="0..1.6"` reports **“1.6px on ios, outside its range 0..1.6.”** Bounds and results need consistent precision.

- **SHOULD-FIX — Nonfinite arithmetic escapes validation.** [measure.rs:463](tamber/src/measure.rs:463) never checks finiteness after addition: `calc(space.md * 1e308 + space.md * 1e308)` produces `calc(space.md * inf)`, which cannot round-trip through either parser. Computed-row use compiles. Separately, [residual.rs:88](tamber/src/residual.rs:88) erases invalid intermediates: `calc(space.md / 0 * 0)` succeeds as `0px`, whereas the equivalent px expression is refused.

- **SHOULD-FIX — Division introduces avoidable overflow.** [measure.rs:515](tamber/src/measure.rs:515) multiplies by the reciprocal. `calc(space.md * 1e-320 / 1e-320)` is refused because the reciprocal overflows, although dividing the coefficient directly yields one.

- **SHOULD-FIX — Normalization is not idempotent after underflow.** [residual.rs:91](tamber/src/residual.rs:91) retains coefficients that become zero. Reproduced: `calc(space.md * 1e-200 * 1e-200)` prints `calc(space.md * 0)`; normalizing that output produces `0px`. Remove zero terms after scaling.

- **SHOULD-FIX — Folding loses base-read metadata.** [check.rs:1105](tamber/src/check.rs:1105) collects bases from the folded result. `calc(space.md - space.md + 3px)` therefore loses its `space.md` read; equivalent references through aliases remain discoverable through `names`. Record reads during lookup to satisfy the promised bookkeeping consistently.

- **NIT — Shadow diagnostics remain inconsistent.** [shadow.rs:262](tamber/src/shadow.rs:262) sends a bare-base alias through the rem/em refusal. An inline `calc(space.md * 2)` instead falls into a colour/type diagnostic. Both remain safely refused, but should name shadows and B3.

What checks out: ordinary signs, nested functions, constant folding/deduplication, `calc(x)` becoming an alias, kind inference, and non-length refusals. Kernel substitution preserves grouping, records reads, respects runtime base definitions, and bounds recursion. Terminal conversion uses `COLUMN`, with rounding left to layout. The inherited-row change, JS variable declarations, SVG folding, release/dev paths, and ordinary neutral-column exports look consistent. No schema/wire change or unrelated existing-app output change found.

Eight selected unit tests passed using prebuilt binaries from the matching commit; stdin probes reproduced the failures above. Tests miss these arithmetic extremes, fractional range boundaries, parser limits, and canceled reads. The conformance fixture checks integral cell spacing, not fractional rounding. Full host runs and app-output byte comparisons were not rerun. No files changed.
