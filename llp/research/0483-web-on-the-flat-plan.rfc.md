# LLP 0483: Web on the Flat Plan — resumability by construction, and the parts Next.js is actually winning on

**Type:** RFC
**Status:** Accepted (curation pass 2026-08-23)
**Systems:** Web, Contract (compiler/runtime), SSR/SSG, Router, Verification
**Author:** Charlie Cheever / Claude (Opus 5)
**Date:** 2026-08-18
**Revised:** 2026-08-20 (clock-join sync per the 2026-08-20 holistic corpus reviews — §2's pre-decision heading corrected (the runner language IS decided — option (b)); the option-(a) inlining passage labeled recorded pricing of the rejected option.) 2026-08-20 (r6 — LLP 0505 r3 decision propagation: the §2
runner question is DECIDED = option (b), the same Rust plan engine via
wasm; option (a) is dead. Cutover gates written into §2: corpus green
on the wasm/DOM path, the priced wasm costs measured, and the TS-seam
gate — declared synchronous plan→app-TS calls cheap and correct across
the wasm→page-JS boundary. The Summary's "open runner decision"
phrasing is superseded by §2's decided block.)
2026-08-20 (r5 — post-loop fold of the round-2 dual-family
materials, authorized by the author 2026-08-20; no reviewer has seen this
revision: the Summary's instrument-only wasm sentence reconciled with
§2's open runner decision (one statement, not two); §2 pins the wasm
**role** distinction — option (b) is a wasm plan runner applying ops to
browser-layout DOM, a different role than 0328 D5's Taffy-absolute
instrument, so choosing (b) is a scoped D5/D11 amendment, and 0500 D2's
cost acceptance does not itself choose it; the runner-inlining claim
scoped to option (a); §3's restore step imports 0485's confidentiality
lattice (client-projection slots only); §3 reconciles "adopt" with
LLP 0386's hydration transaction (reuse-or-supersede stated; pre-runner
interaction policy options named); §5 rewritten onto 0482's corrected
active-instance job DAG with the nested-fixtures obligation; the startup
claim's scaling list completed and LLP 0351's `clientEntry:false`
zero-runtime route retained; §9.7 closed — the `form` tag carries native
meaning per 0485 §12.4.6.)
2026-08-20 (r4 — program super-refine round 1, both families
NOT READY on r3: composed with frozen LLP 0500 D2 — the web dev loop
runs **the production runner** with a patch channel and the DOM mirror
retires; the runner-language question (fixed JS runner vs the same Rust
engine compiled to wasm) is surfaced as **an explicit open decision for
the owners to take atomically** (§2), with the facts stated on both
sides (this document and 0485 Track R specify a JS runner and keep wasm
instrument/embed per 0328 D5/D11; 0500 D2's recorded cost list accepts
"wasm in the web dev loop"; grok's round-1 recommendation is the
same-Rust-engine wasm runner as the true tax exit) — not decided here;
"instance data, not code" narrowed to the class-1 claim (no
app-specific JS closure/module graph — plan opcodes are executable
program material, and payload gates count the complete transitive
build); "memcpy-shaped" state serialization withdrawn per 0485
(schema-derived encoding plus region/instance/key/generation identity);
real-CSS/cascade promoted from open question to owed start-gate design
(0485 §12.4.7); the form surface pointed at 0485 Track W's proposed
built-in `form` tag under 0091's authority instead of re-asking; the
Related line's "0480 §10.6" cite corrected to 0480 §10 item 6.)
2026-08-18 (r2 after super-refine round 1, 3/3 NOT READY:
cross-site cache claim withdrawn (browsers partition the HTTP cache);
handler identity corrected to instance-level data per `hydration.ts`;
progressive enhancement scoped to a classified, server-admitted subset;
streaming placeholder contradiction resolved; cross-references fixed).
2026-08-18 (r3 after super-refine round 2, 3/3 NOT READY: §6
re-grounded — Contract has no `form` tag and `submit=` is an Enter-key
lowering, so PE is classified per complete form flow and requires a
form surface Contract must first grow; §3's serialized datum
corrected — curried arguments are re-evaluated at dispatch
(`makeHandler`), never captured at render; "hydration: none" restated
as plan-directed resumption whose adoption/restore/bind work remains;
§4's region deferral restricted to async-by-construction regions; the
Related line's D6 claim corrected to name the web-speed replacement
ask, now enumerated in LLP 0480 §10.6)
**Related:** LLP 0480 (The Plan Is the Program — the umbrella), LLP 0481
(Semantic tightenings — §3 and §6 here depend on its change 3), LLP 0482
(Parallelism tiers — §5 here is the web face of its tier 1; its tier 5,
region placement, is owned by LLP 0482 §6 and has no section here), LLP 0288
(Contract Is the Web Production Target — **Active**; this document proposes
no change to that posture), LLP 0351 (Server-Generation Strategy Guide —
the authority map this document must compose with, not replace), LLP 0472
(web-standards-first runtime APIs), LLP 0478 (Contract at Full Speed —
D6 keeps web on real DOM **and** keeps TS-AOT as the web speed story
(W1, its E14); this document retains the real-DOM half untouched and
proposes replacing the *speed mechanism* with the fixed runner + plan —
an ask LLP 0480 §10 item 6 now enumerates to 0478's owner), LLP 0500
(dev-loop decision, Active — D2 puts this document's runner in the web
dev loop with a patch channel and retires the DOM mirror; §2 composes
with it), LLP 0328 (native
execution tiers — D5/D11 keep wasm as instrument/embed), LLP 0154 / LLP 0160 (single-source routes,
Contract by default), LLP 0010 (router v2), `llp/contract/0086` (refresh and
state-preserving HMR), LLP 0365 (the Exact Guide)

## Summary

Goal (b) of the LLP 0480 derivation is that Exact should be a
**zero-compromise way to build a website** — as good as or better than
Next.js, TanStack Start, Vue, and Svelte. This document argues that the flat
plan makes three things structurally true on web that competitors achieve
only through significant engineering, and is honest that a fourth thing —
the ecosystem — is not winnable and must be answered differently.

Structurally true, given LLP 0480's plan and LLP 0481's changes:

1. **Resumption without graph-reconstruction hydration.** The client
   never re-executes app code to learn the reactive graph, because the
   graph *is* the plan; what ships for class-1 routes is **no
   app-specific JS closure/module graph** — plan opcodes are executable
   program material, and admitted routes may also carry capability
   implementations, referenced subprograms, and explicit escape chunks,
   so the honest claim is the deleted *category* (per-app compiled JS
   the client must execute to learn the graph), not "only inert data"
   (§3). DOM adoption, state restoration, and handler activation
   remain — "no hydration" names the deleted category precisely, not
   zero resume work. Qwik had to invent a serialization discipline to
   get this; here it is a consequence of the artifact being data.
2. **A fixed-size, app-independent runtime, cached per site across visits,
   routes, and deploys.** One small plan runner serves every route;
   payload scales with *your plan* — dense binary, compresses well — not
   with framework surface. (The first draft's cross-site cache
   sharing is withdrawn in §2 — browsers partition the HTTP cache.)
3. **Progressive enhancement that is derived rather than authored — for
   classified complete form flows, pending a form surface Contract does
   not yet have (§6).** Flows with one real document-submit boundary,
   serializable fields, and admitted effect signatures compile to plain
   form POSTs the server replays (LLP 0481 §4). Within that subset
   nothing can be forgotten — the advantage over hand-authored server
   actions; outside it, the compiler says so.

Retained without change: **real DOM** (LLP 0288, LLP 0478 §D6/§E14). Real
accessibility, real text selection, real SEO, real CSS, real extensions, real
devtools. Do not paint the web. On wasm, one statement (details in §2):
0328 D5/D11's Taffy-absolute instrument/embed role is untouched and never
becomes production web; the fixed runner itself **is the same Rust engine
compiled to wasm, applying ops to browser-layout real DOM** — a different
role than D5's — decided by the owner 2026-08-20 (LLP 0505 r3 row 1),
gated as §2 states.

**Nothing here is measured.** Every claim is a mechanism, per RFC 0478 §P7.

---

## 1. What the incumbents are actually selling

Be precise about the competition: "better than Next.js" is usually
asserted against the wrong axis. Next.js, TanStack Start, SvelteKit, and
Nuxt sell five things, in roughly this order of real value:

1. **Routing plus data loading as one coherent model** — file routes,
   loaders, nested layouts, revalidation, mutations that invalidate.
2. **Deployment that works** — adapters, edge/node targets, image and font
   pipelines, caching headers, middleware.
3. **The React (or Vue, or Svelte) ecosystem.**
4. **Streaming SSR and partial hydration**, increasingly.
5. **A dev loop that is fast enough not to think about.**

Notice that raw runtime performance is not on the list. **Hydration cost and
bundle size are on the list only as pain, not as product.** That is the
opening: a design that deletes hydration as a category, rather than
optimizing it, competes on an axis the incumbents cannot follow without
changing their own execution model.

Notice also that item 3 is not winnable. §7 says what to do about that
instead of pretending otherwise.

---

## 2. What the plan changes on web

On web the plan is not a native-tier artifact — it is the **same
artifact, consumed by a fixed runner over real DOM.** LLP 0478's W1
keeps web fast via TS-AOT; the clean-slate version keeps the target
(real DOM) and changes what is shipped to it.

**The dev loop is decided; the runner's language now is too** (§2's DECIDED block: option (b), the same Rust engine via wasm — this heading predated the ruling). Frozen
LLP 0500 D2 puts *this* runner in the web dev loop — dev runs the
production runner plus a patch channel and an introspection layer, and
the DOM mirror retires — so there is exactly one web runner, in dev and
production alike. What remains **an explicit open decision the owners
must take atomically** is what that one runner is written in:
(a) a fixed JavaScript runner (this document's r3 shape and 0485
Track R's current text), or (b) **the same Rust plan engine compiled to
wasm** — no second dispatch-loop lowering of the plan, the 0479 tax
argument's true exit. Pin the role distinction so (b) is priced
correctly: option (b) is a **wasm plan runner applying ops to
browser-layout real DOM** — a *different wasm role* than 0328 D5's
Taffy-absolute instrument (which is citizenship-incompatible with real
DOM and stays instrument-only either way) — so choosing (b) is a
scoped D5/D11 amendment for this one surface, not a promotion of the
instrument. And note the decision's true inputs: frozen 0500 D2's
recorded acceptance of "wasm in the web dev loop" is a *cost already
accepted*, not a choice of (b); choosing (b) must additionally price
compressed wasm bytes, compile/instantiate latency, CSP/compatibility,
and the wasm↔DOM call boundary, while choosing (a) must price the
retained per-engine opcode-behavior duplication (0479 §2.1's residual
tax) against 0500's one-runner rule.

**DECIDED (Charlie, 2026-08-20, LLP 0505 r3 row 1 — the one-engine
ruling): option (b).** The one web runner is **the same Rust plan
engine compiled to wasm**; option (a), the fixed JavaScript runner, is
dead — no second dispatch-loop lowering of the plan is ever built.
This is a direction decision with **cutover gates**, not a switch-flip
(today's Rust module is a non-default I4 spike; the existing wasm
feature is conformance/debug):

1. the runner exists and passes the conformance corpus on the wasm/DOM
   path;
2. the priced costs above (compressed bytes, compile/instantiate
   latency, CSP/compatibility, the wasm↔DOM call boundary) are measured
   and acceptable; and
3. **the TS-seam gate**: the plan's declared synchronous calls into
   imported app TS — the "my derive calls my utility function" path —
   are cheap and correct across the wasm→page-JS boundary. App-tier TS
   (providers, action effects, declared function imports) remains
   first-class; only Contract *semantics* execution is Rust.

Until the gates are met, W1's TS-AOT carries today's-tier web
performance (0478 D14(4)). The scoped D5/D11 role amendment (wasm plan
runner over browser-layout real DOM; the Taffy-absolute instrument
stays instrument-only) proceeds under this ruling. The behavior
specification below is unchanged and now describes the (b) runner:

| | Today's web tier | On the flat plan |
| --- | --- | --- |
| Shipped artifact | compiled JS closures per app | fixed runner + per-app plan (data) |
| Runtime size | scales with app + framework | fixed runner; app scales with plan |
| Startup work | parse + execute app JS, build the graph | fetch/decode plan; graph already exists |
| Graph reconstruction | re-execute app code to rebuild it | **none** — the graph is the plan |
| Resumption work | hydration: re-walk and reattach | plan-directed: adopt DOM, restore slots, bind lazily (§3) |
| SSR | render the tree | evaluate regions; emit segments |
| Caching | per-app bundle | immutable runner + content-addressed plan chunks, per site |

One caching claim from the first draft must be withdrawn: the cross-site
shared runner. Browsers have partitioned the HTTP cache by top-level site
since Chrome 86 / Firefox 85, WebKit earlier — a visitor to site A
shares nothing cached from site B, whatever the URL. The
claim that survives is same-site and still strong: one small, fixed,
content-addressed runner cached under immutable headers across every
visit, route, and deploy of *your* site, with per-route plan chunks
content-addressed the same way — app deploys never re-ship the runner, and
navigation fetches only new plan bytes. And under option (a) — **recorded pricing of the rejected option; (a) is dead per §2's DECIDED block** — because a
JS runner is small and fixed, **inlining it into the first response** was
a live option no framework-sized runtime has (under option (b) the same
move must be re-priced against wasm blob size and instantiate latency —
likely a fetch-with-immutable-cache, not an inline). One more retained
route: LLP 0351's `clientEntry: false` path survives unchanged — an
inert static route ships **neither** runner nor client plan; the fixed
runner is for routes with client behavior, not a new floor under every
page.

---

## 3. Resumability, and why it is free here

Hydration exists because the client must reconstruct, by executing code,
knowledge the server already had: which components exist, what state they
hold, which DOM node each handler owns. Every framework's hydration
cost is the price of transmitting that knowledge as *code to run* rather than
as *data*.

On the flat plan, the client already has the knowledge:

- **Structure** is the node and region tables (LLP 0481 §5).
- **State** is a flat slot array — LLP 0481's change 1 makes it typed
  and contiguous. (The first drafts said serialization is therefore
  "memcpy-shaped"; 0485 withdraws that — slots contain arena handles,
  so serialization is schema-derived encoding with per-value work, and
  resumption additionally carries region/site identity, instance paths,
  keys, generations, and cursor state. Still far cheaper than a cyclic
  object-graph walk; benchmark a state-heavy page rather than
  extrapolating from fixed slots.)
- **The dependency graph** is the plan's dep table, computed at build time
  (LLP 0481 §3), not rediscovered by running.
- **A handler** is data — but the datum must be chosen correctly, and
  r2 chose wrong. Repeated `each` instances and curried bindings
  (`press=press(dep.id)`) mean handler identity is (plan id,
  binding-site id, **instance path / row environment**, action index) —
  and **not** r2's "bound argument values": `makeHandler`
  (`runtime/view.ts:6394`) evaluates curried arguments **at event time
  from live instance scope**, so serializing SSR-time values would be a
  breaking capture-at-render semantics change (a row whose `dep.id`
  moved between render and click would dispatch the stale value).
  Resumption serializes the *environment* — instance path, row key —
  and re-evaluates arguments at dispatch, which is what the runtime
  already does; `runtime/hydration.ts` records node/event/action
  identity and resolves DOM anchors today.

So resumption is plan-directed, in three named steps that remain real
work: **adopt** the server-emitted DOM against the plan's node table,
**restore** the slot array and instance tables, and **bind** handlers —
lazily, on first interaction, or at idle. Two imported disciplines make
those steps sound rather than hopeful:

- **Restore restores the client projection only.** Frozen 0485's
  confidentiality lattice (`client < server-only < secret`, computed
  over the Deps graph) governs what may ship: the serialized state is
  the **client-disclosable slot projection plus required instance
  metadata**, never "the slot array"; a client-executable context that
  reads above-`client` state rejects at build or is server-placed. This
  is a build-classification consequence, not a runtime filter.
- **Adopt is LLP 0386's hydration transaction, reused — not a new
  mechanism.** The repository's hydration plan already requires total
  correspondence before mutation, activation fencing, explicit fallback
  custody, and preservation of live input/focus/scroll across the
  commit boundary, with registered verification exercising total
  preflight and whole-root fallback. The plan runner **reuses that
  transaction** (adoption = its correspondence+commit phases over the
  plan's node table); if Track R ever supersedes it, that is an explicit
  0386 amendment, not a silent second adopt path. One policy the
  transaction does not settle and this design must choose per route:
  **pre-runner interaction** — capture-and-replay, runner-before-
  interactive-paint, disabled-until-bound controls, or an acknowledged
  lost-event window (PE-classified flows already degrade to real form
  semantics, §6).

What is deleted is graph-reconstruction hydration: no app code executes
to learn what a click does. The defensible claim is **client startup
work independent of app *code* size** — it scales with the compressed
eager plan, the client-projected state and instance tables, capability
implementations, referenced subprograms, and any escape chunks, not
with app modules — a hypothesis until §8.2's measurement runs, not a
number that "approaches zero."

Qwik demonstrates this is achievable and also demonstrates its cost: a
serialization discipline, a lazy-loading segmentation scheme, and
authoring constraints (`$` boundaries) authors must think about. Here the
constraints are already the language's (LLP 0481), so the authoring
surface carries none of it.

**Honest boundary:** resumability moves the cost, it does not delete it.
The state slots and instance tables must be transmitted, and for a
data-dense page that is real bytes. The mitigation is that slots are
typed and dense — far cheaper than the JSON-shaped state trees frameworks
serialize today — and that regions can defer their state until
interaction (§4).

---

## 4. Payload, and the plan-size problem

LLP 0481 §5.4 records the cost honestly: **static structure resolution
makes plans encode every branch.** On a `mmap`ed native artifact that is
cheap; over a cold mobile network it is not.

The answers, in order:

1. **Route-level plans.** A route's plan is its own artifact; the router
   fetches plans on navigation. This is the existing single-source route
   model (LLP 0154 / LLP 0160) doing double duty.
2. **Region-level deferral — restricted to async-by-construction
   regions.** r2 allowed any false-`when` region to defer its plan
   slice; that injects fallible network I/O between a committed state
   change and view output the plan owes *now* — a `when` flipping true
   is synchronous by contract, and its plan slice may be across a cold
   network. Deferral is sound only where the region already has a
   declared pending arm — `resource`/`async`/`boundary` regions
   (LLP 0481 §5.2's state-machine regions) — or under an explicit
   `lazy` marker priced as a semantic addition. A bare `when` is not
   deferrable. Regions remain the unit; the eligible set shrinks.
3. **Shared component plans are content-addressed** (LLP 0480 §7.2). Under
   cache partitioning (§2) the reuse is same-site: Facet component plans
   are identical bytes across your routes and deploys, so a deploy
   invalidates only what changed. (Whether monomorphization and theme
   resolution preserve byte-identity is the open question LLP 0480 §7.2
   poses.)
4. **Dense binary compresses well.** RFC 0478 §1.2 carries the correct
   caution here: the compact-plan backend's 80 KB vs 203 KB *minified*
   ratio shrank to 1.45× under deterministic gzip. **Any payload
   claim in this document must be made on compressed bytes or not at all.**

Whether the net is smaller than an equivalent Svelte or Qwik app is
**unknown and must be measured** on a real app before it is claimed. That
measurement is cheap and should precede any external statement.

---

## 5. SSR, streaming, and the region partition

LLP 0482 §2 is the mechanism; this is its web face.

Server rendering becomes: expand the region schema against state/data
into **active-instance jobs** (LLP 0482 §2.2's corrected unit — guard
state, parent/template edge, row environments, cell/fetch readiness,
document position; a nested `when → each` is not two unconditional peer
jobs), evaluate ready jobs in parallel, emit HTML segments, concatenate
the rope by document-position token. Because the *schema* partition is a
compile-time fact, there is no dynamic dependency-discovery overhead and
no risk of a cross-partition read mid-render (queueing/dispatch/join
remain real, charged costs, per 0482 §2.1). The conformance obligation
rides with it: nested `when`/`each`/cell SSR fixtures with
document-position assertions, before any fan-out claim is quoted.

**Streaming is one honest fork, not a slogan** — the first draft's
"document order, with placeholders" was self-contradictory. Document-order
flush never needs placeholders and never emits past a still-pending
region: TTFB is the first segment, but a slow early region head-of-line
blocks everything after it. Out-of-order backfill — placeholder now,
content later, which is what React Suspense streaming does — requires a
client receiver to move late content into place, spending exactly the
no-JS window §6 prizes. The rule: routes declaring a no-JS guarantee
stream in document order; routes that accept the receiver may backfill.
The partition gives both; it does not make the trade disappear.

**Static generation should parallelize near-linearly** (unmeasured;
LLP 0482 §7) — routes and regions are independent. For a large content
site this is the difference between a build that gates deploys and one
that does not.

This must compose with, not replace, LLP 0351's server-generation strategy
map and the runnable references in `examples/server-generation/`. Its
discipline — the least runtime-capable strategy that satisfies the route;
copy existing generated-route and artifact shapes rather than inventing
parallel registries — applies unchanged.

---

## 6. Progressive enhancement, derived — for classified form flows

LLP 0481's change 3 makes an action a reducer with a closed effect
signature. The web consequence is real but narrower than the first
draft's "every interaction, no JS, for free" — and narrower than r2's
version too, which classified individual *bindings* as form-shaped. The
ground truth r3 must start from: **Contract has no form surface today.**
`CONTRACT_TAG_LOWERING` carries no `form` tag; the `submit=` attr is an
Enter-key convenience lowered to `onKeyDown`
(`runtime/view.ts:699, 1981`), not a document-submit boundary; and a
`change=` binding on its own has no no-JS delivery vehicle at all.
LLP 0351 already records the consequence: its progressive-form modifier
is React's `<Form progressive>`, a logged exception — "no canonical
Contract form surface is claimed."

**The unit is a complete form flow, not a binding.** PE-eligibility is a
property a flow earns as a whole:

1. a **form grouping with a real document-submit boundary** — an
   action/method pair the browser can submit without script. This is
   the surface Contract must grow (a `form` container or equivalent
   semantic grouping, exactly one submit boundary per flow); until it
   exists, nothing below applies;
2. **successful controls** — named inputs inside the grouping whose
   values serialize as form fields, covering the submit action's event
   payload and any curried arguments;
3. **request-time deployment** — a server exists to receive the POST
   (LLP 0351's ladder; an SSG-only route cannot PE);
4. a **server-admitted effect signature** (change 3) for the flow's
   submit action;
5. the **full response protocol**, designed rather than implied:
   validation failure → a re-rendered document carrying errors and
   prior values (422); success → redirect (POST/redirect/GET); stale
   document → refusal via plan digest and state epoch — each as a
   document navigation, since there is no runner yet to patch state.

Contract's wider event vocabulary — `builtin-schema.ts` carries focus,
blur, key, pointer, scroll, and collection events — is not form-shaped
and never PE-eligible; those bindings are enhancements over the
discoverable form path, and a `press` participates only as a flow's
submit control. The compiler *classifies* every flow; a route may
declare a no-JS guarantee, and the build then lists exactly which flows
(and which stray interactive bindings) fail the classification. Within
the subset nothing can be forgotten — that, not universality, is the
advantage over hand-authored server actions (Next.js/Remix/TanStack
form actions are opt-in per interaction, absent wherever a developer
reached for `onClick`).

**The concession this section owes plainly:** shipping any of this
requires a form surface Contract does not have. That is a named gap in
the LLP 0365 sense — declared in the Guide, not discovered by a user.
**The construct's proposed spelling now exists**: Accepted 0485
Track W proposes the built-in `form` tag as the resolution, under
`llp/contract/0091`'s form-IR authority (0485 §12.4.6's owner
amendments gate PE shipping); this section consumes that proposal
rather than re-asking tag-vs-grouping.

**What the server must hold before replay is sound** — all owed design:

- **Prior-state authority.** The transition needs the state it applies to.
  Three candidate loci — a server session store; slots serialized into the
  form under an integrity signature; stateless refetch/re-derive —
  choosing one is the §8.3 design's first decision.
- **Staleness refusal.** The form carries the plan digest and a state
  epoch; the server rejects a POST from a stale document instead of
  replaying against a moved world.
- **Request integrity.** CSRF/origin evidence, idempotency keys for
  retried POSTs, and payload validation against the action's declared
  event type.
- **Authorization at the capability boundary**, never assumed from the
  client having rendered the button — LLP 0476's authority-native-held
  pattern and LLP 0333's boundary-schema discipline are the frames.

Consequences, correctly scoped: once the form surface exists,
PE-classified routes work with JavaScript disabled and during the
pre-runner window; the fallback is real form semantics, an accessibility
gain; and the classification is a build artifact an agent can assert
against ("this checkout flow is no-JS complete"), not a hope.

---

## 7. What this does not win, and the honest answer

**The ecosystem.** A site that needs Stripe Elements, a specific rich-text
editor, a mapping SDK, or any of ten thousand npm packages with DOM
assumptions is better served by React today, and will be for years. LLP 0160
already answers this correctly at the policy level — React remains a fully
supported tier, and ecosystem-heavy apps stay React — and nothing in this
series changes that.

The honest positioning is not "replace React on web." It is:

> For the surfaces where **payload, startup, and interaction latency are the
> product** — content sites, storefronts, documentation, dashboards, landing
> pages, embedded and low-end-device experiences — the flat plan is
> structurally better, and the same source runs natively. For surfaces where
> **ecosystem reach is the product**, use React, in the same repository,
> under the same router.

**Second thing not won: deployment surface.** Adapters, image pipelines,
font optimization, middleware, edge runtimes, and cache-header policy are
years of unglamorous work that Next.js has done and we have not. LLP 0351's
map and `examples/server-generation/` are the beginning of it. Any claim of
"better than Next.js" that ignores this is not credible, and the gap should
be stated in the Guide rather than discovered by a user.

**Third: the dev loop must be at least as good.** Vite-class HMR is table
stakes. The plan's advantage — hot reload as a plan swap plus a computed
state migration over named, typed slots (LLP 0480 §9.5,
`llp/contract/0086`) — should make state-preserving reload *more* reliable
than module-graph HMR, since slot identity is explicit rather than inferred.
That is a claim to demonstrate early, because the dev loop is the first
thing an evaluator experiences.

---

## 8. What is owed

1. **A compressed-bytes payload comparison** on a real application against
   Svelte, Qwik, and Next.js. Cheap, and it gates every payload claim
   (§4) — measured over the **complete transitive build** (runner +
   plans + capability implementations + referenced subprograms + escape
   chunks), never the plan bytes alone (§Summary's narrowed claim).
2. **A resumption-cost measurement**: time from HTML paint to first
   successful interaction, versus hydrated equivalents. This is the metric
   the design actually optimizes; no one has measured it here.
3. **The form surface and server-replay design** (§6): 0485 Track W's
   proposed `form` tag under 0091's authority first, then state locus,
   staleness refusal, request integrity, authorization — before any
   progressive-enhancement claim ships.
4. **Region-level plan deferral** design (§4.2) — the router and the plan
   loader are the same problem.
5. **Composition with LLP 0351**: where region partitioning sits in that
   document's strategy ladder, and whether it is a new rung or a property of
   existing ones.
6. **A Guide page stating the ecosystem and deployment boundaries** (§7)
   plainly, per LLP 0365's discipline — a GAP declared is worth more than a
   gap discovered.
7. **The real-CSS/cascade design — a start gate, not an open question.**
   Tokens and recipes must emit real CSS with real cascade (custom
   properties; classes/rules, not inline styles) or accessibility and
   print break, and no credible payload/interaction comparison (item 1)
   can run without it. 0485 §12.4.7 already treats style lowering as
   Track W design that starts immediately; this document adopts that
   gate (moved here from the r3 open-questions list).

## 9. Open questions

1. Does the fixed runner stay genuinely fixed, or does feature growth make
   it app-dependent again? If it drifts, the §2 caching argument dies. What
   enforces it — a size gate in `exact-verify.json`?
2. Is the plan decoded once into JS structures, or indexed lazily out of an
   `ArrayBuffer`? The first is faster to run and slower to start; the
   second is the opposite; the answer probably differs by route size.
3. *(Promoted to §8 item 7 — a start gate, no longer open.)* Residual
   question only: which cascade layers/namespacing conventions the
   emitted CSS uses.
4. Does resumability interact badly with third-party scripts and browser
   extensions that mutate the DOM before the runner binds?
5. What happens to `view-transition`, browser back/forward cache, and
   scroll restoration under region-level deferral?
6. Is there a credible story for embedding a React island inside a
   plan-rendered page, so §7's positioning is a gradient rather than a
   per-site choice?
7. *(Closed — 0485 Track W proposes the built-in `form` tag under
   0091's authority, §6, and 0485 §12.4.6 already carries the native
   answer: the tag has native meaning — grouping, default-submit, and
   accessibility semantics ride it on every tier, per that section's
   cross-reference to this question. Not web-only surface.)*
