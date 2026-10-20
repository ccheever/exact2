# Code review: the canvas frames a 127.0.0.1 page as localhost (LLP 1122 T2 follow-up), round 2, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `d63853859`.
- **Method:** one brief (sha256 `e265a40b79e8c78d9c8a6e3a608f3cb39424ec56c63cb1f31815788c57e3cac7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (1 medium).
- **Disposition:** taken, but Astra 4 in part (no busy-frame workload test, as in round 1).
  - A taken [::1] port now stops the dev loop, as a taken IPv4 port does (`alsoOnIPv6Loopback` rejects with EADDRINUSE). The automatic port (`freePort`) must be free on [::1] too. host/web/tests/ipv6-loopback.test.mjs covers both, and the T2 drive checks that the [::1] answer carries this loop's `frame-ancestors`.
  - A `[::1]` page is now framed as localhost and opened at 127.0.0.1, the address the gate admits (Grok low 1).
  - D3 and the as-built canvas note say the frame is `localhost` on the dev loop's port and keeps its own storage, and so does the README's first canvas sentence (Grok low 2).

---

No High finding established. **One Medium issue remains from round one.**

1. **Medium — An IPv6 port conflict still sends the canvas to another server.** [host/web/serve.mjs:927](/tmp/rv-dt-frame-r2/host/web/serve.mjs:927)

   If another service owns `[::1]:8765` while `127.0.0.1:8765` is free, the new listener logs `EADDRINUSE` and the dev loop continues normally. Its advertised page still becomes `http://localhost:8765` in the canvas. A browser connecting successfully over IPv6 therefore loads that other service; it has no reason to fall back to IPv4. `--lan` has the same problem. The automatic port selection also probes only IPv4.

   **Fix:** Await ownership of both listeners before advertising readiness. On an IPv6 conflict, fail an explicit port or select another automatic port; alternatively, withhold the frame URL. Add the competing IPv6-listener case requested in round one. The new successful-GET test does not exercise it.

The other round-one dispositions check out:

- **URL validation:** [app.ts:92](/tmp/rv-dt-frame-r2/apps/design-tool/app.ts:92) canonicalizes numeric ports and rejects port 7740 for all three accepted hosts, including padded ports. Case variants, trailing dots, paths, credentials, alternate IPv4 spellings and invalid ports fail closed. Read-only evaluation confirmed these cases.
- **Sandbox and CSP:** The frame remains cross-origin, and `allow-same-origin` grants no access to the parent. Top navigation remains prohibited. [FRAME_ANCESTORS](/tmp/rv-dt-frame-r2/scripts/tamber-server.mjs:877) correctly names the unchanged parent, `127.0.0.1:7740`; both dev loops apply it. The tool’s localhost redirect preserves that parent origin.
- **Canonical URL:** “Open in a tab” again uses the printed IPv4 URL, preserving the fallback and compatibility with the server’s address-only `--via` checks. I found no remaining agent or protocol comparison requiring the canvas URL itself to use IPv4.
- **Origin consequences:** `exact.dev.build` and `exact.dev.inspect` start separately at localhost, then survive that frame’s reloads when storage is available. Storage and BroadcastChannel are also partitioned by the embedding site. Cookies/authentication at the printed URL do not transfer. Relative dev streams, reports and generation fetches follow localhost; registration uses its nonce and build ID, not `hello.page`. Absolute application requests retain their URLs and can encounter different CORS or credential behavior. The README now acknowledges the storage split. [Chrome’s partitioning documentation](https://privacysandbox.google.com/cookies/storage-partitioning)

One existing IPv6 limitation remains: [pageOf’s literal `[::1]` branch](/tmp/rv-dt-frame-r2/apps/design-tool/app.ts:96) is accepted, but [developmentGate](/tmp/rv-dt-frame-r2/scripts/install-page.mjs:276) rejects a literal `[::1]:<port>` Host with 421. The added test deliberately supplies `Host: localhost`, so it proves IPv6 transport for localhost only. Supporting literal IPv6 URLs would require admitting that exact loopback origin in the gate.

The [CDP assertion](/tmp/rv-dt-frame-r2/scripts/design-tool.test.mjs:954) meaningfully checks an out-of-process iframe in the launched Chrome instance; it is more than a URL assertion. It does not test busy-frame responsiveness, other browsers, or isolation after navigation. The browser qualification and RFC amendment address the documentation finding; the omitted workload test remains the recorded partial disposition. [Chrome’s target model](https://developer.chrome.com/docs/extensions/reference/api/debugger#work-with-frames)

The change preserves the RFC’s iframe/server-control architecture and stays within the source-size limits. No files were changed; builds and browser tests were not run.

**Verdict: Changes requested—logging an IPv6 bind conflict does not make the localhost frame belong to this dev loop.**