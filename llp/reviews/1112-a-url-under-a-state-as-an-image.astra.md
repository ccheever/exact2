# Review: LLP 1106: A URL under a state, as an image (r1, astra)

> **Renumbered 2026-10-08.** These RFCs were drafted as LLP 1105, 1106 and 1107 in a checkout 542 commits behind origin, where those numbers had since been allocated to other documents. They were committed as LLP 1111 (apps as tools), 1112 (a URL under a state) and 1113 (settled and the two clocks); LLP 1101.003 kept its number. The reviews below are transcribed as returned and keep the numbers they were written against: read 1105 as 1111, 1106 as 1112, 1107 as 1113.

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only (the local `codex` shim also passed `--dangerously-bypass-hook-trust`), `-C` a detached worktree at `2ce193976` with the three drafts copied in uncommitted; 2026-10-07T04:16:30Z to 2026-10-07T04:34:16Z.
- **Method:** one brief per RFC (sha256 `20fb57e3539e62e9bb3f747ca716cd8144e7c5d2ca28f657dd98cce9f80f621a`), shared verbatim with the other reviewer; round 1, blind to the other review. Requested by Charlie ("have astra max and grok 4.7 xhigh review the RFCs"). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted: the brief and documents hold no secrets.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r2 (2026-10-06) takes every concern: one definition of settled (D1a, data then motion, no timer fires); cooperative and hard deadlines with no public placeholder and the Hermes claim corrected (D11); a named anonymous mode with `Anonymous` moved to `exact-data` and the private-address refusal as new work (D10); the store snapshot removed from D1 with a defaults/precedence table; fixed answers after the grant on every executor; a checked capability table and strict refusal at build and per request; Serif and Sans Mono pinned with a fallback order; collision-free picture URLs and containment; the build hook at `host/web-js/build.mjs`; a status/cache table matching `serve.rs`; memory caps; PDF as a separate artifact; parity as an approved-only async step. The admission (consumer, implementer, take) stays Charlie's (§5, §7). The note on LLP 1105's TypeScript-on-Linux claim is that sibling's.

---

**1. Overall assessment**

The problem is worth solving: public social cards and reproducible documentation images can reuse Exact’s existing authoring model, routes, and painter. The proposed scope largely respects the repository’s rules. However, this draft is not ready to implement. Its settling rules contradict each other, its hard deadline cannot guarantee the promised placeholder image, and its shared “state” includes a store snapshot that does not currently exist in the described form. The public rendering path also needs a concrete separation from normal Linux app execution.

I reviewed the code at `2ce193976` read-only. No builds or benchmarks were run. “Verified” below means verified by source inspection; the draft’s historical timing measurements remain unverified for this commit.

**2. Strengths**

- **D1: authored inputs rather than runner snapshots.** This preserves the existing runner and Agent API model. The seven existing launch-line names and screenshot grammar are accurately described: `contract/syntax/src/parser/steps.rs:716` and `:570`. Keeping state outside the application’s query string is sensible.
- **D2: reuse the painter and retain Chrome as the oracle.** The existing conformance comparison really is opt-in layout comparison at 0.5 px, without pixel comparison: `host/web-js/conform.mjs:26`. The draft correctly identifies the missing visual evidence.
- **D3–D4: several underlying mechanisms are real.** Declared faces are registered in `host/linux/src/text/catalog.rs:143`; the raster side limit is 16,384 at `host/linux/src/raster.rs:383`; document extent currently uses root frames at `host/linux/src/presenter.rs:803`. Deferring full-page capture until the extent problem is addressed is sound.
- **D6: per-app Linux wrappers are a credible reuse path.** Contrary to sibling 1105 §1, TypeScript-backed Linux hosts already exist: `apps/messages/linux/src/main.rs:5` constructs an `exact_js::Module`, and `js/build.rs:187` provisions Linux Hermes. I also verified the cited Interview wrapper exists, although that external checkout is not evidence pinned to this commit. The sibling’s blanket “Linux execution remains unbuilt” statement needs reconciliation; it is not a reason to reject 1106.
- **D5, D7–D8: useful scope boundaries.** Ordinary card routes preserve one GUI authoring model. Named public presets avoid exposing arbitrary drives. Keeping personal pictures, paged media, and runtime screenshot capabilities out is appropriate. The initial screen-media PDF also agrees with sibling 1101.003 D6.

**3. Concerns**

**C1 — BLOCKING — D1, D4, D8: “settled” has incompatible meanings.**

D1 and D4 require `clock settle`; D8 forbids timers and actions. Existing `clock settle` advances the runner clock and fires timers crossed while finishing transitions, potentially starting further transitions: [host/linux/src/agent.rs:700](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/linux/src/agent.rs:700). That is observably different from the render host’s resource settlement.

The existing `EXACT_SHOT` path does not already provide either complete algorithm: it waits up to 500 ms for images, signals first pixel, then paints and writes without a general data-settlement loop (`host/linux/src/app.rs:549`).

**Resolve:** Specify the lifecycle for each invocation: facts before boot, source activation, resource settlement, image/font readiness, motion sampling, and capture. Explicitly distinguish authored-drive settlement from anonymous rendering. Define the outcome when settlement cannot finish.

**C2 — BLOCKING — D8: a killed child cannot guarantee a placeholder PNG.**

D8 promises both a hard process kill at the deadline and a picture containing the app’s pending placeholders. If the child is stuck in source execution, layout, painting, or encoding, the parent has neither a finished PNG nor the child’s current tree. These promises require an additional protocol.

