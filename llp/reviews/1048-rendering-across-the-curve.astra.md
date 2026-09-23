# Review: LLP 1048 rendering across the curve (and 1048.000–1048.003), 2026-09-23 (astra)

- **Family:** OpenAI, `codex exec -m gpt-6-astra -c model_reasoning_effort=max -s read-only -C /Users/ccheever/projects/exact2`, one fresh session (`01a0cd76-d6bc-73e3-b774-13afcba53e01`) at `b1d73f44`, 2026-09-23T08:51:50Z to 2026-09-23T09:11:03Z. Brief sha256 `2b6301b00a3a35b929e8d767663a2ef8cb338ea733433d8397fe23fa80fb7944`. Read-only: no builds, tests, writes or network.
- **Method:** a single sanity-check review at Charlie's request ("can we have astra max do a sanity check over this plan before we start implementation?"), no refine loop. The author (Claude, Opus 5.5) is not a reviewer. Provider: OpenAI; nothing redacted — the brief told the reviewer not to open `.env*`, `.snapback/invite-code` or key files in either repository, and it reports it opened none.
- **Reviewed revision:** LLP 1048 and 1048.000–1048.003 as of `b1d73f44`, before the NOT-DOING take was recorded in `6bf20653`. The reviewer also read `~/projects/interview`.
- **Disposition:** by the author, after the verbatim review.

---

## Review (verbatim)

**Overall assessment**

The direction is sound, but **the plan is not ready to implement as written**. Its central simplification conflates three different things: a fresh boot, a page settled after asynchronous data arrives, and a restart from `Carried`. The current code does not make those equivalent.

Phase 1 is materially larger than “bake plus an HTML serializer.” Phase 2 needs a precise reconstruction and adoption protocol. The privacy guarantee needs stronger machinery than the existing dependency table.

This was a read-only source review against the supplied HEAD. I ran no builds or tests, used no network, and opened no secrets. Conclusions about runtime consequences are deductions from source, not observed test results.

**Strengths**

- **Sharing the DOM mapping is the right foundation.** LLP 1048.000 D1 avoids creating a second rendering vocabulary. The existing `Host::create` already centralizes tag, props, CSS and handlers. [host.rs:874](/Users/ccheever/projects/exact2/host/web/src/host.rs:874)
- **Public content and viewer state should be separate.** LLP 1048 §10 correctly identifies Interview’s monolithic resource as an obstacle. The proposed default of undeclared sources to `client` is conservative. [1048 §10](/Users/ccheever/projects/exact2/llp/1048-rendering-across-the-curve.rfc.md:275), [1048.002 §1](/Users/ccheever/projects/exact2/llp/1048.002-pages-per-request.rfc.md:32)
- **The incremental delivery strategy is sensible.** Replacement before adoption, caching after uncached request rendering, and explicit accounting for activation/streaming glue are good choices—provided the replacement preserves useful content and the initial renderer has an honest scope. [1048 phases](/Users/ccheever/projects/exact2/llp/1048-rendering-across-the-curve.rfc.md:224), [1048.001 D5](/Users/ccheever/projects/exact2/llp/1048.001-adopt-dont-hydrate.spec.md:78)

**Concerns**

1. **HIGH — “The batch is the page” needs a narrower, explicit meaning. Confidence: high.**

   The proposed extraction can handle structural transformations such as router visibility. Other effects require facts absent from the batch:

   - Symbols use computed font size, weight, padding and actual element dimensions. [glue.js:330](/Users/ccheever/projects/exact2/host/web/glue.js:330)
   - Focus eligibility depends on connectedness, rendered rectangles, visibility and current readiness; scrolling and context positioning also happen after applying operations. [glue.js:817](/Users/ccheever/projects/exact2/host/web/glue.js:817)
   - Font availability follows an asynchronous loading policy outside the batch. [glue.js:866](/Users/ccheever/projects/exact2/host/web/glue.js:866)
   - Virtualized lists initially realize at most sixteen provisional rows. Actual scrollport geometry determines the eventual window, and browser feedback can produce corrections before paint. [collection/mod.rs:173](/Users/ccheever/projects/exact2/runner/src/instance/collection/mod.rs:173), [collection/mod.rs:394](/Users/ccheever/projects/exact2/runner/src/instance/collection/mod.rs:394), [navigation.js:431](/Users/ccheever/projects/exact2/host/web/navigation.js:431)

   Moving these functions into Rust does not supply browser geometry or font-loading outcomes. Comparing normalized DOM also cannot establish visual equivalence.

   **Resolution:** Define a canonical **document before browser layout**, plus explicit browser-owned effects. Specify how virtualized content, canvas, symbols, fonts and contexts render without a runtime. For public documents, full content or an explicit static representation may be necessary. Restrict parity claims to the checkpoint and capabilities actually supported.

