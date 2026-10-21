# Code review: dotted deep links round 2, 2026-10-13 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `ee93da522a29d13ead366cbbe8aec10f4087c4fff209dbbb8b24d0dc784280f1`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY (Astra), READY (Grok).
- **Disposition:** Both MINORs on the extension list and Astra's on quoted Accept parameters taken in round 3's code; Astra's MAJOR (a location whose last label is a file extension, `.zip`) is inherent to a server without the app's routes: said in the source and QUEUE.

---

- **MAJOR — [scripts/origin.mjs:85](/Users/admin/x2p2/apps/exact2-comp/scripts/origin.mjs:85): valid dotted profile locations still 404.** `/profile/alice.zip` with `Accept: text/html` returns 404 because `.zip` is excluded globally. [`.zip` is a delegated TLD](https://www.iana.org/domains/root/db/zip.html), and [ATProto handles use DNS hostnames](https://atproto.com/specs/handle). The suffix alone cannot distinguish this route parameter from a download. File exclusions need to respect app routes or file namespaces.

- **MINOR — [scripts/origin.mjs:62](/Users/admin/x2p2/apps/exact2-comp/scripts/origin.mjs:62): missing build files still receive the shell.** The list omits existing build formats including `.hbc`, `.model`, and `.tex`. Probes of missing `/app.hbc` and `/assets/missing.model` with HTML Accept returned `index.html` and 200. Missing linked downloads therefore remain misclassified. Account for generated filenames and file namespaces; extending this list alone will keep missing arbitrary asset formats.

- **MINOR — [scripts/origin.mjs:68](/Users/admin/x2p2/apps/exact2-comp/scripts/origin.mjs:68): Accept parsing still mistakes parameter contents for media types.** `acceptsHTML('application/json;profile="a,text/html,b"')` returns `true`, causing a dotted request to receive the shell despite never accepting HTML. Splitting must respect [quoted strings and parameters](https://httpwg.org/specs/rfc9110.html#quoted.strings), including escaped quotes.

The ordinary `.team` navigation, missing `.html`/`.pdf`, and guarded-path probes behave correctly. HEAD fixes the three reported Accept examples. The first-failed-build branch now shares the predicate, and the resident dev server retains its separate envelope negotiation.

Requested notes:

- [metrics.mjs:369](/Users/admin/x2p2/apps/exact2-comp/scripts/metrics.mjs:369) still omits Accept. Its current browser load targets `/`, so this does not break today’s measurement; passing Accept would preserve serving parity.
- `DirectoryOrigin` needs no routing change: published files served through `serveStatic` already receive the updated fallback.
- [build.mjs:421](/Users/admin/x2p2/apps/exact2-comp/host/web/build.mjs:421)’s `404.html` should remain a not-found document. External static hosting needs the corresponding request rewrite policy; changing that file cannot implement Accept-dependent routing.

Verification: read-only Bun probes reproduced the findings; published-origin probes used mocked storage reads. The committed Accept test passed, while the integration fixture was blocked by sandbox `EPERM` at temporary-directory creation. Working tree unchanged.

**NOT READY**
