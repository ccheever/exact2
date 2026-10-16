# Code review: LLP 1122 T1 commit 5 (the changeset, commit and history), round 1, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `81011fafc`.
- **Method:** one brief (sha256 `824dcc2cda00345b9179cb25323dfcfd1b97797c9c6a7032c0ded610a64a8504`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 high, 3 medium, 1 low).
- **Disposition:** all taken, as in the astra record; a second Commit is off (nothing open), and the message is built from the server's scope, never from hand edits.

---

The changeset records writes, but the before/after text, own-write diffs, and a server restart are wrong in cases the tests never hit. The commit and history paths are closer to the brief.

## High

**Before/after is one net delta copied onto every write in the window, then frozen** — `apps/design-tool/app.ts:367`

`valuesFor` compares only the previous stored read with this one, and applies that pair to every changeset entry with `last.since < seq <= since`. A design refresh after Apply does not move the baseline: while `since` is unchanged it returns immediately (`app.ts:372`), so those intermediate reads are discarded. The next poll often covers several events (another Apply, an agent, a hand edit) and every one of them is labeled with the same net change. If any named token differed, that list is stored and never recomputed (`n.values !== null`), including when some of the event's tokens belong to a file this read did not open.

Scenario: the baseline is `tone.base` at hue 200. You apply 210, then an agent sets hue to 220, before the next read finishes. Both entries say `tone.base oklch(…200) → oklch(…220)`. Commit sends that sentence as the git message (`app.ts:571`).

Fix: keep each design read keyed by the file digest `hello` just saw. Give an entry values only when this read's digest equals that event's `files` digest and the previous read equals the digest before it. If several events share one window, fill a token only when a single event in the window names it; leave the others unset so a later aligned read can fill them. Do not treat "some tokens moved" as "this entry is fully measured."

**A hand edit is permanently "edited", with no token values** — `apps/design-tool/app.ts:347`

The watcher announces a hand edit as `changed: null` (`scripts/tamber-server.mjs:423`). `noted` stores `tokens: []`. On the next read, `[].every(...)` is true, so `values` becomes `[]` and `plain` (`app.ts:384`) shows `app.tamber: edited · changed on disk`. The two design reads already hold the real token delta, and they are never consulted. README and D9 both ask for those values.

Scenario: with the file on screen, someone changes `tone.warm` in an editor. The row never says `tone.warm oklch(0.7 …) → oklch(0.71 …)`. If your own write is measured in that same window, `tone.warm` is on neither event's token list, so the change disappears.

Fix: when an event names no tokens, diff the union of the two snapshots (added, removed, and the first tuple that moved). Same rule as above for not freezing a partial result.

**A new server epoch keeps the old changeset and then mis-measures** — `apps/design-tool/app.ts:554`

An epoch change clears undo and `mine`, and it does not clear `changeset` or `measured`. Sequence numbers start over. `valuesFor` then hits `since <= last.since` and returns without updating the baseline (`app.ts:372`), so later writes get no before/after until the new sequence passes the old one. Rows reuse `key={e.seq}` (`app.contract:563`). Commit still turns every uncommitted row into the message (`app.ts:571`) and, on success, marks every row whose file was committed (`app.ts:578`, and the `committed` event at `app.ts:341`).

Scenario: the server restarts (the undo comment already treats this as a new run, possibly another app on the port). The panel still lists the previous app's edits. Commit either says the new server wrote nothing, or it commits the new session under the old wording and greys out both sessions' `app.tamber` rows.

Fix: on epoch change, drop `changeset` and `measured` the same way undo is dropped, and start the baseline from the next design read.

## Medium

**Own-write diffs are matched by client name, in a queue** — `apps/design-tool/app.ts:346`

`mine` shifts whenever `e.client` equals the oldest queued client. The client string is `design-tool:${person}` (`app.contract:217`), persisted and shared by every tab. A `missed` poll delivers no events (`scripts/tamber-server.mjs:328`) and does not clear `mine` (only an epoch change does, `app.ts:554`). The first poll also skips `noted` entirely (`if (epoch)` at `app.ts:558`) while a write made before that poll still queued a diff.

Scenario: two tabs, both Ada. Tab B applies a change. Tab A's event arrives first, both with client `design-tool:Ada`. Tab B attaches its proposal diff to A's row and shows its own write as someone else's, with no diff. After a missed gap, the next own write gets the same off-by-one for the rest of the session.

Fix: match a queued write to the event whose `files` digest equals the digest in that write's reply (and whose `client` is this page's), not the next event with the same name. If a poll is `missed`, or the first answer is applied, drop `mine` with the events you did not record.

**History's Older button stops at 100 commits** — `apps/design-tool/app.ts:584`

`moreHistory` only increments `historyPages` (`app.contract:381`). The request is `limit: min(100, 10 * pages)` and never sends `skip`, which the server already accepts up to 10,000 (`scripts/tamber-server.mjs:549`). Past 100 design commits, `more` stays true and every Older click repeats the same page.

Scenario: a repo with 150 design commits. The oldest 50 cannot be opened or reverted from the tool.

Fix: page with `skip` (`(pages - 1) * pageSize`) and a fixed `limit`, and hide Older when a page comes back short.

**Commit stays armed after it succeeded, and the message lists edits git will not contain** — `apps/design-tool/app.contract:569`

The button is disabled only when the list is empty, not when every row is already committed. A second click calls `commit` again; the server answers `the session has written nothing to commit` and that error replaces the "Committed …" summary. The message is built from every uncommitted row (`app.ts:571`), including `changed on disk` rows for files this session never wrote. Those are not in the server's commit set, but a successful commit still records them in the message (`design: 2 changes` when git has one file).

Scenario: you commit the hue edit, the summary appears, you click Commit again. The summary is gone and the button shows a refusal, while `HEAD` did move on the first click. Or a hand edit to `brand.tamber` sits in the list beside a real session write to `app.tamber`: the commit contains only `app.tamber`, and the message claims both.

Fix: disable Commit unless some row is uncommitted, and build the message only from those rows whose author is not `changed on disk`. Keep the previous success on screen when the server reports that the session has written nothing.

## Low

**D9 asks for the lines under every edit; only this tab's proposals get a diff** — `apps/design-tool/app.contract:566`

That matches the brief for the tool's own Apply, and it leaves undo, redo, other clients, and hand edits with no lines. Undo and redo queue `diff: ''` (`app.ts:511`, `app.ts:527`). Combined with the measurement bugs, those rows are a file name and a person.

Fix: once before/after is trustworthy, show it for every row; keep the proposal diff under the write that produced it. Undo can show the forward entry's diff as removed lines.

The tests do not catch these. `scripts/design-tool.test.mjs` commits one write after the read has settled, so the net delta equals that write, and it does prove the gate you named: dirty, staged, and hand-edited-between-writes are refused with the server's reason, other staging stays, and the remote does not move. History and revert are checked for that one commit. `app.test.contract` asserts the other client's row as `surface.card · agent:codex` with no values, against a stub whose resolve data never changes. Nothing covers two events between reads, a hand edit's token text, a new epoch, Older past 100, or a second Commit.

Verdict: the commit and refusal wiring matches the gate, but the changeset text is wrong for more than one write between reads, for hand edits, and after a server restart, and those are the rows Commit writes into git.