The supporting claim about Hermes is also wrong at this commit. The render host arms an interrupt watchdog (`host/render/src/lib.rs:422`, `:629`); the JS source exposes an interrupt (`js/src/source.rs:186`) that reaches Hermes’ asynchronous timeout (`js/src/engine.rs:94`). Process termination remains valuable for non-interruptible native work, but the stated rationale is outdated.

**Resolve:** Separate a cooperative source deadline, after which painting may still complete, from a hard process deadline, after which the response is an error. Define whether the two seconds includes boot, settlement, painting, and encoding, and specify child-result framing and abnormal-exit handling.

**C3 — BLOCKING — D6–D8: normal Linux execution is not the anonymous render environment.**

The existing screenshot path boots the ordinary presenter. That host configures app storage (`host/linux/src/host.rs:676`), establishes app-file roots before source activation (`:221`), and permits `app:/` image reads (`host/linux/src/image/assets.rs:91`). The headless path also retains development/update behavior (`host/linux/src/app.rs:556`).

By contrast, the render host’s [Anonymous adapter](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/render/src/source.rs:1) withholds storage configuration and narrows grants. Merely choosing a fresh process or omitting `--storage` does not establish that boundary. Public rendering also cannot depend on enabling development-agent overrides: production launches remove those variables (`host/linux/src/app.rs:487`).

**Resolve:** Specify a dedicated anonymous rendering mode or constructor, including its source adapter, asset access, environment, update behavior, and artifact selection. Keep local fixture/store/step inputs structurally unavailable to the public invocation. State that the app’s native code is trusted; a fresh process supplies cancellation and isolation of state, not an OS security sandbox.

**C4 — MAJOR — D1: the shared state shape is incomplete, and the store claim is wrong.**

`--storage <name>` selects persistent scratch storage; it is not a portable snapshot input. Authored tests actually create fresh isolated stores (`scripts/agent-test.mjs:107`, `:132`). The cited LLP 1018 explicitly leaves the plain tier unbuilt (`llp/1018-durable-client-state.rfc.md:137`), and `Store::new` admits `secret.keep` entries, not arbitrary plain application state (`runner/src/store.rs:120`).

This affects sibling consistency: 1105 D8 expects a snapshot containing three Fieldnotes notes, but those notes live in SQLite (`apps/fieldnotes/app.ts:29`). A runner Store snapshot cannot supply them.

Other gaps undermine the “three spellings, each a subset” claim:

- Scale appears in the manifest but has no proposed authored-test launch spelling.
- The claimed existing `--online`-style flags are absent from `scripts/agent-launch.mjs:126`.
- Defaults and precedence for manifest locale, time zone, epoch, and seed are unspecified.
- Multiple `prefer` entries need merging by fact name; the current generic launch merge works by operation name (`steps.rs:747`).

**Resolve:** Publish a small field/default/precedence table. Either define a bounded, isolated storage-fixture format—including its relationship to SQLite—or explicitly remove that promise from the initial shared shape and its sibling examples.

**C5 — MAJOR — D1 fixed answers: reusing the fault interception point changes grant semantics.**

Native fault work runs before transport grant validation: `host/linux/src/presenter.rs:701` supplies replacement work, and [executor_core.rs:852](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/apple/src/executor_core.rs:852) returns it before consulting bindings. The web checks grants first (`host/web-js/admission.js:38`). LLP 1103 documents this difference because both existing outcomes are failures.

Turning the native replacement into a successful 200 response can make an ungranted request succeed in a fixture while the web refuses it. Tests could then validate behavior that cannot run normally.

**Resolve:** Require grant validation before successful substitution on every executor. Specify fail/answer conflicts, fixture-path resolution and byte limits, response URL/header semantics, and unmatched-request behavior. Live unmatched requests should not silently support a claim of reproducibility.

**C6 — MAJOR — D2: the capability inventory is inaccurate, and refusal behavior contradicts itself.**

The actual gaps matter directly to social cards:

- Portable `symbol:` roles **are supported**; `sf/` names remain empty (`host/linux/src/paint/svg.rs:71`).
- HTTP(S) image sources are not loaded by the current image resolver (`host/linux/src/image/assets.rs:55`, `:91`).
- JPEG decoding refuses on non-Android targets (`host/linux/src/image/jpeg.rs:206`).
- CPU rendering omits platform video, iframe, and native-view content (`host/linux/src/paint/native.rs:1`; `paint/backend.rs:67`).

D2 says unsupported drawing is “refused,” then says a partial picture is written unless `--strict` is supplied. That is not equivalent to the JS build’s fail-on-refusal policy.

**Resolve:** Replace the inventory with verified capabilities and define which omissions cause failure. Build and public outputs should fail on unsupported visible content unless an explicit product policy authorizes a partial image. Decide whether unsupported but offscreen nodes count. Exercise the chosen consumer’s real image formats and sources.

**C7 — MAJOR — D3 and D2 verification: pinned bytes do not yet specify font behavior.**

The proposed fallback contains DejaVu **Sans Book and Bold**, not corresponding serif and monospace faces (`scripts/fixtures/fonts/SOURCE.md:3`). Mapping every generic family to those files would change meaningful CSS behavior.

The existing catalog also reports that missing glyphs use platform fallback rather than the remaining authored families (`host/linux/src/text/catalog.rs:109`). Disabling system scanning exposes this gap; it does not solve it. Emoji and CJK are only part of the problem.

