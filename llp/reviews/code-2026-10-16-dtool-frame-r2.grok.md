# Code review: the canvas frames a 127.0.0.1 page as localhost (LLP 1122 T2 follow-up), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `d63853859`.
- **Method:** one brief (sha256 `e265a40b79e8c78d9c8a6e3a608f3cb39424ec56c63cb1f31815788c57e3cac7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 2 low).
- **Disposition:** taken, but Astra 4 in part (no busy-frame workload test, as in round 1).
  - A taken [::1] port now stops the dev loop, as a taken IPv4 port does (`alsoOnIPv6Loopback` rejects with EADDRINUSE). The automatic port (`freePort`) must be free on [::1] too. host/web/tests/ipv6-loopback.test.mjs covers both, and the T2 drive checks that the [::1] answer carries this loop's `frame-ancestors`.
  - A `[::1]` page is now framed as localhost and opened at 127.0.0.1, the address the gate admits (Grok low 1).
  - D3 and the as-built canvas note say the frame is `localhost` on the dev loop's port and keeps its own storage, and so does the README's first canvas sentence (Grok low 2).

---

No High findings. `pageOf` (`apps/design-tool/app.ts:92-96`) accepts only `http://127.0.0.1`, `http://localhost`, or `http://[::1]` with a numeric port and nothing after the port. The port is compared as a number with `7740`, then the iframe `src` is rebuilt, so case, a trailing dot, userinfo, a path, a query, and a padded `07740` never become `http://127.0.0.1:7740` or `http://localhost:7740`. `[::1]` stays `[::1]`, which is a different site from the tool. The iframe sandbox is unchanged (`app.contract:885`): scripts and popups stay inside it, with no top navigation. `frame-ancestors 'self' http://127.0.0.1:7740` (`scripts/tamber-server.mjs:877`) still names the parent, because the tool document is that origin (`scripts/tamber-server.mjs:120-121`). On this machine `localhost` resolves to `::1`, then `127.0.0.1`, and a Bun probe of the same handoff (`server.emit('connection', socket)`) returned the HTML, the frame-ancestors header, a POST body, and an event-stream chunk from `[::1]`, with `remoteAddress === '::1'`. The dev gate already allows `Host: localhost:<port>` from that peer (`scripts/install-page.mjs:276-293`), and the page's dev fetches are relative (`host/web/dev.js:3`, `:332`), so registration does not depend on `hello.page` or on the `127.0.0.1` cookie jar. `hello.page` stays the printed `http://127.0.0.1:<port>`, which is what "Open in a tab" and `--via` use. `--lan` still binds `0.0.0.0` and also binds `[::1]`.

## Medium

**A taken `[::1]` port is only logged, and the canvas still frames it.** `host/web/serve.mjs:927`, `host/web/dev.mjs:65`, `apps/design-tool/app.ts:96`.

`freePort` probes `127.0.0.1` or, with `--lan`, `0.0.0.0`. It never probes `::1`. `alsoOnIPv6Loopback` then binds `::1` on that same port and, on `EADDRINUSE`, prints one stderr line and leaves the process up. `pageOf` still sets the iframe to `http://localhost:<port>`. `localhost` tries `::1` first. A connection that is accepted does not fall back to `127.0.0.1`, so the canvas loads the other listener: no dev document, no `EventSource("/__dev")`, no `POST /__dev/report`. The same thing happens for an explicit `--port`. The T2 check (`scripts/design-tool.test.mjs:761-762`) treats any HTTP 200 from `::1` as this loop.

Fix: treat `[::1]` `EADDRINUSE` like the IPv4 listen failure and exit. Have `freePort` require the port to be free on `::1` as well. On the `::1` response, assert this loop's `frame-ancestors` value, not only status 200.

## Low

**The `[::1]` frame is a URL this dev loop's gate answers with 421.** `apps/design-tool/app.ts:96`, `scripts/install-page.mjs:276`.

The dev loops publish `http://127.0.0.1:<port>`, which the canvas rewrites to `localhost` and the gate allows. `pageOf`'s other branch keeps `http://[::1]:<port>` as both `page` and `frame` (the contract test at `apps/design-tool/app.test.contract:461-468` expects a frame). A browser request to that URL sends `Host: [::1]:<port>`. The gate's names are only `127.0.0.1:<port>` and `localhost:<port>`, so `host/web/dev.mjs:1261` returns 421. The new listener accepts the TCP connection, and the canvas stays blank. "Open in a tab" is the same URL.

Fix: admit `http://[::1]:<port>` in `developmentGate` the same way as `localhost`, including the loopback `local` check for a `::1` peer.

**The spec still says the iframe is the dev loop's own URL, and that a second frame would share its storage.** `llp/1122-design-tool.rfc.md:259`, `llp/1122-design-tool.rfc.md:271`, `llp/1122-design-tool.rfc.md:1027`, `apps/design-tool/README.md:17`, `apps/design-tool/app.contract:871`.

D3 says one iframe at the dev loop's URL, and that two frames of that URL share storage. The as-built bullet still begins "one sandboxed iframe at `hello.page`" and only then says a `127.0.0.1` page is framed as `localhost`. The README's canvas bullet does the same. What ships is an iframe at `hello.frame` (`http://localhost:<port>`), with `hello.page` kept for the tab. Those two origins do not share cookies, `localStorage`, or `sessionStorage` (`exact.dev.build`, `exact.dev.inspect`). The T2b check (`scripts/design-tool.test.mjs:954-955`) looks for a Chrome iframe target whose URL is that localhost origin. In Chrome that target type is an out-of-process frame, which is evidence of a separate target in that browser. It does not record a process id, and it does not load a busy frame. The README's "a busy one cannot stall the tool" is the site-isolation claim, qualified to a browser that isolates sites.

Fix: change D3, the as-built lead sentence, and the README's first canvas sentence so the framed origin is `localhost` and the printed `page` stays the address. Say there that the canvas jar is separate from a tab at the printed URL.

Verdict: the tool origin stays unframed and the localhost handoff works when `[::1]` is free; fix the taken-port case before relying on the canvas.