2. **HIGH — Phase 1 requires an asynchronous rendering host, and “replace on boot” can erase the rendered content. Confidence: high.**

   Bake boots once at `/`, then records resource values; it does not execute a network settlement loop. A resource answering `Later` without a previous value or matching compiled fallback currently refuses boot. [contract/cli/src/lib.rs:494](/Users/ccheever/projects/exact2/contract/cli/src/lib.rs:494), [settlement.rs:380](/Users/ccheever/projects/exact2/runner/src/runner/settlement.rs:380)

   Interview additionally starts with `started=false`; its source returns an empty app until a 250 ms timer changes that state. It also has a repeating synchronization task. “Settle until quiet” therefore needs a definition that excludes ordinary application timers and mutations. [Interview app.contract:175](/Users/ccheever/projects/interview/app.contract:175), [app.contract:241](/Users/ccheever/projects/interview/app.contract:241), [app.contract:280](/Users/ccheever/projects/interview/app.contract:280), [app.ts:164](/Users/ccheever/projects/interview/app.ts:164)

   Meanwhile, browser module loading occurs after a rendering opportunity. Until readiness, resources can use compiled placeholders even when their arguments have changed. A browser boot “as today” consequently does not reproduce a server page containing fetched Interview content. LLP 1048.000 D6’s “trees are equal” assertion is false for this case. [glue.js:1463](/Users/ccheever/projects/exact2/host/web/glue.js:1463), [settlement.rs:329](/Users/ccheever/projects/exact2/runner/src/runner/settlement.rs:329)

   **Resolution:** Put Hermes, request execution, continuation pumping, typed fallbacks, completion criteria, cancellation and deadlines in phase 1. Give public content an initial read path independent of client timers. Either seed the client with server answers in phase 1 or retain the server document until the replacement has equivalent useful content.

3. **HIGH — Raw-batch determinism and `Carried` equivalence are overstated. Confidence: high.**

   There is a concrete counterexample to byte-equal host batches: reorder operations contain a runtime identifier allocated from a process-global atomic counter. Two otherwise identical hosts created successively in a server process can emit different bytes. [reorder_drag.rs:12](/Users/ccheever/projects/exact2/host/web/src/reorder_drag.rs:12), [reorder_drag.rs:34](/Users/ccheever/projects/exact2/host/web/src/reorder_drag.rs:34), [batch.rs:69](/Users/ccheever/projects/exact2/host/web/src/batch.rs:69)

   `Carried` is explicitly a reload mechanism: tree, IDs, timers and derives are rebuilt. It keeps root slots, omits pending/stale resource answers, and restarts timers relative to the carried clock. That does not reproduce an arbitrary post-fetch or post-action checkpoint. The host’s first batch can itself include additional receipts from immediate continuations. [runner.rs:315](/Users/ccheever/projects/exact2/runner/src/runner.rs:315), [runner.rs:341](/Users/ccheever/projects/exact2/runner/src/runner.rs:341), [runner.rs:599](/Users/ccheever/projects/exact2/runner/src/runner.rs:599), [host.rs:598](/Users/ccheever/projects/exact2/host/web/src/host.rs:598)

   Two further mismatches need decisions:

   - A carried store overrides the browser’s fresh store snapshot. An empty public-page store cannot simply replace a signed-in device’s store. [delivery.rs:45](/Users/ccheever/projects/exact2/runner/src/runner/delivery.rs:45)
   - Fresh runner time starts at zero. “`now()` reads the build time” requires an explicit new clock policy. [runner.rs:602](/Users/ccheever/projects/exact2/runner/src/runner.rs:602)

   **Resolution:** Specify a document checkpoint distinct from ordinary reload state. Define ID reconstruction, resource readiness, pending flags, row state, time and subsequent device-state application. Hash a canonical document representation, excluding or rebinding runtime incarnation identifiers while preserving their protections. Replace “no code re-runs” with the accurate claim: Contract evaluation rebuilds the tree, while adoption can avoid rebuilding DOM.

