# Code review: the canvas frames a 127.0.0.1 page as localhost (LLP 1122 T2 follow-up), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `42984b043`.
- **Method:** one brief (sha256 `dbb99c18f1f0f9acf2c13fa5073b52c54d34c81d37609d3c64a53a2215807178`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** approve with nits (2 low).
- **Disposition:** taken. docs/tamber-protocol.md's `page` says the tool opens it as given and frames it as localhost, with the frame's own storage. The [::1] checks in host/web/tests/ipv6-loopback.test.mjs and the T2 drive run only where the machine has IPv6, and a machine without it serves IPv4 alone. The busy-frame workload test is still left out, as in rounds 1 and 2.

---

No High or Medium findings. Two Low findings remain.

1. **Low — The tests require IPv6 despite the implementation’s IPv4-only fallback.** [host/web/tests/ipv6-loopback.test.mjs:15](/tmp/rv-dt-frame-r3/host/web/tests/ipv6-loopback.test.mjs:15), [scripts/design-tool.test.mjs:761](/tmp/rv-dt-frame-r3/scripts/design-tool.test.mjs:761)  
   On a machine with IPv6 disabled, `alsoOnIPv6Loopback` intentionally resolves `null`, but the unit test requires a truthy listener. The canvas drive likewise unconditionally requests `::1`, although the browser could successfully use IPv4. Correct fallback behavior therefore fails these tests.  
   **Fix:** Detect unsupported IPv6 explicitly, verify the fallback, and condition only the IPv6 transport assertions on availability. Keep `EADDRINUSE` a failure.

2. **Low — The protocol guide still identifies `hello.page` as the framed origin.** [docs/tamber-protocol.md:147](/tmp/rv-dt-frame-r3/docs/tamber-protocol.md:147)  
   An author following this description embeds the printed IPv4 URL and recreates the same-site behavior this commit fixes. It also obscures the canvas’s separate storage. The README and RFC were corrected, but this author-facing guide was missed.  
   **Fix:** Describe `page` as the printed/tab URL and explain the tool’s localhost rewrite and storage separation. `frame` is tool-local state, not a new protocol field.

The prior functional fixes check out:

- **URL validation:** [app.ts:93](/tmp/rv-dt-frame-r3/apps/design-tool/app.ts:93) reconstructs both URLs from a validated numeric port and excludes 7740 for all accepted hosts. Read-only evaluation covered every port from 0 through 65536 for all three hosts, plus padded ports, case, trailing dots, paths, credentials and alternate address spellings. No bypass was found. Literal `[::1]` now becomes a working localhost frame and IPv4 tab link.
- **Listener ownership:** [serve.mjs:926](/tmp/rv-dt-frame-r3/host/web/serve.mjs:926) rejects IPv6 conflicts; both dev loops await it before printing their URLs. [freePort](/tmp/rv-dt-frame-r3/host/web/dev.mjs:62) checks both families. `--lan` retains its IPv4 wildcard listener and adds only IPv6 loopback. The competing-listener regression is covered.
- **Security:** The unchanged sandbox withholds top navigation. `allow-same-origin` preserves the guest’s origin without granting parent access. [FRAME_ANCESTORS](/tmp/rv-dt-frame-r3/scripts/tamber-server.mjs:877) still correctly names the parent’s IPv4 origin. Both loops apply it, and the gate admits localhost requests from `::1`, including matching-origin POSTs.
- **Origin consequences:** `exact.dev.build` and `exact.dev.inspect` start separately at localhost and survive that frame’s reloads when storage is available. BroadcastChannel and storage are partitioned by the embedding site. IPv4 cookies/authentication do not transfer. Relative streams, reports, assets and generation fetches follow localhost; registration uses nonce/build ID, not `hello.page`. Absolute application requests can encounter changed CORS or credential behavior. The preserved IPv4 tab link remains compatible with address-only `--via`; I found no remaining agent comparison requiring the canvas URL to be IPv4. [Chrome’s partitioning documentation](https://privacysandbox.google.com/cookies/storage-partitioning)

The [CDP assertion](/tmp/rv-dt-frame-r3/scripts/design-tool.test.mjs:954) meaningfully checks an out-of-process iframe in the launched Chrome instance. It does not prove busy-frame responsiveness, other browsers, or isolation after navigation. The omitted workload test remains the explicitly recorded partial disposition. [Chrome’s target model](https://developer.chrome.com/docs/extensions/reference/api/debugger#work-with-frames)

RFC D3 now matches the implementation; changed source files remain below 1,500 lines. No files were changed. Builds, listener tests and browser drives were not run under the read-only restriction.

**Verdict: Approve with nits; the previous blocking findings are resolved.**