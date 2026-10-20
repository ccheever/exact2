# Code review: the hold replay by call-site chain (LLP 1122 D8, stage 8 E), round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `755a462401`.
- **Method:** one brief (sha256 `3bcc8f4eb0348f73e0a609e7d6ceba80484f846f0f583b9ecb9037e053c2b579`), shared by both reviewers, blind to the other review; commits d6f71f7e25 (before the rebase onto stage 9 E). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high).
- **Disposition:** documented in `748afd4ce9`, per theme2's 18:53 (a) (a reorder may be documented as the known limit): the protocol now says the correspondence is by places alone (the map carries no other identity of a call or element), so a reorder that keeps its spacing, a call or element moved onto another's old place (line or column), or a copy added or removed while one keeps its line can carry the hold to the other copy, the host letting it go (`gone`) where the copies' instance paths differ. The rule is not made stricter: no rule over places alone separates these from the edits it should follow (a line written above, between or inside), and a stricter one clears holds on those. Not taken: a persisted malformed-map replay test through `tamberServer` (the exception path is two lines; the channel test drives replay).

**Request changes: two wrong-target replay cases remain.** Both reproduced through `tamberServer` using in-memory maps, with no `hold-cleared` event.

1. **[P1] A reorder can still pass the “one edit” check.** [tamber-inspect.mjs:349](/tmp/rv-dt-t4r5/scripts/tamber-inspect.mjs:349)

   Put calls A and B at lines 7 and 12. Cut B plus its three following blank lines and paste before line 6. The new calls are **B at 6, A at 11**. No old call coordinate is reused.

   Rank pairing sees displacements `[-1, -1]`, accepts them, and replays B’s hold onto A. The [protocol’s claim that crossing calls clear](/tmp/rv-dt-t4r5/docs/tamber-protocol.md:990), except for reused coordinates, therefore remains false. Theme2’s ruling permits documenting this reorder limitation, but the current wording excludes this case.

2. **[P1] Reusing a node’s declaration position transfers its hold, even with unchanged calls.** [tamber-inspect.mjs:371](/tmp/rv-dt-t4r5/scripts/tamber-inspect.mjs:371)

   A component contains adjacent pressable buttons A and B at lines 20 and 21. Hold B, then insert a text element above A. A moves to 21 and B to 22; every component call remains unchanged.

   `same` now identifies A as the old B. The identical-chain shortcut accepts it; button-plus-label counts also match. The server re-holds **A**. This is an additional hole in the reviewed rule: the documented call reorder, copy-count and slot-outlet limits do not name declaration-position reuse.

The specific round 2 regressions are fixed: mixed depths no longer throw; file-first ordering fixes the unrelated Footer case; and `(4, 9) → (3, 5)` now clears. I also verified that a matching exception clears the hold and emits the diagnostic. The earlier button/label and nested-regrouping regression tests still pass.

The recorded coordinate-reuse and slot-order limits describe the implementation’s choices. Omitting duplicate server tests for every matching case is reasonable. **Smaller coverage gap:** the new exception handler at [tamber-server.mjs:215](/tmp/rv-dt-t4r5/scripts/tamber-server.mjs:215) deserves one persisted malformed-map replay test; the matcher test and stubbed channel test cannot exercise it.

The two targeted tests passed, with **67 assertions**. No files changed, no tree-modifying git commands ran, and the working tree is clean.