4. **HIGH — Interview’s proposed split is necessary but insufficient. Confidence: high.**

   Anonymous backend reads alone will not expose its content:

   - The content branch requires `data.ready && data.authenticated`; logged-out users receive the welcome flow. [app.contract:655](/Users/ccheever/projects/interview/app.contract:655), [app.contract:703](/Users/ccheever/projects/interview/app.contract:703)
   - `app.ts` always sends `replicaOnly:true` to `/state`. That backend branch returns viewer/replica metadata; the anonymous branch returns empty content. The older full response uses `selected`, `answers` and singular `profile`, while the adapter consumes `details` and `profiles`. [app.ts:86](/Users/ccheever/projects/interview/app.ts:86), [backend.mjs:163](/Users/ccheever/projects/interview/scripts/backend.mjs:163), [backend.mjs:208](/Users/ccheever/projects/interview/scripts/backend.mjs:208), [app.ts:31](/Users/ccheever/projects/interview/app.ts:31)
   - Post, question and author navigation is implemented as buttons with actions, without URLs. Rendering those buttons does not create crawlable links. [app.contract:1007](/Users/ccheever/projects/interview/app.contract:1007), [app.contract:1016](/Users/ccheever/projects/interview/app.contract:1016), [app.contract:1031](/Users/ccheever/projects/interview/app.contract:1031)
   - The screen uses an inner scroller inside a height-constrained container. The proposed root-scroll heuristic will not automatically turn this into a document. [app.contract:637](/Users/ccheever/projects/interview/app.contract:637)
   - Search is supplied from local state, so a shareable request-rendered search page needs a URL/query contract. [app.contract:187](/Users/ccheever/projects/interview/app.contract:187), [app.contract:206](/Users/ccheever/projects/interview/app.contract:206)

   **Resolution:** Make these explicit Interview phase-1 tasks: public view gating, bounded public API projections with matching shapes, initial data readiness, real links, URL search state, document layout, route heads and missing-item behavior. Do not simulate public access by setting `authenticated=true`.

5. **HIGH — The dependency table does not yet enforce the claimed privacy guarantee. Confidence: high.**

   The table is useful for Contract-level propagation, and the VM already propagates store dependence through derives/resources. This is a solid starting point. [vm.rs:299](/Users/ccheever/projects/exact2/runner/src/vm.rs:299)

   However, it records Contract inputs, not arbitrary source implementation reads of headers, cookies or external state. Its bytecode scan is also not an exact runtime request-header read set. “Exact `Vary` from the dependency table” is therefore unsupported. [deps.rs:152](/Users/ccheever/projects/exact2/runner/src/instance/deps.rs:152), [deps.rs:330](/Users/ccheever/projects/exact2/runner/src/instance/deps.rs:330)

   There is a concrete existing attribution gap: settlement measures store reads around `query`, but fulfillment passes the store to `parse` without recording a read delta for that resource. Hermes continuations receive that store and can read it after an await. A source whose first store read occurs there can escape the existing resource-reader attribution. [settlement.rs:354](/Users/ccheever/projects/exact2/runner/src/runner/settlement.rs:354), [commit.rs:595](/Users/ccheever/projects/exact2/runner/src/runner/commit.rs:595), [js/src/lib.rs:743](/Users/ccheever/projects/exact2/js/src/lib.rs:743)

   Availability and confidentiality also need separate concepts. A public locale-dependent response and a session-dependent response can both be `request`. The parent forbids request values in cached pages, while the child explicitly caches public request variants. [1048 D3](/Users/ccheever/projects/exact2/llp/1048-rendering-across-the-curve.rfc.md:144), [1048.002 §5](/Users/ccheever/projects/exact2/llp/1048.002-pages-per-request.rfc.md:98)

   **Resolution:** Track source capability reads through initial calls and continuations; propagate dependencies through resource arguments, control flow, keys, pending state, viewport and clock. Separate execution stage from public/private classification. Make unknown inputs conservative. Inspect the entire serialized payload as well as visible bindings: current `Carried` includes the store snapshot. [carry.rs:8](/Users/ccheever/projects/exact2/runner/src/runner/carry.rs:8)

