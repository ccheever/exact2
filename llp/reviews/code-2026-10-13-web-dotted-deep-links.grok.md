# Code review: a navigation to a dotted deep link is the app's document, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `3a118d7b57a898ce64e842057a8939de907a6a925ade4f216b636ffcc2a67081`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MAJOR (a missing .html or .pdf a navigation names got the shell) and both MINORs taken in round 2's code; Grok's run ended without a review.

---

## MAJOR

`scripts/origin.mjs:70` (`host/web/serve.mjs:774`, `host/web/serve.mjs:625`, `host/web/dev.mjs:1162`)

`appDocumentPath` drops the extension check for every request whose `Accept` contains `text/html`. That token is what a browser sends for every document navigation, and what the native client sends on the page request (`application/vnd.exact.envelope+json, text/html;q=0.9` in `PlanURL.swift` and `host/linux/src/fetch.rs`). Probed with `readWebRequest` against a tree that contains `index.html` and `exact.json`:

| Request | Accept | Result |
|---|---|---|
| `/profile/jay.bsky.team` | browser navigation | `/index.html` |
| `/profile/jay.bsky.team` | native page | `/exact.json` |
| `/missing.html`, `/missing.png` | browser navigation | `/index.html` |
| `/missing.html`, `/missing.png` | native page | `/exact.json` |
| `/missing.png` | `image/*` or `*/*` | absent |

`serveStatic` then answers the document with 200. An `<iframe src>` of a missing `.html` file is a document navigation, so the frame gets the app shell. An `<a href>` to a missing `.html`, `.png`, or `.pdf` does too. LLP 1023 §7 still says a file-looking path such as `/missing.png` stays 404. An existing file still wins (`/assets/app.css` with a navigation `Accept` stays that file), and a subresource fetch still misses. `host/web/deep-link.test.mjs` passes and never requests a missing known type with a navigation `Accept`.

Keep the navigation gate, and still require an empty extension or one `webContentType` does not serve. `.team` is not in that map, so `/profile/jay.bsky.team` stays the document. `.html`, `.png`, `.pdf`, `.js`, `.css`, and `.json` stay 404 when the file is absent. Pin those in the new test.

## MINOR

`host/web-js/dev.mjs:225`

The first-failed-build shell still requires a path with no dot anywhere. `buildTreeFile` can return `index.html` for a dotted navigation only after that file exists. Until the first successful build, `/profile/jay.bsky.team` takes the 404 on the next line, while `/profile/alice` gets the error page. Drive this branch with the same `appDocumentPath(..., navigation)` predicate so a `text/html` request for that location gets the shell and a dotted asset fetch still 404s.

## Checked, no defect

Raw-target guards are unchanged. `navigation` is only the last conjunct, so it cannot admit a path the other checks reject. With `navigation` true, these stay false: `/.exact/x`, `/.exact/auth/callback`, `/__dev`, `/__dev/page`, `/foo/.hidden`, `/assets/%2e%2e/app.wasm`, `/%2e/post/42`, `/assets/../app.wasm`, `/post/%00/42`. The new test pins `/.exact/x` only.

`DirectoryOrigin` and `build.mjs`'s `404.html` should stay as they are. The origin adapter does not choose HTTP bodies. A published directory served by `serve.mjs` already goes through `readWebRequest`. `404.html` is what an external static host serves for an unlisted location (LLP 1048). That host does not run this predicate, and a dotted deep link already misses there the same way an extensionless one does. LLP 1023 §7 keeps that fallback out of origin reads.

`scripts/metrics.mjs:369` calls `buildTreeFile` without `Accept`. The harness only loads `/?agent=1`, which resolves as an extensionless directory index. That run is unchanged.

The call-site split is sound. `host/web/dev.mjs:1162` passes `'text/html'` or `''` into `readWebRequest`, so that function's envelope branch stays off the raw header, and `wantsEnvelope` still reads the real `Accept`. Extensionless dev negotiation is unchanged, and a native open of `/profile/jay.bsky.team` on that server becomes the live envelope. `bun test host/web/deep-link.test.mjs`: 1 pass.

NOT READY
