# Code review: held :hover and :focus-visible looks editable, the source map's state parts (LLP 1122 queue item (1), c1 and c2), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `709c0bb8bc`.
- **Method:** one brief (sha256 `47f231dbf86268ecdbaefd39d662bc474abdf2a10cd64610342aeff900160729`), shared by both reviewers, each blind to the other's review; commits d76b990cb7 (c1) and 709c0bb8bc (c2). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 1 SHOULD-FIX, 1 NIT.
- **Disposition:** all taken in the round's fix commit (named in round 2's records):
  - A state part's border side is named as the source writes it at its place, `border-color` (its four sides whole) or the side's own longhand, and none for anything else (`writtenAs` in scripts/tamber-inspect.mjs); tests for a style's later `border-top-color` over its shorthand, an own `border-left-color`, and a place that says neither (1).
  - A look row's class-or-own origin is decided by the class's spans (`class_spans`), as a base row's is; states.rs's positional flag is reverted; a test with a class whose animation longhands are composed, which gave `class:Pulse` for the node's own hover on c1 (2).
  - The look origins are built only when a map is asked for (3).

One blocker found.

1. **BLOCKER — c2: directly authored border sides target the wrong property.** [scripts/tamber-inspect.mjs:184](/tmp/rv-dt-q1a/scripts/tamber-inspect.mjs:184) maps every `border_color_top` to `border-color`, but state parts also admit explicit `border-top-color`. With `style Card → :hover → border-top-color="#111111"`, `stateRung` offers `row: "border-color"` without its current text. Saving inserts a new shorthand, changing all four sides. If a shorthand already exists beneath a later top-side override, saving changes the other sides while leaving the selected top unchanged. An own-node longhand instead gets no editable rung. Determine the authored property from the mapped source; use `border-color` only when that attribute supplied the row.

2. **SHOULD-FIX — c1: look origins trust a class boundary invalidated by rewriting.** [contract/lower/src/nodes.rs:769](/tmp/rv-dt-q1a/contract/lower/src/nodes.rs:769) uses the positional flag from `fold_states`. Consider a `Pulse` class containing `animation-name` and `animation-duration`, applied to `box class=Pulse testId="card"` with its own `:hover opacity=0.5`. Animation composition removes those two attributes and appends one, moving the own hover attribute inside the old `class_len`. Its map entry becomes `class:Pulse` at the node’s own part. The join consequently offers neither the state nor template rung. Use the existing `class_spans` membership, as base-origin attribution does.

3. **NIT — c1: unmapped compilation constructs unused look provenance.** [contract/lower/src/nodes.rs:751](/tmp/rv-dt-q1a/contract/lower/src/nodes.rs:751) allocates the origins vector, boxed origins and class-label copies even when `self.sites` is absent; only storing them is guarded. Gate their construction as the base-binding path does.

I found no additional issue in row/state merging, reader migration, held-owner freshness checks, or focus spelling. c1 preserves the lowering calls and their order; I found no path that changes plan bytes.

Read-only review; no builds or tests run, and the recorded byte comparisons were not rerun.
