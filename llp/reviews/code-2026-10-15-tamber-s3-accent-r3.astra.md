# Code review: stage 3 commit 6 follow-up: role offers, round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `3e2f53a75895b5c834a70dfa50dff978769a55d9f0fd6c05eaf833853a008fe8`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Astra: removal breaks consumers, a role's own when lines, unpaired roles; Grok: other roles and spellings).
- **Disposition:** applied without re-review in the third-round commit: no removal op, every design role checked and named

---

I found three blocking issues.

1. **P1 — A role’s own literal `when` lines can become varying after axis expansion.** [tamber/src/import/shadcn.rs:363](/tmp/rv-ac3/tamber/src/import/shadcn.rs:363) checks references inside the role’s values, ignoring its conditions. A valid design with `theme = old`, `AccentColor = #ff0000`, and `when theme=old: #0000ff` initially has one role value. Importing primary colours that differ between `:root` and `.theme-blue` adds `default` and `blue` axis values. The replacement is withheld, but the literal-valued role earns no removal offer. It now varies and fails `tamber-role-axis`, even with every offer accepted.

2. **P1 — Existing-role repair skips roles without their prescribed source token.** [tamber/src/import/shadcn.rs:355](/tmp/rv-ac3/tamber/src/import/shadcn.rs:355) skips `AccentColor` unless the source maps `--primary`. For example, an existing design can bind `AccentColor → shadcn.primary → shadcn.brand`. Importing a theme-dependent `--brand` without `--primary` makes that chain vary, but offers no repair for `AccentColor`. Roles outside the five prescribed mappings are similarly missed. Surviving role overrides need checking independently of the source-to-role mapping.

3. **P1 — Removing an override can invalidate its consumers.** [tamber/src/import/shadcn.rs:378](/tmp/rv-ac3/tamber/src/import/shadcn.rs:378) deletes the role without accounting for expressions using its concrete colour. With `AccentColor = shadcn.primary` and `wash = light-dark(AccentColor, #000000)`, removal turns `AccentColor` into a host role and `wash` fails `tamber-residual`. This also happens with source spellings `#ff0000` and `red`: structural variation triggers deletion although the folded role remains invariant. I confirmed the deletion failure with a read-only compiler probe.

The retained-`when` predicate matches reconcile’s keep/unset rules. I found no additional new-binding hole from dials, profile stand-ins, or `light-dark()`. Removal correctly targets the design’s spelling, and keeping it declinable is appropriate; declining a necessary repair leaves the explanatory `tamber-role-axis` diagnostic.

All **28 importer tests passed** using the existing HEAD binary. The Contract integration test lacks the cases above; I inspected it without rerunning it because it writes fixtures. No files were changed.
