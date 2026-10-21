# Code review: LLP 1122 T2 step 1 (the canvas, hello.page, framing), round 2, 2026-10-16 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `10b70a9bb` (commit `47a9b3607` and its round-1 fixes).
- **Method:** one brief (sha256 `17436dbaf6680c5ace7c67ec1c0884e1cfeb800bb6dc2165fcf4d560a5ad67e2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 medium, 1 low).
- **Disposition:** taken: the sandbox drops `allow-popups-to-escape-sandbox`, so a popup the guest opens is sandboxed too and cannot navigate the tool (an OAuth popup that needs a normal window is the open-in-a-tab link's); the canvas never frames the tool's own origin (`pageOf`); `tamber tool --via` is validated before any reuse, and a running tool is reused only when it forwards to the same server run as the --via target (the same `epoch`), so the identical --via is reused (a dev loop on port 80, or a standalone `tamber serve`, alike) and another target or a standalone tool is refused; `frameOnlyByTool` takes a head's headers in every form writeHead does (object, flat list, pairs), moves them to the response's own, keeps a policy already there beside FRAME_ANCESTORS (both are enforced, so it only narrows) and removes X-Frame-Options; the tool on port 7740 left tamber-server.test.mjs (the async lane runs it beside design-tool.test.mjs): the --via page, reuse and refusals are in design-tool.test.mjs; the stub's foreign-host case reloads after setting the page, and the tool's own origin is refused; the real-server drive checks the tablet size too and that the framed page opened its dev stream. Not tested: the sandbox and the link's href as rendered (the agent's tree carries neither; the built plan is checked for the sandbox).

---

The framing header is on the HTML both dev loops actually send, and `pageOf` keeps the canvas on a loopback origin. Two round-2 holes remain: the sandbox still lets the guest take the tool window, and `tamber tool --via` refuses a second run against a loop on port 80.

## Medium

**The canvas sandbox still lets the guest navigate the tool.** `apps/design-tool/app.contract:627` sets `sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox allow-modals allow-downloads"`. Omitting `allow-top-navigation` blocks `top.location` and a form or link aimed at `_top`. `allow-popups-to-escape-sandbox` then opens a popup that is not sandboxed, with `window.opener` set to the iframe.

Scenario: the app under design, or any script it loads, runs `window.open('about:blank')` and that popup assigns `opener.top.location = 'https://evil.example/tool'`. The popup is a normal window, so the sandbox flag on the iframe is not consulted. The tool at `http://127.0.0.1:7740` and the app are different origins, so the popup cannot read the tool. It can still replace the window. Neither dev loop sends `Cross-Origin-Opener-Policy`, so `opener` stays set. The plan check in `scripts/design-tool.test.mjs:675` only asserts that this token string was compiled in.

Fix: drop `allow-popups-to-escape-sandbox`. Popups then stay sandboxed and cannot navigate the tool. If an OAuth popup has to be a normal window, the tool document needs a policy that clears `opener` (`Cross-Origin-Opener-Policy: same-origin` on the tool response); the sandbox token alone does not do that.

**`tamber tool --via` refuses a second run of the same loop when the port is 80, and an unparseable `--via` reuses a tool that has no page.** `scripts/tamber-server.mjs:848-850` compares `hello.page` to `new URL(via).origin`. The loops set `page` by concatenation, `http://127.0.0.1:${port}` (`host/web/dev.mjs:281`, `host/web-js/dev.mjs:252`). `URL.origin` omits the default port, so those strings differ for port 80. If `new URL(via)` throws, `wanted` stays `null` and the `catch` comment says the command is refused below. When the running tool's `page` is also `null`, `null !== null` is false and the command falls through to reuse.

Scenario: a dev loop on `--port 80` prints `http://127.0.0.1:80/`. `tamber tool --via http://127.0.0.1:80` starts and the canvas frames that origin. The same command again exits 1 with "already serves this design through the dev loop at http://127.0.0.1:80, not through http://127.0.0.1:80". Separately, a standalone tool is already up (`page` null) and the user passes `--via http://127.0.0.1:99999` (the constructor throws). The command reuses that tool and leaves `page` null. A normal port such as 8765 matches and is reused. The test at `scripts/tamber-server.test.mjs:417-421` only refuses a different port (`127.0.0.1:1`).

Fix: parse both sides and refuse unless both parse and their origins are equal.

```js
let wanted = null, have = null;
try { wanted = via && new URL(via).origin; } catch { /* refuse */ }
try { have = there.page && new URL(there.page).origin; } catch { /* refuse */ }
if (via && (!wanted || have !== wanted)) { /* the existing error */ return 1; }
```

## Low

**`frameOnlyByTool` misses two ways Node writes a head, and a permissive policy already on the response stays.** `scripts/tamber-server.mjs:782-784` takes the first non-array object in the `writeHead` arguments and does nothing when `content-security-policy` is already set. Node also accepts a flat list (`['Content-Type','text/html',…]`) and a list of pairs. Both are arrays, so the finder skips them. A CSP set with `setHeader` before `writeHead` makes `hasHeader` true, so the permissive value is left in place and the header object is not cleaned.

Scenario: a route does `res.writeHead(200, ['Content-Type','text/html','Content-Security-Policy','frame-ancestors *'])`, or `res.setHeader('content-security-policy','frame-ancestors *')` and then `writeHead` with `text/html`. The response is HTML and frameable by any origin. I probed these on this machine's Node: both keep `frame-ancestors *`. The same probe shows the forms these servers actually use are covered: object headers, a status message plus object, `setHeader` then `writeHead`, an implicit head from `end`, `HEAD`, and `writeHead(status, incoming.headers)` as the JS loop forwards at `host/web-js/dev.mjs:380` (a permissive CSP on that object is replaced; `X-Frame-Options`, which neither loop sets, is copied through). No dev-loop call site uses the array form or pre-sets CSP, so today's install page, `/__dev/open`, the auth callback (`text/html` from `webContentType`), generation HTML, the tool page, and the JS build-error page do get `FRAME_ANCESTORS`. The unit test at `scripts/tamber-server.test.mjs:352-368` claims every way of writing a head and does not include the two forms that fail.

Fix: derive the content type and drop any CSP from object and array header lists, and on an HTML response always `setHeader` `FRAME_ANCESTORS` over a CSP that is already there. Strip `X-Frame-Options` on that same path so a forwarded one cannot override the policy.

Round 1's other fixes hold. `'self'` is still required, because `frame-ancestors` checks the whole ancestor chain, and the protocol page says that. `pageOf` (`apps/design-tool/app.ts:86`) accepts only `http://127.0.0.1:<port>` or `http://[::1]:<port>` with no userinfo, path, query, or fragment. The empty state tells you to stop the standalone tool before `--via`. Both loops call `frameOnlyByTool` before any other write on that response.

Verdict: Request changes — the sandbox does not stop the guest from navigating the tool, and a second `--via` to a loop on port 80 is refused.
