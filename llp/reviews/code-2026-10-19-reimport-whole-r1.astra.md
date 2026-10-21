# Code review: Tamber re-import judged whole (theme2's option B), round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `0e495cd3e7`.
- **Method:** one brief (sha256 `619445fe687d9402c7b3b256943643483219c341254a0d894fda9d1ac23436d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** three blocking (dial candidates' literal fallbacks judged in the batch; a lexical read closure blaming an unrelated member; the batch skipping the parse stepping does between operations), three should-fix (report order, quadratic refusal bookkeeping, §10's 800-token row)
- **Disposition:** all taken in 33e803a09f: candidates judged alone; non-member diagnostics halve; each operation's lines parsed alone in the batch; indexed bookkeeping; §10 rewritten with the 800-token row and the base check. The report order is as before option B (the example came from the candidate issue)

I found **three blocking issues** in HEAD `0e495cd3e7`. These are static traces; I did not build, run tests, or modify files.

1. **Blocking — literal fallbacks are refused before dial conversion can make the proposal valid.** [export.rs:1352](/tmp/rv-optb-1/tamber/src/export.rs:1352)

   Consider a colour dial `accent = #000000`, with `a = accent` and `b = light-dark(accent, #ffffff)`. Change both schemes of `a` to `#111111` and both schemes of `b` to `#222222`.

   The first round applies both literal replacements, making `accent` inert. Halving accepts `a` first, then refuses `b`. Later, `dials()` converts `a` into `SetDial(accent, #111111)`, restoring its reference—but `b` is already permanently reported. The actual proposal, that dial change plus `b = #222222`, checks and satisfies both edits. The parent’s per-token planning accepts this case.

   Resolve dial candidates before definitive whole-proposal refusals, or reconsider refusals after conversion. The interaction tests need this end-to-end case.

2. **Blocking — the lexical “read closure” can blame a completely unrelated member.** [export_batch.rs:133](/tmp/rv-optb-1/tamber/src/export_batch.rs:133)

   With the normal vocabulary containing `Canvas`, consider:

   ```text
   theme
     canvas = #000000
     srgb = #111111
     preview = color-mix(in srgb, Canvas, #ffffff) scheme=fixed
   ```

   Propose `canvas = light-dark(#ff0000, #000000)` and `srgb = #222222`. Only the Canvas change makes `preview` violate `scheme=fixed`.

   The closure walker nevertheless recognizes `srgb`—a colour-space keyword here—as a token dependency, while missing `Canvas` because the declaration’s spelling is `canvas`. It consequently refuses the unrelated `srgb` member with `preview`’s diagnostic, then eventually refuses `canvas` too.

   Dependency attribution must follow the checker’s reference semantics, including role aliases and CSS keyword positions. An uncertain dependency set should fall back to splitting, not establish unique blame.

3. **Blocking — batching does not preserve intermediate parse refusals.** [edit/batch.rs:106](/tmp/rv-optb-1/tamber/src/edit/batch.rs:106)

   On a parseable design containing `d: number = 1`, use these operations:

   ```text
   SetDial(d, "x=y z")
   SetDial(d, "2")
   ```

   `"x=y z"` passes `valid()`: it has no comment, newline, `in`, or final metadata word. However, the resulting dial line fails `syntax::ranged`, which encounters metadata before any value.

   Sequential stepping refuses when the second operation reparses that document. Batching skips that parse, overwrites the bad value, and returns success. This violates the explicit refusal-equivalence condition. The differential generator’s invalid values are caught by `valid()` and do not exercise this boundary.

4. **Should-fix — final reports and transaction members are not consistently in source order.** [export.rs:1355](/tmp/rv-optb-1/tamber/src/export.rs:1355)

   Planning reports are appended first, dial refusals afterward, then transaction refusals. Dial fallbacks are also appended after ordinary accepted tokens.

   For source order `a, b, z`, let `a` and `b` be the only followers of one dial, change them to different colours, and give `z` an unreadable incoming value. The rounds refuse `b`’s literal for making the dial inert; its report is deferred through dial handling. `z` is reported immediately, producing `z, b`.

   Sorting `Judged.refused` only guarantees order within that helper. Carry the original position through dial handling and restore order before final judgment and reporting.

5. **Should-fix — the new refusal path still contains quadratic work.** [export_batch.rs:78](/tmp/rv-optb-1/tamber/src/export_batch.rs:78), [export_batch.rs:175](/tmp/rv-optb-1/tamber/src/export_batch.rs:175)

   Filtering survivors scans all blamed members for each member, and accumulating diagnostics linearly searches the growing output. With one diagnostic per member, both are quadratic. Non-member diagnostics also independently walk and tokenize shared dependency closures.

   Use indexed refusal bookkeeping and reuse dependency analysis. Existing costs also remain outside this new helper: token scans per operation in [marks](/tmp/rv-optb-1/tamber/src/export_dials.rs:290), dial scans in `candidate`, whole-design work per dial, and suffix movement for every inserted `when` in `Doc.lines`.

6. **Should-fix — §10 does not document the required 800-token measurement.** [1121-tamber.rfc.md:4234](/tmp/rv-optb-1/llp/1121-tamber.rfc.md:4234)

   The table records 400, 3,000, 2,000 × 64, and refusal-heavy results, but no 800-token result. That acceptance evidence remains missing. Also, “checked once” omits `judged`’s unconditional base check: a successful nonempty judgment checks both the base and the edited text.

By inspection, the ordinary `when` insertion bookkeeping, condition-order preservation, CRLF handling, first-of token index, folding-index alignment, and nonempty-member `k` mapping are consistent. The closure walk terminates, and diagnostics at line 0/1 fall back to splitting. The principal test gaps are the three concrete blocking cases above.
