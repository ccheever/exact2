# Code review: a navigation to a dotted deep link is the app's document, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `3a118d7b57a898ce64e842057a8939de907a6a925ade4f216b636ffcc2a67081`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** Astra's MAJOR (a missing .html or .pdf a navigation names got the shell) and both MINORs taken in round 2's code; Grok's run ended without a review.

---

Reviewed HEAD `09d4a0209`.

- **MAJOR — [scripts/origin.mjs:70](/Users/admin/x2p2/apps/exact2-comp/scripts/origin.mjs:70): Missing document assets now return the app shell with 200.** Reproduced for `/deck/missing.html` and `/assets/missing.pdf` with navigation Accept headers. Iframes also send HTML Accept headers ([Fetch Standard](https://fetch.spec.whatwg.org/#main-fetch)); Caltrain uses `/deck/index.html` in an iframe. A missing deck or linked download therefore silently becomes the app. Preserve 404s for known asset paths before applying document fallback, and cover these cases in the regression test.

- **MINOR — [host/web/serve.mjs:625](/Users/admin/x2p2/apps/exact2-comp/host/web/serve.mjs:625), [774](/Users/admin/x2p2/apps/exact2-comp/host/web/serve.mjs:774): The Accept regex misclassifies requests.** Reproduced: `text/html;q=0` and `application/json;profile="text/html"` enable fallback, while `TEXT/HTML` does not. Use one shared predicate that matches the actual media type, case-insensitively, with positive quality. [RFC 9110](https://httpwg.org/specs/rfc9110.html#field.accept) defines these semantics.

- **MINOR — [host/web-js/dev.mjs:225](/Users/admin/x2p2/apps/exact2-comp/host/web-js/dev.mjs:225): Dotted routes still miss first-build error recovery.** This is an unchanged gap: when the initial build fails, `/profile/jay.bsky.team` receives 404 without the error overlay or reload subscription. Apply the shared document predicate here too.

The raw-target guards, private-path exclusions and existing-file precedence remain intact. The resident dev server retains its own envelope negotiation.

Publication note: `DirectoryOrigin` needs no separate routing change—its HTTP serving already uses `serveStatic` → `readWebRequest`, so this behavior changes transitively. Leave [build.mjs:421](/Users/admin/x2p2/apps/exact2-comp/host/web/build.mjs:421)’s generated `404.html` unchanged; external static hosts need appropriate routing configuration, which generated HTML cannot implement.

[metrics.mjs:369](/Users/admin/x2p2/apps/exact2-comp/scripts/metrics.mjs:369) still omits Accept. Passing it would maintain consistency, but today’s measurement navigates only to `/`, so it remains unaffected.

Read-only probes reproduced the findings; 12 guard cases passed. Published routing was exercised with an in-memory inventory. The Bun regression test was blocked at setup by `EPERM` creating its temporary directory. No files changed.

**NOT READY**
