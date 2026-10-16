# Code review: LLP 1122 T1 commit 1 (Tamber server: history, revert, import, the tool's files), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `0ef88b7fd`.
- **Method:** one brief (sha256 `37fb25c19813b21bcfcbd05773260b6c34e7582ddc637c660965f83d119165bb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** do not land (1 high, 4 medium, 2 low).
- **Disposition:** all taken in round 1 (with Astra's): axis order restored or refused; kind and metadata in a token's identity; per-file diffs by literal path; no link out of the tool's files; one file a compare request, its transport error returned; `sources` for DTCG; `skip` capped at 10,000.

---

A revert of an axis edit can drop or rewrite value order, which changes the default, and still come back as success or "already undone." History shows that edit; the planner does not put it back.

## High

**Axis revert ignores order, so the default can stay wrong or land on a third list.** `tamber/src/history.rs:235` (`Axis.values` is ordered, and the first value is the default: `tamber/src/syntax.rs:38`).

`edits` records an axis change with `axis_text`, which joins values in order (`history.rs:418`). `revert` then only removes values the commit added and inserts values it removed (`history.rs:241`). A value present on both sides is left where it sits. If nothing was added or removed, the plan is empty.

- Reorder only. Parent `theme = light | dark`, commit `theme = dark | light`, file still that. `current` is not the before text, so it is not "already". No value is added or removed, so there are no ops and no conflict. `scripts/tamber-server.mjs:533` answers 409 "nothing to revert… undone already". The default is still `dark`.
- Mixed. Parent `a | b | c`, commit and current `c | a` (`b` removed, `c` moved first). The only op is `add-axis-value b` before `c`, which yields `b | c | a`. That is neither the commit's before list nor its after list. Accepting it makes `b` the default.

`removals_come_back_and_dials_added_cannot_go` only appends `navy`, and `scripts/tamber-server.test.mjs` only changes token values, so both stay green.

Fix: build the target list (the commit's before order, plus values added after that commit), and emit `remove-axis-value` / `add-axis-value` until that list is reached. If `axis_text` of the result would not be the before text, conflict with the axis name instead of returning an empty plan.

## Medium

**Adding or removing a token treats "same values" as "same token", so kind and metadata are dropped.** `tamber/src/history.rs:400` (`same_token`), used at `:299` and `:315`.

`same_token` compares branch values only. Kind and metadata are not an `other` edit on an add or a removal, because that edit is emitted only when both versions have the token (`history.rs:135`).

- The commit adds `card: color = #303030`. A later hand edit sets `scheme=fixed` and leaves the value. Revert sees the same values and `remove-token`s `card`, including the metadata.
- The commit removes `ink: color = #000` with `scheme=fixed`. Someone adds `ink = #000` back with no kind and no metadata. Revert marks that edit `already` and does not restore `color` or `scheme=fixed`. If that was the only edit, the wire says nothing to revert.

Fix: treat `token_shape` (kind and metadata) as part of identity. If the token is present but the shape differs, conflict "changed since" and do not `remove-token` or mark it already.

**A file whose git path is quoted loses its diff, and an unquoted prefix can steal another file's diff.** `scripts/tamber-history.mjs:84`.

The diff is joined with `part.startsWith(`diff --git a/${repoPath} `)`. Git quotes paths that contain a space, a non-ASCII character, or a quote (`diff --git "a/Brand Dark.tamber" …`), so the prefix never matches and `diff` is `''` while the token edits (from `cat-file -z`) are fine. With `core.quotePath=false`, `brand.tamber` matches the header of `brand.tamber copy.tamber` first and takes its diff. The history test only uses `app.tamber`.

Fix: take the diff from `git diff -z` (NUL-separated paths) and key it by exact path, not by a textual prefix of the default header.

**`GET /__tamber/tool/…` follows a symlink out of the tool directory. Any local request can read it; the token is not required.** `scripts/tamber-server.mjs:269`.

`..`, a leading dot, `\`, and NUL are rejected, and the lexical `startsWith(toolDir)` check holds. `resolve` does not resolve symlinks, and `statSync` / `readFileSync` (`:272`, `:274`) follow them. A build output with `assets` → `~/.ssh` makes `/__tamber/tool/assets/id_rsa` return that file to any loopback client. The test checks encoded `..` and a remote address, not a symlink.

Fix: `realpath` each candidate and require the result to stay inside `realpath(toolDir)`. Refuse a path whose `lstat` is a symlink.

**A compare that exceeds the 1 MiB stdio line is reported as a successful history with no edits.** `scripts/tamber-server.mjs:510` and `:176`.

`history` and `revert` send every file's before and after text for one commit on one line. `ask` refuses at 1 MiB (413). `revert` returns that status. `history` still returns 200 and, for every file in that commit, `edits: null` and `refused: "a request is at most 1 MiB"`, the same shape as a version that does not parse. One commit that touches enough `.tamber` files blanks the token edits for all of them; the raw diff is still there. The new tests use files of a few dozen bytes.

Fix: compare one file per request. If `compare` itself fails, return that status for the history call instead of a 200 with null edits.

## Low

**The wire's DTCG import is one file, so a multi-file source is not the import the CLI runs.** `contract/cli/src/tamber_import.rs:145`.

`import-plan` calls `dtcg::import` with `[(name, source)]`. Figma modes and Tokens Studio sets are several JSON files, and a resolver `$ref` looks up siblings (`tamber/src/import/dtcg.rs:649`). One Figma mode file is read as a single base layer, so the other modes never become an axis. shadcn and Tailwind are one stylesheet, which matches `{kind, source}`.

Fix: accept `files: [{name, text}]` on `import` and pass that slice through, still under the line limit.

**`history`'s `skip` is an unbounded integer, interpolated into `git log`.** `scripts/tamber-server.mjs:504`, `scripts/tamber-history.mjs:99`.

`limit` is capped at 100, which blocks `--max-count` option injection. `skip` only has to be a non-negative integer, so `skip: 9007199254740991` becomes `--skip=9007199254740991` and `git log` walks the repo on the server's thread (`spawnSync`). `limit` and `skip` are not otherwise injectable: `revert`'s revision is rejected if it is empty, starts with `-`, or contains whitespace or NUL, and it is passed after `--end-of-options`.

Fix: cap `skip` (for example at 10000) before building the git argument.

## Verdict

Do not land it until an axis revert restores value order or refuses, instead of dropping the edit or writing a third list.
