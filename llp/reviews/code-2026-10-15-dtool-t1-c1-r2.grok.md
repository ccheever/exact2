# Code review: LLP 1122 T1 commit 1 (Tamber server: history, revert, import, the tool's files), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `7e62930b3` (the range `9479c6464..7e62930b3`).
- **Method:** one brief (sha256 `9b06e667e8ff7c8224874f102d4de02191523a3bafabbbb8f58db9d9dc5bbb65`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 2 low).
- **Disposition:** taken in round 2: a removed dial is "already" only with its whole declaration back, else a conflict; a file over 1 MiB is a named conflict in the revert; a partial accept keeps its selection (with Astra's); DTCG sources sorted by name, as the CLI reads a directory.

---

Round 1’s fixes hold. Two gaps remain: a dial removal is treated as already undone when only the value matches, and an oversized file aborts a whole revert with status 413 and no file name.

## Round 1

These are fixed in the current tree.

- Axis order is refused. A reorder, or a membership change whose remaining values are no longer in the old order, conflicts (`tamber/src/history.rs:286`, `:322`). A pure membership restore still puts the removed value back in its old place; the test applies that plan and gets `black | dim | navy`.
- Token identity includes kind and metadata (`same_token`, `:477`). Reverting an added `card` after a later `scheme=fixed` yields no operations and a conflict on `card`.
- A held revert that conflicts is planned again from the commit sha, with new digests (`scripts/tamber-server.mjs:475`). The test moves the file after the proposal is held: the commit’s own edit stays 409 and unwritten; a later edit on another token is recomputed and kept.
- Tool files are read through `realpath`, and the canonical path must stay inside the build and must not be a dot path (`scripts/tamber-server.mjs:278`). The test covers a symlink out to `package.json` and a symlink onto `.secret`.
- History compares one file per request. A non-413 failure is the history call’s status. A pair over 1 MiB stays a 200 with that file’s `edits: null` and an explicit refusal (`:536`).
- A name or a `when` declared twice refuses the reading (`ambiguous`, `history.rs:164`).
- `import-plan` takes `sources` (1 to 256) and passes the slice to DTCG. The Figma fixture is counted `{source: 104, mapped: 40, reported: 64}` on both the stdio test and the wire test.
- Each file’s diff is `git diff -- :(literal)<path>` (`scripts/tamber-history.mjs:81`). The `ünï.tamber` commit’s diff contains the added line. `skip` is capped at 10,000.

Git arguments are passed as an array, revisions cannot start with `-` or contain whitespace or NUL, and `rev-parse` uses `--end-of-options`. The static handler rejects encoded `..`, a remote address, and a non-GET. I did not re-run the test binaries.

## Medium

**A removed dial is “already undone” when the value matches and the declaration does not.** `tamber/src/history.rs:260` (the value edit is built at `:106`; `dial_shape` is recorded only when both versions still have the dial, `:115`).

The already-check compares the trimmed value. Kind, range, and `step` are absent from a removal, because that shape edit is emitted only when the dial exists on both sides. Tokens were given `same_token` for this case; dials were not.

Parent has `accent: color = #1083fe  step=0.1` and `ink = #000000`. The commit deletes the dial and sets `ink` to `#111111`. Today the file has `accent: number = #1083fe` and `ink` still `#111111`. The dial edit’s before and the current value are both `#1083fe`, so it is pushed to `already`. `ink` is planned as `set-token`. Conflicts are empty, so the wire holds the proposal. Accepting it restores `ink` and leaves the number dial in place. If `ink` was already back as well, `planRevert` returns 409 “nothing to revert” (`scripts/tamber-server.mjs:570`) while the number dial is still there. `removals_come_back_and_dials_added_cannot_go` only covers a dial the commit added.

Fix: before marking a dial removal already, require the current dial’s `dial_shape` to equal the old dial’s. A present dial with another declaration conflicts (“a dial by that name is there again, with another declaration”). A missing dial keeps today’s “no operation adds or removes a dial” conflict.

**A file over the 1 MiB line fails the whole revert as 413 and names no file.** `scripts/tamber-server.mjs:561` (`ask` refuses at `:179`).

History turns that same 413 into a per-file refusal and continues. `planRevert` returns the 413 as soon as one internal `revert-plan` line is at least 1 MiB. A commit that changes a small `app.tamber` and a `big.tamber` of about 400 KiB before and 400 KiB after is answered 413 “a request is at most 1 MiB” on a tiny `{method, client, commit}` body. The small file is never proposed.

Fix: record a conflict on that file (“its versions are too large to compare (over 1 MiB)”) and fall through to the existing 409 path, which already refuses the commit and lists each conflict.

## Low

**A partial accept of a revert, after the file moves, is replaced by a plan of the whole commit.** `scripts/tamber-server.mjs:503` calls `recompute` on the chosen items; a digest conflict at `:499` calls `replan` (`:479`), which plans every edit of the sha. The new proposal is held (`recomputed: true`) and is not applied. The chosen subset is gone. Fix: plan again, then keep only the operations that correspond to the accepted items, or refuse the partial accept and leave the selection for the caller to make on the new plan.

**DTCG source order is the client’s, and a second themes or resolver document is dropped.** `contract/cli/src/tamber_import.rs:149` keeps `sources` in request order; the CLI sorts (`:210`). In `tamber/src/import/dtcg.rs`, the first `$themes` (`:410`) and the first `resolutionOrder` document (`:439`) win, and a later plain layer wins. Two JSON files that are not in `tokenSetOrder` and both set `color.ink` import differently from `contract tamber import dtcg <dir>` when the tool sends them in the other order. The Figma count stays stable because collections are grouped in a `BTreeMap` and modes are sorted by name. Fix: refuse a second `$themes` or `resolutionOrder` file, and sort by name when the import is meant to match a directory.

The new tests do not cover this dial case, a revert whose compare exceeds 1 MiB, `skip` above 10,000, or a directory symlink. The unicode test checks the diff text and does not assert that `edits` is present.

Verdict: request changes — dial removal is still “already” on the value alone, so that edit is dropped and the rest of the commit can revert.
