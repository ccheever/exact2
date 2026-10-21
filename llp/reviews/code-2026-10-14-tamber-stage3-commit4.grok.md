# Code review: LLP 1121 stage 3, commit 4, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `06a5448b65e0d7385cf0228519dd8e1c08ef626d6fecb778db98bd79b4a661ee`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, nine medium, three low; Grok: three medium, five low.
- **Disposition:** fixed in round 1 (no redirects for the token, writes split from hand edits, the file set asked again, a bounded bridge, one failure path, a long poll for buffered clients); exactStream's coalescing is LLP 1016 D4's, not changed

---

## Medium

**[medium] A request at the 1 MiB ceiling hangs, and a write then stops the change stream — `scripts/tamber-server.mjs:128`**

The wire accepts a body of exactly 1 MiB (`size > BODY_LIMIT` at line 130), then sends `JSON.stringify({...request, id})`. That line is longer than the server's 1 MiB limit, so `contract tamber serve` answers `{"id": null, "error": ...}` (`contract/cli/src/tamber_serve.rs:78`). The reader drops any line whose `id` is not in `waiting` (`scripts/tamber-server.mjs:80`). `ask` never settles. For a `transaction` or `undo`, `writing` was already incremented (line 161) and the `finally` never runs, so every later `rescan` hits `if (writing) { again = true; return; }` and returns without reading. Hand edits and checkouts stop producing events for the rest of the process, and the HTTP request never completes. Bun's 5-minute `requestTimeout` does not apply once the body has arrived.

Fix: if the encoded line would exceed 1 MiB, answer 413 and do not write it. Treat an `id: null` line as a failure of the oldest waiter. Give `ask` a timeout so `writing` cannot stick.

**[medium] A server that never starts, or that dies, still looks like a live subscription — `scripts/tamber-server.mjs:144`**

`ready` resolves even when hello fails (the lock is held, cargo exits). Subscribe still answers 200, sends `subscribed` with empty `files`, and adds the response. The exit handler (line 87) completes in-flight POSTs with 503 and does not write to subscribers or end them. A second dev loop, or a server that exits mid-session, leaves the tool's stream open and idle.

Fix: if hello has no `ok`, or `exited` is set, answer 503 and do not subscribe. On exit, write one error event and end each subscriber.

**[medium] A `.tamber` file created after hello is seen and then ignored — `scripts/tamber-server.mjs:107`**

The watcher is armed only for directories of files in the hello snapshot (lines 110–118). A new `extra.tamber` in that directory does fire `fs.watch` (the name is `extra.tamber`), and the suffix check lets it through. `rescan` only hashes `known.keys()`, so the create is dropped and the path is never added. The stdio server has the same hole: it collects `*.tamber` once at start (`contract/cli/src/tamber_serve.rs:287`). A file added while the dev loop is up never reaches a subscriber and is not editable until restart.

Fix: on a `*.tamber` name that is not in `known`, rescan the directory and send hello again. That only works if the server rebuilds its file list too.

## Low

**[low] The body reader neither cuts off an oversized upload nor finishes if the client leaves — `scripts/tamber-server.mjs:128`**

Past 1 MiB it sets `over` and keeps reading until `end`, so one token holder can hold the connection open until Bun's 5-minute `requestTimeout`. A client that aborts emits `close` without `end` or `error`, and `body()` never settles. `writing` is incremented only after the body is parsed, so this does not stick the watcher.

Fix: `req.destroy()` once over the limit, and settle the promise on `close`.

**[low] The suffix filter drops a design file that does not end in `.tamber` — `scripts/tamber-server.mjs:116`**

`app.json`'s `"tamber"` may name any file. Hello puts that path in `known`, but the callback returns immediately when the filename does not end with `.tamber`. A hand edit of `theme.json` never reaches `rescan`. Atomic saves of `app.tamber` are fine on this machine: the temp name is ignored and the `app.tamber` event still arrives.

Fix: rescan whenever the event's name is missing or is one of the known basenames. Digests already suppress no-op rescans.

**[low] A snapshot failure becomes a successful hello with `files: null`, and the watcher never arms — `contract/cli/src/tamber_serve.rs:524`**

`snapshot()` failure is swallowed (`unwrap_or(Value::Null)`), hello still returns `ok`, and the wire treats null as `{}` (`scripts/tamber-server.mjs:111`). Syntax errors still return digests. An unreadable file makes the change stream silently empty while clients see `"files": null`.

Fix: return that error from hello, and do not arm an empty watch on a failed hello.

**[low] Stop does not wait for the design process to drop the lock — `scripts/tamber-server.mjs:174`**

`close()` ends stdin and sends `SIGTERM` to the `cargo run` pid only. The binary is a grandchild. It does exit, because its stdin is that pipe and EOF ends the read loop, including after `SIGKILL` of the dev loop (the kernel closes the pipe). `host/web-js/dev.mjs:275` and `host/web/dev.mjs:744` then call `process.exit` without waiting, and the tamber child is not in the resident loop's exit list. A restart in that window is refused with "another tamber server".

Fix: spawn `detached`, kill the process group, and await `exit` before the dev loop exits, as the other cargo children do.

## Checked, and not findings

The token is minted with `randomBytes`, kept off disk, and returned only from `GET /__tamber/token` when `access.local` is true. Anything else gets `{message: "Not found."}`, the same body as the install endpoint. It is not written into URLs, logs, error bodies, or page HTML. `tokenOf` uses `timingSafeEqual` after a length check. The length is fixed (64 hex chars), so the early return does not leak the value.

The dev loops call `gate.check` before `handle`. `local` requires a loopback host, a loopback peer, and no `forwarded` / `x-forwarded-for` / `x-forwarded-host`. `--lan` and `--allow-host` are admitted and not local. Cross-site POSTs are refused by the gate. DNS rebinding under an unlisted name is refused.

A forwarded request cannot reach the token path. `host/web-js/dev.mjs` handles `/__tamber` before `forward`, and `forward` only covers `/exact.json`, `/__dev`, and generation paths. The resident loop is started with `--serve-as`, and `host/web/dev.mjs:245` mounts the wire only when `servedAs == null`. For an ordinary app, `web/dev.mjs` awaits the JS loop and never reaches its own `tamberServer`, so only one process holds the lock. The JS loop is the one that sees the real peer.

A client's write is published from the reply, which updates `known` before `writing` drops. A later rescan sees the same digests and does not publish again. One timer is shared across watched directories, so a checkout of several files in the same tick is one rescan. `main` in `scripts/exact.mjs` is not `async`, and the module `await`s the promise it returns, so `tamber serve` and `tamber op` stay alive until `process.exit`.

`host/web/dev.mjs` is 1207 lines. The 1500-line cap is not close. `scripts/tamber-server.mjs` is 234.

The POST stream is fetch-readable SSE (`data: ` + JSON, custom header, no token in the URL), which is what a same-origin JS data module can consume. There is no CORS header. LLP 1122's tool has to be served by this same origin. A second origin will fail the preflight. Commit 5 can pass new fields through, but `commit` and any other new method are rejected until they are added to `METHODS` (line 30).

The tests never cover a body near 1 MiB, a lock already held, a file created after hello, or the server exiting under a live subscriber. The "announced once" test waits 400 ms and asserts a count. That passes if a late duplicate arrives after the sleep.
