# Code review: LLP 1121 stage 3, commit 4, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `08203a2830d7df045821f2a4bd0d0a0c223302161cf418e659e50b0d7b6a4a1e`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: six medium, one low; Grok: two medium, four low.
- **Disposition:** fixed in round 2; commit 5's own events (held proposals) are commit 5's

---

No high findings. Two medium, four low. The round-1 fixes hold, and the races, the long-poll close path, `-pid`, Windows `detached: false`, and `exits.push(tamber?.close())` do too.

## Medium

**`files()` treats a directory-read failure as “no siblings.”** `contract/cli/src/tamber_serve.rs:461`

Startup used to be `read_dir(&dir).map_err(...)?`. The per-request list drops that error with `.into_iter().flatten()`, so a failed `read_dir` yields only the design name. `hello` then succeeds with that partial map, and `refresh` publishes every sibling in `gone` (`scripts/tamber-server.mjs:148`). Journal validation uses the same list (`tamber_serve.rs:422`): a real sibling target becomes `` `brand.tamber` is no file of this design `` and the server refuses to start, journal kept. Per-entry `e.ok()?` was already there; the new hole is the directory open itself.

Return `Result` from `files()` and fail the hello, the snapshot, and journal parsing on that error.

**A missing design file sticks the stream.** `contract/cli/src/tamber_serve.rs:472` and `:481`, `scripts/tamber-server.mjs:145`

`files()` always yields `self.design`, even when the file is gone. That is load-bearing: journal recovery needs the name in the list so `contains` passes and a crash with nothing landed can withdraw. `snapshot` turns that absence into `{message: "app.tamber is gone"}`, `hello` fails, and `refresh` logs and returns. Nothing is published, `known` keeps the last digest, and every later hello fails the same way, so sibling edits during the gap are silent too. A checkout that removes the design never becomes an event. A sibling that disappears is omitted from the list, the snapshot succeeds, and `gone` is published. A sibling that vanishes between `read_dir` and `read` fails one hello and the next rescan heals it, because the name is no longer listed.

Keep the design name on the journal list. Have `hello` report a `NotFound` design as absent (other IO errors stay errors) so the wire can publish it in `gone` and keep scanning.

## Low

**A restarted sequence is not `missed`.** `scripts/tamber-server.mjs:218`

`missed` is `after < seq && oldest > after + 1`. After the dev process comes back, `seq` is 0. A long-poll with the previous `after` gets `missed: false`, `events: []`, and then waits 25s whenever it retries with that `after`, because nothing in the buffer has a greater sequence. The body does include the lower `seq` and the current `files`. The dev loop next door already exists for this: a restarted `seq` of 1 must be newer than the previous server’s `seq` of N (`host/web/dev.mjs:239`). Treat `after > seq` as missed, or send the same kind of epoch on `subscribed` and on the poll.

**The Content-Length ceiling does not cut the socket.** `scripts/tamber-server.mjs:177`

The comment on `body()` says the connection is cut. The streaming path calls `req.destroy()` once the accumulator passes the limit (`:182`). The `Content-Length` path returns null and leaves the body unread. On Node 26 a fully buffered body still lets the next request on that connection parse. A slow body larger than the stream high-water mark is never consumed, so the parser stays paused on that one socket. The bridge itself is no longer stalled: the 413 is decided before `stdin.write`. Destroy, or drain and discard, on the fast path too.

**Commit 5 events emitted as Rust `event` lines will be dropped.** `scripts/tamber-server.mjs:120`

`if (v.event) continue` is right for today’s writes: `commit_reply` emits a `changed` line (`tamber_serve.rs:546`) and the wire republishes from the reply, using `written`. Held proposals and attribution (LLP 1122 D10) are announcements with no digest change. `publish` only sends when something was edited or `rest.client` is set (`tamber-server.mjs:91`), and a client-null proposal therefore produces no event. `METHODS` is a closed set (`:41`); `commit` has to be added there, and it should stay out of `WRITES` so a git commit is not treated as a design-file write. The conflict object is still `{conflict, message, files}`; naming who moved the file (D9) can be added without breaking this shape. `hello.builds` is `[]` until stage 4.

**The dev-loop test can bind the wrong port, and its cleanup can leave the process.** `scripts/tamber-server.test.mjs:223` and `:246`