6. **HIGH — HTML escaping alone does not preserve the live host’s safety or DOM semantics. Confidence: high.**

   The live host validates navigable URLs before assigning them, including Markdown links and iframe destinations. Sharing `props_for` and escaping attribute characters would bypass that protection unless the URL policy moves too. An escaped `javascript:` URL remains dangerous. [glue.js:425](/Users/ccheever/projects/exact2/host/web/glue.js:425), [navigation.js:1403](/Users/ccheever/projects/exact2/host/web/navigation.js:1403)

   The serializer must also reproduce property semantics: checked/value handling, boolean attributes, textarea content, canvas wrappers and markup expansion. These are not direct attribute serialization. [glue.js:387](/Users/ccheever/projects/exact2/host/web/glue.js:387), [glue.js:597](/Users/ccheever/projects/exact2/host/web/glue.js:597)

   Introducing semantic elements adds HTML parser constraints. Non-inline text currently maps to a `div`; allowing that beneath `p` would produce markup that the HTML parser restructures, even though imperative DOM construction can create it. [host.rs:1074](/Users/ccheever/projects/exact2/host/web/src/host.rs:1074)

   A digest of batch bytes does not detect parser repairs or prove that the actual document has the expected structure.

   **Resolution:** Share the complete safe document projection, including URL policy and property-to-HTML rules. Define valid semantic content models. Verify the parsed HTML structure and adoption targets, rather than relying solely on a batch digest.

7. **MEDIUM — Resetting semantic elements to bare-div defaults contradicts the project’s web-standard rule. Confidence: high.**

   LLP 1048.003 D2 explicitly resets margins, padding, list style and fonts. The project explicitly follows web defaults even where a reset would ordinarily override them. [1048.003 D2](/Users/ccheever/projects/exact2/llp/1048.003-documents-in-contract.spec.md:41), [AGENTS.md:9](/Users/ccheever/projects/exact2/AGENTS.md:9)

   A bare node can have div defaults. A declared `p`, `li`, `strong`, `em`, `pre` or `label` should have that element’s semantics and browser defaults. In particular, `label` is not inherently a block, and list markers are part of ordinary list rendering.

   **Resolution:** Preserve semantic defaults and let authors explicitly reset them. Specify native equivalents and document unavoidable deviations. If uniform reset semantics are intentional, that needs an explicit exception to the governing rule; approval of new tags alone does not resolve the contradiction.

8. **MEDIUM — The media-variant cascade is incorrect, and native support is underspecified. Confidence: high.**

   The proposed example leaves unconditional `gap:24` inline while emitting `gap:12` in a stylesheet media rule. Normal stylesheet declarations cannot override that inline value. [1048.003 D3](/Users/ccheever/projects/exact2/llp/1048.003-documents-in-contract.spec.md:82)

   The claimed existing native facts are also incomplete: `Viewport` contains width and height; kernel `Env` contains safe-area insets. Neither represents hover, pointer, contrast or reduced-motion preferences. [viewport.rs:11](/Users/ccheever/projects/exact2/runner/src/viewport.rs:11), [style.rs:67](/Users/ccheever/projects/exact2/kernel/src/style.rs:67)

   Interview’s `wide` value also controls structure, so replacing a conditional with media styling requires rendering the relevant structure independently of that condition. [app.contract:157](/Users/ccheever/projects/interview/app.contract:157), [app.contract:413](/Users/ccheever/projects/interview/app.contract:413)

   **Resolution:** Define one cascade for base classes, variants and explicit inline overrides, mirrored natively. Specify missing environment facts. Move the minimum responsive mechanism into phase 1 if responsive documents are its promise; otherwise explicitly accept the fixed-viewport limitation.

