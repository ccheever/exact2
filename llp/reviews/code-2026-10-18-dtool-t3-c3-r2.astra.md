# Code review: LLP 1122 T3 step 4, the re-scope, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `5c7138e94` (before dt/t2's rebase onto main; it is `2d59d8578` after, the shas named in the briefs map as /tmp/dt-t3/sha-map-1.txt says).
- **Method:** one brief (sha256 `aee04452d09a6031cad46b26b2e25916860fca7db40315d1b56f0ff2cd68e94d`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (a second run: the first hit the model's capacity).
- **Verdict:** request changes (2 high, 3 medium).
- **Disposition:** taken in `f45cc65ed` and `8209bcef9`: the other rung's value is the one the moved line's copies show (its most specific when that the edit's when holds, else its unconditional value), so a finer mode moves to the alias it shows; an icon param is skipped; the app's design opened by its file offers the move; any server refusal of the move withdraws it (a loop through another token included); the move requires every file it was worked out on, so a component default changed in its Contract source since the edit refuses it (the real-server test drives that). Not taken, as limits for §10: a param's first when line (two operations) offers no move; a loop through another token is refused by the server rather than foreseen.

**Changes requested: two correctness bugs still block.**

1. **[P1] Component-source changes bypass the move’s digest guard.** [app.ts:895](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:895) sends the saved digests as `files`, but the server checks those only for files being edited; other dependencies require `requires` ([tamber_serve.rs:1033](/tmp/rv-dt-t3r3/contract/cli/src/tamber_serve.rs:1033)).

   Example: `Card.radius` defaults to `radius.md`. Alice applies `8px`, then someone changes the component’s `.contract` default to `radius.sm` (`4px`). Re-scope still commits `unset-param Card.radius` plus `radius.md=8px`: the card becomes **4px**, and unrelated consumers of `radius.md` change. The source dependencies must remain guarded through the original write and the move. Matching the reads beforehand does not close this gap.

2. **[P1] A more specific mode still chooses the wrong alias.** [rescope.ts:45](/tmp/rv-dt-t3r3/apps/design-tool/rescope.ts:45) checks for an identical `when`, then falls straight back to the unconditional value.

   Given `Card.radius=radius.md` with `when density=compact: radius.sm`, choose compact/small, select **“only this mode”**, and apply `8px`. The new branch is `density=compact size=small`. Re-scope incorrectly targets `radius.md`; removing the new param branch restores the broader compact branch, which still follows `radius.sm`. The card loses the requested value. Reverse-direction eligibility uses the same incorrect fallback at [rescope.ts:80](/tmp/rv-dt-t3r3/apps/design-tool/rescope.ts:80). Exact-line existence for the inverse and effective alias selection need separate handling.

Smaller points:

- **[P2] The earlier cycle finding is only partially fixed.** [rescope.ts:67](/tmp/rv-dt-t3r3/apps/design-tool/rescope.ts:67) excludes only a value naming the destination directly. With `radius.alias=radius.md`, changing the param to `radius.alias` still offers a move that creates `radius.md → radius.alias → radius.md`. Expressions referencing the destination also pass. The server refuses atomically, but [app.ts:899](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:899) clears the offer only when `byWhom` is present, so checker refusals leave the impossible button available.

- **[P2] A param’s first conditional edit gets no re-scope.** [app.ts:873](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:873) excludes every two-operation write. When the param has no app line, “only this mode” legitimately writes its default line plus the conditional value. Apply succeeds, but offers no move despite the default directly aliasing a token.

- **[P2] Selecting the app design by filename disables re-scope.** [app.ts:705](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:705) accepts only `file === ""`. In the design-file selector, choosing `app.tamber` instead of “The app’s design” shows the same component panel and permits the same edits, but none receives an offer.

The other earlier fixes look right: matching snapshot/explain revisions, intervening design-file conflict refusal and button clearing, redo clearing, role spellings, scrub Keep, short names, and restart isolation. Expression inputs are reachable through the param branch editor, consistent with the recorded disposition.

I ran the existing pure T3 case and additional in-memory probes of the actual TypeScript with protocol mocks. The Rust transaction test still uses handwritten operations, so it cannot catch alias-generation errors. I did not run the file-writing integration suites. The checkout is unchanged.
