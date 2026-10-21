# Code review: the canvas frames a 127.0.0.1 page as localhost (LLP 1122 T2 follow-up), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `42984b043`.
- **Method:** one brief (sha256 `dbb99c18f1f0f9acf2c13fa5073b52c54d34c81d37609d3c64a53a2215807178`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve (no findings).
- **Disposition:** taken. docs/tamber-protocol.md's `page` says the tool opens it as given and frames it as localhost, with the frame's own storage. The [::1] checks in host/web/tests/ipv6-loopback.test.mjs and the T2 drive run only where the machine has IPv6, and a machine without it serves IPv4 alone. The busy-frame workload test is still left out, as in rounds 1 and 2.

---

No High, Medium, or Low findings. The round-2 fixes hold: the canvas still frames only a loopback dev loop, and the `[::1]` listener is what answers that name.

`pageOf` (`apps/design-tool/app.ts:93-97`) accepts only `http://127.0.0.1`, `http://localhost`, or `http://[::1]` with a numeric port and nothing after it. The port is compared as a number with `7740`, then both URLs are rebuilt, so `localhost`, `127.0.0.1`, `[::1]`, `07740`, a path, a query, userinfo, a trailing dot, and a different case never become the tool origin. Anything else fails closed. The open link and `--via` stay on `http://127.0.0.1:<port>` (`scripts/tamber-server.mjs:958` still rejects a `localhost` name). Nothing else in the tool, the agent, or the server compares the frame URL to `127.0.0.1`.

The sandbox is unchanged (`apps/design-tool/app.contract:885`): no top navigation, and `allow-same-origin` does not reach the parent because the frame is `http://localhost:<port>`. `frame-ancestors 'self' http://127.0.0.1:7740` (`scripts/tamber-server.mjs:877`) still names the parent. A localhost tool URL is redirected to that origin before any tool HTML (`scripts/tamber-server.mjs:120-121`). A probe of the production listen order (`[::1]` first, then `127.0.0.1` or `0.0.0.0`) got that header from `::1`, `127.0.0.1`, and the hostname `localhost`.

On this machine `localhost` resolves to `::1`, then `127.0.0.1`. Both dev loops bind `[::1]` on the same port and hand each connection to the HTTP server (`host/web/serve.mjs:926-932`, called from `host/web/dev.mjs:1404` and `host/web-js/dev.mjs:576`). A taken `[::1]` port rejects with `EADDRINUSE` and the process exits; `freePort` requires the port free there too (`host/web/dev.mjs:67`). `host/web/tests/ipv6-loopback.test.mjs` passed. The same handoff kept a POST body, a second keep-alive POST, and an event-stream chunk, with `remoteAddress === '::1'`. The dev gate then treats `Host: localhost:<port>` from that peer as local (`scripts/install-page.mjs:276-293`). `--lan` still binds `0.0.0.0` and also binds `[::1]`. The internal `--serve-as` resident does not bind `[::1]`; the public loop does. Dev fetches, the generation envelope, and `/__dev/report` are relative, so registration does not use `hello.page` or the `127.0.0.1` cookie jar. `exact.dev.build` and `exact.dev.inspect` are stored on the `localhost` origin, which is the split D3 and the README describe.

The contract tests prove the rewrite and the port refusal, not a process. The T2 drive checks the frame URL, the `[::1]` response, and this loop's `frame-ancestors`. The T2b drive asks Chrome for an `iframe` target at the localhost frame (`scripts/design-tool.test.mjs:954-955`). In Chrome that type is an out-of-process iframe, so a pass is evidence of a separate process in that browser. It does not record a process id and does not load a busy frame; that gap was left on purpose in rounds 1 and 2. D3, the as-built note, and the README match what ships.

Verdict: the tool origin stays unframed, `[::1]` is this dev loop or the loop stops, and the localhost frame is the process split this change set out to make.
