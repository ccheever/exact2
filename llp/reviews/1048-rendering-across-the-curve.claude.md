# Review: LLP 1048 r2 (and 1048.000–1048.003), 2026-09-23 (claude)

- **Family:** Anthropic — Claude Opus 5.5, a fresh Claude Code subagent (general-purpose, the parent session's model, read-only by instruction), run on this machine against `/Users/ccheever/projects/exact2` at `3d64f1f3` and `~/projects/interview`, 2026-09-23T09:28:40Z to 09:42:50Z; 55 tool calls. The same brief as the Grok and Fable reviews (sha256 `a86eeefc626904862c7ede6707c982c3d5cdf6b93bf8a50be8f5a3b76c0310a1`).
- **Method:** Charlie asked the author to "do your own review". The session that wrote the plan can't review it independently (LLP 0005), so this is a fresh session of the author's model without the authoring context: one round, blind to the Grok and Fable reviews; it read Astra's review and its disposition. It is the same model and family as the author — the least independent of the three. Nothing redacted; the brief forbade opening secrets.
- **Reviewed revision:** LLP 1048 r2 and 1048.000–1048.003 r2.
- **Disposition:** after the verbatim review (the author's, shared across the three directional reviews).

---

## Review (verbatim)

**Overall assessment**

The core bet is right, and it's exact2's real advantage. The runner is deterministic, there is one Rust DOM projection, and data enters through declared sources. That makes "one renderer, a render policy per route" far cheaper than it is for React frameworks. Adoption checked by a digest is also a sounder contract than hydration. Keep both.

Four things need to change, and they're about sequencing and scope rather than direction:
- Phase 1 is gated on a consumer, Interview, that can't be public or hosted yet.
- Its seeding mechanism can't carry that consumer's content.
- Part A's new tags and width variants are really kernel work.
- Phase 3 builds server machinery driven by inference, which a local-first app doesn't need. Meanwhile the cheapest large win, plain HTTP caching of public documents, is switched off.

What I'd do differently is narrow it: **the server renders the public document, and the device renders the person.** The render server never sees a credential, caching is standard HTTP, and private parts are client holes filled after adoption. The plan already has that last mechanism.

**Major issues**

1. **HIGH: phase 1 is gated on Interview decisions and infrastructure that don't exist.**
   - **Evidence:** Interview's `rules/NOT-DOING.md` rules out public access *and* production hosting ("this is local development"). Its README says there is "no production hosting or identity service yet", and §9.5 is still open. Every signed-in screen reads a local Snapback4 replica, so each public page needs a new backend projection. Link previews can't be observed without a public URL.
   - **Change:** split phase 1.
     - 1a: build-time documents, head, `dist/` and parity for the in-repo apps, with no server and no Hermes (the plan's own steps 1–2). It ships on its own.
     - 1b: the server and Interview, gated on the §9.5 ruling and a named hosting target.

2. **HIGH: the seeded boot uses a mechanism that can't carry it, and the device's takeover can erase the server's content.**
   - **Evidence:** 1048.000 D6 seeds through the kept-answers path. At boot, that path consults only store-reading resources (`runner/src/runner.rs` ~576–593). It reads from the device store, caps an answer at 8 KB of hex ("never a feed", `kept.rs:18`), and re-asks at `data_ready`.
     - A public source reads no store, so it is skipped entirely. A single 6,000-character Interview post already exceeds the cap.
     - On re-ask, Interview answers a network failure with an empty, error-bearing value (`app.ts:186`).
     - For a signed-in reader, `app` switches to the replica, which may show "Opening saved conversations…" (`app.ts:132`).
     - Googlebot indexes the DOM after JavaScript runs, so these regressions get indexed.
   - **Change:** build the checkpoint's answers in phase 1 as a first-class runner input:
     - keyed by source, arguments and logic identity;
     - kept outside the store;
     - not re-asked until its freshness lapses, and kept when a refresh fails.

     Extend "never less than the server" to the moment the device's store takes over. Add a signed-in reader with a cold replica to the acceptance cases. Phase 2 then has nothing to delete.

3. **HIGH: caching is off where it's free, and the first production service has no operations design.**
   - **Evidence:**
     - Phase-1 renders get no cookies and an empty store (1048.000 D9), yet documents are served `no-store` (D11). So every crawler and unfurler hit is a fresh-realm render. `no-store` also keeps pages out of the back/forward cache in several browsers.
     - Nothing in the workspace serves HTTP natively; `ibex2` is a client.
     - D10 bounds the work, but says nothing about the deployment unit, health checks, metrics, panic containment, draining on deploy, or version skew between open pages, assets, checkpoints and the data endpoint.
   - **Change:** from phase 1, serve public documents with `public, s-maxage, stale-while-revalidate` and surrogate keys, behind an ordinary CDN. Phase 3's `cached` then becomes headers plus purge-by-tag, not a server cache store or KV. Specify the deployment unit, observability and a skew policy in 1048.000.

4. **MEDIUM: Part A is kernel work on four hosts, and it contradicts web work already in progress.**
   - **Evidence:** r2 accepted HTML defaults (Astra's finding 7).
     - Those defaults need `em` lengths, but the kernel's `Dimension` is Auto/Points/Percent/Env. They also need `display: list-item` with markers and counters, and the kernel's display values are block/flex/grid/none.
     - Width variants need classes at runtime. But `class=` is flattened into each node's rows at compile time (`contract/lower/src/lib.rs` ~678), and the web host writes those rows as inline `cssText` (`glue.js:607`).
     - Commit `5db4a7e4` on the `core/fixes` branch adds a global `h1…h6 { margin:0; font-size:inherit; font-weight:inherit }`. It sits beside main's existing `button, input, textarea { all: unset }`. Together they would strip an authored `h1`'s defaults on the web only, breaking parity by construction.
   - **Change:**
     - Phase 1 ships meaning as roles on plain nodes, the way `5db4a7e4` does for headings. That is enough for crawlers and screen readers.
     - Limit the resets now to elements standing in for plain nodes.
     - Give UA defaults and media variants their own kernel spec with an implementer, and accept a mobile-first document in phase 1.

5. **MEDIUM: Interview's main content would be semantically flat.**
   - **Evidence:** posts and answers are `text … markup="markdown"` (`app.contract` ~783–841). D1 keeps Markdown as "spans and anchors", and `renderMarkup` (`navigation.js:1420`) emits styled spans and `<br>`. There are no paragraphs, headings, lists, quotes or code blocks.
   - **Change:** the shared Rust projection emits semantic HTML for Markdown blocks, in both the live page and the served document.

6. **MEDIUM: the plan infers where it should declare, and phase 3 is speculative.**
   - **Evidence:**
     - An inferred default policy (D2) can move a route onto a server the deployment doesn't have.
     - Deriving `Vary` from reads tracked at runtime makes that tracking a privacy boundary between users. One missed read serves one user's page to another.
     - The edge has no consumer: Interview is TypeScript, and D9 starts the edge with Rust-sourced apps.
     - Private holes filled from the request context need cookies and headers, but Interview's private data lives on the device.
   - **Change:**
     - Render policy stays opt-in per route, and the compiler reports what it would infer.
     - Cached renders get no request context except a closed list of declared public facts, such as a locale, and those facts are explicitly part of the cache key.
     - Drop the edge and the credentialed holes. Streaming waits for a measurement.

7. **MEDIUM: conflicts with the project's rules.**
   - D11 satisfies "no app JS before first pixel" by moving the app's JavaScript to the server. A `request` page runs realm creation, module evaluation and source calls before its first byte. The rule exists so startup is counted and budgeted, so the server path needs the same treatment: count the per-render module graph and source calls, and budget time to first byte.
   - 1048.001 is typed Spec with no implementer, and its §4 is a list of unsettled questions. The rules say a spec needs an implementer and a date.
   - The 1048 documents take 5 of the 15 working-set slots. Fold 1048.001 and 1048.002 into a "later" section until their kickoffs. The next signal should come from 1a running, not from another review.

8. **LOW: cheap to decide now.**
   - **Forms:** part A ships `form` but leaves its behaviour before the runtime loads unspecified. Make it a standard GET navigation, so `/search?q=` works without JavaScript. That's navigation, not a refused server action.
   - **Security:** D6 embeds answers, which include user text, in a `<script>` element without naming an encoding. Require one that can never contain `</script`.
   - **Accessibility:** phase 1's one-time DOM replacement should keep focus, selection and `img` elements, matched by `data-view`.
   - **Native apps:** public URLs should open the native apps through associated domains (Universal Links).
   - **Link previews:** the headless Linux painter can render preview images from a Contract view.
   - **Public access:** making content written while Interview was invitation-only public is a consent question for its authors, not a toggle.

**Top three changes**

1. Re-slice phase 1: 1a is static documents for the in-repo apps; 1b is the server and Interview, gated on the §9.5 ruling and a hosting target.
2. Replace kept-path seeding with checkpoint answers as a runner input that survives a failed refresh, and make sure the device's takeover never shows less than the server did.
3. Use HTTP caching behind a CDN from phase 1 and keep credentials off the render server. Cut the server cache store, the inferred `Vary`, the credentialed holes and the edge.

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
| 1 | Phase 1 is gated on Interview decisions and infrastructure that don't exist | **Accepted.** Phase 1 splits into 1a and 1b as proposed. | 1048 §7; 1048.000 §5 (r3) |
| 2 | The kept path can't seed the page, and the device store taking over can erase it | **Accepted** (change 1).<br>• "Never less than the server" holds while the store takes over.<br>• The acceptance cases add a signed-in reader with a cold replica.<br>• Interview's source reports a failed fetch as a failure, not as an empty answer (`app.ts` ~186), so the checkpoint's answer survives it. | 1048.000 D6, §4; 1048 §10 (r3) |
| 3 | Caching is off where it's free, and there is no operations design | **Accepted.**<br>• 1b sits behind a CDN from its first day.<br>• `cached` becomes headers plus purge-by-tag, with no cache store on the server.<br>• 1048.000 specifies the deployment unit, health checks, metrics, panic containment, draining, and a skew policy: content-addressed assets are kept for pages still open, and each document names its bundle. | 1048.000 D10–D11; 1048.002 (r3) |
| 4 | Part A is kernel work, and it contradicts web work in progress | **Accepted** (change 5).<br>• The reset (`44f8643c` on main, not `core/fixes`) is right while every `h1`–`h6` is a lowered text node.<br>• The spec for HTML defaults scopes it to lowered nodes before authored headings exist. | 1048.003 (r3) |
| 5 | Interview's content would be semantically flat | **Accepted for 1b.** The Rust projection emits Markdown blocks as semantic HTML in both the live page and the document, coordinated with the Markdown work in progress. | 1048.000 D1 (r3) |
| 6 | The plan infers where it should declare | **Accepted.**<br>• Each route declares its policy, and the compiler reports what it would infer.<br>• Declared public facts go into the cache key.<br>• The edge and credentialed holes are dropped.<br>• Streaming waits for a measurement. | 1048 D2; 1048.002 (r3) |
| 7 | Conflicts with the project's rules | **Accepted.**<br>• The server counts its startup: the module graph, the source calls and time to first byte.<br>• The working set changes. | 1048.000 §4; 1048 (r3) |
| 8 | Cheap to decide now | **Accepted:**<br>• escaping;<br>• Universal Links and preview images drawn by the Linux painter, both for 1b;<br>• focus, selection and images, preserved on the fresh-render path (adoption keeps them otherwise).<br>Forms: as in Fable's row 8.<br>Consent: ★ Charlie (§9.5). | 1048.000 (r3) |

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
