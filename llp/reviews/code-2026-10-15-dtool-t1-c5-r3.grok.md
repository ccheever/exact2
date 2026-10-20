# Code review: LLP 1122 T1 commit 5 (the changeset, commit and history), round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f42acb24d` (commit `81011fafc` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `3a39a300bd957a5be0e5c35b04bd80aa88cb650c21832eeac2a6583029d1b0e7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 high, 2 medium, 1 low).
- **Disposition:** all taken, as in the astra record: edits matched by write number (a multi-file write's row describes each of its files in that file's words), the dry run bound to the poll's run, removed modes said, the success list keyed by position.

---

The earlier fixes hold: per-file digests, restart and gap baselines, matching this tab's own diff by write number, history `skip`, sequence-bounded committed marks, the 200-row cap, and a dry run that returns `scope` and refuses when the files are already HEAD. Four problems are still open. The gate checks in `scripts/design-tool.test.mjs` still drive a real server through the proxy. I read them and did not re-run the suite.

## High

**An edit with no changeset row takes a later row's before and after** — `apps/design-tool/app.ts:843`

`describe` walks the server's edits from oldest to newest and gives each the oldest unused row with the same client, the same file, and any shared token. A write from before this page opened, or one dropped by a missed poll, has no row. That edit still matches, and it consumes the row that belongs to a later write. The later write is left as a bare token name. `session.wrote` stores `{ client, tokens }` only (`scripts/tamber-commit.mjs:132`); the write number assigned at `scripts/tamber-server.mjs:433` never reaches the dry-run edits, so the row's `write` cannot be used.

An agent sets `tone.base` from 200 to 210, then the tool opens and drops that event. The agent sets 210 to 220. The panel's one row is `210 → 220 · agent:codex`. Commit's dry run lists both edits. The git message's first bullet is `210 → 220` for the edit that was 200 to 210, and the 210 to 220 edit is only `tone.base`.

Put the write number on each session edit and on the dry-run and commit payloads. Describe an edit from the row with that write number, using that file's sentence, and let one multi-file row cover each of its files. An edit with no row keeps the token-and-client fallback.

**Commit's dry run still follows a token refresh onto a replacement server** — `apps/design-tool/app.ts:837`

The committing call is bound to `dry.used` (`app.ts:852`), and `session !== at` is checked only after that call returns (`app.ts:854`). The dry run itself uses `call` with no bound token, and a bare 404 retries once onto whatever now holds the port (`app.ts:72`). `loadScope` does the same (`app.ts:818`). The old changeset is cleared only when the poll applies the new epoch, which is after that retry.

The server is replaced, the open poll fails, and the cached token is cleared. Before the poll's retry lands, another tab of Ada writes `tone.base` from 180 to 190. Commit dry-runs the new server, `describe` still sees the old row `200 → 210`, and the bound follow-up commits the new server's file under that sentence. The panel then shows the commit as successful.

Bind the dry run and `loadScope` to the token the poll is on, so a dead token is a restart. If `session` has already moved, return before sending the committing call.

## Medium

**One multi-file write is two server edits and one row, and the first edit consumes the row** — `apps/design-tool/app.ts:844`

A transaction that changes `app.tamber` and `brand.tamber` is one changeset row and two edits. The first edit takes the row, and `plain` prints both files in that one bullet. The second edit finds nothing left and falls back to token names, so the heading says `2 changes` and the brand values sit inside the other change's bullet.

Same fix as the write-number match: one row may describe each of its files, and each bullet is that file's sentence.

**A mode present only on the before side is dropped** — `apps/design-tool/app.ts:548`

`moved` walks the after side's tuples. An added mode is reported (`then === undefined`). A mode that existed only before is never visited, and if every remaining tuple is unchanged the function returns null. A hand edit then records no token at all (`app.ts:568`), so the row says `edited`.

With the file on screen, delete `surface.inset`'s `when density=compact` branch and leave the default. The row says `app.tamber: edited · changed on disk`. `surface.inset` is absent.

After the after-side loop, report each before-side key that the after side does not have as a removed mode.

## Low

**The success list keys bullets by their text** — `apps/design-tool/app.contract:804`

`for t in c.edits key=t` collapses two identical sentences into one. Reload after two edits of `tone.base`, then commit: the changeset is empty, both bullets are `app.tamber: tone.base · design-tool:Ada`, the git message has both lines, and the panel shows one. Key the list by index.

The tests do not catch these. `scripts/design-tool.test.mjs:416` accepts any `oklch(…) → oklch(…)`, so a swapped baseline still passes. The restart test (`apps/design-tool/app.test.contract:282`) checks that a row and a highlight exist, and the stub's resolve data does not change with the digest. The stub's commit ignores `scope` (`apps/design-tool/stub/tamber-stub.ts:64`). Nothing covers two edits of one token where the first has no row, a dry run after the token has changed, or a removed mode.

Verdict: Request changes — a commit can still store one edit's before and after on a different edit, and a dry run can still commit a replacement server's work under the previous run's wording.
