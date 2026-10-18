# Code review: LLP 1122 T2 step 1 (the canvas, hello.page, framing), round 1, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `47a9b3607`.
- **Method:** one brief (sha256 `c809dd416ec420e6e35503bd77c739ee0f0f6ce3e4b94ebc8e121f3e441487bc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium, 3 low).
- **Disposition:** taken, and 2 (astra) left as it is: one helper (`frameOnlyByTool`, scripts/tamber-server.mjs) wraps each dev-loop response's head in both loops, first thing per request, so every `text/html` answer carries `frame-ancestors` whichever route sends it (the page, install pages, the auth callback, `/__dev/open` and the JS loop's forwarded copy, generation HTML, the tool page, the JS build-error page), the implicit head included; `'self'` stays, because a child naming only the tool's origin would refuse to render inside the canvas (frame-ancestors checks the whole chain), and the docs, comment and test title now say the tool's origin and the page's own; the canvas iframe is sandboxed (scripts, same origin, forms, popups escaping the sandbox, modals, downloads; no top navigation); `hello.page` is framed only as `http://127.0.0.1:<port>` or `http://[::1]:<port>`, else none; `tamber tool --via` refuses while a tool for that design runs without that loop (it no longer hands back a null page), and the empty state says to stop this tool first; the stub canvas test reloads after setting the page and checks the empty-state text and kept size, and a loopback-only test is added; tests: the helper with each way of writing a head, the JS loop's install page, the wasm loop (caltrain) answering `/`, its install page and `/__dev/open` with the header and no X-Frame-Options, the refused `--via`, and a real-server canvas drive checking the frame's URL, its laid-out 390x844 and 1280x800, and the built sandbox. Not tested: HEAD requests, generation HTML and the JS build-error page (the helper keys on the content type, not the route).

---

The wasm loop’s frame policy covers the app shell only. The install page, which can start a local build, is still frameable, and `/__dev/open` is frameable from both loops.

## High

**Wasm responses other than the app shell never get `frame-ancestors`, including the install page that can start a device build.** `host/web/dev.mjs:1280-1281` adds `FRAME_ANCESTORS` only inside `index ? { vary: 'Accept', content-security-policy }`. `index` is true for `/` and `/index.html` (`host/web/serve.mjs:778`). These HTML responses are sent with no such header:

- `/.exact/install/` and `/.exact/install/{web,ios,macos}/` (`host/web/serve.mjs:8`, served through `readWebRequest`)
- any other `*.html` file (deck, assets)
- `/__dev/generation/<epoch>/<seq>/…` when the file’s type is HTML (`host/web/dev.mjs:1199`)

On a loopback load, `localInstaller.page` writes the per-process install token into that HTML and a “Build and install” button (`host/web/local-install.mjs:87-92`, `scripts/install-page.mjs:165-213`). The button’s `POST /__dev/install/ios` is accepted when `Origin` is the dev origin (`scripts/install-page.mjs:107` via `host/web/local-install.mjs:107`) and the token matches. The development gate applies its cross-site check to POSTs (`scripts/install-page.mjs:288-291`), so a cross-site GET of the install page from this machine is still `access.local`.

Scenario: `bun host/web/dev.mjs --wasm` (or a game, which stays on this loop) is listening on `127.0.0.1:8765`. A page on another origin iframes `http://127.0.0.1:8765/.exact/install/`. The parent cannot read the token, but after the target list enables the button, one clickjacked click runs `host/apple/build.mjs … --run` for a simulator or a paired phone. The JS loop does send the header for any `found.route` ending in `.html` (`host/web-js/dev.mjs:348`), so the same install URL is protected there. The commit message treats `host/web/dev.mjs` as the same header. The predicate is different, and T2’s loop is the wasm one.

Fix: set `content-security-policy` to `FRAME_ANCESTORS` on every dev-loop response whose content type is HTML, including install pages, deck and asset HTML, and HTML bytes from `/__dev/generation/`. Keep `vary: Accept` on the negotiated shell only.

## Medium

**`/__dev/open` is an HTML page with build tokens and no framing header, on both loops.** `host/web/dev.mjs:1177-1182` answers `text/html` with `referrer-policy: no-referrer` and no `Content-Security-Policy`. The body includes `scheme://open?url=…&token=<64 hex>` for each development client built against this origin (`host/web/serve.mjs:46-64`, `host/apple/build.mjs:280-288`). The JS loop does not serve this path itself. It forwards `/__dev/open` to the `--serve-as` producer and copies that response’s headers (`host/web-js/dev.mjs:326-327`, `host/web-js/dev.mjs:379`).

Scenario: with either loop up, another origin iframes `http://127.0.0.1:<port>/__dev/open`. A click on “Open in the Simulator client” follows the token-bearing custom-scheme link. The parent cannot read the DOM, so this is clickjacking of a credentialized control, which is the case `frame-ancestors` is there to close.

Fix: add `FRAME_ANCESTORS` on this response in `host/web/dev.mjs`. The forwarded JS-loop response then carries it too.

**The auth callback is HTML and misses the header on both loops.** `webContentType` returns `text/html` for `/.exact/auth/callback` (`host/web/serve.mjs:681`), and the page’s script posts `location.href` (the OAuth code) on `BroadcastChannel('exact-auth')` (`host/web/auth-callback.js:7-10`). The JS loop keys the header on `.html`. The wasm loop keys it on `index`. This path matches neither. The production renderer’s policy for the same page is `frame-ancestors 'self'` (`host/render/src/serve.rs:1486`).

Scenario: during a dev login, a foreign page iframes `http://127.0.0.1:<port>/.exact/auth/callback`. A code that lands in that frame is posted to every same-origin listener. A foreign parent does not receive `postMessage` (the target origin is the dev origin), but the document the production server refuses to embed is embeddable in development.

Fix: same content-type rule as the install pages. One helper used by both loops, `/__dev/open`, and the generation handler.

**The canvas iframe is unsandboxed, so the guest can navigate the tool.** `apps/design-tool/app.contract:627` sets `src`, `width`, and `height` only. There is no `sandbox`. `allow` is unset, so camera, microphone, and geolocation stay at the cross-origin default (withheld). That matches LLP 1020’s deferred permission delegation and D3’s “open in a tab” escape hatch. `target="_blank"` on the open link does get `rel="noopener"` from the JS runtime (`host/web-js/rt.js:705`). The frame itself has no such limit: the guest may assign `window.top.location`.

Scenario: the app under design, or any script it loads, runs `top.location = 'https://attacker.example/tool'`. The design tool at `127.0.0.1:7740` is replaced. The guest cannot read the tool (different origin), and it can still take the window.

Fix: sandbox the canvas iframe with `allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox allow-modals allow-downloads`, and leave out `allow-top-navigation`. That keeps storage, login, and OAuth popups, and stops the guest from navigating the tool.

## Low

**`'self'` is a second ancestor, and the write-up says the tool is the only one.** `scripts/tamber-server.mjs:772` is `frame-ancestors 'self' http://127.0.0.1:7740`. `'self'` is the document’s own origin, so any document that origin serves (the app, the install page, `/__dev/open`) may frame it. A third origin may not. `'self'` is what lets the app embed its own HTML while the tool is also an ancestor: `frame-ancestors` checks the whole chain, and a child whose policy named only `127.0.0.1:7740` would refuse to render inside the canvas. `docs/tamber-protocol.md:42-44` says “no other origin may.” The test title says “only the design tool may frame that page” (`scripts/tamber-server.test.mjs:355`) and then asserts a header that also allows `'self'`.

Fix: keep `'self'` and make the comment, the protocol page, and the test title say the tool’s origin and the page’s own origin.

**The tool will iframe whatever string `hello.page` is.** `apps/design-tool/app.ts` keeps any string (`typeof ok.page === 'string' ? ok.page : ''`). The loops pass `http://127.0.0.1:${port}` (`host/web/dev.mjs:281`, `host/web-js/dev.mjs:252`). `--via` forwards the upstream body unchanged (`scripts/tamber-server.mjs:914-915`), which is right for an honest loop and is what the test checks. A loopback server the user pointed `--via` at can still answer `"page":"https://attacker.example/"` and have that URL rendered in the canvas. `navigable()` blocks `javascript:`; it allows any `http:` or `https:` URL.

Fix: in `loadHello`, accept `page` only when it is `http://127.0.0.1:<port>` or `http://[::1]:<port>` with no userinfo, path, query, or fragment. Otherwise treat it as no page.

**The tests do not prove the claims the commit states.**

- `scripts/tamber-server.test.mjs:364-368` fetches `GET /` on the JS loop only. The `for` loop has a single entry (`:354`). Nothing requests `/__dev/open`, `/.exact/install/`, `/.exact/auth/callback`, a non-shell `.html` file, or the wasm loop. The comment at `:352-353` says the wasm loop sends the same header.
- The JS build-failure document does send the header (`host/web-js/dev.mjs:330`). No test hits it. The wasm loop has no HTML error body (empty 404 at `host/web/dev.mjs:1273`).
- `apps/design-tool/app.test.contract:419-423` checks that `canvas-frame` exists and that `canvasSize` becomes `"tablet"`. It never checks the iframe’s `src`, `width`, or `height` (390×844, 820×1180, 1280×800 at `app.contract:627`). The last `page: null` request (`:424`) has no assertion.

Fix: assert `src` and the three sizes on the frame; request the install page, `/__dev/open`, and the auth callback on a wasm loop and assert the header and a null `x-frame-options`.

## Checked and holding

`--lan` still binds `0.0.0.0` and the gate still admits `127.0.0.1` and `localhost` (`scripts/install-page.mjs:253-276`). `hello.page` stays `http://127.0.0.1:<port>`, which is the origin the tool at `127.0.0.1:7740` can load, and `FRAME_ANCESTORS` names that tool origin, so the canvas is allowed. A LAN address is a different origin. The tool listens only on loopback, which matches D12: a remote frame of a loopback URL would be the remote machine. `--serve-as` does not mount the wire (`host/web/dev.mjs:281`), so it publishes no `page`. The outer JS loop publishes its own port. `--via` does not rewrite `hello` onto 7740. A server with no dev loop sends `page: null` (`scripts/tamber-server.mjs:147` default, merged at `:445`). Neither dev loop sets `X-Frame-Options`. The header is only in the two dev loops. The production renderer still sends `frame-ancestors 'self'`, so a release page stays unframeable by the tool. The contract iframe has no `message` handler and no script that touches the frame.

Verdict: Do not land until every HTML response from both dev loops carries `frame-ancestors`, including the install pages and `/__dev/open`, and a test actually requests those URLs on the wasm loop.
