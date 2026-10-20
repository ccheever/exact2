# Code review: LLP 1122 T2 step 1 (the canvas, hello.page, framing), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `47a9b3607`.
- **Method:** one brief (sha256 `c809dd416ec420e6e35503bd77c739ee0f0f6ce3e4b94ebc8e121f3e441487bc`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 3 medium, 1 low).
- **Disposition:** taken, and 2 (astra) left as it is: one helper (`frameOnlyByTool`, scripts/tamber-server.mjs) wraps each dev-loop response's head in both loops, first thing per request, so every `text/html` answer carries `frame-ancestors` whichever route sends it (the page, install pages, the auth callback, `/__dev/open` and the JS loop's forwarded copy, generation HTML, the tool page, the JS build-error page), the implicit head included; `'self'` stays, because a child naming only the tool's origin would refuse to render inside the canvas (frame-ancestors checks the whole chain), and the docs, comment and test title now say the tool's origin and the page's own; the canvas iframe is sandboxed (scripts, same origin, forms, popups escaping the sandbox, modals, downloads; no top navigation); `hello.page` is framed only as `http://127.0.0.1:<port>` or `http://[::1]:<port>`, else none; `tamber tool --via` refuses while a tool for that design runs without that loop (it no longer hands back a null page), and the empty state says to stop this tool first; the stub canvas test reloads after setting the page and checks the empty-state text and kept size, and a loopback-only test is added; tests: the helper with each way of writing a head, the JS loop's install page, the wasm loop (caltrain) answering `/`, its install page and `/__dev/open` with the header and no X-Frame-Options, the refused `--via`, and a real-server canvas drive checking the frame's URL, its laid-out 390x844 and 1280x800, and the built sandbox. Not tested: HEAD requests, generation HTML and the JS build-error page (the helper keys on the content type, not the route).

---

1. **High — Framing protection misses several HTML responses.** [host/web/dev.mjs:1281](/tmp/rv-dt-t2a/host/web/dev.mjs:1281), [host/web-js/dev.mjs:348](/tmp/rv-dt-t2a/host/web-js/dev.mjs:348).
   The wasm loop adds CSP only when `index` is true, leaving install pages, HTML assets and the auth callback unprotected. `/__dev/open` returns earlier without CSP at [dev.mjs:1181](/tmp/rv-dt-t2a/host/web/dev.mjs:1181); the JS loop forwards that response unchanged from its `--serve-as` producer. The JS loop also misses `/.exact/auth/callback`, whose MIME type is HTML despite lacking `.html`. Tamber’s tool-page handler bypasses both additions too.

   **Scenario:** an untrusted page frames the wasm loop’s installation page and disguises its build button. The framed page receives its local installation token; its own POST passes the gate. Protecting `/` does not protect this response.
   **Fix:** apply the policy consistently at the development response boundary, including early returns and forwarded HTML. Test both loops, GET/HEAD, install/open/callback pages, and LAN access. The existing test checks only the JS root; its compile-error HTML does correctly carry CSP.

2. **Medium — `'self'` contradicts “the tool’s origin only.”** [scripts/tamber-server.mjs:772](/tmp/rv-dt-t2a/scripts/tamber-server.mjs:772).
   This explicitly permits the dev page’s own origin as an ancestor. For example, a top-level `/assets/preview.html` served by the same loop can embed `/` without the tool. Under `--lan`, the exception also applies to that response’s LAN origin. This is an additional allowed origin, not a requirement for opening the app in a tab. [CSP specification](https://www.w3.org/TR/CSP3/#directive-frame-ancestors).
   **Fix:** remove `'self'`, update the documentation and literal assertion, and test successful framing from 7740 plus rejection from the dev origin and another port.

3. **Medium — The empty-state instructions cannot transition a standalone tool to a canvas.** [apps/design-tool/app.contract:631](/tmp/rv-dt-t2a/apps/design-tool/app.contract:631).
   **Scenario:** start `tamber tool` without `--via`, then follow the displayed instructions. That standalone server still holds `.exact/tamber.lock`, so the new loop’s Tamber process fails. Running `tamber tool --via …` subsequently reuses the existing standalone instance through [tamber-server.mjs:822](/tmp/rv-dt-t2a/scripts/tamber-server.mjs:822), leaving `page` null.
   **Fix:** explicitly instruct the user to stop the standalone tool server first, start the dev loop, then restart the tool with `--via`.

4. **Medium — The canvas test depends on unrelated stub history to refresh hello.** [apps/design-tool/app.test.contract:412](/tmp/rv-dt-t2a/apps/design-tool/app.test.contract:412), [stub/tamber-stub.ts:141](/tmp/rv-dt-t2a/apps/design-tool/stub/tamber-stub.ts:141).
   `/__stub/page` changes `PAGE` without publishing an event. Changing the person’s name does not refresh `hello`. With an empty event log, `changes.seq == since`, neither request failed, and no events were missed, so `advance` never refreshes it. The cached null page remains and the frame assertion fails. Earlier tests’ events can conceal this dependency.
   **Fix:** reload after configuring the stub, or explicitly trigger the refresh path. Ensure this case works independently.

5. **Low — The tests do not establish the claimed frame behavior.** [apps/design-tool/app.test.contract:419](/tmp/rv-dt-t2a/apps/design-tool/app.test.contract:419).
   Presence checks and `canvasSize == "tablet"` would still pass with an incorrect iframe URL or fixed 300×150 dimensions. They also never verify the link destination, loaded guest, or startup instructions.
   **Fix:** assert `src`, link destination and target, and rendered dimensions for all three presets; verify persistence after reload and the empty-state text.

The supported local origin flow otherwise looks consistent: `--lan` retains loopback access, `--via` forwards hello unchanged, and the producer publishes no separate hello. The iframe introduces no grants or permission delegation, and I found no tool code scripting it. Its unsandboxed behavior matches D3’s real-app embed.

This was static, read-only inspection; builds and tests were not run because they create files.

**Verdict: Request changes.**
