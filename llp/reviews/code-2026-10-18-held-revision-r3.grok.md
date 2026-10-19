# Code review: tamber/1.12, the long poll's held revision, round 3, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the held-revision commit and rounds 1 and 2.
- **Method:** one brief (sha256 `5597f47847187ac5849e339ac9fc6af9a4dbc67e9bb05e07912eae447becb178`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (nits: the open-list sentence; QUEUE names 1.11).
- **Disposition:** fixed in round 3's commit.

---

Nothing blocking. One answer's `held` and `proposals` stay a single snapshot, and the design tool's revision state does not drop or freeze the list on the paths below. The version bump is complete on the surfaces that report the current protocol.

Checked and holding:

- `answer()` in `scripts/tamber-server.mjs` reads `heldRevision` and `held()` synchronously, and `json()` stringifies before it returns. A long poll woken from `send()` runs that function as a microtask after the current mutation burst (hold's insert, eviction, accept's publish-then-settled, an expire sweep), so the pair matches the map at the end of that turn.
- A poll parked across `accept`'s await sees the proposal already removed and the revision already bumped. Success sends `proposal-dropped` without a second bump; a client that stored the post-delete revision already lacks it. Failure restores and bumps, so the next poll refetches. `recompute` inserts the replacement before `replaced()` emits the old drop; the tool replaces the list wholesale from `proposals`.
- Down: a parked poll woken by `fail()` takes the 200 path and includes the `down` event. The next poll, with nothing after that seq, is 503 and carries no `held`. The tool returns on `error` before touching `heldAt`. A restart is a new epoch, and an epoch mismatch always sends the full list.
- The tool sends `held` only when its stored epoch matches the caller's. The first poll sends none. An error or a superseded poll (the check before the list update) leaves `heldAt` alone. A rename does not reset it; the list is global. An omitted `proposals` keeps `heldRaw` and redraws only when `Math.max(...snapshots.keys())` moves, which is the same key `reviewOf` reads. Keys only grow.
- `hello` is `tamber/1.12` in `contract/cli/src/tamber_serve.rs`, `docs/tamber-protocol.md`, and the Rust, server, and design-tool tests. Hosts, the agent, `docs/recipes/tamber.md`, and the LLP §10 notes do not claim 1.11 is current. The remaining `tamber/1.11` strings are the stage-8 history line, the param-rung test's "added in 1.11" comment, and §10's stage-8 commit note. Stage 9 already called 1.12 the next window.

1. **Nit** — `docs/tamber-protocol.md:579`. The sentence says the open list rides every long poll as `proposals`. Two sentences later, and again on the `proposals?` shape at line 772, a poll that names the current revision and this epoch gets no `proposals`. A client that implements only the first sentence (`list = body.proposals ?? []`) clears its list on every quiet poll. **Fix:** put the omission in that sentence: `subscribed` always carries `proposals`; a long poll carries it unless `held` names the current revision for this epoch.

2. **Nit** — `QUEUE.md:7`. The batch-`resolve` note still says a new optional field would land under `tamber/1.11`. That window is closed; the protocol page says later additive stage-9 fields share 1.12. This is not a `hello` claim. **Fix:** name `tamber/1.12` as the version that field would be added under.
