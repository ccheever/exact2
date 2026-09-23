# Review: LLP 1048 r2 (and 1048.000–1048.003), 2026-09-23 (grok)

- **Family:** xAI, `grok -p <brief> -m grok-4.7 --effort xhigh --no-subagents --permission-mode dontAsk --disable-web-search --output-format plain` (grok CLI 1.0.30), run in `/Users/ccheever/projects/exact2` at `3d64f1f3`, 2026-09-23T09:28:37Z to 2026-09-23T09:36:38Z. Brief sha256 `a86eeefc626904862c7ede6707c982c3d5cdf6b93bf8a50be8f5a3b76c0310a1`, shared with the Claude and Fable reviews of the same revision. Read-only by the brief.
- **Method:** a directional review at Charlie's request ("have grok 4.7 do a review as well … we just want to be directionally right here"), one round, mutually blind to the Claude and Fable reviews; it read Astra's review and its disposition. The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted; the brief forbade opening secrets.
- **Reviewed revision:** LLP 1048 r2 and 1048.000–1048.003 r2.
- **Disposition:** after the verbatim review (the author's, shared across the three directional reviews).

---

## Review (verbatim; the CLI's plain output, including its opening narration)

I'll review the rendering plan against the project's rules and the first review, staying at architecture level and only reading what's needed to test the claims.The first review’s findings were accepted into r2. I’ll read the revised plan and check whether those fixes actually hold, and what they still leave open.The revised plan accepts the first review’s fixes. I’ll check the code behind the claims that still decide whether phase 1 is the right cut.The end state is the right one. Keep one document projection shared with the live host, a render policy per route, adoption from a checkpoint that is not reload state, and stage and confidentiality as separate labels. exact2 can do this without the usual machinery: the tree is a function of declared inputs, and the runtime is one wasm (about 390 KB brotli; LLP 1047 measures Caltrain at 382 KiB), so per-component islands buy nothing. Server-driven UI and progressive server actions are correctly refused. They are a second interaction model, and they are still on the not-doing list. The edge should stay deferred. It will not run Interview, which is TypeScript on Hermes, not a Rust source that fits a worker.

The first slice is not that architecture. The revision answered the earlier review by growing phase 1 into an uncached native render server, an eager wasm boot, and a DOM replacement. That is the costly point on the curve, pointed at a consumer whose own rules still forbid public hosting, and it seeds the page through a path that cannot carry one. Caltrain at build is the right first code for the serializer. Interview is the right first website, but only after the product ruling below, and not through per-request server rendering.

**1. HIGH — Public pages are specified as uncached server rendering that then throws the document away.**

Interview's question, post, and profile routes are `request` with `Cache-Control: no-store` (1048.000 D11, 1048 §10). Phase 1 gives sources no cookies and an empty store (D9), so those pages depend only on the URL. Caching them does not need holes or the phase 3 cache-key design. A file written at publish time, served from the static host LLP 1030 already deploys, is the whole win. Instead each view cold-starts a runner and a fresh Hermes realm (D10). The head preloads the glue and the wasm (D3), and the client then replaces the DOM (D6). Replacement drops scroll, selection, and text fragments, which is what `scroll document` was added to get (1048.003 D4), for every reader who has JavaScript. Crawlers never needed the runtime. Loading on interaction is the only island model a single wasm supports, and it is phase 2 (1048.001 D4). Question pages have handlers, so the inferred phase 1 policy is an eager boot.

Change: static documents for public URLs, on the existing static origin. Do not preload the wasm. Load the runtime on interaction and bind the existing nodes. Add a resident render server only when a page actually varies by viewer.

**2. HIGH — Seeding via kept answers cannot hold this page, and it breaks the checkpoint rules.**

1048.000 D6 and 1048.001 D1 push the server's answers through `runner/src/runner/kept.rs`. `keep_answer` does nothing unless the resource reads the store, and it rejects anything over 8 KiB ("never a feed"). A public source that stays out of the store, which is what D3 requires, will not be seeded. The first client frame falls through to the empty placeholder. Kept answers are also device-store entries, which contradicts "the checkpoint carries no store" and "never overrides the device's store" (1048 D4). When the data module loads, the runner asks again. The client source, with an empty token, posts `replicaOnly` and gets no content. With a saved account it renders local SQLite as authenticated (`interview/app.ts`). That second fetch is how a correct server page becomes the welcome screen. Values baked into the plan cannot stand in: they are per build, not per URL, and the client plan digest must stay the one the wasm was built with.

Change: the checkpoint is its own channel, big enough for a question, not written to the store, and not refetched against the session replica. Implement the anonymous page as the replica's existing view, over a read-only snapshot and an anonymous principal. A second public interface will drift. The field names already did (`selected` / `answers` / `profile` versus `details` / `profiles`, 1048 §10).

**3. HIGH — Nothing in either repo is allowed to host this, and a late render is an indexable blank.**

Interview still excludes production hosting and anonymous or public access (`interview/rules/NOT-DOING.md`). 1048 §9.5 asks which fields are public and does not mention the hosting ban. LLP 1030's deploy is a signed static bundle and a pointer flip. A long-running binary that must match the client plan digest is a second production system, with no story for the moment seeding is refused. On a 2-second deadline the server paints placeholders (1048.000 D9) and returns 200 (D11). Google may render JavaScript later, and incompletely. Link unfurlers (Slack, iMessage, X, Discord, LinkedIn) and typical AI crawlers will not run this wasm, and they will keep the placeholder. Search and the feeds are in the phase 1 table, so the server is also an open render fan-out. The answers island is user-authored content inside HTML, and the plan never requires that JSON to be safe inside a script. A source marked public publishes its whole value: labels do not redact fields. The module's grants include SQLite and network fetch.

Change: before any server, name the public fields and take static hosting off Interview's not-doing list. Unsettled indexable URLs are 503 with `Retry-After`. Unknown items are 404. Search is `noindex`. No SQLite and no credentials on the public render. Escape the checkpoint. Keep the clock out of cache keys. D3 marks it request-time and public, and 1048.002 §5 keys on every such read, so a relative timestamp makes the cache miss forever. Stale HTML is what a cache is for.

**4. MEDIUM — Tree equality is still one viewport wide, and the document vocabulary is a four-host kernel change.**

1048.000 D4 draws viewport branches at 390×844 and corrects them after boot. Interview's rail is `width >= 900` (`app.contract`). §10.9 instead emits both structures and switches with a media query. Those are not the same document. A parity run at one Chrome size (1048.000 §4) can pass while desktop and most phones disagree, and the correction is layout shift. Both branches also lay out on iOS and macOS, which already know their width. 1048.003 D2 then installs HTML's default stylesheet for a dozen new elements on every host. The rule is right. Doing the whole stylesheet before one public URL is how native parity becomes the project. Document scrolling is safe only because it is opt-in and does not hijack app shells.

Change: responsiveness is CSS only. The inactive branch is `display: none`, so it stays out of the accessibility tree. Native hosts may drop that branch. Phase 1 elements are headings, paragraphs, lists, links, and the document title.

**5. MEDIUM — Two of the repo's own rules are still open.**

Phase 1's replace-the-DOM boot, retired by phase 2's adoption, is a migration path. The rules say delete, don't deprecate, before 1.0. The dev loop is `host/web/dev.mjs` at about 20 ms, with a 100 ms budget to present. The plan never says how a request-rendered route runs there, and a Hermes server will not fit. `llp/current/` is at its cap of 15, five of them this program, and 1048.001 and 1048.002 have no implementer. A spec nobody is assigned to build is not supposed to be in the working set.

Change: one boot path, written only as far as the first slice uses it. Produce the document from the existing dev server and the same projection. Move 1048.002 out of the working set until some page has a private region inside a public shell.

**The three changes**

1. Recut phase 1 to static public documents on the current static origin. Boot on interaction and adopt the DOM. No wasm preload, and no resident render server, until a page varies by viewer.
2. Drop kept-answer seeding. The checkpoint is store-free and large enough for a page. The anonymous render is the replica's own view, not a parallel API.
3. Do not index unsettled HTML, and get an explicit ruling that names the public fields and strikes static hosting from Interview's not-doing list before that work starts. Never key a cache on the clock.

VERDICT: DIRECTIONALLY RIGHT WITH MAJOR CHANGES

---

## Disposition (Claude, Opus 5.5 — the author), 2026-09-23

Three reviewers ran on the same brief, each blind to the others:
- Grok 4.7 (xAI);
- Fable 5.1;
- a fresh Opus 5.5 session.

Fable and Opus are Anthropic, the author's family. All three answered DIRECTIONALLY RIGHT WITH MAJOR CHANGES, and they agree on the end state:
- one document projection shared with the live host;
- a render policy per route;
- adoption from a document checkpoint that isn't reload state.

They converge on five major changes:

1. **Seeding.** Kept answers are the wrong way to carry the server's answers (all three). They seed only resources that read the store, they cap an answer at 8 KiB, and they are asked again at `data_ready` (`runner/src/runner/kept.rs:18`, `runner/src/runner.rs` ~573–593). A question page fails all three. Checkpoint answers become their own runner input:
   - kept out of the store;
   - keyed by source, arguments and logic identity;
   - sized for a page;
   - not asked again until their freshness lapses;
   - kept when a refresh fails.
2. **The first slice.** Phase 1 aimed the most expensive point on the curve at a consumer whose own rules exclude public access and production hosting (all three; `interview/rules/NOT-DOING.md` ~16–18). That point is a per-request server with no cache, a wasm boot that starts at once, and a DOM replacement.
3. **Caching.** A render that sees no cookies and an empty store depends only on its URL and public data. It is cached from the start (Claude, Grok), not served `no-store`.
4. **An anonymous server.** The server renders the public document, and the device renders the person (Fable, Claude, and Grok's "no credentials on the public render"). With no credentials and no private data on the render path, confidentiality labels, runtime read tracking and an inferred `Vary` protect nothing. Grok would keep the confidentiality label, but on such a server "private" can only mean "rendered on the device", and stage already says that.
5. **The Contract surface.** Phase 1's is too large (all three):
   - Tags with HTML defaults are kernel work on four hosts: `em` lengths, `list-item`, and classes at runtime.
   - Anonymous readers never see Interview's rail (`app.contract:413`, `when data.authenticated and wide`).
   - NOT-DOING says "roughly 15 built-in tags, not 40".

This reverses three r2 acceptances of Astra's review: rows 5 (confidentiality labels and a tracked `Vary`), 7 (tags with HTML defaults in phase 1) and 8 (width variants in phase 1). The problems those rows found still stand. What changes is the answer: nothing on the server can leak, and the tags and variants wait for a consumer.

The code claims I spot-checked hold:
- the kept path;
- `app.ts` ~132 and ~186;
- the rail's guard;
- the Linux executor sharing `executor_core.rs` by `#[path]` (`host/linux/src/executor.rs:9`);
- `renderMarkup`'s spans and `<br>` (`host/web/navigation.js:1420`);
- `readWebRequest` negotiating on `Accept` (`host/web/serve.mjs:591`);
- Interview's NOT-DOING.

One claim is misplaced. The global `h1`…`h6` reset is the web lane's commit, now on main as `44f8643c`; it isn't a `core/fixes` commit.

**What r3 will say.** This is a proposal. Charlie rules on the items marked ★.

- **1a: documents for the in-repo apps.**
  - They are rendered at build into `dist/` and served by whatever serves `dist/` today.
  - Covers the safe projection and its parity check, the head, `scroll document`, explicit route fields, the sitemap and the 404.
  - No server, no Hermes and no preload: the runtime loads when the page is idle or at the first interaction.
  - The render lane's steps 1–3 are this.
- **1b: Interview.**
  - **Gated on:**
    - ★ which content is public, including content written while Interview was invitation-only (§9.5);
    - ★ Interview's NOT-DOING lifting its public-access and production-hosting lines;
    - ★ a hosting target.
  - **Proposed topology:**
    - the Linux host in a serve mode, a native Rust binary (ruling 4), updated through delivery;
    - it holds a read-only replica of Interview's public partition, so the render path has no Node, no credentials and no private data;
    - it renders on request behind an ordinary CDN (`s-maxage`, `stale-while-revalidate`), purged by surrogate keys taken from the records a render read.
  - **Why render on request:** Grok would instead write static files when content changes. But a new question's URL has to answer at once, and the CDN keeps the index from page to record that rewriting on change would have to keep itself.
  - A render that doesn't settle answers 503 with `Retry-After`; an unknown item answers 404.
  - The server counts and budgets its startup: the module graph each render loads, the source calls, and time to first byte.
- **One boot path.**
  - The runtime adopts the document when the checkpoint digest matches, and renders fresh when it doesn't.
  - There is no replace-then-adopt migration.
  - The parts of 1048.001 that 1a uses (the checkpoint, the digest and adoption) fold into 1048.000.
- **Anonymous forever.**
  - Stage stays.
  - Confidentiality labels, runtime read tracking and an inferred `Vary` go, and so do credentialed holes and the edge.
  - Request inputs are a declared, closed list of public facts, and each one is part of the cache key.
  - Streaming waits for a measurement.
  - ★ This takes server-rendered signed-in content off the curve. A signed-in reader's parts are holes the device fills after adoption.
- **The Contract surface.**
  - Phase 1 adds `head`, `scroll document` and route fields.
  - Meaning comes from roles on plain nodes, lowered to HTML elements that keep a bare div's box, as `44f8643c` does for headings.
  - Authored tags with HTML defaults, and width and media variants, get their own kernel spec when a consumer needs them.
- **The working set.** 1048.001 and 1048.002 leave `llp/current/` until their kickoffs.

**This review's findings.** "Change N" refers to the five changes above.

| # | Finding | Disposition | Where it goes |
|---|---|---|---|
| 1 | Public pages are server renders with no cache, which the client then throws away | **Accepted, except the static-only answer for Interview.**<br>• 1a writes documents at build for the static host.<br>• Nothing preloads the wasm; the runtime loads when the page is idle or at the first interaction.<br>• One boot path adopts the document instead of replacing it.<br>• For Interview, 1b proposes rendering on request behind a CDN rather than writing files when content changes (see the topology above; ★ Charlie). | 1048 §7; 1048.000 D3, D6, D11 (r3) |
| 2 | Kept answers can't carry the page, and the client's second ask turns it into the welcome screen | **Accepted** (change 1).<br>• The anonymous page is the replica's own view: the public source returns the rows the replica's queries return.<br>• On the server, it reads a read-only replica of the public partition.<br>• There is no second public interface. | 1048.000 D6; 1048 §10 (r3) |
| 3 | Nothing may host this; a late render is an indexable blank; escaping; a public source publishes its whole value; the clock gets into cache keys | **Accepted.**<br>• ★ Charlie names the public fields and lifts Interview's NOT-DOING lines.<br>• A render that doesn't settle answers 503 with `Retry-After`; an unknown item answers 404; search is `noindex`.<br>• The render carries no credentials, and its server holds only the public partition.<br>• The checkpoint uses an encoding that can't contain `</script` or `<!--`.<br>• The checkpoint carries its render time, and no cache is keyed on the clock. | 1048.000 D5, D9–D11 (r3) |
| 4 | The tree is checked at one viewport width, and the new elements are a kernel change on four hosts | **Accepted.**<br>• Width variants leave phase 1: Interview's anonymous routes have no branch on width.<br>• Until variants exist, a route whose tree branches on width keeps r2's D4 rule: it renders at the page viewport (mobile-first), the runtime renders it fresh at a wider width, and parity is claimed only at that viewport.<br>• When variants come, the inactive branch is `display: none`, and native hosts may drop it.<br>• Phase 1's elements are the title plus roles lowered to HTML: headings, paragraphs, lists and links. | 1048.003 (r3) |
| 5 | Replace-then-adopt is a migration path; the dev loop is unspecified; the working set is full | **Accepted.**<br>• One boot path.<br>• The dev server makes documents with the same projection.<br>• 1048.001 and 1048.002 leave the working set. | 1048, 1048.000 (r3) |

**Proposed next step:**
- Charlie rules on the ★ items.
- r3 revises 1048, 1048.000 and 1048.003, and moves 1048.001 and 1048.002 out of the working set.
- The render lane continues 1a, with a policy declared on each route and no preload.
- The Interview lane continues behind its switch, which stays off by default.

No further review round is needed to settle the direction. Status stays Draft; Charlie decides.

**Update, 2026-09-23: after Charlie's rulings (r3).** Each item marked ★ is now resolved:
- **Public content:** "questions, answers, posts, profiles — but all at the preference of the user". Each author chooses, and the choice is off by default (LLP 1048 §9.5).
- **Hosting:** "for now just hosted locally, eventually on a CDN/hosting provider" (§9.6). Interview's NOT-DOING records public reading at the author's choice, and still keeps production hosting out.
- **The server's shape:** Charlie wasn't sure and asked the author to decide. It renders on request from a serve mode of the Linux host, on loopback, with headers ready for a cache (§9.7).
- **Pages for signed-in readers:** Charlie asked "why not server render the personal stuff?" They are now rendered on the server in a simple form: the whole page for that reader, `private, no-store`, never shared (§9.8, LLP 1048.002).
  - This reverses the disposition's "anonymous forever".
  - The reviews' objection still holds and that form stays out: personal parts mixed into cached pages, guarded by labels and read tracking.
- **The robots policy for AI crawlers** waits for real hosting.

r3 is in `llp/1048-rendering-across-the-curve.rfc.md` and its sub-LLPs. LLP 1048.001 became an RFC, and it and 1048.002 left the working set until their kickoffs.