**Resolve:** Define generic-family mappings, fallback order, missing-face/glyph outcomes, and identical font configuration for the Chrome comparison. Scope the determinism claim to identified renderer, font, asset, and input versions. Specify the perceptual metric and acceptance threshold—or make selecting them an explicit measured acceptance task before relying on the output.

**C8 — MAJOR — D7 and §3: static-picture identity and the build integration are underspecified.**

The example maps a location to `card/talks/42.png`, but locations include queries. `/talks/42?tab=a` and `/talks/42?tab=b` require distinct pictures. Nested `&`, percent escapes, root/trailing-slash cases, and output-path containment need defined handling.

The change inventory also misses the main page-generation implementation. Ordinary web builds delegate to `host/web-js/build.mjs` and exit (`host/web/build.mjs:55`). The default Rust page-render invocation and document writes occur at [host/web-js/build.mjs:553](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv-surfaces/host/web-js/build.mjs:553).

**Resolve:** Define URL parsing/encoding and a collision-free artifact key derived from the complete location and resolved preset. Publish a head URL only after successful image creation. Identify the integration point for the default JS-target build, plus the retained alternative build modes. Define how the server receives the matching executable, plan, modules, assets, and fonts.

**C9 — MAJOR — D8: HTTP and cache behavior diverge from the cited contract.**

`max-age=60` permits downstream caching; it is not “never kept.” Existing deadline responses are 503 with `no-store` and `Retry-After` (`host/render/src/lib.rs:84`; `host/render/src/serve.rs:1299`). Existing `render=request` pages also use `no-store` (`serve.rs:1309`).

“Complete” is not proof of complete content: refused resources can retain placeholders while settlement reports Complete (`host/render/src/lib.rs:773`). Images have no later hydration opportunity.

The draft also leaves “the page’s public lifetime” ambiguous when a page points at a separate card route, and does not define handling for unknown locations, head status, or unknown presets.

**Resolve:** Add an explicit status/cache table, retaining the existing contract unless deliberately amended. Identify which route owns cache policy. Define how preset changes and source/asset/font/renderer changes invalidate pictures. State that request hints and cookies cannot alter the manifest-fixed render, unlike ordinary page rendering under LLP 1048.006.

**C10 — MAJOR — D4, D8: frame dimensions and cache limits do not bound rendering memory.**

At the allowed maximum, one RGBA frame is 1 GiB. The CPU painter allocates another full-frame pixmap for each opacity layer (`host/linux/src/raster.rs:395`, `:1106`). A 64 MiB encoded-image cache does not bound those allocations or concurrent children.

Invalid scale currently warns and falls back to 1 (`host/linux/src/app.rs:56`), which is unsuitable as an implicit successful response to a requested render configuration.

**Resolve:** Define request and aggregate-server accounting for frame/layer memory, decoded assets, encoding buffers, queued work, and output bytes, with explicit overload outcomes. Initial numeric budgets can follow measurement, as LLP 1041 allows. Specify invalid-manifest rejection, child reaping, cancellation, and temporary-output cleanup.

**C11 — MINOR — D5: PDF requires changes beyond adding a backend implementation.**