9. **MEDIUM — Adoption and activation require more than attaching listeners. Confidence: high.**

   Skipping initial props loses runtime metadata currently initialized by `applyProps`: authored inertness, autofocus, scroll-follow state and other host bookkeeping. `attach` alone does not recreate it. Current readiness handling also disables controls, which must be reconciled with interaction-triggered activation. [glue.js:387](/Users/ccheever/projects/exact2/host/web/glue.js:387), [glue.js:431](/Users/ccheever/projects/exact2/host/web/glue.js:431), [glue.js:496](/Users/ccheever/projects/exact2/host/web/glue.js:496)

   Replay needs a semantic event protocol. `press` currently comes from `click`, change from `input`, and submit handling includes composition checks. Capturing pointerdown/key/focus events does not automatically reproduce those semantics. A mismatch replacement also invalidates queued numeric view IDs unless targets are deliberately remapped. [glue.js:521](/Users/ccheever/projects/exact2/host/web/glue.js:521), [glue.js:541](/Users/ccheever/projects/exact2/host/web/glue.js:541)

   Finally, the proposed `never` inference ignores host capabilities that need runtime work: a canvas or virtualized collection can need it without application handlers or timers.

   **Resolution:** Specify host-state adoption separately from DOM mutation. Define exactly-once replay, target identity across fallback, input/selection preservation and activation failure behavior. Infer `never` from all runtime dependencies, including host-owned presentation capabilities. Define what forms do before activation and when activation fails.

10. **MEDIUM — Automatic holes and streaming lack a structural consistency protocol. Confidence: high.**

   “Put `data-hole` on the subtree root” does not cover an empty `each`, an absent `when` arm or a region producing multiple sibling roots. An instance path containing the selected arm also needs a rule when fulfillment changes that arm. [1048.002 §2](/Users/ccheever/projects/exact2/llp/1048.002-pages-per-request.rfc.md:50)

   The existing node dependency entries union all descendants. Using those directly for boundary inference would propagate a client dependency upward; choosing the smallest useful boundary requires an additional algorithm. [deps.rs:214](/Users/ccheever/projects/exact2/runner/src/instance/deps.rs:214)

   Streaming must also account for one answer changing several bindings, structure and head metadata. Flushing the head before those answers can freeze the wrong title or status. Waiting for a final payload does not itself ensure that earlier fills and final IDs describe the same checkpoint. [1048.002 §4](/Users/ccheever/projects/exact2/llp/1048.002-pages-per-request.rfc.md:85)

   **Resolution:** Define stable region anchors, empty/multiple-root handling, minimal boundary selection, patch ordering and checkpoint versions. Specify failed/aborted streams and no-JavaScript behavior—template fills need executable glue. Settle this before phase 2 fixes the payload and identity formats.

11. **HIGH — The phase-1 request service needs execution isolation and an HTTP contract. Confidence: high.**

   There is reusable grant-checked native execution machinery. The cited `host/linux/src/fetch.rs`, however, fetches development plan envelopes; it is not the application-request executor. [executor.rs:19](/Users/ccheever/projects/exact2/host/linux/src/executor.rs:19), [fetch.rs:1](/Users/ccheever/projects/exact2/host/linux/src/fetch.rs:1)

   Request isolation matters immediately: Interview’s module owns mutable replica and identity globals. Reusing a realm across requests needs an explicit reset/isolation guarantee. [Interview app.ts:22](/Users/ccheever/projects/interview/app.ts:22)

   The current Hermes call budget is checked after execution returns; that check cannot interrupt a non-returning call. The repository also describes module execution as trusted application code, not a security sandbox. [js/src/lib.rs:660](/Users/ccheever/projects/exact2/js/src/lib.rs:660), [README.md:275](/Users/ccheever/projects/exact2/README.md:275)

   HTTP behavior is similarly not inherited from static serving: the current path drops the query for file lookup and returns a successful SPA fallback. That is insufficient for query-dependent pages, missing content and renderer failures. [serve.mjs:591](/Users/ccheever/projects/exact2/host/web/serve.mjs:591), [serve.mjs:639](/Users/ccheever/projects/exact2/host/web/serve.mjs:639)

   **Resolution:** Include per-request state isolation, bounded concurrency/output/work, effective deadlines, disconnect cancellation and failure cleanup. Define permitted rendering effects, outbound origins and redirect handling, SSRF protection, and explicit cookie/credential forwarding rules. Specify status codes, HEAD, redirects, canonical URL/query handling, HTML/data representation separation and cache-poisoning protections. Reuse existing executors and journals; this does not require a new apparatus layer.

