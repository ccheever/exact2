# Code review: the ghost builds' follow-up (res close backstops, a test per door), round 1 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `619360e3c0`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; one SHOULD-FIX (the wasm /__dev backstop untested), answered in 5741c64e03.

I'll review the HEAD commit read-only, starting with the change and the stream handlers it touches.The commit is one change on a clean tree. I'll read the diff and the stream handlers it touches, then check the guard and each door test.The diff is truncated on the web dev loop. I'll read that handler and the other close paths in full.Nothing blocks. The `gone`/`done` guard is correct on all three handlers, including `host/web/dev.mjs` around the relay hello.

On that handler the listeners are attached before the `await`. A close before or during `relayChannel` sets `gone` while `opened` is still null, and the existing `if (gone) { opened?.close(); return; }` at line 1426 closes the channel once it exists and returns before `clients.add`. A close after that check is a later turn: `end` closes the channel and deletes the client, and the second of `req`/`res` is a no-op. `buildChannel`'s `close` and the relay's `going = false` are both idempotent. The JS loop and the gallery have no `await` between attaching the listener and `clients.add`, so a close delivered on a later turn cannot land in that gap.

`res` `'close'` firing early does not apply here. The early fire is the refused-body case (`scripts/tamber-request.mjs:15`: unread body, connection closing before the answer is out). These are GET SSE responses after `writeHead`, left open, or the subscribe POST whose body is read before `writeHead`. A close at `writeHead` would drop the build before the hand-off test registers it, and would make the subscription's pin 409 while the stream is still open.

`host/web-js/hand-off.preload.mjs` loads only through `bun --preload` on that one dev process. It patches `http.Server.prototype.listen` and opens the side port only when the port matches `EXACT_TEST_HAND_OFF` and the host is not `::1`, so the `[::1]` listener is left alone. The resident child is spawned without `--preload`. Nothing else imports the file.

No page-stream handler is still on `req` `'close'` alone. The three handlers listen to both. Subscribe, the long poll, import, the canvas flight, and the JS forward were already on `res` `'close'`. The two body readers you excluded are unchanged.

The 5–10 s waits are idle time after an immediate close, with the same or more slack than the sibling assertions on those paths. They fail the assertion when the close never comes.

1. **SHOULD-FIX** `host/web/dev.mjs:1424` — the `/__dev` backstop, the one with the `await`, is never what a test runs. `dev-page-streams.test.mjs:77` spawns `host/web/dev.mjs` without `--wasm`, and `host/web/dev.mjs:76` hands a non-game app to `devJs` and does not return. The request is `GET /__dev/page` (`dev-page-streams.test.mjs:92`), which only `host/web-js/dev.mjs:925` serves. Removing `res.on('close', end)` on the wasm `/__dev` handler would not fail that test. The canvas and relay tests stop at the gateway's existing `res` `'close'` (`scripts/tamber-canvas.mjs:502`) against a fake loop.
