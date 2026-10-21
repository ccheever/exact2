# Code review: Stage 11 C2: the terminal held to its column (exact-terminal layout and plan), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d7b83957df`.
- **Method:** one brief (sha256 `ec68f0fd4097918f4f96c93bf31d127aec5dcafe38062a68895ff5e91c2301cb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; three P2s (a step's text split; percentage radii reported 0; per-node cells lost from the report), one P3 (missing cells or radii passing)
- **Disposition:** all taken in 6b9d8def04: terminalVerb keeps a text tail whole; corners sized as hosts size them (at_basis across and down); every row carries its box and cells; an even box or a corner the reply leaves unmeasured fails; QUEUE reworded

Reviewed HEAD `d7b83957df`. No P0/P1 findings. I found these issues:

1. **P2 — Terminal steps lose text containing spaces.** [scale-check.mjs:322](/tmp/rv-s11c2t-1/scripts/scale-check.mjs:322) splits every step into whitespace-separated arguments. `type field hello world` types only `hello`, then attempts operation `world`; quoting does not fix this split. Preserve the text tail as one argument. The current tap-only fixture avoids this bug.

2. **P2 — Percentage radii are reported as zero.** [cli.rs:176](/tmp/rv-s11c2t-1/host/terminal/src/cli.rs:176) treats every non-`Points` result as zero, but `Dimension::resolve()` leaves percentages and percentage-bearing calculations unresolved. An admitted `border-radius="50%"` therefore becomes indistinguishable from an explicit square corner. Resolve against the box or report that the value is unsupported. The painter has the same existing limitation, but that does not make zero an accurate *resolved kernel radius*. The scale’s zero-valued terminal radii are unaffected.

3. **P2 — Per-node cells disappear from scale-check’s published output.** [scale-check.mjs:384](/tmp/rv-s11c2t-1/scripts/scale-check.mjs:384) discards the layout after producing assertion rows. Both output formats retain only derived padding margins; header, gap and corner nodes lose their cell rectangles entirely. Requirement (a), “each node’s cells in the run output,” is met by the subprocess’s `layout` reply, but not by the scale-check report. Preserve the per-phase layout or include those rectangles in its output.

4. **P3 — Missing terminal measurements can pass.** At [scale-check.mjs:249](/tmp/rv-s11c2t-1/scripts/scale-check.mjs:249), an `even` assertion silently skips its cell check if either rectangle is absent. At [scale-check.mjs:287](/tmp/rv-s11c2t-1/scripts/scale-check.mjs:287), `radius: []`, `[0]`, or four `null`s passes an expected-zero check. Require integer cell rectangles and exactly four finite numeric radii. The new tests do not exercise these cases.

The remaining checks look sound for this fixture. Traversal preserves root/child order and skips hidden subtrees; it matches `tree()`’s box traversal, although `tree()` additionally lists named inline runs. String escaping handles controls, quotes, backslashes and Unicode correctly. Finite float formatting, exponent notation and negative zero are valid JSON. Nonfinite formatting would be invalid, but I found no ordinary admitted-input path demonstrating that failure.

The digest check uses the actual compiled `.plan`, whose canonical re-encoding is what the runner hashes. Separate `.contract` compilation can use different terminal-profile lowering, but scale-check does not take that path. Width-based conversion is correct for today’s `ch` semantics on both axes. The fixed terminal viewport is 512×1024 px and accommodates this fixture; repeated `cargo run` adds overhead, but I did not measure it. The final appended `layout` also avoids an earlier `"nodes"` line winning in this drive.

The intentional **20/21 result is not itself a blocking defect**: the assertion exposes the unresolved vertical-spacing decision. Keep it visible until theme2 accepts a specific exception or changes the policy. QUEUE’s fractions are correct, though “every half-row space drifts so” overstates matters—edge asymmetry also depends on the child’s height and position.

The CLI test checks substrings and digest shape, not JSON parsing or digest equality; those remain coverage gaps. No files changed, and no builds or tests ran.
