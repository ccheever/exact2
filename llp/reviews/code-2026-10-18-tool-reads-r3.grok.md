# Code review: the design tool's reads during a scrub, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the net change after the redesign.
- **Method:** one brief (sha256 `6b33a16fcee0556d7286b18c4ab5f09a1133e817529ff73df364347c7d54f400`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (a release whose write throws never tells its end; the bystander draft; no test; a nit on hello retries).
- **Disposition:** fixed in round 3's commit.

---

I'll review the net design-tool change from `HEAD~3` to `HEAD` and check the scrub-open tracking against the server event shapes. Gathering the commits, the diff, and the scrub code first.The net diff is small. Next I’ll read how `paced` is computed and how the server emits scrub open and end events.Nothing here is blocking. Every normal scrub end emits one `scrub` event with the same id, and a crash or a gap drops the tool out of paced mode. Two holes are worth fixing, and the new behavior has no test.

1. **should-fix** — `scripts/tamber-scrub.mjs:218` and `apps/design-tool/app.ts:1128`. A Keep that throws after the session is cleared never emits an end, and the tool then stays in "scrub open" until a new run or a gap.

`release` sets `open = null` and only then awaits `released()` and `write()`. `ended({ scrub: id, … })` runs after that, and it is not in a `finally`. If `released()` throws (the compiler pipe is dead) or `write()` rejects (`stdin.write` throws), no `scrub` event is sent. `end()` itself is fine: it sends `ended` with `was.id` whenever the scrub had previewed (`tamber-scrub.mjs:101`), and `opened` is sent with that same id only once a preview has succeeded (`tamber-scrub.mjs:146`, wrapped at `scripts/tamber-server.mjs:267`). Idle, mismatch, cancel, a competing write, disconnect, server-lost, a successful Keep (`released`), and a refused Keep (`refused`) all go through one of those two sends.

The tool only removes an id when a later event names it (`app.ts:1136`), or it clears the whole set on a new run or a gap (`app.ts:1128`). A `down` event does not clear it. So after that thrown Keep the server has no scrub, but `paced` stays `scrub <fileChanges> <commitsTold>` for the rest of the process. hello, the dry commit, and uses go back to moving only on a file change or a git commit — the key round 2 put back because it misses a live flip, a source-only edit, and a page with no plan. A later write refreshes once (the counter moves) and then pacing sticks again. The 64-id cap never fires for a single ghost.

Server down does not stick on its own. `fail` calls `scrubs.lost()` while `open` is still set, so a shown scrub gets `server lost` before the `down` event, and a new process has a new epoch, which clears the set. A ring-buffer gap is `missed` with no events, which also clears it (the tool then over-reads until the scrub really ends, which is the safe direction).

Fix: in `release`, `try/finally` so `ended` always runs once `open` has been cleared, including when the write throws. On the tool, replace the set from the server's current scrub on each answer (`scrubs.current()` is already there), and clear it when the answer carries `down`. Replaying events cannot heal an end that was never sent.

2. **should-fix** — `apps/design-tool/app.contract:702` (and the same assignment in `reconnect` at `715`). A second tool that has a draft open re-proposes on every build status during someone else's scrub.

`proposeAt` is held only while this tool's own `scrubShown` is true. Another client's scrub sets `scrubsOpen` but not `scrubShown`, so `proposeAt` still becomes `changes.answer` on every poll. `proposeEdit` then calls `propose` whenever a draft has a kind and a target (`app.ts:868`). Those calls share the design server with the scrub's fold, which is the load this change removes for the scrubbing tool. The chip can wait: a second scrub is refused while one is open, Apply writes the reviewed ops against digests that do not move until Keep, and Keep moves `filesAt`, which re-proposes anyway.

Fix: update `proposeAt` when `changes.paced` changes, including while `scrubShown` is false. Outside a scrub, `paced` changes every answer, so the proposal still follows the poll. Inside one, it moves on a file change, a commit, and the transitions in and out.

3. **should-fix** — `apps/design-tool/app.test.contract:458`. None of the three commits touch a test, and the only nearby one still only checks that a status plus an `opened` event does not re-read the design (that key was already `filesAt`).

It injects `reason: "opened"` and never an end, so it now also turns pacing on, then asserts nothing about hello, the dry commit, or uses. Nothing covers the one refresh on open and on end, a `changed` or `committed` event while the set is non-empty, a second tool's proposal, or a gap or a new epoch clearing the set.

4. **nit** — `apps/design-tool/app.contract:695`. A failed hello during an open scrub is asked again on every following status, because `not hello.ok` is separate from the paced comparison. One timeout at the start of a scrub puts a hello back on each build status until one succeeds. Retry when the previous hello has settled, not on every answer.

What holds:

- Own and other clients' scrubs share one set. The scrubbing tool additionally freezes its own proposal via `scrubShown`.
- Entering and leaving change `paced` between `answer <n>` and `scrub <files> <commits>`, so hello refreshes once there. `advance` records `pacedAt` after that comparison; `reconnect` only snaps it to the answer already in hand and still refreshes hello. uses and scope follow `changes.paced` directly, so that snap does not skip their transition read.
- A Keep's `changed` is published before `ended`. In one answer the set is empty at the end and the new digest is in `files`, so the design, hello, scope, and uses all refresh once. Split across answers, `fileChanges` moves `paced` while the id is still held, then the end refreshes again.
- A hand edit calls `compete()` (end event, same id) and then publishes `changed`, both before the poll answers. Another client's git commit is a `committed` event, which increments `commitsTold` and refreshes scope without ending the scrub. Another client's write ends the scrub first.
- `hello.page` is fixed for the process. uses of a token whose sources move either end the scrub (a new plan calls `compete()`) or are published on a map the end-of-scrub read then picks up; the per-answer reads resume after that.
