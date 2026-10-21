# Code review: dotted deep links round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `ee93da522a29d13ead366cbbe8aec10f4087c4fff209dbbb8b24d0dc784280f1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY (Astra), READY (Grok).
- **Disposition:** Both MINORs on the extension list and Astra's on quoted Accept parameters taken in round 3's code; Astra's MAJOR (a location whose last label is a file extension, `.zip`) is inherent to a server without the app's routes: said in the source and QUEUE.

---

## MINOR

**A navigation to a missing game asset or module file still gets the app shell.** `scripts/origin.mjs:62`

`FILE_EXTENSIONS` is what keeps a dotted navigation on a real file from falling through to `index.html`. It omits extensions this server already treats as files: `.model` and `.tex` (`host/web/serve.mjs:693`) and `.hbc` (`host/web/serve.mjs:23`). A browser navigation (`Accept: text/html`) to a missing `/assets/hero.model`, `/assets/albedo.tex`, or `/app.hbc` is `appDocumentPath` true, so `readWebRequest` and `buildTreeFile` answer `index.html` with 200. A fetch of the same path still 404s, because `*/*` does not count as HTML. Add those three to the set and cover one of them in `host/web/deep-link.test.mjs`.

## Checked

Raw-target guards are unchanged and still win when `navigation` is true. `webRequestURL` is untouched. The dot-segment, `..`, backslash, `%00`, `/.exact/`, `/__dev`, and `.`-prefixed checks in `appDocumentPath` are still applied before the extension clause. Probed with `navigation` true: `/%2e/post/42`, `/assets/%2e%2e/app.wasm`, `/.exact/%2e%2e/post/42`, `/__dev/missing`, `/.git/config`, and `/.exact/auth/callback` all stay false.

Browser navigations that should stay 404 do. An iframe `src` of `/deck/missing.html` and a link to `/assets/missing.pdf` stay missing with an HTML `Accept`, including `.HTML`. An existing `/assets/app.css` is still that file. A fetch of `/profile/jay.bsky.team` with no HTML `Accept` is still null. `/missing.png` stays false. A trailing slash on the handle still counts as the document.

`acceptsHTML` matches the round-1 cases: `TEXT/HTML` is true, `text/html;q=0` and `text/html;q=0.0` are false, `text/html;q=0, */*;q=0.8` is false, `profile="text/html"` is false, `*/*` and a missing header are false. The native opener's `application/vnd.exact.envelope+json, text/html;q=0.9` is true, so a dotted deep link still negotiates the envelope. `host/web/dev.mjs:1162` passes only `text/html` or `''` into `readWebRequest`, so that function's envelope branch stays off and the dev server's own `wantsEnvelope` check still sees the original header.

The first-failed-build page at `host/web-js/dev.mjs:225` uses the same predicate. A navigation to `/profile/jay.bsky.team` while the first build has failed gets the error page. A missing `.html` or `.pdf` does not.

`scripts/metrics.mjs:369` calls `buildTreeFile` without `Accept`. That harness only loads `/?agent=1`. Extensionless fallback still applies. The wasm branch uses `readStaticFile`, which has no document fallback. Leave that call as it is.

`DirectoryOrigin` does no HTTP routing, so it should stay as it is. A published directory served by `serveStatic` already picks up the new rule through `readWebRequest`. `build.mjs`'s `404.html` is the rendered not-found document for an external static host. Those hosts apply their own fallback. Changing that file would not fix a host that treats a dotted path as a file, and this server's unknown locations are the shell (`index.html`), which is what a client-routed profile URL needs.

`bun test host/web/deep-link.test.mjs`: 2 pass, 18 assertions.

One limit of the allowlist, not a change to make here: `.zip` is both a file extension and a real handle suffix, so `/profile/photos.zip` still 404s to a navigation. Taking `.zip` out would make a missing archive come back as the shell.

READY
