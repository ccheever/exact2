# Code review: tamber tool --via's canvas check (a page that won't let the tool frame it), round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `4042504ca8`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; six SHOULD-FIXes (paths, ports, IPv6, origin naming, the local gate, the port advice), all taken in 9936d5baa9.

No BLOCKER found.

1. **SHOULD-FIX — [scripts/tamber-frame.mjs:30](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:30):** Paths are discarded. `frame-ancestors http://127.0.0.1:17977/private/` incorrectly allows that tool origin, suppressing the warning while Chrome blocks. CSP3 matches against the ancestor’s serialized origin. [Specification](https://www.w3.org/TR/CSP3/#directive-frame-ancestors)

2. **SHOULD-FIX — [scripts/tamber-frame.mjs:40](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:40):** `http://example.test` incorrectly rejects `https://example.test`: the omitted port uses the source scheme’s default. Line 27 also misses `'self'`’s permitted HTTPS upgrade. Both produce false warnings. [CSP3 matching](https://www.w3.org/TR/CSP3/#match-url-to-source-expression)

3. **SHOULD-FIX — [scripts/tamber-frame.mjs:30](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:30):** Bracketed IPv6 sources such as `http://[::1]:17977` are accepted, although CSP3’s host-source grammar excludes them. This can falsely report framing as allowed. [Source grammar](https://www.w3.org/TR/CSP3/#framework-directive-source-list)

4. **SHOULD-FIX — [scripts/tamber-frame.mjs:14](/tmp/rv-dt-framecheck/scripts/tamber-frame.mjs:14):** `origins` echoes raw source tokens. A refusing source such as `https://example.test/private/path` exposes that path through canvas-check. Sanitize the diagnostic fields to satisfy the “no path” condition.

5. **SHOULD-FIX — [apps/design-tool/app.contract:1633](/tmp/rv-dt-framecheck/apps/design-tool/app.contract:1633):** The local-loop warning also renders after pairing with a remote server, whose canvas uses a different gateway. Gate it on `hello.local`.

6. **SHOULD-FIX — [apps/design-tool/canvas-check.ts:12](/tmp/rv-dt-framecheck/apps/design-tool/canvas-check.ts:12):** The unconditional `EXACT_TOOL_PORT` remedy cannot fix an authored `'none'` or another rejecting policy. Qualify that advice in both UI and terminal wording.

Forwarding placement, fetch grants, and request guards look sound. No changes, builds, or tests performed.
