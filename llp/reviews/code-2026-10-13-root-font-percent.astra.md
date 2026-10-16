# Code review: setRootFontSize("125%"), round 1, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `bbb66f0c659100ad0ef6b737ac8993a75331665df6aacc7b6563fad72781d066`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: fix (three MINORs); Grok: ship (the same three as MINORs).
- **Disposition:** the underflow and the JS share check fixed with tests; tests added for an overflowing host size and bad input under a percentage; no failed-commit rollback test (no plan makes a commit fail on demand; the snapshot covers it)

---

- **MINOR — [host/web/navigation.js:876](/Users/admin/x2p2/apps/exact2-comp/host/web/navigation.js:876): Percentage validation differs across targets.** JavaScript accepts `"1e300%"` and `"1e-100%"` and overwrites the current rule. The runner rejects both at a 16px host size because the resolved `f32` becomes infinity or zero, preserving the previous size. Reproduced the JS behavior. Validate the resolved size before replacing the rule.

- **MINOR — [num/src/lib.rs:92](/Users/admin/x2p2/apps/exact2-comp/num/src/lib.rs:92): Underflow produces an accepted zero ratio.** `"5e-324%"` passes the positive-number check but dividing by 100 produces `Some(0.0)`. The compiler consequently accepts the literal while both runtime paths refuse it. Check positivity after division; add this boundary case.

- **MINOR — [contract/cli/tests/it/rem.rs:114](/Users/admin/x2p2/apps/exact2-comp/contract/cli/tests/it/rem.rs:114), [host/web/agent.test.mjs:641](/Users/admin/x2p2/apps/exact2-comp/host/web/agent.test.mjs:641): Missing regression coverage.** Add failed-commit rollback under an active percentage, rejection of a host change whose scaled size overflows, and preservation of the active override after invalid input. The web test checks generated strings, without asserting computed sizes before and after a host-size change.

No MAJOR finding in the ordinary px/percentage/medium state transitions. The snapshot at [commit.rs:112](/Users/admin/x2p2/apps/exact2-comp/runner/src/runner/commit.rs:112) includes the complete `RootFont` and restores it at line 182; the host setter separately restores its saved host size on failure. Apple and Linux already delegate correctly, and both web targets call the shared helper.

The emitted `calc()` is valid CSS: multiplication by a unitless number is permitted. [CSS Values specification](https://www.w3.org/TR/css-values-3/#calc-type-checking)

Validation: three focused Bun tests passed. No fresh Rust build or browser execution was performed in this read-only review.

VERDICT: fix
