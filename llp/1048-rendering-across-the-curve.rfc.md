# LLP 1048: Rendering across the curve — one renderer, from static pages to per-request HTML

**Type:** RFC
**Status:** Draft r2 — direction approved by Charlie 2026-09-23 ("ok let's do pre-rendering etc. … we might as well have a great solution for almost every point on that curve"). Ruled the same day (§9): the Contract surface in LLP 1048.003 is approved; Interview is the website consumer (§10); the server target is a native Rust binary; the NOT-DOING take is Messages as the Snapback4 consumer (recorded in `rules/NOT-DOING.md`). r2 folds in Astra max's sanity check (`llp/reviews/1048-rendering-across-the-curve.astra.md`: MATERIAL FINDINGS, all dispositioned there). Open: Interview's public access (§9.5)
**Systems:** Web host (`host/web`: a document serializer sharing `Host::create`'s projection; a seeded boot, later adoption, in `glue.js`; the page shell); build (`contract::bake`, `host/web/build.mjs`); a native render server (the async render host, per-request isolation, the HTTP contract); Runner (document checkpoints, stage and confidentiality labels, placeholders); Contract (head, semantic elements, media variants, route policies — LLP 1048.003); Router (enumeration, navigation data); Delivery (web releases carry rendered pages)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** assigned by Charlie per phase (§7)
**Date:** 2026-09-23
**Related:** LLP 1007 (web host), 1023 (one URL per app), 1038 (router), 1039 (viewport facts), 1016 (async settlement), 1018 (the store), 1027 (data sources), 1030 (delivery), 1047 (pay for what you use), 1005 §8 (the dependency table); `rules/NOT-DOING.md` (the 2026-09-23 entry); sub-LLPs 1048.000 (pages at build and per request), 1048.001 (adopt, don't hydrate), 1048.002 (holes, caching, streaming, the edge), 1048.003 (documents in Contract)

## Summary

Websites are a goal (Charlie, 2026-09-23). Today an exact2 page is an empty
`#exact-root` until about 390 KB (brotli) of wasm arrives and boots. Crawlers,
link unfurlers and readers without JavaScript see nothing, and phones wait: the
2026-09-22 website trial painted first at 2.26 s on a 6 Mbit/s, 150 ms profile.

The web's answer has split into a dozen named strategies — SSG, SSR, ISR,
streaming, partial prerendering, islands, resumability, server components.
Most are local optima around two costs:
- a component framework cannot prove that the server's HTML equals what the
  client will render, so it re-runs components on the client to attach behavior
  (hydration);
- it cannot see what a component depends on, so authors mark boundaries.

exact2 can pay both costs differently. Its runner is deterministic over declared
inputs, the web host's DOM is one Rust projection, every binding's reads are
known, and all data enters through declared sources. So most of the curve can
collapse into **one renderer with three settings**:

- **when** a page renders — a render policy per route: `client`, `build`,
  `cached` or `request`;
- **how** it arrives — complete, or streamed as its slower sources answer;
- **when** it becomes interactive — an activation policy: `eager`, `idle`,
  `visible`, `interaction` or `never`.

Two definitions make the collapse real, and r2 states them precisely:

- A **document** is the DOM projection of a settled checkpoint, before browser
  layout (D1).
- A **document checkpoint** is the state that projection came from (D4). It is
  not the dev loop's reload state.

Partial prerendering follows from each input's stage and confidentiality (D3).
**Adoption** replaces hydration: Contract evaluation rebuilds the tree from the
checkpoint, and the DOM is kept rather than rebuilt when the canonical documents
match.

## 1. The curve today

About ten rendering strategies that people choose for real reasons, plus two
axes that cut across all of them:

| # | Strategy | What it is | Why people choose it | Examples (2026) |
|---|---|---|---|---|
| 1 | Client rendering (SPA) | An empty shell; JavaScript renders everything | Apps behind a login, no SEO need, static hosting | Vite SPA; React Router SPA mode |
| 2 | Static generation (SSG) | Every page rendered at build | Content known at build; CDN speed and cost; SEO | Astro, Eleventy, Hugo; Next `output: export` |
| 3 | Incremental / on-demand regeneration (ISR, `use cache`) | Render on first request, cache, revalidate by TTL or tag | Too many pages to build; content that changes occasionally | Next revalidate/`revalidateTag`; Nuxt route rules |
| 4 | Server rendering per request (SSR) | Render on every request | Personalized or fresh data; auth; SEO for dynamic pages | Next dynamic routes; React Router; SvelteKit; Rails |
| 5 | Streaming SSR | Flush the shell, stream slow parts out of order | First paint not blocked by the slowest query | React Suspense streaming; SolidStart; Marko |
| 6 | Partial prerendering / server islands | A static shell from the CDN, dynamic holes per request | CDN speed and personalization in one page | Next 16 Cache Components; React 19.2 `prerender`/`resume`; Astro 6 `server:defer` |
| 7 | Edge rendering | 3–6, run near the user | Latency; geography | Cloudflare Workers; Deno Deploy |
| 8 | Server-driven UI (HTML over the wire) | The server renders fragments per interaction | Server-owned logic, minimal client code, realtime | htmx; Hotwire; LiveView; Blazor Server |
| 9 | Progressive enhancement / server actions | Forms work without JavaScript; mutations run on the server | Resilience; accessibility | React Router forms; SvelteKit actions; Next server actions |
| 10 | Server components (RSC) | Server-only components ship no client code | Smaller bundles; data colocated with UI; secrets stay server-side | Next App Router |

**Becoming interactive:** full hydration (React); incremental hydration with
event replay (Angular 21, Nuxt); islands (Astro, Fresh); resumability (Qwik 2);
none.

**Navigating:** client routing with data payloads and prefetch; or multi-page
navigation made instant by Speculation Rules, the back/forward cache and
cross-document view transitions.

Serious frameworks mix these per route (Astro's per-page `prerender`; TanStack
Start's `ssr: true | 'data-only' | false`).

## 2. What is different about exact2 — and what is not

1. **A deterministic tree.** Over equal declared inputs — plan, location, source
   answers, clock, store, viewport — the runner builds equal trees, with view ids
   from a counter (`Ids::fresh`). Host batches are *not* byte-identical across
   hosts in one process: runtime incarnation ids differ, for example the
   process-global reorder runtime counter (`host/web/src/reorder_drag.rs:12`).
   Equality is therefore claimed of a canonical document (D1), not of batch
   bytes.
2. **One DOM projection.** `Host::create`, `tag_for`, `props_for`, `css.rs` and
   the glue's property rules (`glue.js` `applyProps`) decide every element's tag,
   attributes and properties. Once that projection, including the URL policy, is
   shared Rust, a server document and the live DOM agree by construction.
3. **State that can be checkpointed.** `Runner::carry()`/`boot_carrying` restart a
   runner for the dev loop (LLP 1007 §6). That is reload state: it rebuilds tree,
   ids, timers and derives, keeps only root slots, omits pending answers,
   restarts timers from the carried clock, and replaces the store. A page needs a
   **document checkpoint** instead (D4). Carry's encodings can be reused; its
   semantics cannot.
4. **Known reads.** The dependency table (LLP 1005 §8, 2026-09-22) records what
   every binding and site reads; the VM propagates store dependence. Two gaps
   remain before this can carry a privacy guarantee:
   - it doesn't see what a source reads *inside* its own code;
   - store reads during `parse` (`runner/src/runner/commit.rs` ~595) aren't
     attributed.

   D3 closes both.
5. **Declared data.** Resources enter through named sources that answer now or
   later; the host does all I/O (LLP 1016). The same sources can run at build, on
   a server (Hermes for TypeScript), or in the browser.
6. **A fixed runtime cost.** Interactivity costs one wasm and a boot proportional
   to the tree, not per-component JavaScript. There is nothing to split into
   islands — only "load the runtime" or "don't".

Server components' main benefit — no client code for server-only logic — does
**not** follow from stages alone. Today the build ships the whole paired module
(`host/web/build.mjs` ~98–104). Server-only source artifacts are a phase-3
question (LLP 1048.002 §7).

## 3. The synthesis

| We build | Covers (§1) | What it collapses |
|---|---|---|
| **A render policy per route** — `client`, `build`, `cached`, `request` — for one renderer, deployed to a static `dist/`, the native server, or later the edge | 1, 2, 3, 4, 7; SPA shells; static export | SSG, ISR and SSR as separate systems |
| **Stage-inferred partial prerendering** — each input is available at `build`, `request` or `client`; a subtree renders at the earliest stage its inputs allow and leaves a hole | 6; SPA shells | Suspense boundaries; `use cache` annotations; server islands |
| **Streaming** — a request page flushes what is ready and streams holes as their sources answer | 5 | — |
| **Adoption with an activation policy** — rebuild the tree from the document checkpoint and keep the DOM when the canonical documents match; `interaction` replays the triggering event; `never` ships no runtime | full and incremental hydration; resumability; islands; zero-JS pages | hydration's rebuild of the DOM, and its mismatch class |
| **Navigation with route data** — client navigation reuses the destination's answers when their identity matches (D7); every route is also a real document | client routing with prefetch; multi-page navigation; Speculation Rules | — |
| **Documents in Contract** — head, semantic elements, media variants, document scrolling | table stakes for every row | — |

Server components (10) partly collapse: data answered on the server travels as
data, but code separation waits for server-only artifacts.

**Deferred, not specified** (no implementer; each returns with a consumer):
server actions that work without the runtime (9); server-driven UI (8); offline
service workers; bot-only rendering (never — every page is real HTML).

What an author chooses stays small: a render policy per route, rarely an
activation policy, and each source's stage and confidentiality. Everything else
is inferred.

## 4. Decisions

**D1 — The document is the page.** A route's HTML is its **document**: the DOM
projection of a settled checkpoint (D4), before browser layout, produced by the
live host's own projection. That projection includes:
- the URL policy (`871e2631`);
- the property-to-HTML rules: checked, value, booleans, `textarea` content,
  canvas wrappers, markup expansion;
- content models for semantic elements.

Browser-owned effects are named and excluded: font loading, symbol sizing from
computed styles, focus, scrolling, context positioning, and windows chosen from
scrollport geometry.

A public document renders a virtualized collection's items in full, within a
bound, as static HTML; a canvas renders its fallback content. Parity is claimed
for the document only, and checked by parsing the served HTML (LLP 1048.000 §4).

**D2 — A render policy per route:**
- `client`: the shell only, as today;
- `build`: every enumerated location, rendered at build;
- `cached`: rendered on first request, cached, revalidated;
- `request`: rendered per request.

The default is inferred from the route's inputs (D3). Authors override per
route (LLP 1048.003 D5).

**D3 — Stage and confidentiality are separate labels.**
- **Stage:** when an input can be known — `build`, `request` or `client`.
- **Confidentiality:** who may see it — `public` or `private`.

Sources declare both. The runner's own inputs have fixed labels:
- the store: `client`, `private`;
- the viewport: `client`, or `request` from client hints; `public`;
- the clock: `request`, `public`;
- the router location: `build` for enumerated routes, `request` otherwise;
  `public`.

A binding takes the latest stage and the most private confidentiality among its
inputs, propagated through resource arguments, control flow, keys and pending
state. What a source reads *inside its own code* is tracked at runtime: store
and request-context reads are recorded per call and per continuation, and the
`parse` attribution gap is fixed first. Unknown means private.

A `build` or `cached` page may contain public request inputs (a locale, a
country), never private ones. The renderer checks the whole serialized payload,
not only visible bindings, and public payloads carry no store.

**D4 — Adopt, don't hydrate.** A rendered page embeds its **document
checkpoint** (specified in LLP 1048.001 §1):
- resource answers, with their arguments and logic identity;
- pending flags;
- the router location;
- the time;
- row state, where the document holds it;
- never a store.

The client rebuilds its tree by Contract evaluation from that checkpoint. If its
canonical document equals the page's — the digest excludes runtime incarnation
ids — it binds the existing DOM instead of rebuilding it; otherwise it replaces
the document once. The device's own store and clock then apply as ordinary
updates. A page's checkpoint never overrides the device's store.

**D5 — Activation is a policy, inferred by default.** `never` when a page needs
no runtime: no handlers, timers, pending sources or client holes, and no host
capability that needs runtime work (canvas surfaces, virtualized collections).
Otherwise `eager`. Authors may choose `idle`, `visible` or `interaction`; events
before adoption are replayed with their semantics (LLP 1048.001 D5).

**D6 — Placeholders, never refusals.** A resource whose source cannot answer at
the current stage renders its placeholder with `pending(x)` true. Boot no longer
refuses an unanswerable resource (`runner/src/runner/settlement.rs` ~380 today;
the website trial's deep-link crash). The compiler requires a placeholder where
a shape has no empty value (LLP 1048.003 D6).

**D7 — Route data for navigation.** Rendered routes publish their answers.
Client navigation reuses an answer only when its source, arguments, logic
identity, freshness and principal all match; otherwise it asks the source. The
uncached request-data endpoint arrives in phase 2 with navigation (LLP 1048.001
D7).

**D8 — The head is part of the view.** A `head` element in the active subtree
sets title, description, canonical URL, image and robots; the innermost wins.
The renderer writes it into `<head>`; the web host keeps `document.title` and
the meta tags in sync; native hosts map `title` to the window or scene title
(LLP 1048.003 D1).

**D9 — One renderer, a native server.** Rendering is a Rust library over the
runner and the host's projection, with an **async render host**:
- Hermes for TypeScript sources;
- the native executor core for requests;
- continuation pumping;
- a completion rule that ignores ordinary timers and mutations;
- deadlines and cancellation.

Two callers use it: the build, producing a static `dist/`, and a native server
binary (LLP 1048.000 D9–D11). The edge comes later (LLP 1048.002 D9). Node is
not on the render path.

**D10 — Portable static output.** `dist/` holds one `index.html` per rendered
route, `404.html`, `sitemap.xml`, `robots.txt`, content-hashed assets and
precompressed files. It deploys to any static host.

**D11 — The rules still hold.** No app JavaScript runs before first pixel — the
first pixels are HTML. The boot and adoption glue is host code, counted by
`boot.mjs` and budgeted in bytes (LLP 1047 D9).

## 5. NOT-DOING (recorded 2026-09-23)

`rules/NOT-DOING.md` used to refuse "Server generation in every form: SSR,
streaming, static export, progressive forms, hydration, route payloads, response
caching."

Its 2026-09-23 entry moves these onto the doing-list, with Interview as the
consumer:
- pages at build and per request;
- streaming;
- adoption;
- route data;
- cache policies.

The take: Messages as the Snapback4 consumer — Interview, built on Snapback4, is
the consumer now. Still refused: progressive forms and server-driven UI. No Node
on the render path.

## 6. Prerequisites

- **Pending in the web lane** (2026-09-23): `h1`–`h6` from heading levels,
  `lang`, same-origin link interception, brotli, caching and preload, font
  fallbacks.
- **Landed:** one URL allowlist for every navigating attribute (`871e2631`).
- **LLP 1048.003, phase 1:** head, semantic elements with their HTML defaults,
  explicit document scrolling, route fields, width variants.
- **LLP 1048.003, phase 2:** the remaining media features and placeholders.
- **LLP 1047:** the runtime's size — the cost of interactivity once the HTML has
  painted.

## 7. Phases

| Phase | Sub-LLPs | What users get |
|---|---|---|
| 1 | **1048.000** (the document and its safe projection; the async render host; rendering at build and per request from the native server with its isolation and HTTP contract; seeded client boots) + 1048.003 part A (head, semantic elements, document scrolling, route fields, width variants) | Interview's public question, post and profile pages as real HTML with titles, link previews and real links; delayed data handled; correct 404s; no content lost when the runtime starts |
| 2 | **1048.001** (checkpoint digest and adoption, host-state adoption, activation, replay, navigation data and the request-data endpoint) + 1048.003 part B (remaining media features, placeholders) | No DOM rebuild, zero-JS pages, a lazy runtime, instant navigation |
| 3 | **1048.002** (holes with anchors, cache policies, streaming, the edge, server-only artifacts) | Cached question pages at CDN speed; private holes; streaming |

Phase 1 is materially larger than "bake plus a serializer". Rendering Interview's
data-driven pages needs the async render host, and a seeded boot is what keeps
the runtime's first tree equal to the server's. `client` stays the default until
phase 1 ships.

## 8. Verification

- **First acceptance case:** an anonymous Interview question URL with
  deliberately delayed data. It must deliver:
  - useful HTML and metadata with JavaScript off;
  - real links;
  - correct 404 and 5xx responses;
  - no content lost when the runtime starts.

  Then: signed-in activation, desktop layout, navigation.
- **Parity:** the served HTML, parsed, equals the live host's document for every
  rendered route of every app (normalized; a difference names the element and
  attribute).
- **Counterexamples**, each a test:
  - delayed replies;
  - a store read after an await;
  - differing viewports;
  - repeated server requests in one process;
  - malformed URLs;
  - empty regions;
  - edits before activation.
- **Measurements:** server work and time to first byte, meaningful content paint,
  layout shift, payload bytes and activation cost, at 150 ms RTT and 6 Mbit/s.
- **Privacy:** no private value anywhere in a `build` or `cached` payload — a
  renderer refusal, tested per app.

## 9. Rulings (Charlie, 2026-09-23)

1. **The NOT-DOING take (§5): Messages as the Snapback4 consumer** comes off the
   doing-list ("We don't need that on the list"); Interview is the Snapback4
   consumer. The game engine stays (Charlie is working on it).
2. **The Contract surface in LLP 1048.003: approved** — the `head` element,
   semantic elements, media variants, route policy fields and placeholders.
3. **The website consumer: Interview** (`~/projects/interview`, §10).
4. **The server target: a native Rust binary.**
5. **Open — Interview's public access.** Interview's own `rules/NOT-DOING.md`
   (~16) excludes "anonymous posting or public access" in its current milestone.
   Rendering its pages for crawlers and link previews needs some content to be
   public. Charlie decides which — questions, answers, posts, profiles — and
   whether that includes content written while invitation-only.

## 10. The consumer: Interview

Interview (`~/projects/interview`) is a successor to Quora built on exact2 and
Snapback4: questions, AI and human answers, profiles, votes, comments, search, a
feed and Markdown posts, on iOS, macOS and the web. Its question pages are what
people share and what search engines should index — the case this LLP exists
for. Its routes (`app.contract`'s `routes nav`) map onto render policies:

| Route | Content | Phase 1 | Phase 3 |
|---|---|---|---|
| `/prompt/:question`, `/post/:post`, `/people/:person` | Public content (§9.5) | `request` (native server) | `cached`, revalidated by the item's tag |
| `/`, `/prompts` | Feeds | `request` (anonymous view) | `cached` shell; the viewer's parts are private holes |
| `/search?q=` | Query results | `request` | `request` |
| `/prompts/new`, `/prompt/:question/write`, `/messages`, `/notifications`, `/profile`, `/settings` | Private | `client` | `client` |

Interview's own work, in its repository — all phase 1:

1. **Public view gating.** Content renders only for
   `data.ready and data.authenticated` today (`app.contract` ~655, ~703), so
   logged-out readers get the welcome flow. Public routes must render public
   content to anonymous readers.
2. **A public content source**, split from the viewer, answered by bounded,
   anonymous backend projections whose shapes match what the views read.
   - Today `app.ts` sends `replicaOnly: true` to `/state`, and the anonymous
     branch of that response returns no content (`app.ts` ~86;
     `scripts/backend.mjs` ~163, ~208).
   - The older full response's field names (`selected`, `answers`, `profile`)
     don't match what the adapter reads (`details`, `profiles`).
   - The viewer, drafts and accounts stay `client`/`private`.
   - Setting `authenticated=true` to fake public access is ruled out.
3. **Initial data without the `started` timer.** `state started = false` holds
   the source empty until a 250 ms timer (`app.contract` ~175, ~241). Public
   routes read their data immediately.
4. **Real links.** Posts, questions and authors navigate through buttons with
   actions (`app.contract` ~1007, ~1016, ~1031). They become `link href=` with
   the same actions, so they are crawlable and open in new tabs.
5. **Search in the URL.** `/search?q=` drives the query instead of local state
   (`app.contract` ~187, ~206).
6. **A document layout.** The screen scrolls in an inner scroller inside a
   height-constrained container (`app.contract` ~637). Public routes use
   explicit document scrolling (LLP 1048.003 D4).
7. **A `head` per route:** the question's text as the title, the leading
   answer's excerpt as the description, the author, an image.
8. **Missing items:** an unknown question, post or person renders the not-found
   route with a 404 status.
9. **The rail.** `wide` switches structure (tabs or rail) from `exactViewport()`
   (`app.contract` ~157, ~413). Both structures render, and width variants (LLP
   1048.003 D3) choose which is displayed, so one document is right at every
   width.
