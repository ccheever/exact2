# Code review: the canvas frames a 127.0.0.1 page as localhost (LLP 1122 T2 follow-up), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `bca4378e8`.
- **Method:** one brief (sha256 `effccc632617dcb02ba8eb191bf862c2d47e4da66a957f4a229f48a97b96cc02`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 3 low).
- **Disposition:** taken, but Astra 4 in part.
  - Both dev loops answer on [::1] too, on the same port (host/web/serve.mjs `alsoOnIPv6Loopback`, each connection handed to the HTTP server). So `localhost`'s first address reaches this dev loop, not an empty frame. If another server holds [::1] on that port, the dev loop says so (Astra 1, Grok 1).
  - hello's page keeps the URL as the dev loop prints it (127.0.0.1), for "Open in a tab" and `--via`. The canvas has its own `frame` URL, the localhost one (Astra 2, Grok 2, Grok 4).
  - Ports are compared as numbers and canonicalized, and the tool's port is refused for [::1] too. Tests cover `07740` and `[::1]:7740` (Astra 3, Grok 3).
  - The README says the process split holds in a browser with site isolation (Chrome's desktop default), and that the canvas keeps its own cookies and storage, apart from the printed URL's tab. The RFC's as-built canvas note and its follow-up list say so (Astra 4, Grok 5).
  - The T2 drive checks that the dev loop answers on [::1] for a localhost Host. The T2b drive keeps its CDP iframe-target check.
  - In part (Astra 4): no busy-frame workload test.

---

`pageOf` still frames only a loopback origin, and the URL it produces is a different site from the tool (`http://127.0.0.1:7740`). For a document that actually loads as `http://localhost:<port>`, the sandbox and `frame-ancestors` still hold. The canvas can still fail to reach the dev loop, and the framed page is no longer the origin the dev loop prints.

No High findings. The tool's origin is not what gets framed: `127.0.0.1:7740`, `localhost:7740`, case, trailing dots, paths, userinfo, and non-loopback hosts all become an empty page. `[::1]` is kept as its own site. A loaded localhost document is cross-origin to the tool, so `allow-same-origin` does not reach the parent, and there is no `allow-top-navigation`. `frame-ancestors 'self' http://127.0.0.1:7740` (`scripts/tamber-server.mjs:877`, applied from `host/web/dev.mjs:1259` and `host/web-js/dev.mjs:431`) still names the parent, because `serveTool` sends a localhost tool URL to `http://127.0.0.1` before any tool HTML (`scripts/tamber-server.mjs:120-121`).

## Medium

**The framed host is `localhost`, and neither dev-loop bind accepts that name's first address.** `host/web/dev.mjs:81`, `host/web/dev.mjs:1402`, `host/web-js/dev.mjs:575`, `apps/design-tool/app.ts:93`.

On this machine `localhost` resolves to `::1`, then `127.0.0.1`. The loop listens on `127.0.0.1`, or on `0.0.0.0` with `--lan` (`host/web/dev.mjs:81`). A throwaway listener on both of those addresses accepted `127.0.0.1` and refused `::1` with connection refused. `hello.page` is still `http://127.0.0.1:<port>` (`host/web/dev.mjs:369`, `host/web-js/dev.mjs:426`); only the canvas rewrites it. A client that connects to the first result gets an empty iframe: no document, no `EventSource("/__dev")` (`host/web/dev.js:3`), no `POST /__dev/report` registration (`host/web/dev.js:332`, `:700-704`). `--lan` does not change that. Desktop Chrome and Safari usually try `127.0.0.1` after an immediate refusal, which is why the Chrome drive can still paint. The gate's `Host: localhost` check (`scripts/install-page.mjs:275-293`) only matters after the TCP connection lands.

Fix: listen on two loopback sockets, `127.0.0.1` and `::1`, same port. `listen(port, 'localhost')` binds only the first result and would drop `127.0.0.1`.

**The canvas origin is no longer the origin the dev loop prints, so cookies and the dev page's storage split.** `apps/design-tool/app.ts:93`, `apps/design-tool/app.contract:879-882`, `host/web/dev.js:313-328`, `host/web/dev.js:591-662`.

The server, the startup line, and `--via` stay on `http://127.0.0.1:<port>`. The iframe and "Open in a tab" use `http://localhost:<port>`. Those hosts do not share cookies. The iframe is also a third-party context under the tool, so a `SameSite=Lax` cookie set at the printed URL is withheld, and Chrome partitions the ones it does store. A session established at the printed URL shows up logged out in the canvas. `exact.dev.build` and `exact.dev.inspect` still persist across a reload of that iframe (partitioned `sessionStorage`, and the duplicate-tab `BroadcastChannel` still joins two canvases under the tool). They do not join a tab at the printed URL, or the first-party localhost tab the open link creates. The build channel itself does not need those cookies: its fetches are relative, and the gate admits `Host: localhost:<port>` when `Origin` names that same host. Asset URLs in the dev envelope are path-absolute (`host/web/dev.mjs:458-467`), so they follow the document origin once the document has loaded.

Fix: one advertised loopback origin for the printed URL, the frame, and the open link, after both loopback addresses listen. Keep `--via` on the address form; a name is refused on purpose (`scripts/tamber-server.mjs:952-960`).

## Low

**The tool-port check compares port text, and it skips `[::1]`.** `apps/design-tool/app.ts:91-93`.

`http://127.0.0.1:07740` and `http://localhost:07740` match the regex, `Number("07740")` is 7740, and `"07740" !== "7740"`, so they are framed as `http://localhost:07740`. The browser's origin for that is `http://localhost:7740`: the tool's port on the other host. `GET /` there is the tool's JSON 404 (`scripts/tamber-server.mjs:382-389`); the localhost redirect runs only for `/__tamber/tool`. That document is a different site from `http://127.0.0.1:7740`, so the sandbox stays down. `http://[::1]:7740` and `http://[::1]:07740` are framed as given. The tool does not listen on `::1`, so this is a refused connection rather than the tool document. The contract covers `7740` and `localhost:7740` (`apps/design-tool/app.test.contract:447-454`) and stops there.

Fix: reject every candidate whose parsed port equals `Number(new URL(API).port)`, including `[::1]` and leading zeros, and add those strings to the contract test.

**Copying the framed URL into `--via` is refused.** `scripts/tamber-server.mjs:958-960`, `apps/design-tool/app.contract:886`.

`--via` allows only `127.x` and `[::1]`, and `scripts/design-tool.test.mjs:122` expects `http://localhost:9` to fail. The empty canvas tells the reader to pass "its URL". The terminal still prints `127.0.0.1`, which works. The open-link and `hello.page` in the tool are now `localhost`, and that spelling is refused with an error that asks for `http://127.0.0.1:<port>`.

Fix: in that empty-state line, name the address from the dev loop's own startup line.

**The process-split check is a Chrome iframe target, and the RFC's normative canvas line still says the dev loop's own origin.** `scripts/design-tool.test.mjs:945-950`, `llp/1122-design-tool.rfc.md:1027`, `llp/1122-design-tool.rfc.md:1063`.

T2b launches headless Chrome and calls `Target.getTargets` on a page CDP session. In Chrome, `type === "iframe"` means an out-of-process frame, so a passing result is evidence of a separate target for that localhost URL. The payload has no process id, the contract tests only check the URL string (`apps/design-tool/app.test.contract:420`), and `open` uses the default browser. The normative T2 sentence still says one sandboxed iframe at `hello.page`, the dev loop's own origin. The gate follow-up asks for `localhost`. The README matches the follow-up (`apps/design-tool/README.md:17-22`); the RFC sentence does not, and it says nothing about the bind or the second origin.

Fix: attach `Target.getTargets` to a browser CDP session and treat a missing iframe target as failure. Amend the RFC sentence so the framed origin is `localhost` while the server's `page` stays the loopback address.

Verdict: the sandbox and frame-ancestors survive the rewrite, and the tool origin stays unframed; ship only after the dev loop accepts `::1` and the printed origin and the canvas origin are an intentional pair.