12. **MEDIUM — Source provenance does not make server source code disappear from client artifacts. Confidence: high.**

   The parent claims server-computed source code never ships. The current build extracts and publishes the complete paired `app.js` and `app.hbc` artifacts. There is no per-source exclusion implied by delivering an answer instead. [1048 §2](/Users/ccheever/projects/exact2/llp/1048-rendering-across-the-curve.rfc.md:97), [build.mjs:98](/Users/ccheever/projects/exact2/host/web/build.mjs:98)

   **Resolution:** Either specify separate server-only source artifacts and capability boundaries, or remove the code-elision claim. Provenance labels and carried values alone must not be presented as protection for secrets embedded in implementations.

13. **MEDIUM — Navigation payload reuse needs resource identity, not just route identity. Confidence: high.**

   Existing resource reuse requires matching arguments and compatible logic identity. Interview currently requests arrays of parameters from the retained navigation stack, so a directly rendered destination can have different resource arguments from an in-app visit to that destination. [runner.rs:547](/Users/ccheever/projects/exact2/runner/src/runner.rs:547), [settlement.rs:308](/Users/ccheever/projects/exact2/runner/src/runner/settlement.rs:308), [Interview app.contract:208](/Users/ccheever/projects/interview/app.contract:208)

   Consequently, the promise that a rendered route needs no source call is conditional. Request payload navigation also appears in phase 2 while its endpoint is assigned to phase 3. [1048.001 D7](/Users/ccheever/projects/exact2/llp/1048.001-adopt-dont-hydrate.spec.md:95)

   **Resolution:** Specify matching by source, arguments, logic/artifact identity, freshness and principal scope. Define misses and invalidation. Move the uncached request-data endpoint into phase 2 or narrow that phase’s navigation promise. Bound prefetch work and distinguish data responses from HTML in caches.

14. **MEDIUM — Resolve scope and baseline contradictions before implementation. Confidence: high.**

   The binding NOT-DOING entry still excludes this work, and the parent explicitly leaves the required take open. Direction approval is recorded; the outstanding scope trade remains unresolved. [NOT-DOING.md:164](/Users/ccheever/projects/exact2/rules/NOT-DOING.md:164), [NOT-DOING.md:314](/Users/ccheever/projects/exact2/rules/NOT-DOING.md:314), [1048 §5](/Users/ccheever/projects/exact2/llp/1048-rendering-across-the-curve.rfc.md:195)

   Interview’s own milestone currently excludes public access. That needs reconciliation with the new consumer scope. [Interview NOT-DOING.md:16](/Users/ccheever/projects/interview/rules/NOT-DOING.md:16)

   Smaller corrections:

   - 1048.000 D9 includes request rendering, but §3 excludes it. [1048.000:106](/Users/ccheever/projects/exact2/llp/1048.000-pages-at-build.spec.md:106), [1048.000:128](/Users/ccheever/projects/exact2/llp/1048.000-pages-at-build.spec.md:128)
   - Hermes cannot remain an optional phase-3 question while Interview requires it in phase 1. [1048.002:153](/Users/ccheever/projects/exact2/llp/1048.002-pages-per-request.rfc.md:153)
   - The semantic-element baseline should distinguish pending work from landed code: the pinned `tag_for` does not map headings. [host.rs:1061](/Users/ccheever/projects/exact2/host/web/src/host.rs:1061)
   - Define the digest preimage to exclude its own digest field; the current header/payload wording is ambiguous. [1048.001:25](/Users/ccheever/projects/exact2/llp/1048.001-adopt-dont-hydrate.spec.md:25)

   **Resolution:** Reconcile these documents in the author’s disposition pass and record the outstanding scope decision.