It listens on port 0, closes, then spawns the dev loop on that port. The `until` helper is bounded (20ms poll, throw at 5s, `:124`), so the suite no longer hangs on a missed event. The port grab is a gap another process can win, and `finally` races `SIGTERM` against 30s and then returns either way, so a stuck `stop` leaks the dev loop. Pass `port: 0` through to the dev loop, or hold the listener until the child has bound, and kill the process group if `stop` does not exit.

## Checked, and holding

Round 1 is in the code. `op` sets `redirect: 'error'` on the token fetch, subscribe, and the post (`scripts/tamber-server.mjs:316`, `:321`, `:332`). A write whose `inverse` is a string publishes the other files with `client: null` before the writer’s event (`:245`). `hello` runs again on each rescan, and the watch accepts the design’s basename (`:154`). A request at the 1 MiB ceiling is refused before it is written to the child (`:133`); an id-less reply goes to the oldest waiter (`:123`); asks time out at 120s (`:135`). `fail` is one path: `down`, subscribers ended, later requests 503 (`:96`). Writes with no `client` are 400 (`:236`). The long poll exists beside subscribe (`:212`). `serveAlone` polls `EXACT_LAUNCHER_PID` and stops on `ESRCH` (`:301`). Both dev loops await `close`.

`refresh` and writes share one stdin pipe, and reply continuations run in reply order. A hello queued ahead of a write publishes the older snapshot first; `known` then takes the write. A hello behind a write sees the digests the write already stored and sends nothing, because its `client` is null and nothing new is edited. `writing` / `again` defer the watcher across the server’s own rename, so that rename is announced from the reply only. The first hello is the baseline, and watches are installed only after it returns, so it cannot race a second refresh.

Long-poll missed-detection is right while `seq` only increases and the buffer stays contiguous: caught up (`after === seq`) is not missed, and a gap older than the 256 kept events is. Registration is synchronous between the `events.some` check and `polls.add`, so an event in that gap wakes the poller. `Set` deletion during `send`’s iteration is safe. On this Node, aborting a poll sets `destroyed` with `writableEnded` false, so the guard skips `answer()`. A disconnected subscriber’s `res.write` returns false and does not throw, and `res.end()` after close does not throw, so one dead client does not skip the rest of the fan-out. Many pollers are uncapped; each holds a socket for at most 25s. That is acceptable on a token-gated dev server. Same for uncapped subscribers and in-flight asks. The caps that exist: 256 events, the Rust inverse cap, a body just under 1 MiB, 120s per ask, 25s per poll.

`spawnContract` uses `detached` only off Windows (`:63`), so the child process group id is its own pid. `process.kill(-child.pid, 'SIGTERM')` (`:270`) signals that group and not the dev loop or the compiler groups, which are separate `setsid`s. `close` checks `exitCode` / `signalCode` / `down` before the kill. On Windows `detached: false` is the right call (a detached spawn would be a new console), `kill(-pid)` throws and the `catch` falls through to `child.kill`, and the server returns before taking the lock on non-Unix (`tamber_serve.rs:228`). `close` waits at most 5s and does not escalate to `SIGKILL`; an uninterruptible flush can hold the lock past that. The listen-error and compiler-exit paths (`host/web/dev.mjs:395`, `:542`, `:572`, `:577`, `:1193`) call `process.exit` without `tamber.close()`. The child’s stdin is the pipe `spawn` opened, so that exit still delivers EOF and the Rust loop drops the lock.

`exits.push(tamber?.close())` (`host/web/dev.mjs:745`) matches the array: `stop` builds exit promises for the compiler children, `killCompiler` signals those groups, and `Promise.all` counts a non-promise. With no design, `tamber?.close()` is `undefined` and the wait still settles. With a design, `close` resolves on child exit or at 5s. The JS loop has no `exits` array; `await tamber?.close()` (`host/web-js/dev.mjs:278`) is the same wait. The resident `--serve-as` child does not mount a second wire (`host/web/dev.mjs:245`).

An inverse whose file is no longer in the snapshot conflicts and is not applied (`tamber_serve.rs:705`). The primary file disappearing fails `snapshot` first, so the inverse is not applied either. A sibling that is already gone at startup fails journal validation the same way the old frozen list did. The read-error swallow above is the new part.