The backend currently finishes with `Result<Pixmap, String>` (`host/linux/src/paint/backend.rs:119`). Its glyph-run helper retains glyph IDs and positions but not source-text cluster ranges (`host/linux/src/text.rs:466`, `:981`). Those ranges matter for searchable text: the candidate [krilla Glyph API](https://docs.rs/krilla/latest/krilla/text/trait.Glyph.html) requires them. Typst’s use of krilla is correctly stated. [Typst source](https://github.com/typst/typst/blob/main/crates/typst-pdf/src/outline.rs)

A backdrop blur also depends on previously painted content, so rasterizing just the element “in its box” is not a complete compositing specification.

**Resolve:** Keep PDF deferred, but identify the output-interface change, text mapping, CSS-pixel-to-PDF-unit conversion, and raster-fallback compositing requirements. These need not block a narrower PNG implementation.

**C12 — MINOR — §1, D2, D8: correct the remaining current-behavior assertions.**

- **Caltrain does not currently declare `og:image`.** Its head supplies a description (`apps/caltrain/app.contract:171`). The framework emits `og:image` only when `head.image` exists (`host/web/src/page.rs:87`). Change the motivation to describe framework capability.
- **HTML rendering is not necessarily “one paint away.”** The render host can project without a kernel (`host/render/src/lib.rs:191`); its kernel path uses an on-demand monospace measurer (`:427`). Picture layout and text measurement are additional work.
- **The web escape hatch is reusable, not already complete.** CDP screenshot capture exists (`scripts/agent.mjs:514`), but DPR is hardcoded to 1 (`:239`), and `exact render` is not an existing verb (`scripts/exact.mjs:541`).
- **“Pure Rust painter” is narrower than “dependency-free executable.”** The current Linux crate unconditionally depends on vello/wgpu (`host/linux/Cargo.toml:59`); TypeScript apps additionally require Hermes.
- **LLP 1048.002 is a Draft proposal**, not evidence that personal rendering is shipped.
- The old Caltrain, GPU, font-scan, and browser timings should remain historical observations, not evidence for current end-to-end request cost.

**4. Suggestions**

- Choose one real consumer and use its card as the acceptance case. Caltrain can validate build output; it does not validate Interview’s public request path.
- Narrow the first implementation to viewport PNG, a complete launch contract, pinned fonts, and reliable failure reporting. Keep full-page and PDF work behind their stated consumers.
- Extend existing conformance tooling where practical. A new painter-parity script needs explicit apparatus approval under `rules/RULES.md:64`; this review does not supply that approval.
- Before admission, name the implementer and record the selected take or waiver. Postponing an already-refused tape is not automatically a take from current work; removing the shipped JS render option needs an explicit compatibility and consumer decision.
- Validate the design with focused cases: a delayed source, crossed timer, missing image, unsupported glyph, query-distinct cards, missing route, and a child killed during painting.

**5. Open questions for the author**

- Which concrete Interview route and data source can produce the public card using only anonymous granted fetches?
- Must local, build, and request invocations produce identical pixels for identical resolved inputs, or is anonymous rendering intentionally a different settlement profile?
- What exact artifact supplies the “three notes” storage state used by sibling 1105?
- Should crawlers receive a placeholder picture after a cooperative timeout, or a retryable error? What happens after a hard kill?
- Which consumer, implementer, take, and parity-tooling scope will the admission actually name?

Verdict: NOT READY
## Round 2 (2026-10-07), on r2

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the three r2 drafts and only Astra's own r1 reviews; 2026-10-07T20:14:49Z to 2026-10-07T20:27:42Z.
- **Method:** one round-2 brief per RFC (sha256 `36d3ad14341b707711e9450de12681013143216d6272832bb5988616c948dd8e`), shared verbatim with the other reviewer; blind to the other family's reviews, though r2's dispositions section names concerns both raised. Requested by Charlie ("ok sounds good" to a second round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** r3 (2026-10-07) took every concern. N1: D1a is now a procedure, (a) activation, (b) a data fixed point that pumps announcements, (c) a motion fixed point in ms, (d) images to decode or failure and back to (b), (e) the deadline. N2: (c) says seeking moves infinite animations and the session clock. N3: a separate publishable predicate carries `head.status`. N4: refusal is recorded inside the paint walk. N10: D9's omitted-field table by launch mode; `answers` development only. N11: minor fixes. N5–N9 (file naming, cache identity, memory, private addresses, build handoff) moved to §5 as direction, with r3 narrowed to slice 1 (`answer fetch`) and slice 2 (a local Caltrain PNG with the sky off).

## 1. Overall assessment

**R2 substantially improves the proposal, but the first complete PNG slice still requires design decisions.** Removing the store snapshot, requiring successful fixture answers to pass grants, separating cooperative and hard deadlines, and naming anonymous execution all address real r1 problems.

The standalone development-test `answer fetch` slice is sufficiently concrete to implement separately. The picture pipeline is not: settlement can finish prematurely, the proposed motion operation cannot produce the specified sample, and public output identity and success conditions remain incomplete.

I reviewed commit `2ce193976` and the uncommitted drafts by source inspection. I made no changes and ran no builds or benchmarks.

## 2. R1 concerns

- **C1 — Partly resolved.** D1a resolves the timer contradiction, but activation, image/layout convergence, and motion sampling remain incomplete; see N1–N2.
- **C2 — Resolved.** D11 separates the cooperative deadline from the hard kill, promises no PNG after termination, and correctly describes Hermes interruption (`js/src/engine.rs:94`).
- **C3 — Partly resolved.** D10 specifies the anonymous source, storage/file restrictions, update exclusions, and trusted-native-code boundary; network enforcement and artifact handoff remain unresolved, N8–N9.
- **C4 — Partly resolved.** D1 removes the nonexistent snapshot and specifies fields and precedence; 1105 now seeds notes through actions. Shared default profiles still disagree, N10.
- **C5 — Resolved.** D1 specifies grant-before-answer, prefix precedence, fixture containment, response semantics, limits, and reporting of live requests.
- **C6 — Partly resolved.** D2 corrects the capability inventory and requires strict public output; its new visibility definition disagrees with CSS painting, N4.
- **C7 — Resolved at RFC level.** D2–D3 specify generic mappings, fallback, missing-glyph failure, identical Chrome fonts, and threshold selection as an acceptance task.
- **C8 — Partly resolved.** D7 names the correct build hook and restricts build-time queries, but filesystem collisions, query-sensitive serving, and artifact matching remain, N5–N6 and N9.
- **C9 — Partly resolved.** D8 fixes deadline/request caching and identifies the policy-owning route; successful publication, head status, and cache identity remain incomplete, N3 and N6.
- **C10 — Partly resolved.** D4 bounds dimensions and frame/layer allocations; it does not establish the claimed aggregate rendering-memory bound, N7.
- **C11 — Resolved.** D5 explicitly defers PDF and identifies the separate artifact, output-trait change, text clusters, unit conversion, and backdrop-aware raster fallback.
- **C12 — Partly resolved.** The Caltrain, layout, CDP, dependency, and personal-rendering corrections are present; D2 still states historical CPU timing as a current cost, N11.

## 3. New concerns

These include unresolved r1 issues exposed by r2’s replacement decisions.

### N1 — BLOCKING — D1a: settlement is neither activation-complete nor a fixed point

The stated predicate can succeed before deferred data activates. A source that is not ready can retain a placeholder without issuing a request ([runner/src/runner/settlement.rs:533](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/runner/src/runner/settlement.rs:533)). The existing render host explicitly performs `preload`, `activate`, and `data_ready` before settling (`host/render/src/lib.rs:721`); D1a needs that prerequisite.

The sequence “data, motion, images, paint” also does not remain settled. Image reports change intrinsic sizes and rerun layout (`host/linux/src/host.rs:1005`), then queue collection work and refresh geometry ([presenter/images.rs:18](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/linux/src/presenter/images.rs:18)). Newly visible images or resulting work can appear after the earlier checks. `wait_images` returns whether reports arrived, not whether decoding completed successfully.

**Resolve:** Specify activation first, then a deadline-bounded loop that drains data and continuations, applies layout/presentation/image changes, and repeats until all relevant domains are quiescent. Define the final readiness checks separately from “a wait function returned.”

### N2 — MAJOR — D1a: the existing motion API cannot produce the promised mixed sample

The claim “This is possible today” is too strong. [`Host::tick`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/linux/src/host.rs:1175) advances the presentation clock. `Engine::advance` advances animations too (`motion/src/engine.rs:524`), and animation sampling uses that single current time (`motion/src/engine/animate.rs:245`).

Consequently, seeking a finite fade to its end also advances an infinite spinner. It does **not** leave the spinner at the earlier data-settled clock. The engine rejects backward time (`engine.rs:549`), so restoring the clock afterward is not an available solution. This also matters when a screenshot is followed by further `--test` steps.

**Resolve:** Name the required new sampling mechanism and specify whether capture mutates the continuing session. Cover concurrent finite/infinite animations and a subsequent input step.

### N3 — MAJOR — D1a/D8: “settled” still does not establish publishable content

A failed response can leave no pending request (`runner/src/runner/settlement.rs:1022`), and a failed image preparation is explicitly non-pending (`host/linux/src/image.rs:439`). Neither observation establishes a complete picture.

D1a’s claim that existing page settlement differs only by `aria-busy` is also inaccurate. That loop permits refused requests to retain placeholders while returning `Complete` (`host/render/src/lib.rs:760–779`); it does not implement D1a’s explicit continuation/background predicate.

Separately, D8’s table does **not** match page status handling: [`Rendered::status`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/render/src/lib.rs:84) respects `head.status`, including 410 and 503. D8 maps settled output to 200 and unknown locations to 404, losing those distinctions.

**Resolve:** Define publishability independently of quiescence. Specify outcomes for unresolved placeholders, source failures, image failures, and authored error cards; carry head status and failure details through the child report. Then define HTTP precedence consistent with LLP 1048.000, or explicitly amend it.

### N4 — MAJOR — D2: the visibility predicate disagrees with the painter

A raw frame intersecting the viewport is insufficient. An offscreen layout frame can be transformed onscreen; an intersecting frame can be clipped away or suppressed by an ancestor’s opacity.

The painter already applies transforms, clipping, inherited visibility, and group opacity ([host/linux/src/paint.rs:855](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/linux/src/paint.rs:855), particularly `:884`, `:903`, `:917`, `:929`). D2’s new rule can therefore both publish a visibly incomplete picture and reject an actually invisible unsupported node.

**Resolve:** Base refusal on the painter’s transformed, clipped, effective visibility, including ancestor opacity. Avoid introducing a second visibility model.

### N5 — MAJOR — D7: the URL-to-file mapping is not collision-free

Two valid locations demonstrate a file/directory collision:

- `/a` → `dist/.exact/picture/card/a.png`
- `/a.png/b` → `dist/.exact/picture/card/a.png/b.png`

The same filesystem entry must be both a file and a directory.

Encoding also needs an explicit filesystem rule. [`location_of`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/route/src/location.rs:5) preserves existing percent-escape spelling, whereas static serving percent-decodes paths (`host/render/src/files.rs:35`). Calling the former does not define a collision-free filename codec.

Finally, the root form writes `<state>.png`, outside the stated containment root `<state>/`.

**Resolve:** Specify one complete URL/filesystem encoding, validation order, and root rule. Either prevent collisions structurally or explicitly reject conflicting locations. Include percent aliases, encoded separators, and the file/directory example above.

### N6 — MAJOR — D8: cache and built-file selection omit rendering inputs

D8 serves a built file whenever one exists. Thus a built `/talks/42` picture can answer a request for `/talks/42?tab=b`, although the query is explicitly part of the requested location. Including the query in the dynamic cache key does not fix this earlier shortcut.

The digest list also omits the bundled asset set. Assets can be read outside the executable through the asset resolver ([host/linux/src/image/assets.rs:68](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/linux/src/image/assets.rs:68)); D2 itself includes asset identity in the determinism claim.

Finally, named states use the server clock. If the resolved epoch enters “resolved facts,” every request can get a different key; if it does not, the cache needs an explicit time/freshness policy.

**Resolve:** Require exact input identity for built-file reuse, include an asset/deployment digest, and define how server time interacts with cache lifetime.

### N7 — MAJOR — D4/D8: the memory cap covers only part of a picture

The frame/layer cap is useful, but worker count × 256 MiB is not an aggregate rendering-memory bound.

For a permitted 4096² frame, the current decoded-image budget alone becomes **512 MiB**: eight viewport-sized RGBA images ([host/linux/src/image.rs:100](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/linux/src/image.rs:100)). Canvas bitmaps, masks, encoded buffers, source/runtime memory, and retained picture-cache bytes are additional allocations.

**Resolve:** Specify accounting for decoded images, canvases, encoding, and retained cache output, and distinguish bounded rendering allocations from process overhead. Either establish an aggregate admission budget or narrow the stated guarantee accordingly.

### N8 — MAJOR — D10: private-address refusal is assigned to an insufficient enforcement point

The executor core calls `fetch.stream(...).collect()` once ([executor_core.rs:882](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/apple/src/executor_core.rs:882)). Redirect processing happens inside ibex2 (`vendor/ibex2/src/stdlib/fetch.rs:346`), and DNS resolution and connection selection happen lower still (`transport/rustls_http.rs:197`, `:254`).

An initial-URL check in the core cannot establish refusal for every redirect and actual destination address. D10 also leaves unclear whether LLP 1048.000’s “unless granted” exception survives, and what grants that exception.

**Resolve:** Define the anonymous network policy at the actual connection boundary, covering every hop and resolved address without a separate unchecked resolution. State its scope across supported transports and whether private destinations have any explicit exception.

### N9 — MAJOR — D7/D10/§4: build dependencies and artifact handoff remain unspecified

The work order builds public pictures in step 3 but implements anonymous mode in step 4, although D7 requires that mode.

The current build invokes a development render executable with an explicit web-output `--plan` ([host/web-js/build.mjs:553](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/host/web-js/build.mjs:553)). A Linux executable independently bakes its plan (`apps/caltrain/linux/build.rs:9`). “Ship both binaries” does not establish which plan, source module, asset root, and fonts the picture child loads or how compatibility is checked.

The claim that the Linux crate already builds both binaries is also overgeneral: the cited lookup accepts render entries under **either `linux/` or `web/`**.

**Resolve:** Move anonymous execution before build pictures. Specify the child’s artifact inputs and matching checks, plus build/deploy ownership. Define picture behavior for the retained `--render js` option; LLP 1071’s ruling still keeps it available.

### N10 — MAJOR — D1/D9/D11: the shared launch contract needs explicit consumer profiles

Three policy boundaries remain unresolved:

- **Defaults:** D1 supplies fixed epoch/seed defaults, but [1101.003 D6](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/llp/1101.003-the-command-line.rfc.md:349) says the same record omits them to obtain system time and entropy.
- **Deadlines:** [1105 D4](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/llp/1105-apps-as-tools-for-agents.rfc.md:274) gives tool work ten seconds, while its D8 imports 1106’s two-second cooperative and 2.5-second hard deadlines. Specify whether remote calls intentionally receive the shorter budget. D11 names only the server as hard-deadline owner, leaving build/local handling of non-returning native work unspecified.
- **Production fixtures:** D9 admits launch records in every build and forbids answers only in anonymous mode. D1 makes answers part of LLP 1103’s fault table, whose [D3 explicitly makes those launch facts development-only](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv2-astra/llp/1103-making-a-source-fail-in-a-test.rfc.md:83). Calling them “facts, not operations” does not resolve that policy change.

**Resolve:** Add a mode-by-mode contract for omitted fields, permitted fixture inputs, clock behavior, and deadline ownership. Explicitly amend existing production restrictions if that expansion is intended.

### N11 — MINOR — D2/D9: examples and remaining current-behavior claims need correction

- D9’s example contains `answers` with `picture.mode: "anonymous"`; the next paragraph requires rejecting it.
- D2’s “fetches [a network image] in its data module and shows it from bundled assets” does not specify a usable transfer path. Runtime fetches do not become bundled assets. The resolver supports `data:` URLs (`host/linux/src/image/assets.rs:97`), if that is the intended bounded solution.
- D9 should say **agent overrides** are agent-only: ordinary launches already obtain locale, zone, and entropy from the environment/system (`host/linux/src/zone.rs:93`).
- D2 still says the painter “costs ~4–7 ms” in the present tense. Those are historical LLP 1015 measurements (`llp/1015-linux-host-v1.spec.md:618`), not measurements at this commit. Its cited pixel metric is in §8 (`:684`), not §7.

**Resolve:** Repair the example, describe an actual image-data path, and distinguish historical measurements and agent overrides from current guarantees.

## 4. Suggestions

- Keep `answer fetch` as a separate development-test slice. Make the next slice one local viewport PNG with explicit activation, convergent settlement, pinned fonts, and structured failure reporting.
- Before public build output, settle the artifact contract and anonymous execution dependencies.
- Use focused acceptance cases: deferred activation; image-induced relayout; simultaneous finite/infinite animation; a screenshot followed by input; transformed unsupported content; failed decoding; query-distinct cards; file/directory collisions; and termination during native work.
- Retain §5’s explicit admission requirements. This review supplies neither an implementer/take decision nor approval for new parity apparatus.

## 5. Open questions for the author

1. Should an app-authored error card be publishable, and how should its `head.status` affect the image response?
2. Does screenshot settlement alter the continuing test session, or produce an isolated presentation sample?
3. Which exact Caltrain card and asset set will establish acceptance for the first PNG slice?
4. Are production fixture answers intentional, or should they remain confined to development launches?

Verdict: NOT READY
## Round 3 (2026-10-07), on r3

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, `--ephemeral`, `-C` a detached worktree at `2ce193976` holding the three r3 drafts and only Astra's own r1 and r2 reviews; 2026-10-07T20:54:22Z to 2026-10-07T21:22:59Z.
- **Method:** one round-3 brief per RFC (sha256 `e5d544eee109275270836d540637fe449e2a1501293520259f3395e3a13d079d`), shared verbatim with the other reviewer; blind to the other family's reviews, though the dispositions sections name concerns both raised. Requested by Charlie ("sounds good, try that" to the narrowed r3 and a third round). The author (Claude, Opus 5.5) is not a reviewer. Nothing redacted.
- **Transcription:** the review as returned, unedited.
- **Verdict:** NOT READY.
- **Disposition:** pending; the loop stops here after three rounds; not revised against this review.

**1. Overall assessment.** R3 narrows the scope appropriately, but an implementer still cannot build both specified slices without making consequential decisions or contradicting the cited code.

The strongest remaining problems are native grant admission and capture settlement. The newly specified native insertion point precedes the actual origin check. D1a can also declare settlement before the final paint starts additional image work.

[§5](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1106-a-url-under-a-state-as-an-image.rfc.md:378) clearly marks later slices as unspecified and honestly lists their open problems. I have not counted their incompleteness against this verdict. The siblings’ differentiated settlement policies and launch defaults are substantially consistent; one stale deadline reference remains.

This review is based on source inspection at `2ce193976` and the uncommitted drafts. I made no changes and ran no builds or benchmarks.

**2. Round-2 concerns.** Status against the narrowed scope:

- **N1 — Partly resolved.** Activation, announcement pumping and image re-entry are now explicit; presentation/image convergence and loop exhaustion remain incomplete. See R3-2.
- **N2 — Partly resolved.** Infinite animations now sample the correct sought time, but timer suppression and the continuing session’s clock do not follow from the cited methods. See R3-3.
- **N3 — Resolved for the original resource/image/status concerns.** D1a explicitly separates publishability and carries `head.status`; final-paint failures introduce the additional gap in R3-4.
- **N4 — Partly resolved.** Recording inside the paint walk improves visibility handling, but the proposed refusal sites and clipping predicate remain incorrect. See R3-5.
- **N5 — moved.** URL/file collisions and encoding are explicitly open in §5.
- **N6 — moved.** Exact-location reuse, asset identity and time-sensitive caching are explicitly open in §5.
- **N7 — moved.** Aggregate memory accounting is explicitly deferred; the misleading aggregate guarantee is gone.
- **N8 — moved.** Connection-level private-address enforcement and exceptions are explicitly open in §5.
- **N9 — moved.** Build ordering, artifact matching and `--render js` are explicitly open in §5.
- **N10 — Resolved for the original conflicts.** D9 distinguishes fixed picture/test defaults from CLI defaults, confines answers to development, and D11 assigns the local hard deadline to `exact render`. The new test-mode integration gap is R3-6.
- **N11 — Partly resolved.** The example, image-data path and historical timing are corrected; the “facts only in agent mode” claim remains false. See R3-7.

**3. New concerns.** These include unresolved issues exposed by r3’s concrete procedures.

**R3-1 — BLOCKING — §3, native `answer fetch`: the specified substitution still bypasses origin admission.**

[RFC lines 308–315](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1106-a-url-under-a-state-as-an-image.rfc.md:308) place substitution after `bindings` resolves and `timeout_refusal` passes. Those checks do not admit the requested origin.

`Bindings` can contain a valid grant set for a different origin: [`Host::endow`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/vendor/ibex2/src/host.rs:98) merely attaches the grants. The actual URL parsing and `boundary::admit(Operation::Fetch { origin })` happen inside [`fetch_stream`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/vendor/ibex2/src/stdlib/fetch.rs:358), reached through `executor_core.rs:882–886`.

Consequently, implementing the prescribed insertion point can return a successful fixture for an ungranted origin. This reopens r1 C5, whose grant-before-answer requirement I considered resolved in r2.

**Resolution:** Require the actual admission operation against the effective, possibly narrowed, grants before substitution. Preserve request validation and applicable response-size limits. The denial test must include **valid bindings granting another origin**, not merely absent or invalid bindings.

**R3-2 — BLOCKING — D1a: settlement still excludes work initiated by the final presentation.**

The sequence paints during activation, then settles data, motion and images. It returns to data only when an image report changes layout. That does not establish a fixed point over presentation and image visibility.

Image visibility uses the previous painted boxes in [`presenter/images.rs:70`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/presenter/images.rs:70). An offscreen image with prepared metadata can have `pending() == false` because `visible` is false ([`image.rs:420`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/image.rs:420)). Motion can move it onscreen without updating those boxes. The final `frame()` replaces the boxes and calls `sync_images()` **after painting** ([`content_region/presenter.rs:195`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/content_region/presenter.rs:195), `:227`). Decoding can therefore begin after the supposedly settled capture.

`frame()` also performs collection refinement at `content_region/presenter.rs:63`, another potential source of work after the earlier checks.

Separately, the two 16-round limits have no defined exhaustion outcome. A quiet pass with `aria-busy=true` also lacks a wait/retry transition. The terminal sibling explicitly supplies that transition in [`1101.003:144`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1101.003-the-command-line.rfc.md:144).

**Resolution:** Include presentation and visibility refresh in the deadline-bounded outer loop. Accept a frame only after a complete pass initiates no further required work. Define round exhaustion and busy-without-pending behavior; neither can silently become `settled: "complete"`.

**R3-3 — MAJOR — D1a: the cited methods do not guarantee timer-free settlement or one continuing clock.**

D1a says no step calls `advance_timed`, so no timer fires. There is an indirect path:

`Presenter::tick` → `refresh_transform_geometry` → `Host::dispatch_at` → `Runner::dispatch_at` → `advance_timed`.

The relevant calls are [`presenter.rs:1399`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/presenter.rs:1399), [`transform_geometry.rs:183`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/presenter/transform_geometry.rs:183), `host.rs:873`, and [`runner/commit.rs:189`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/runner/src/runner/commit.rs:189). Image reports can also refresh transform geometry. A qualifying callback after a motion seek can therefore fire crossed timers during settlement. A subsequent ordinary input can fire them too, contrary to the claim that they wait for a `clock` step.

Conversely, [`Host::tick`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/host.rs:1175) advances host/presentation time without advancing the runner’s clock. `Runner::land_then` uses the runner’s existing time (`commit.rs:285`). The driver has another cached clock: relative advances use `s.now` at [`scripts/agent.mjs:1225`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/scripts/agent.mjs:1225), whereas ordinary screenshot handling at `:1279` does not update it.

**Resolution:** Specify the required clock synchronization and timer suppression through all settlement-generated callbacks. Define when overdue timers run after capture, including ordinary input, and update the driver’s clock before subsequent relative steps.

**R3-4 — MAJOR — D1a/§4: successful activation does not establish successful final painting.**

The publishable predicate covers resources, images, status, unsupported content and glyphs, but omits final-frame success and other execution errors.

[`Presenter::frame`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/content_region/presenter.rs:147) records `last_frame_succeeded`, then returns retained pixels or a white fallback on paint failure (`:161–180`). It does not return an error. [`Presenter::screenshot`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/presenter.rs:1412) encodes whatever frame it receives.

Thus successful activation followed by failed final painting can satisfy the listed predicate while yielding stale or blank pixels. D1a also needs to handle errors returned by pumping/continuations; the existing agent explicitly checks them at `host/linux/src/agent.rs:757–762`. §4 assigns no outcome to final paint, encode or output-write failure.

**Resolution:** Require a successful final frame and explicit handling of settlement errors. Define their report and exit behavior, including encode/write failures. Publication must also preserve the promised “nothing written” outcome on hard termination.

**R3-5 — MAJOR — D2: the capability table and refusal sites misclassify actual painting.**

The assertion that a 3D transform is refused at `paint.rs:867` is incorrect. That branch calls `spatial`, then disables row/damage optimizations. [`paint/space.rs:233`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/paint/space.rs:233) paints the subtree into a CPU surface, warps it and draws it through `surface_image`; the CPU implementation exists at [`raster.rs:919`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/raster.rs:919). A row-recording refusal is not a rendering refusal.

The proposed sites also miss a listed omission: unsupported `symbol:sf/...` returns silently in [`paint/svg.rs:90`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/paint/svg.rs:90). It reaches neither `native()` nor `spatial`; the image loader explicitly clears its refusal at `image.rs:193`.

Finally, rectangle intersection with `clip_rect` is not identical to effective painter visibility. CSS masks and clip paths are applied separately to the backend at [`paint.rs:939`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/paint.rs:939).

**Resolution:** Correct the 3D capability claim, distinguish optimization fallback from missing output, and identify every actual omission site. Account for CSS clipping/masking, or explicitly describe any conservative refusal policy instead of claiming equivalence with painted visibility.

**R3-6 — MAJOR — D9/§4: picture mode and the required test mode have conflicting contracts.**

[D9:252–256](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1106-a-url-under-a-state-as-an-image.rfc.md:252) says the `picture` object enters picture mode, opens no agent channel and keeps `EXACT_AGENT` off. §4 says `--test` uses a launch record containing `picture` and runs through the existing agent host.

That driver sets [`EXACT_AGENT: '1'`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/scripts/agent.mjs:606), and authored screenshot steps call `s.screenshot` directly (`scripts/agent-test.mjs:276`). The document needs an explicit test profile explaining how those steps receive picture-mode settlement, pinned fonts, strict validation and reports without capturing/exiting before `tap sky-toggle`.

D9’s omitted-field table also omits viewport, scale, location and output behavior. This matters concretely: the Linux default is [`420×860`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/app.rs:215), while the agent default is [`420×900`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/scripts/agent.mjs:79). The acceptance shorthand omits output and size.

**Resolution:** Explicitly permit the development agent channel for `--test`, define per-screenshot picture behavior, and complete the local defaults. Specify whether deadlines/reports apply per capture or per test, and how `--strict` and deadline settings reach the child.

**R3-7 — MINOR — D1a/D9 and sibling reference: remaining factual inaccuracies.**

- D1a says no host reads `accessibilityBusy`; the terminal already reads it in [`host/terminal/src/host.rs:389`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/terminal/src/host.rs:389).
- D9 and the summary say device facts are read only in agent mode. Ordinary launches already read locale, zone and entropy in [`zone.rs:93`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/host/linux/src/zone.rs:93). The restricted feature is the deterministic agent override.
- [`1105:533`](/private/tmp/claude-501/-Users-ccheever-projects-exact2/bc4d76d2-3c4d-45d2-8102-28fc48fc9da5/scratchpad/rv3-astra/llp/1105-apps-as-tools-for-agents.rfc.md:533) still calls a hard deadline “LLP 1106 D11’s 503.” D11 now specifies local exit 124, while server 500 versus 503 is explicitly open.

**Resolution:** Narrow the first two statements and remove the sibling’s stale attribution. The latter is a consistency correction, not a demand to specify public serving now.

**4. Suggestions.**

- Keep the present scope reduction. These corrections do not require specifying the later server or PDF slices.
- Add focused acceptance cases for valid-but-unrelated grants, an image becoming visible after motion, a timer crossed during capture followed by input, and final-paint failure.
- Make the two Caltrain cases fully explicit: launch facts, output paths, expected reports and exits. Ensure `flat` and glyph diagnostics describe the captured frame, so the initial sky does not contaminate the later sky-off result.

**5. Open questions for the author.**

- After capture, should overdue timers run on the next ordinary input, or only on explicit clock advancement?
- Does a multi-screenshot test receive one deadline or a fresh deadline per capture?
- Which omitted viewport/output defaults should the two Caltrain acceptance commands use?

Verdict: NOT READY