# Code review: tamber tool --via's canvas check (a page that won't let the tool frame it), round 1 of 2 (grok)

- **Family:** xAI. Grok via the grok CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** round 1 of 2; HEAD `4042504ca8`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; two SHOULD-FIXes (paths, ports) and two NITs (the page origin, the wording), all taken in 9936d5baa9.

I'll review the canvas-check change against CSP3 and the approved conditions, read-only. First I'll load the review skill and the diff at HEAD.This is a read-only review of `4042504ca8` only. I'll inspect that commit and the surrounding call sites, then check CSP matching, the canvas-check endpoint, and the tool's fetch grants.The commit is small. Next I’ll read the CSP matcher, the server forward path, and the tool’s fetch grants in full.Nothing blocks. The hands-on case is diagnosed correctly: `frame-ancestors 'self' http://127.0.0.1:7740` refuses a tool on another `http://127.0.0.1` port, and the line names both sources and that tool's `EXACT_TOOL_PORT`.

1. **SHOULD-FIX** `scripts/tamber-frame.mjs:30` — A host source's path is matched by the regex and then dropped. CSP3 checks a frame ancestor as its origin URL, whose path is `/`. A missing path or a path of `/` matches that origin; any longer path does not. `frame-ancestors http://127.0.0.1:17977/__tamber/tool/` is treated as allowed here, while Chrome still refuses the frame, so the orange line stays hidden. The whole source is lowercased first; CSP compares percent-decoded path segments with their case intact.

2. **SHOULD-FIX** `scripts/tamber-frame.mjs:40` — An omitted port matches only a URL whose own port is the default (CSP3: a null port-part equals a null URL port). An `http` scheme-part also matches `https`. Line 40 compares port numbers and, when the source omits the port, uses the source scheme's default. `http://127.0.0.1` therefore rejects `https://127.0.0.1` and accepts `https://127.0.0.1:80`. The loop's real header carries an explicit `http` port and the tool is `http://127.0.0.1:<port>`, so this does not affect the reported empty canvas.

3. **NIT** `scripts/tamber-frame.mjs:50` — `'self'` should be the canvas frame, `http://localhost:<loop port>` (`pageOf` in `apps/design-tool/origins.ts`). The page used is `via` with the first `127.0.0.1` replaced. `--via` also accepts `[::1]` and any `127/8` host (`scripts/tamber-server.mjs:1338`). The refusal line then names that host. The allow/deny bit for this tool stays the same: the tool is `127.0.0.1` on a different port.

4. **NIT** `scripts/tamber-frame.mjs:56` — With no sources left (`frame-ancestors 'none'`), the line reads "lets only no page frame its page" and still ends in `EXACT_TOOL_PORT`. An app policy of `'none'` is kept beside the loop's own (`frameOnlyByTool`) and the browser enforces both, so that restart leaves the frame refused. The same sentence is in `apps/design-tool/canvas-check.ts:12`. For the real two-source header, the wording is clear and matches the contract test.

Also checked, and sound for this tool:

- Case and quoted keywords match CSP3: the source is lowercased, `'none'` does not veto sibling sources, and `'self'` is the page origin. On this call the page host is `localhost` and the tool host is `127.0.0.1`, so the extra CSP3 same-host `https`/`wss` upgrade of `'self'` would not allow the tool either.
- A canonical IPv6 source such as `http://[::1]:8080` matches (`hostname` keeps the brackets). A non-canonical or v4-mapped literal is compared as written, so it misses the URL parser's canonical host (`[0:0:0:0:0:0:0:1]` becomes `[::1]`, `[::ffff:127.0.0.1]` becomes `[::ffff:7f00:1]`). The live ancestor is IPv4.
- A trailing slash agrees with the spec, because the ancestor path is `/`.
- The canvas-check body is `{via, allowed, origins}`. `via` is `target.origin`. The tamber token and the request path are absent. A refusal is an empty 404.
- `forward()` handles `/__tamber/tool/canvas-check` before `serveTool` (`scripts/tamber-server.mjs:1421`). That prefix was already the tool's static files, so `/__tamber` wire routes are unchanged. The build has no file named `canvas-check`.
- The page fetches `${API}/__tamber/tool/canvas-check`. `net.fetch ${API}` admits that origin (`admittingGrant` in `host/web/navigation.js` compares scheme, host, and port). The browser's same-origin GET sends the tool's `Origin` and `Sec-Fetch-Site: same-origin`, which `answerCanvasCheck` allows; a missing `Origin` is allowed as well.
