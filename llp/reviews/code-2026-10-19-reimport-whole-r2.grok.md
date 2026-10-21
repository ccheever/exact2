# Code review: Tamber re-import judged whole (theme2's option B), round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `33e803a09f`.
- **Method:** one brief (sha256 `00d5cf5aa6c524686bb1abf8e85db9a63f9668050a86b9af2f1e734c1a63bf2c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (the same), one nit (rustdoc 'source order')
- **Disposition:** the blocker in 7b05a6c2fc, the nit in 7b979de1cf

The four-round cap can commit a proposal that still moved. That breaks the new settle rule, and it is blocking.

**[P1] The last round can add pins or drop a token and still ship — `tamber/src/export_batch.rs:239`**

`settle` runs four iterations and, if that round changed the proposal, just falls out of the loop. The iteration after a change is what re-asks everyone, and it does not exist once the cap is spent. `pinned` counts pin answers only, so a token that answered `Done` on earlier rounds still has `rounds < 3` and may answer `Pins` on the fourth ask. Those pins are appended after `judged` and are never folded. The same iteration can refuse a sibling (`rounds >= 3`, or a non-colour token whose folded values no longer match) only after the others have already answered against a folding that still contains it.

A check-refusal from `judged` is safe: that folding is rebuilt without the refused members before `again`. A refusal or pin that comes from `again` is not. The single-token pin loop still fits (three pin answers, then a fourth iteration that only observes). A multi-token set does not, which is the case re-asking was added for. The transaction's later check catches a design that will not parse or check, not a context that renders the wrong colour. Fail closed when the cap is hit on a dirty round: one confirming pass that cannot add pins, or refuse the members that are not yet stable.

What I checked and do not find blocking:

- **Alone-parse.** For the lines `SetToken` and `SetDial` write, a full parse and the snippet agree. The snippet is `tamber t`, a blank line, `theme` or `dials`, then the target's own lines (name through last `when`, plus the inserted `when`). A checked design's token indent is the block indent, and `when_indent` resets on each token, so a `when` is deeper than its token in both parses; a new `when` uses the existing `when` indent, or the token indent plus two. Dial `in` and `name=value` metadata stay on that one line and `ranged` does not look at siblings. Comment and blank lines are skipped by the lexer in both. CRLF survives as a trailing `\r` on the `Doc` line, `join("\n")` makes `\r\n`, and `str::lines` strips it the same way the full parse does. A false reject only falls back to stepping. `x=y z` fails as a dial line in both, which is the new test.
- **Re-ask inside the budget.** `Done` does not stick. The next iteration, if there is one, asks again, and `Pins` extends that token's ops. `pinned` is the number of pin answers so far, which is what `plan`'s `rounds >= 3` is written against (three pin batches, then a verdict). It is not an off-by-one. The hole is only the shared four-iteration budget above.
- **Candidates alone.** Each dial candidate is its own one-member `judged` on the original text, then `export_dials::dials` keeps `SetDial` or the literal fallback. That is the old per-token pin loop for that token, and the literal is not in the batch with the others. Followers of the dial are still pinned before the dial op is applied; that order predates this change, and `marks` records the movement.
- **Common path.** One base check, one `stepped_all`, one result check, then `plan` per token. The per-op parse is only that token's lines. Refusal bookkeeping is a `HashSet` and a `HashMap`. `marks` is still a linear scan per operation (`O(ops × tokens)`), which §10 already leaves to rt1. Nothing new on the success path is `O(N · design)`.

§10 now matches the code: export order of names, the 800-token and refusal-heavy rows, the base check, and the transaction's "with the rest of the proposal". Report order is still planning (including round refusals), then dial, then transaction. The round-1 transaction example was the candidate literal in the shared batch; that path is gone. The rustdoc on `judged` and `settle` still says "source order".

No test covers a `Done` that later returns `Pins`, or the fourth round dropping a member another token was settled against.
