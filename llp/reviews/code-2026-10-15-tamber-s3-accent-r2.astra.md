# Code review: stage 3 commit 6 follow-up: role offers, round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `c2ea48c92e864b755b0a42fd04a517874fde9b46255282e8e5e2e28a97b19518`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (both P1: variation judged on the reconciled design; Astra P1: a bound role left in place).
- **Disposition:** fixed in the second-round commit; Astra's P2 not taken (safe direction)

---

**Two blocking compile-failure cases remain, plus one unnecessary refusal.**

1. **P1 — `varies` misses conditions retained from the existing design.** [tamber/src/import/shadcn.rs:357](/tmp/rv-ac2/tamber/src/import/shadcn.rs:357)  
   Start with `density = default | compact`, `shadcn.primary = #111111`, and `when density=compact: #222222`. Import `:root { --primary: #333333; }`. The imported token has no `whens`, so `AccentColor` is offered with operations. However, [reconcile:159](/tmp/rv-ac2/tamber/src/import/mod.rs:159) preserves the density condition. Accepting everything produces an axis-varying `AccentColor`, which [Contract lowering:254](/tmp/rv-ac2/contract/lower/src/theme.rs:254) rejects with `tamber-role-axis`. The same problem applies through retained references and conditions on axes declared `under scheme=dark`. Eligibility must examine the reconciled design.

2. **P1 — A note-only offer leaves previously accepted role bindings active.** [tamber/src/import/shadcn.rs:362](/tmp/rv-ac2/tamber/src/import/shadcn.rs:362)  
   Import `:root { --primary: red; }` and accept `AccentColor = shadcn.primary`. Then re-import after adding `.theme-blue { --primary: blue; }`. This time `varies` correctly returns true, but returning no operations leaves the existing `AccentColor` binding untouched. The token transaction adds the variation, and the resulting app fails with `tamber-role-axis` even when accepting only the token operations. The importer needs to detect this existing binding and refuse or provide a valid resolution.

3. **P2 — Presence of a `when` is stricter than Contract’s folded-value comparison.** [tamber/src/import/shadcn.rs:410](/tmp/rv-ac2/tamber/src/import/shadcn.rs:410)  
   `:root { --primary: red; } .theme-blue { --primary: #ff0000; }` produces a retained `when`, so the override is refused. Both tuples nevertheless fold to `#ff0000ff`, and Contract would accept the role. Identically written defaults are already removed by [engine.rs:322](/tmp/rv-ac2/tamber/src/import/engine.rs:322); equivalent spellings, aliases, and scheme-side selection require comparing resolved values rather than detecting conditions.

The other requested checks:

- **References and cycles:** traversal follows imported-token references and terminates on cycles. The assembly engine already reports unresolved cycles or materializes resolvable cross-block cycles. I found no separate blocker there.
- **Scheme:** `light-dark()` alone correctly does not trigger refusal. An axis declared `under scheme=dark` remains an app axis; its folded pairs determine whether the role varies.
- **Note-only offers:** acceptable repeated diagnostics, with minor console noise. They serialize with empty operations, consume no item indices, and retain their notes through partial acceptance. Unchanged re-imports send no new held proposal.
- **Tests:** the new compile test is useful, but [tamber_cli.rs:139](/tmp/rv-ac2/contract/cli/tests/it/tamber_cli.rs:139) always starts from an empty design, missing both P1 cases. The theme-class fixture covers direct variation only. The existing [custom-axis test:353](/tmp/rv-ac2/tamber/tests/import.rs:353) checks Tamber acceptance without Contract compilation, so it does not expose the first failure.

I executed read-only, in-memory checks of the actual JavaScript import and acceptance functions with mocked I/O. Rust compilation tests were not run under the read-only restriction. No files changed.