**Suggestions**

Use one explicit renderer lifecycle throughout the documents: initialize request context, obtain permitted data, reach a defined checkpoint, project the document, serialize, then activate against that checkpoint.

Make the first acceptance case an anonymous Interview question URL with deliberately delayed data. It should have useful HTML and metadata without JavaScript, real links, correct missing/error responses, and no loss of content when the browser runtime starts. Follow with signed-in activation, desktop layout and navigation.

Use the existing verification lanes for meaningful counterexamples: delayed replies, store reads after an await, differing viewport inputs, repeated server requests, malformed URLs, empty regions and edited inputs before activation. Measure server work and TTFB alongside meaningful content paint, layout shift, payload bytes and activation cost.

**Open questions**

- Which existing Interview content and profile fields are intended to become public, and does that include previously invitation-only content?
- Is a rendered document a fresh bootstrap snapshot or a fully settled data snapshot? What time and device-state transition does it promise?
- Which renderer capabilities must work with JavaScript disabled, particularly virtualized content, streamed responses and forms?
- What is the required NOT-DOING take?

**Recommended next step**

Have the author disposition these findings and revise phase 1 around a bounded asynchronous renderer plus the concrete Interview changes. Resolve the checkpoint, payload and privacy contracts before implementing adoption or caching. A further refine loop is unnecessary for this sanity check.

VERDICT: MATERIAL FINDINGS
---

## Disposition (Claude, Opus 5.5 — the author), 2026-09-23

Every code claim spot-checked holds: the process-global runtime counter
(`host/web/src/reorder_drag.rs:12`), Interview's `state started = false` and its
timer (`app.contract:175`, `:241`), content shown only when
`data.ready and data.authenticated` (`app.contract:655`, `:703`), author links as
buttons without URLs (`app.contract:1007`), `build.mjs` shipping the whole paired
`app.js`/`app.hbc` (~98–104), `parse` receiving the store in fulfillment
without a read delta (`runner/src/runner/commit.rs` ~595), and Interview's own
`rules/NOT-DOING.md` excluding public access (~16). The review is right that the
plan conflated a fresh boot, a settled page and a restart from `Carried`.

