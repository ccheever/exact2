# Code review: LLP 1122 T3 step 4, the re-scope, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ebafe6aac`.
- **Method:** one brief (sha256 `9412cadead454ff408d40707e9ffd46e5556ff60325bca7496afef3a003e23d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 high; smaller points).
- **Disposition:** all taken: the intervening-write case in `2c4f3498f` (the move is checked against the digests the edit left, and the real-server test drives the refusal); the rest in `b01b9b07d`: the move is worked out from one revision, the one the edit was reviewed on (the design read and every explain must be of its digests), or not offered; it reads the branch it moves (the token the param aliased on that branch, the one param aliasing the token there, explaining params with when lines); a role override by any spelling of its role; none toward a value that names its destination; refused once, not offered again; a redo offers none; a scrub's Keep offers it too. Pure tests in scripts/design-tool.test.mjs cover the branch, roles, the loop, two params and another revision. Kept: a param whose value is an expression offers its inputs in the branch editor (the param rung's), not in the panel.

**Changes requested: four correctness bugs block this commit.** Line numbers below refer to `e1aa1a4c9`.

1. **[P1] Re-scope overwrites intervening writes.** [app.ts:876](/tmp/rv-dt-t3r2/apps/design-tool/app.ts:876) proposes against today’s files and uses those fresh digests for the transaction, but its inverse describes the original edit. Example: Alice sets `Card.radius=8px`; Bob subsequently sets it to `radius.sm`; Alice re-scopes. The transaction removes Bob’s line and writes `radius.md=8px` successfully. The digest check protects only the interval between this new proposal and transaction. Save the original write’s resulting digests and require them when moving.

   This also affects the failed-move UI: a conflict leaves the button available. I reproduced a first click correctly failing on a concurrent write, then a second click accepting fresh digests and overwriting that write.

2. **[P1] The computed inverse can already be wrong before the original write.** [app.ts:692](/tmp/rv-dt-t3r2/apps/design-tool/app.ts:692) combines the latest cached `describe` snapshot with a fresh `explain`, without checking either against the proposal’s digests; it discards `explain.files`. Example: the snapshot says the param has no app line and defaults to `radius.md`; another client adds `Card.radius=radius.sm`; Alice obtains a fresh proposal and applies `8px` before the design read catches up. The saved inverse is `unset-param`, although it must restore `radius.sm`, and the destination is also stale. All subsequent transactions can pass their digest checks. Compute the move from one revision matching the reviewed write, or omit the offer.

3. **[P1] Redo retains the original move’s obsolete preimage.** [app.ts:1007](/tmp/rv-dt-t3r2/apps/design-tool/app.ts:1007) copies `top.rescope` while replacing only the server inverse. Example: set an initially absent param to `8px`, undo, let another client add `Card.radius=radius.sm`, then redo. Redo intentionally writes against the current files, but its re-scope still unsets the param and targets `radius.md`. It should restore the line preceding **redo** and target its alias. Recompute or clear the offer on redo; adding the guard in finding 1 alone will not fix this.

4. **[P1] Alias selection ignores `when` branches.** [rescope.ts:21](/tmp/rv-dt-t3r2/apps/design-tool/rescope.ts:21), [rescope.ts:52](/tmp/rv-dt-t3r2/apps/design-tool/rescope.ts:52), [rescope.ts:55](/tmp/rv-dt-t3r2/apps/design-tool/rescope.ts:55). Given `Card.radius=radius.md` with `when density=compact: radius.sm`, editing compact to `8px` offers `radius.md`. Moving restores the compact alias to `radius.sm` but edits `radius.md`’s compact branch: the selected component loses the requested value while unrelated consumers change. Keeping the same `when` is insufficient when the destination token is wrong.

   The reverse direction also counts only unconditional aliases. A second param that aliases the token only under compact is invisible to the “exactly one param” check. Eligibility and destination selection must account for branch values.

Smaller points:

- **Role aliases are missed.** [rescope.ts:22](/tmp/rv-dt-t3r2/apps/design-tool/rescope.ts:22) uses exact string membership. A param default of `label` or `LABEL` aliases an overridden role reported as `-exact-label`, but receives no offer. Use the checker’s role-name equivalence.
- A new value naming the destination token, directly or transitively, can produce an alias cycle. The server rejects it atomically, so this does not corrupt files, but the impossible offer remains after rejection.
- Restart isolation and unambiguous short-name handling look sound. The bound server token prevents replay after restart; the server preserves existing param-line spelling.

The [server test:198](/tmp/rv-dt-t3r2/contract/cli/tests/it/tamber_set_param.rs:198) satisfies the mixed-transaction condition: it covers both conditional directions and byte restoration, including both unconditional undo levels. However, its operations are handwritten, so it cannot catch incorrect move generation. The [tool test:339](/tmp/rv-dt-t3r2/scripts/design-tool.test.mjs:339) covers Reset and one unconditional param-to-token happy path. Neither covers the failures above. Expression inputs exist in the branch editor, but the added tests do not exercise a param with an expression default.

I ran the commit’s TypeScript in memory against protocol mocks and direct re-scope probes. I did not run the file-writing integration suites. The checkout is unchanged.
