# Code review: LLP 1122 T2 step 1 (the canvas, hello.page, framing), round 3, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `0cb6a4eb0` (commit `47a9b3607` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `eb3ba1533019cf3436be6d40ac51b294940ef7178801dcb455e4d6db4d6a16d1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 2 low).
- **Disposition:** taken, applied without re-review (round 3 was the last): `pageOf` catches a port the URL parser refuses and gives no page, with a stub case (port 70000) that also checks the rest of hello stands; `frameOnlyByTool` treats a response as HTML when any Content-Type it would send says so (a repeated one included), with a unit case; the canvas drive pauses on the real clock while the tool is open and waits for the framed page's dev stream there, rather than reading the log after it closed. Not taken: a known element of the guest asserted inside the frame (the tool never scripts the frame, and the agent's tree does not enter it; step 2's drive checks the frame's build registers and reports). Grok's first run died mid-review (its context compacted); it was run again on the same brief and worktree.

---

1. **Medium — The canvas test races the guest’s load.** [scripts/design-tool.test.mjs:692](/tmp/rv-dt-t2a-r3/scripts/design-tool.test.mjs:692) checks `page connected` after `drive()` finishes and closes the browser. Its `clock data` settles the tool’s resources; it does not wait for the iframe’s requests. A delayed guest response can therefore make a correct canvas fail this test. Conversely, the dev loop’s build-error page also opens that stream, so the assertion does not establish that the app booted.

   **Fix:** keep the browser session open, await a bounded guest-ready condition, and assert a known element from the fixture before closing it.

2. **Low — Repeated Content-Type values can bypass the framing policy.** [scripts/tamber-server.mjs:789](/tmp/rv-dt-t2a-r3/scripts/tamber-server.mjs:789) examines only the first matching header. For example, `writeHead(200, ['Content-Type', 'text/plain', 'Content-Type', 'text/html'])` takes the unchanged forwarding branch and receives no CSP. Browser MIME extraction selects the later valid `text/html` value. [Fetch specification](https://fetch.spec.whatwg.org/#concept-header-extract-mime-type)

   **Fix:** reject conflicting Content-Type values or normalize the effective MIME type before deciding whether to add CSP. Cover repeated raw headers and array-valued stored headers. No current route emits these conflicting values; this is a remaining helper gap, not a demonstrated exposure in today’s routes.

3. **Low — Invalid ports throw from `pageOf` instead of producing an empty page.** [apps/design-tool/app.ts:87](/tmp/rv-dt-t2a-r3/apps/design-tool/app.ts:87) accepts five-digit ports syntactically, then calls `new URL()` without catching failure. A forwarded hello containing `http://127.0.0.1:70000` passes the regex but throws, failing the entire `loadHello` resource and retaining its fallback or previous value. [URL specification](https://url.spec.whatwg.org/#port-state)

   **Fix:** catch parsing failures and return `''`; add a malformed-port case that also verifies the remaining hello fields survive.

By inspection, the current HTML routes in both loops reach the wrapper, including HEAD, forwarded HTML, install/open/callback pages and error-page HTML. Production serving remains unchanged. The sandbox correction, own-origin rejection, epoch-based reuse, fixed-port test consolidation and contract-test reloads address their prior findings. The rendered sandbox and link destination remain untested, as the disposition acknowledges.

No files were changed; builds and tests were not run.

**Verdict: Request changes — 1 Medium, 2 Low; no High findings.**