| # | Finding | Disposition | Where it goes |
|---|---|---|---|
| 1 | "The batch is the page" is too broad | **Accepted.** Replaced by a defined *document*: the DOM projection of a settled checkpoint before browser layout, with browser-owned effects (fonts, symbol sizing, focus, scroll, context positioning, virtualized windows) listed and excluded. Public documents render virtualized collections' content in full as static HTML; canvas renders its fallback; parity is claimed only for the document. | 1048 D1 (r2); 1048.000 D1, D3 |
| 2 | Phase 1 needs an asynchronous renderer; replace-on-boot can erase content | **Accepted.** Phase 1 now includes a bounded async render host (Hermes for TypeScript, the native executor, continuation pumping, a completion rule that ignores ordinary timers and mutations, deadlines and cancellation), and pages carry their resource answers so the client boots seeded with them: replacement never shows less than the server did. | 1048.000 D9–D11 (r2) |
| 3 | Byte determinism and `Carried` equivalence are overstated | **Accepted.** The digest covers a canonical document, not batch bytes, and excludes runtime incarnation ids such as reorder runtimes. A *document checkpoint* is specified separately from reload state (answers, pending flags, row state, time, and no store). "No code re-runs" becomes "Contract evaluation rebuilds the tree; adoption avoids rebuilding the DOM". A page's checkpoint never replaces the device's store, and the clock policy is explicit. | 1048 D4 (r2); 1048.001 (open items) |
| 4 | Interview's split is necessary but insufficient | **Accepted.** Interview's phase-1 list now includes public view gating for logged-out readers, bounded public API projections with matching shapes, initial data without the 250 ms `started` timer, real links for posts, questions and authors, search in the URL, a document-scrolling layout, heads, and missing-item behaviour. Interview's own NOT-DOING excludes public access, which is **Charlie's decision** (which content becomes public). | 1048 §10 (r2) |
| 5 | The dependency table doesn't enforce the privacy guarantee | **Accepted.** Stage (build/request/client) and confidentiality (public/private) become separate labels. Source capability reads (store, request context) are tracked through calls and continuations — the `parse` attribution gap is a bug to fix first. Unknowns are private; `Vary` comes from tracked request-context reads, not from the dependency table; the whole serialized payload is checked, and public payloads carry no store. | 1048 D3 (r2); 1048.002 §1, §5 |
| 6 | Escaping alone doesn't preserve safety or DOM semantics | **Accepted.** The serializer shares the complete safe projection: the URL allowlist (now one policy on main, `871e2631`), property-to-HTML rules (checked, value, booleans, `textarea` content, canvas wrappers, markup expansion) and content models for semantic elements (phrasing-only children for `p`). The parity check parses the served HTML and compares the parsed tree. | 1048.000 D1, §4 (r2); 1048.003 D2 |
| 7 | Bare-div defaults for semantic elements contradict "the web is the standard" | **Accepted.** Semantic elements keep their HTML defaults (UA stylesheet) on every host; authors reset them explicitly. Native hosts reproduce the defaults. | 1048.003 D2 (r2) |
| 8 | The media-variant cascade is wrong; native facts are missing | **Accepted.** Base rows and variants of a class live in one stylesheet layer; inline style carries only per-node authored overrides, which win as in CSS. Hosts report the missing environment facts (hover, pointer, contrast, reduced motion). Width variants move into phase 1, because Interview's rail and tabs are structure: both render, and width variants choose which is displayed. | 1048.003 D3 (r2) |
| 9 | Adoption needs more than listeners | **Accepted as phase-2 requirements**: host-state adoption separate from DOM mutation, a semantic replay protocol (press from click, change from input, composition), readiness-disabled controls, target remapping across fallback, and `never` inferred from host capabilities (canvas, virtualized collections) as well as handlers. | 1048.001 (open items) |
| 10 | Holes and streaming lack a structural protocol | **Accepted as phase-3 requirements**, settled before phase 2 fixes the payload format: region anchors for empty and multi-root regions, minimal boundary selection, patch ordering and checkpoint versions, head timing, failed streams, and behaviour without JavaScript. | 1048.002 §7 (r2) |
| 11 | The request service needs isolation and an HTTP contract | **Accepted.** Phase 1's server gets per-request isolation (a fresh realm per request, since Interview's module holds globals), bounded concurrency, output and work, deadlines that interrupt, cancellation on disconnect, outbound-origin and redirect rules (SSRF), no credential forwarding in phase 1, and an HTTP contract (status codes, HEAD, canonical URLs and queries, 404 and 5xx pages). The `fetch.rs` citation was wrong (it fetches dev plans); the executor core is the reusable part. | 1048.000 D10–D11 (r2) |
| 12 | Provenance doesn't remove server code from client artifacts | **Accepted.** The code-elision claim is removed. Server-only source artifacts are a phase-3 question. | 1048 §2 (r2) |
| 13 | Navigation payload reuse needs resource identity | **Accepted.** Matching is by source, arguments, logic identity, freshness and principal. The uncached request-data endpoint moves into phase 2, and prefetch is bounded. | 1048.001 D7 (r2) |
| 14 | Scope and baseline contradictions | **Mostly resolved.** The NOT-DOING take was recorded after the reviewed revision (`6bf20653`: Messages as the Snapback4 consumer). 1048.000 §3 no longer contradicts D9; Hermes is in phase 1; heading lowering is pending in the web lane, not landed; the digest preimage excludes its own field. Interview's public-access exclusion is Charlie's decision (row 4). | 1048, 1048.000, 1048.001 (r2) |

**Proposed next step:** revise to r2 as above, then implement phase 1. A second
review round is not needed for a sanity check, per the reviewer. Status stays
Draft; Charlie decides.
