# Code review: LLP 1121 stage 3, commit 4, round 3, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `e486daba1711c1f69f3c26ce3bb1362446057bf40138b9b890d2e1be704a58cf`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: three medium; Grok: one high, one medium, one low.
- **Disposition:** applied without re-review

---

**High** — `scripts/tamber-server.mjs:146`. A failed first `hello` still clears `first` before the result is checked, and `refresh` then returns. `ready` is that first call, so it resolves on the failure. `known` stays empty, and nothing sends another `hello` until a watcher fires.

That failure is normal on a cold start. `spawnContract` is `cargo run`, and `ask` gives up after 120s (`ANSWER_MS`), while a cold compiler build is allowed 600s in the tests. The late reply is dropped because the waiter was deleted. When a later `hello` succeeds, `first` is already false and `known` is empty, so `publish` reports every design file as a hand edit. Until something on disk changes, a subscriber's `subscribed` event has `files: {}` for the rest of the process.

Only clear `first` after `reply.ok`, and retry a failed `hello` on a timer while the child is up. Resolve `ready` only once that baseline is stored, or once the child is permanently down.

**Medium** — `scripts/tamber-server.mjs:231`. A restarted cursor is `missed` only while `after > seq`. This process's `seq` starts at 0 with no epoch. After enough new edits that `seq >= after`, the ring looks contiguous (`events[0].seq <= after + 1`), `missed` stays false, and the client is told the gap is complete. Those events are a different history. On the `after > seq` path the body is the whole ring (`events`), whose sequence numbers are all `<= seq < after`, not events after `after`. A client that applies that array replays this process's buffer under cursors it has already passed.

Put a per-process epoch on `subscribed`, every event, and every poll. `missed` is true when the epoch differs, when `after > seq`, or when the ring has a gap. In those cases return `events: []` and the current `files` map so the client replaces its snapshot.

The other round-2 fixes hold:

- `before` merged over the unwritten files announces a hand edit of a file the write then changed at the pre-write digest, with `client: null`, and the writer's event second. A hand edit of a different file stays on its own event.
- `watchAll` on a missing directory gets a synchronous `ENOENT` from `fs.watch`, does not record that directory, and tries again on the next successful `hello`. `appDir` and the design's directory are armed before the first read.
- `design()` takes one snapshot and returns an error for a file that is not in it. `files()` surfaces a `read_dir` failure. `hello` omits an absent design and the stream's `gone` list drops it. An oversized `Content-Length` destroys the socket. A client that leaves during `await ready` is not written. `close()` escalates to `SIGKILL` and logs if the child is still alive.

**Low** — `scripts/tamber-server.mjs:284` and `scripts/tamber-server.test.mjs:253`. The 5s `setTimeout` in `close()` and the test's `Bun.sleep(30_000)` are not cleared when the race is won by exit, so the process stays alive for the loser (5s after `close()`, 30s after the dev-loop test). Clear the timer, or `unref` it, when the child has exited.
