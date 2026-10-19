# Code review: Tamber re-import judged whole (theme2's option B), round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `33e803a09f`.
- **Method:** one brief (sha256 `00d5cf5aa6c524686bb1abf8e85db9a63f9668050a86b9af2f1e734c1a63bf2c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (the fourth round could ship an unsettled proposal), one P2 (candidates alone cost a check each, the old semantics' cost)
- **Disposition:** the blocker in 7b05a6c2fc: rounds until one changes nothing, with the termination argument; each round on the design's own folding (halves a candidate's checks). The P2 is the old per-candidate cost, left

**One blocking issue remains.**

1. **[P1 — blocking] The fourth round can return an unsettled proposal.** [export_batch.rs:328](/tmp/rv-optb-2/tamber/src/export_batch.rs:328)

   `moved == true` requests another round, but the fourth iteration simply exits. A refusal returned by `again` changes the proposal after its folding was computed; other members can therefore retain `Done` from a proposal that will never ship.

   Concrete example, traced statically:

   ```text
   tamber t

   theme
     a = light-dark(#000000, b)
     b = #ffffff
     c = light-dark(color-mix(in srgb, a 50%, b), #000000)
   ```

   Change the export’s `a.light` to white, both sides of `b` to black, and `c.dark` to `#111111`, leaving everything else unchanged.

   During rounds 1–3, `a` requests pins because its unchanged dark side now follows black `b`. Those pins repeat `light-dark(#ffffff, b)`. Meanwhile, `c` answers `Done`: its light side still mixes white and black into the original gray.

   Round 4 refuses `a` after its three pin attempts, but judges `c` against the folding that still includes white `a.light`. The returned proposal drops `a`’s edit, so **`c.light` becomes black instead of the unchanged gray**. The final transaction checks design validity, not requested values, and accepts it.

   Budget exhaustion must not imply settlement. Require a stable final proposal or explicitly refuse unresolved members. The same boundary also permits a late `Done → Pins` transition to append operations without another value check.

2. **[P2 — nonblocking] Dial candidates restore whole-design work per token.** [export_batch.rs:212](/tmp/rv-optb-2/tamber/src/export_batch.rs:212)

   Each singleton calls `judged`, which checks both the base and edited design. With N tokens directly following one dial and all requesting the same new color, successful settlement still performs approximately 2N whole-design checks before producing one `SetDial`. This preserves the old candidate semantics, but leaves O(N × design-check-cost) work on a successful path. Deferring literal settlement until the dial actually needs fallback would avoid it.

   Other remaining costs include the acknowledged per-operation token scan in [marks](/tmp/rv-optb-2/tamber/src/export_dials.rs:290), its nested token search when calculating dial impacts, and [Vec::insert for each new `when`](/tmp/rv-optb-2/tamber/src/edit.rs:701), which repeatedly shifts the remaining lines.

I found no additional blocker in the isolated parse: indentation is preserved, `when` indentation is local to its token, metadata stays on the dial’s line, comments are lexed independently, and CRLF handling matches stepping. Earlier-round re-asking and pin counting look correct; the defect is exhaustion of the global four-round limit. Candidate isolation, refusal bookkeeping, report ordering, and §10’s added rows/base-check wording otherwise match the described fixes.

Static review only; no files changed, builds run, or tests run. The reported timings and passing results were not independently verified.
