# Code review: LLP 1122 T1 commit 2 (the design tool's skeleton and its connection), round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `3769f3880` (commit `0beef42fd` and its round-1 fixes).
- **Method:** one brief (sha256 `93314156363e53d503840b424dfc135aae643423847292d0a94d0fec146340ea`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** land with nits (5 low).
- **Disposition:** all taken: while down, a poll still gets the kept events after it (its `down` among them) and 503 only once caught up, and the tool reads `hello` again after a failed poll; `--via` with no URL or as `--via=` refused before anything starts; a tool page opened as localhost is sent to 127.0.0.1, its grant's origin; the restart test starts the stub afresh and names a client no other test leaves; `tamber tool` takes its port only once the design server is up, so a refusal leaves nothing listening.

---

Round 1 is fixed in the current tree. The off-machine `--via` path, the `EXACT_APP_DIR` build, the upstream handshake and self-proxy, the 1 ms re-poll, the proxy abort and backpressure, stale digests, epoch and sequence reuse, the shared token ask, the first poll's empty epoch, Try Again's second poll, and redirect following are all addressed. `165174983` was left alone. Nothing at High or Medium remains.

## Low

**1. A poll that starts after the design server has died drops the buffered `down` event and leaves the file list up.** `scripts/tamber-server.mjs:329`, `apps/design-tool/app.ts:97`, `apps/design-tool/app.contract:56`.

`POST /__tamber/events` returns 503 whenever `down` is set, before it looks at the buffer. A poll already inside the 25 s wait still gets HTTP 200 from `answer()` and includes that event; the next `hello` then fails and the no-server screen appears. A poll that has not reached that wait does not. `waitChanges` keeps the old sequence on any error, and `advance` refreshes `hello` only when `changes.missed`, `hello.ok` is false, or the sequence moved. The file list stays, the footnote shows the 503 text, and there is no Try Again. `down` is sticky, so the 3 s retry repeats until the process is restarted. The events still in the 256-deep buffer, including `down`, are never applied, and the digests stay as they were.

The design server exits in the gap after an empty poll answer and before the next `POST /__tamber/events` is waiting (the `after(1)` re-arm, or while that request is still reading its body). The open page never switches to "No design server". A reload does, because `hello` itself is 503.

In the events handler, if some kept event has `seq > after`, answer 200 with those events (including `down`). Return 503 only when the client is already caught up. On the client, treat `changes.ok === false` as a reason to refresh `hello`, so this path reaches the same screen as a failed `hello`.

**2. `--via` with no value, and `--via=url`, start a design server.** `scripts/tamber-server.mjs:719`.

`at('--via')` is the next argv slot. `--via` at the end of the line yields `undefined`, and `--via=http://127.0.0.1:8080` never matches `indexOf('--via')`. Both fall through to `tamberServer` for the app. `bun exact.mjs tamber tool . --via` while nothing else holds the design starts a server on port 7740 and the dev loop then cannot take the lock. The same command when a dev loop already holds the lock fails with the "another tamber server" hint, which tells the person to pass the flag they already passed.

If argv contains a `--via` token, require the following argument to be an `http://127.0.0.1` or `http://[::1]` URL and refuse otherwise, before `buildTool` or `listen`.

**3. `http://localhost:7740/__tamber/tool/` loads, then every data fetch fails.** `apps/design-tool/app.ts:14`, `scripts/tamber-server.mjs:97`, `scripts/install-page.mjs:275`.

The listener uses `developmentGate`, which treats `Host: localhost:7740` as local, and `serveTool` then returns the page. The data module always calls absolute `http://127.0.0.1:7740`. That is a different origin, and neither `json` nor `forward` sends `Access-Control-Allow-Origin`. The browser rejects the token fetch. The page shows "No design server" while this process is the server.

Opening `http://localhost:7740/__tamber/tool/` (autocomplete, a bookmark) after `tamber tool` has printed the `127.0.0.1` URL. Redirect `Host: localhost:7740` to `http://127.0.0.1:7740`, or serve the tool page only when `Host` is `127.0.0.1:7740`.

**4. The restart contract test's first row can pass on the previous test's event.** `apps/design-tool/app.test.contract:52`, `apps/design-tool/stub/tamber-stub.ts:17`, `scripts/verify.mjs:142`.

`test` and `verify` start `commands.server` once for the file. The stub keeps `log`, `EPOCH`, and `FILES` in that process. The attributed-write test leaves sequence 1 as `app.tamber · agent:codex`. The restart test's first `expect text "change-1"` is that same string, so it passes even when the event posted in the restart test never appears. The later `expect tree missing "change-1"` and the new `design-tool:Grace` row still check the clear. Reset the stub at the start of that test (its `/__stub/restart`), and assert a client string the earlier test did not leave.

**5. Ctrl-C during a failed start exits 0, and the port stays open until then.** `scripts/tamber-server.mjs:657`, `scripts/tamber-server.mjs:757`.

`listen` registers `SIGINT`, `SIGTERM`, and `SIGHUP` to call `process.exit(0)` as soon as the socket is bound, which is before `tamber.ready` and the `down` check. `toolCommand` then waits out `tamber.close()` (about 7 s) and returns 1, without closing that HTTP server first. Ctrl-C in that window reports success to the shell. Until the process actually exits, port 7740 still accepts requests against a design server that is exiting.

On the `down` path, remove those handlers (or make them exit 1) and `server.close()` before returning.

Verdict: Land with nits — the round-1 blockers are fixed, and the leftovers are low.
