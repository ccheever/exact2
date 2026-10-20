# Code review: the canvas frames a 127.0.0.1 page as localhost (LLP 1122 T2 follow-up), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `bca4378e8`.
- **Method:** one brief (sha256 `effccc632617dcb02ba8eb191bf862c2d47e4da66a957f4a229f48a97b96cc02`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (2 medium, 2 low).
- **Disposition:** taken, but Astra 4 in part.
  - Both dev loops answer on [::1] too, on the same port (host/web/serve.mjs `alsoOnIPv6Loopback`, each connection handed to the HTTP server). So `localhost`'s first address reaches this dev loop, not an empty frame. If another server holds [::1] on that port, the dev loop says so (Astra 1, Grok 1).
  - hello's page keeps the URL as the dev loop prints it (127.0.0.1), for "Open in a tab" and `--via`. The canvas has its own `frame` URL, the localhost one (Astra 2, Grok 2, Grok 4).
  - Ports are compared as numbers and canonicalized, and the tool's port is refused for [::1] too. Tests cover `07740` and `[::1]:7740` (Astra 3, Grok 3).
  - The README says the process split holds in a browser with site isolation (Chrome's desktop default), and that the canvas keeps its own cookies and storage, apart from the printed URL's tab. The RFC's as-built canvas note and its follow-up list say so (Astra 4, Grok 5).
  - The T2 drive checks that the dev loop answers on [::1] for a localhost Host. The T2b drive keeps its CDP iframe-target check.
  - In part (Astra 4): no busy-frame workload test.

---

No High finding established. HEAD has two Medium issues and two Low issues.

1. **Medium — `localhost` can reach a different server.** [apps/design-tool/app.ts:93](/tmp/rv-dt-frame/apps/design-tool/app.ts:93)  
   The rewrite changes the connection destination: the dev loop binds IPv4 only ([host/web/dev.mjs:81](/tmp/rv-dt-frame/host/web/dev.mjs:81)). If the app owns `127.0.0.1:8765` and another service owns `[::1]:8765`, a browser connecting to IPv6 first loads that other service. An unanswered IPv6 connection can fall back; a successful connection to the wrong service cannot. `--lan` still binds IPv4 (`0.0.0.0`), so it does not resolve this.

   **Fix:** Have the dev server advertise a frame URL whose listener it controls. For `localhost`, reserve and serve both loopback addresses, handling port conflicts explicitly. Add an IPv6-only competing-listener case.

2. **Medium — Rewriting `hello.page` also removes the canonical “Open in a tab” fallback.** [apps/design-tool/app.ts:93](/tmp/rv-dt-frame/apps/design-tool/app.ts:93)  
   Both the iframe and link consume the rewritten value ([app.contract:879](/tmp/rv-dt-frame/apps/design-tool/app.contract:879)). An app with saved data or authentication at `127.0.0.1:<port>` now opens a different origin in both places. Its IndexedDB/localStorage data is absent; absolute requests retain their original URLs and can encounter new CORS or credential failures. Even a top-level `localhost` tab can have different storage from the cross-site canvas because of [storage partitioning](https://privacysandbox.google.com/cookies/storage-partitioning). Copying that tab’s URL into `--via` also fails: [tamber-server.mjs:958](/tmp/rv-dt-frame/scripts/tamber-server.mjs:958) intentionally rejects `localhost`.

   **Fix:** Preserve the validated original URL for the link and protocol-facing use; derive a separate iframe URL. Document the canvas’s separate storage/session behavior and cover a stateful app and origin-restricted fetch, beyond the theme fixture.

3. **Low — Zero-padded ports bypass the new own-port exclusion.** [apps/design-tool/app.ts:92](/tmp/rv-dt-frame/apps/design-tool/app.ts:92)  
   A read-only evaluation of the actual function confirmed:
   `http://127.0.0.1:07740` → `http://localhost:07740` → browser origin `http://localhost:7740`. The previous `URL.origin` comparison rejected the original spelling. The current tool’s `/` returns 404, so the demonstrated impact is an invalid canvas request, not a sandbox escape.

   **Fix:** Compare normalized numeric ports, validate `1–65535`, and return a canonical URL. If the stated rule is “never the tool’s port,” apply it to `[::1]` too; that branch currently bypasses the check. Add padded-port cases.

4. **Low — The process-isolation promise exceeds the evidence.** [apps/design-tool/README.md:21](/tmp/rv-dt-frame/apps/design-tool/README.md:21)  
   “The page runs in its own process and a busy one cannot stall the tool” is unconditional. The added [CDP assertion](/tmp/rv-dt-frame/scripts/design-tool.test.mjs:949) meaningfully checks an out-of-process iframe in the launched Chrome instance; it is more than a URL assertion. [Chrome documents that target distinction](https://developer.chrome.com/docs/extensions/reference/api/debugger#work-with-frames). It does not establish behavior in browsers/configurations without site isolation, after navigation back to a same-site URL, or under a busy-frame workload.

   **Fix:** Qualify the supported-browser claim and extend the existing test with a bounded busy-frame/tool-interaction check.

The remaining security and compatibility paths look consistent:

- The strict URL pattern rejects paths—including `/`—credentials, case variants, trailing dots, alternate host spellings and oversized ports. These variants fail closed. `[::1]` remains accepted, but the current dev listener/gate does not provide a working IPv6 endpoint.
- The sandbox remains cross-origin and withholds top navigation. `allow-same-origin` preserves the guest’s origin; it does not grant access to the tool. `FRAME_ANCESTORS` still correctly names the **parent’s** unchanged `127.0.0.1:7740` origin; `'self'` becomes the localhost guest origin. Both dev loops apply it to HTML responses.
- Build-channel streams, reports and generation fetches use relative URLs. `developmentGate` explicitly admits localhost and matching POST origins. Registration uses the nonce/build ID, not equality with `hello.page`. Moving origins starts fresh `exact.dev.build` and `exact.dev.inspect` storage; subsequent reloads at localhost retain it when storage is available.
- Cookies for `127.0.0.1` do not transfer to localhost, and the canvas now faces cross-site cookie restrictions, including [SameSite rules](https://web.dev/articles/samesite-cookies-explained). Native/LAN clients retain their existing URLs. The server’s raw `hello.page` remains IPv4, so its existing assertions remain appropriate; I found no additional agent comparison requiring the frame to stay at IPv4.

This remains within the RFC’s iframe/server-control architecture and adds no apparatus. The origin consequences need an explicit contract alongside the performance benefit. No files were changed; browser/build tests were not run because they write fixtures and artifacts.

**Verdict: Changes requested before landing.**