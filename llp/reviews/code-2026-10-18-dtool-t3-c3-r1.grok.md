# Code review: LLP 1122 T3 step 4, the re-scope, round 1, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `ebafe6aac`.
- **Method:** one brief (sha256 `9412cadead454ff408d40707e9ffd46e5556ff60325bca7496afef3a003e23d0`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 2 medium, 1 low).
- **Disposition:** all taken: the intervening-write case in `d489daf59` (the move is checked against the digests the edit left, and the real-server test drives the refusal); the rest in `1a1605b93`: the move is worked out from one revision, the one the edit was reviewed on (the design read and every explain must be of its digests), or not offered; it reads the branch it moves (the token the param aliased on that branch, the one param aliasing the token there, explaining params with when lines); a role override by any spelling of its role; none toward a value that names its destination; refused once, not offered again; a redo offers none; a scrub's Keep offers it too. Pure tests in scripts/design-tool.test.mjs cover the branch, roles, the loop, two params and another revision. Kept: a param whose value is an expression offers its inputs in the branch editor (the param rung's), not in the panel.

Two correctness bugs block this step. The move keeps the same `when`, and `explain` returns source text, so a restored branch stays an alias or an expression. The other rung is chosen from the wrong branch, and the frozen inverse is applied to whatever the file is when the button is clicked.

## Findings

### [P1] A when-edit moves to the unconditional alias — `apps/design-tool/rescope.ts:22`

`paramInfoOf` stores one alias: `set.value`, or the default, and only when that whole string is a token name. `describe` publishes `set.whens` as a count, not the branch values (`contract/cli/src/tamber_params.rs`). `rescopeOf` then sends every edit to that one token, on the edit's own `when` (`rescope.ts:48-52` and `:55-63`). `explain` has already returned the edited branch's source (`was.value`) and the param path ignores it.

```
params
  Card.fill = ink
    when dark-style=dim: paper
```

Change `Card.fill`'s dim line from `paper` to `#112233`. The button says "Apply to the token ink instead". The transaction puts the dim line back to `paper` and writes `ink`'s dim branch to `#112233`. The dim copy was showing `paper`.

The other way is the same map. Edit `ink` on dim and exactly one param has `alias === "ink"`, so the offer is "Apply to Card.fill instead". That overwrites `Card.fill`'s dim line (`paper`) and reverts `ink`. A param that aliases `paper` only on dim (`Card.fill = #000` / `when dim: paper`) has `alias === null`, so editing `paper` offers nothing.

### [P1] The inverse is not bound to the revision it undoes — `apps/design-tool/app.ts:700`

`rescopeFor` takes `there` and the alias from `snapshots.get(issued)`, the last `describe`, and the branch value from a live `explain`. `explain` returns `files`; this call drops them (`app.ts:705`). The cell reader refuses that mismatch (`app.ts:821-824`). The Apply transaction is checked against the proposal's digests, so it can succeed while the snapshot is older than the file.

Last describe: `Card.fill` has no line and defaults to `ink`. Before the next describe, the line becomes `paper`. Apply of `Card.fill = #ff0000` is proposed against that file and writes. The snapshot still has `set === null` and `alias === "ink"`, so the stored inverse is `unset-param` plus `set-token ink`, and `explain`'s `paper` is discarded because `there` is false. The click removes the line and retargets `ink`.

Clicking later does not close this. `rescopeLast` proposes the frozen ops and writes them against that propose's digests (`app.ts:886-890`). The check is "the file matches this propose", so an intervening `Card.fill = #00ff00` is the base the stale `unset-param` deletes. Undo of the original edit would refuse: the server holds the post-write digest. The offer stays on the undo entry through another client's write, and redo copies it back (`app.ts:1018`). A server restart does drop it: the call is bound to the edit's run token, `forget` clears the stacks, and the bar clears when `written.session` falls behind the poll.

A refused move leaves the button up, with the error beside it (`app.contract:1431-1435`). It is not stuck disabled. The next click proposes again and will commit the same ops if the new propose succeeds.

## Smaller

### [P2] A bare role spelling is not the token it names — `apps/design-tool/rescope.ts:18`

The token set is `ir.tokens[].name` only. A role override's row name is `-exact-secondary-label` with `role: "secondary-label"` (`tamber/src/check.rs:139-146`). `Card.fill = secondary-label` gets no alias, so neither direction offers a move. `Card.fill = -exact-secondary-label` does. `inputsOf` in the same tool already treats both spellings as that token (`app.ts:196-207`).

### [P2] An expression default offers no inputs — `apps/design-tool/rescope.ts:22`

A default that is not exactly one token name sets `alias` to null, so there is no button. That matches the commit message. D6's other half is that this param offers the expression's inputs instead. `inputsOf` runs for a grid cell (`app.contract:1316`). Param tokens are removed from `ir.tokens`, and the component panel never lists those inputs, so `mix(accent.fill, white)` is only a field you can overwrite.

### [P3] A kept scrub never offers the move — `apps/design-tool/app.ts:953`

`releaseScrub` pushes the undo entry with no `rescope`. Keep on a param that aliases a token leaves the bar without "Apply to … instead". Apply is the path that stores the move.

## Checked, and the tests

Short names line up. `opOf` writes `lineName` from the same map `paramInfoOf` stores, an ambiguous short falls back to the full name, and the server rewrites the op to the line's existing spelling. `params.get(op.name)` then a short-name search finds that param. A chain is direct only: `Card.fill = accent.fill` with `accent.fill = accent` re-scopes an edit of `accent.fill`, and an edit of `accent` offers nothing. The same `when` is copied onto both ops. The extra unconditional `set-param` is the param's existing default, which a `when` line needs before it can be added (`tamber/src/edit/params.rs:27-30`). It is not the when-value.

`contract/cli/tests/it/tamber_set_param.rs` is the server test the gate asked for. It shows one undo restoring bytes, and a `when` moving param-to-token and token-to-param. Those ops are hand-written. They never build the client's alias, and the param already has its own line, so the client's "write the default first" transaction is not in the test. `scripts/design-tool.test.mjs` covers Reset byte for byte and one re-scope: `Card.fill` with no line, default `accent.fill`, to `#ff0000`, one transaction, one undo, button cleared after the move. It does not cover a `when`, a second param, a role, an expression, a stale describe, or another client's write.
