# Code review: LLP 1122 T2 step 1 (the canvas, hello.page, framing), round 3, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `0cb6a4eb0` (commit `47a9b3607` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `eb3ba1533019cf3436be6d40ac51b294940ef7178801dcb455e4d6db4d6a16d1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one low.
- **Disposition:** taken, applied without re-review (round 3 was the last): `pageOf` catches a port the URL parser refuses and gives no page, with a stub case (port 70000) that also checks the rest of hello stands; `frameOnlyByTool` treats a response as HTML when any Content-Type it would send says so (a repeated one included), with a unit case; the canvas drive pauses on the real clock while the tool is open and waits for the framed page's dev stream there, rather than reading the log after it closed. Not taken: a known element of the guest asserted inside the frame (the tool never scripts the frame, and the agent's tree does not enter it; step 2's drive checks the frame's build registers and reports). Grok's first run died mid-review (its context compacted); it was run again on the same brief and worktree.

---

No High or Medium findings. One Low.

**Low — `pageOf` throws on a port the regex accepts, and that fails hello.** `apps/design-tool/app.ts:87`

`pageOf` treats anything that is not a loopback origin other than the tool as none, but the check is `/\d{1,5}$/` and then an unguarded `new URL(page)`. Ports 65536–99999 match the regex, and `new URL` throws `TypeError` on a port above 65535. `loadHello` calls it with no try (`apps/design-tool/app.ts:655`), so the promise rejects and the `hello` resource fails (`app.contract:339` keeps `else empty(ok=true)`). That placeholder has `ok` true, so the no-server panel stays hidden and `advance` does not `refresh hello` (`app.contract:411` only refreshes when `not hello.ok`). The tool sits on an empty hello.

Scenario: a hello with `page: "http://127.0.0.1:65536"` (or `99999`). The dev loops never emit this — they publish the port they bound — so it takes a non-conformant server. Fix: catch the `TypeError` and return `''`.

The rest of the step holds. Both dev loops call `frameOnlyByTool` before any write (`host/web/dev.mjs:1144`, `host/web-js/dev.mjs:257`). Every `text/html` answer goes through that wrapper: object headers, status message, flat list, pairs, `setHeader` then `writeHead`, implicit `end`, `HEAD`, the JS loop’s forwarded `answer.headers`, and a later error after the head is already sent. JSON, event-stream, 204, and the other non-HTML replies leave the head alone. An existing `Content-Security-Policy` stays beside `frame-ancestors 'self' http://127.0.0.1:7740` (intersection only narrows), and `X-Frame-Options` is removed. The wasm loop publishes `hello.page` as its own origin (`host/web/dev.mjs:281`). Production serving is unchanged.

The canvas sandbox is `allow-scripts allow-same-origin allow-forms allow-popups allow-modals allow-downloads` (`app.contract:627`). Without `allow-popups-to-escape-sandbox`, a popup stays sandboxed and cannot navigate the tool’s top window. `pageOf` rejects `http://127.0.0.1:7740`, including the leading-zero form, because it compares `URL.origin`.

`toolCommand` parses `--via` and refuses a non-loopback URL or the tool’s own origin before build and before reuse (`scripts/tamber-server.mjs:844`). Reuse requires the process on 7740 and the `--via` target to report the same hello epoch, so a second identical `--via` (including port 80 and a null page) is kept, and a standalone tool or a different loop is refused. The 7740 cases live in `scripts/design-tool.test.mjs`, and the canvas contract tests reload after setting `page`.

Residual gaps, not defects: the drive checks the sandbox token sequence in `app.plan`, not the live attribute or the open-link href; `HEAD`, the auth callback, generation HTML, and the JS build-error page are not fetched on a live loop, and they share this `writeHead`.

Verdict: one Low in `pageOf`; the framing header, sandbox, and `--via` epoch reuse match the step.
