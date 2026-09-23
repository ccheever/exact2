# LLP 1048: Rendering across the curve — one renderer, from static pages to per-request HTML

**Type:** RFC
**Status:** Draft r3. Charlie approved the direction on 2026-09-23 ("ok let's do pre-rendering etc. … we might as well have a great solution for almost every point on that curve").
- r2 folded in Astra max's sanity check (`llp/reviews/1048-rendering-across-the-curve.astra.md`).
- r3 folds in three directional reviews, each "directionally right with major changes" (`llp/reviews/1048-rendering-across-the-curve.{grok,fable,claude}.md`): Grok 4.7, Fable 5.1, and a fresh Opus 5.5 session.
- r3 also folds in Charlie's rulings from the same night (§9):
  - Interview's content is public at each author's choice;
  - hosting is local for now;
  - pages for signed-in readers are rendered on the server too, in a simple form (D3).

Nothing is open.
**Systems:**
- Web host (`host/web`): a document projection shared with `Host::create`; the boot glue (render fresh, then adoption); the page shell.
- Build: `contract::bake`, `host/web/build.mjs`.
- The Linux host: a serve mode with the async render host, per-request isolation and the HTTP contract.
- Runner: checkpoint answers, stage labels, placeholders.
- Contract (LLP 1048.003): the head, document scrolling, route policies.
- Router: enumeration, navigation data.
- Delivery: web releases carry rendered pages.
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** phase 1 is two lanes, both from 2026-09-23 (§7):
- the render lane (Claude, `render/phase1`) in exact2;
- the Interview lane (`web/public-pages`) in Interview.

Later phases are assigned at their kickoffs.
**Date:** 2026-09-23
**Related:**
- LLPs: 1007 (web host), 1023 (one URL per app), 1038 (router), 1039 (viewport facts), 1016 (async settlement), 1018 (the store), 1027 (data sources), 1030 (delivery), 1047 (pay for what you use), 1005 §8 (the dependency table).
- `rules/NOT-DOING.md` (the 2026-09-23 entry).
- Sub-LLPs:
  - 1048.000: pages at build and per request (phase 1);
  - 1048.001: activation and navigation (phase 2, RFC);
  - 1048.002: personal pages, and what waits (phase 2, RFC);
  - 1048.003: documents in Contract.

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
collapse into **one renderer with two settings per route**:

- **when** a page renders — a render policy: `client`, `build`, `cached` or
  `request`;
- **when** it becomes interactive — an activation policy. By default the
  runtime loads when the page is idle, and a page that needs no runtime never
  loads it.

The server renders two kinds of page and never mixes them (D3):
- an **anonymous** page, for a request without a session: public data only,
  cacheable by any shared cache;
- a **personal** page, for a request carrying the reader's session: the whole
  page rendered for that reader, never stored by a shared cache (phase 2).

Two definitions make the collapse real:

- A **document** is the DOM projection of a settled checkpoint, before browser
  layout (D1).
- A **document checkpoint** is the state that projection came from, including
  the answers the page used (D4). It is not the dev loop's reload state.

**Adoption** replaces hydration. The runtime boots from the checkpoint and
rebuilds the tree by Contract evaluation. When its canonical document matches
the page's, it keeps the DOM instead of rebuilding it.

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
   restarts timers from the carried clock, and replaces the store.

   A page needs a **document checkpoint** instead (D4), and a runner input of its
   own for the checkpoint's answers. Kept answers (`runner/src/runner/kept.rs`)
   can't carry them: they seed only resources that read the store, cap an answer
   at 8 KiB, and are asked again at `data_ready`.
4. **Known reads.** The dependency table (LLP 1005 §8, 2026-09-22) records what
   every binding and site reads; the VM propagates store dependence. r3 rests no
   privacy guarantee on it: an anonymous render has no credentials to leak, and a
   personal render is never shared (D3). Unattributed store reads during `parse`
   (`runner/src/runner/commit.rs` ~595) remain a correctness bug for re-asks.
5. **Declared data.** Resources enter through named sources that answer now or
   later; the host does all I/O (LLP 1016). The same sources can run at build, on
   a server (Hermes for TypeScript), or in the browser.
6. **A fixed runtime cost.** Interactivity costs one wasm and a boot proportional
   to the tree, not per-component JavaScript. There is nothing to split into
   islands — only "load the runtime" or "don't".

Server components' main benefit — no client code for server-only logic — does
**not** follow from stages alone. Today the build ships the whole paired module
(`host/web/build.mjs` ~98–104). Server-only source artifacts wait for a consumer
(LLP 1048.002 §2).

## 3. The synthesis

| We build | Covers (§1) | What it collapses |
|---|---|---|
| **A render policy per route** — `client`, `build`, `cached`, `request` — for one renderer, deployed to a static `dist/` or the native server | 1, 2, 3, 4; SPA shells; static export | SSG, ISR and SSR as separate systems |
| **HTTP caching from the start** — anonymous documents carry `s-maxage`, `stale-while-revalidate` and surrogate keys. Any CDN or shared cache serves them, and a purge by key refreshes them | 3 | a framework cache store; `revalidateTag` |
| **Stages and placeholders** — each source declares when it can answer: `build`, `request` or `client`. On the server, a later source answers with its placeholder (the author's own pending state), and the device asks it after adoption | 6 (holes filled on the device); SPA shells | Suspense boundaries; server islands |
| **Personal pages** (phase 2) — a request carrying the reader's session renders the whole page for that reader, private and never shared-cached | 4, for signed-in readers | — |
| **Adoption with an activation policy** — boot from the checkpoint and keep the DOM when the canonical documents match. The runtime loads when idle by default, on interaction or visibility when chosen, and never when nothing needs it | full and incremental hydration; resumability; islands; zero-JS pages | hydration's rebuild of the DOM, and its mismatch class |
| **Navigation with route data** (phase 2) — client navigation reuses the destination's answers when their identity matches; every route is also a real document | client routing with prefetch; multi-page navigation; Speculation Rules | — |
| **Documents in Contract** — the head, document scrolling and route fields; meaning from roles and Markdown, lowered to HTML | table stakes for every row | — |

Server components (10) partly collapse: data answered on the server travels as
data, but code separation waits for server-only artifacts.

**Deferred until a measurement or a consumer asks** (no implementer):
- streaming (5);
- personal holes inside cached pages (6 with credentials);
- the edge (7);
- server-only artifacts (10's code separation).

**Refused** (NOT-DOING): server actions that work without the runtime (9), and
server-driven UI (8).

**Never:** bot-only rendering. Every page is real HTML.

What an author chooses stays small: a render policy per route, sometimes a cache
lifetime or an activation policy, and each source's stage.

## 4. Decisions

**D1 — The document is the page.** A route's HTML is its **document**: the DOM
projection of a settled checkpoint (D4), before browser layout, produced by the
live host's own projection. That projection includes:
- the URL policy (`871e2631`);
- the property-to-HTML rules: checked, value, booleans, `textarea` content,
  canvas wrappers, markup expansion;
- content models, where the parser would otherwise restructure output.

Meaning comes from roles and markup, lowered to HTML:
- a text with a heading level is `h1`–`h6` (`44f8643c`);
- Markdown's blocks become paragraphs, headings, lists, quotes and code blocks.

Each keeps the box a bare `div` would have, so layout doesn't change on any host
(LLP 1048.003 D2). Entries covered in a stacked presentation are marked in the
document exactly as the live host marks them.

Browser-owned effects are named and excluded: font loading, symbol sizing from
computed styles, focus, scrolling, context positioning, and windows chosen from
scrollport geometry.

A public document renders a virtualized collection's items in full, within a
bound, as static HTML; a canvas renders its fallback content. Parity is claimed
for the document only, and checked by parsing the served HTML (LLP 1048.000 §4).

**D2 — A render policy per route:**
- `client`: the shell only, as today;
- `build`: every enumerated location, rendered at build;
- `cached`: rendered per request, and kept by shared caches for its lifetime and
  refreshed by key (D11);
- `request`: rendered per request, and not kept.

A route's policy is declared (LLP 1048.003 D5). An undeclared route is `client`.
The compiler reports the policy a route's inputs would allow, as a note, and
never applies it: a guess could put a route on a server the deployment doesn't
have.

**D3 — Stage is the one label, and anonymous and personal renders never mix.**
- **Stage.** Each source declares when it can answer: `build`, `request` or
  `client`. An undeclared source is `client`. The runner's own inputs have fixed
  stages:
  - the store: `client`;
  - the viewport: `client`;
  - the clock: `request`;
  - the router location: `build` for enumerated routes, `request` otherwise.
- **Later sources on the server.** A source whose stage is later than the
  render's answers with its placeholder, with `pending(x)` true. The client asks
  it after adoption.
- **Until sources declare a stage** (nothing does yet), a render runs every
  source in its own environment. That environment has no store, no cookies and
  no device capabilities: the render host refuses SQLite and kept secrets. A
  source refused there keeps its placeholder, and the client asks it after
  adoption. So a render answers what a new anonymous device would see. Nothing
  private can reach it, because the environment holds nothing private. Declared
  stages come when a consumer needs a source kept off the server (1b, 2026-09-23:
  under "undeclared is `client`", 296 of the repo's 304 resources would never
  render).
- **Anonymous renders.** No cookies, no store, and no request context except the
  URL. Declared public request facts, such as a locale, can join later
  (LLP 1048.002 §2); each one is then part of the cache key.
- **Personal renders** (phase 2, LLP 1048.002). The render gets the reader's
  session from a cookie the app's grants name, renders the whole page for that
  reader, and answers `Cache-Control: private, no-store`. No server-side cache
  holds it.

The rule is structural: a render that received a session is personal. Nothing
depends on tracking which reads were personal. No page mixes a shared-cached part
with a personal part.

**D4 — Adopt, don't hydrate.** A rendered page embeds its **document
checkpoint** (LLP 1048.000 D6). The checkpoint holds:
- the answers, each with its source, arguments, logic identity and freshness;
- the pending flags;
- the router location;
- the time;
- never a store.

The runner takes the checkpoint's answers as an input of their own, neither the
store nor kept answers. It treats them as answers it already has, so it doesn't
ask them again at boot or when the module loads. It asks again only when their
arguments or inputs change, as for any answer. Failure stays data (LLP 1016
D4): a later refresh that fails reaches the source's `parse`, and the app
decides what the page shows.

The runtime then compares its canonical document with the page's; the digest
excludes runtime incarnation ids.
- **Match:** the runtime binds the existing DOM.
- **Mismatch:** it keeps the page on screen until its own tree has settled, then
  renders fresh, once.

That is the one boot path, and the reader never sees less than the server sent.
The device's store and clock then apply as ordinary updates. A checkpoint never
overrides the store.

**D5 — Activation.**
- **`never`** when a page needs no runtime: no handlers, no timers, no pending
  sources, and no host capability that needs runtime work (canvas surfaces,
  virtualized collections).
- **Otherwise,** the runtime loads when the page is idle or at the first
  interaction, whichever comes first.
  - Nothing preloads it, so it doesn't compete with the page's own images and
    fonts.
  - A press before adoption replays once, after it.
  - The document never disables controls while activation is pending.
- **Phase 2** adds `visible` and `interaction`, with full semantic replay, after
  LLP 1047's byte budget (LLP 1048.001).

**D6 — Placeholders, never refusals.** A resource whose source cannot answer at
the current stage renders its placeholder with `pending(x)` true. Boot no longer
refuses an unanswerable resource (`runner/src/runner/settlement.rs` ~380 today;
the website trial's deep-link crash). The compiler requires a placeholder where
a shape has no empty value (LLP 1048.003 D6).

**D7 — Route data for navigation (phase 2).** Rendered routes publish their
answers. Client navigation reuses an answer only when its source, arguments,
logic identity, freshness and principal all match; otherwise it asks the source.
The data has its own URL (LLP 1048.001 D7). It is never negotiated on the HTML
URL, so shared caches don't split on `Accept`.

**D8 — The head is part of the view.** A `head` element in the active subtree
sets the title, description, canonical URL, image, robots, structured data
(JSON-LD) and, for a not-found view, the status; the innermost wins. The renderer writes it into `<head>`, escaping
structured data like the checkpoint. The web host keeps `document.title` and the
meta tags in sync. Native hosts map `title` to the window or scene title
(LLP 1048.003 D1).

**D9 — One renderer, a native server.** Rendering is a Rust library over the
runner and the host's projection, with an **async render host**:
- Hermes for TypeScript sources;
- the native executor core for requests;
- continuation pumping;
- a completion rule that ignores ordinary timers and mutations;
- deadlines and cancellation.

Two callers use it:
- the build, which produces a static `dist/`;
- the server, a serve mode of the Linux host binary. That binary already has the
  executor core, grants, Hermes loading and delivery (LLP 1048.000 D10).

Hosting is local for now (§9.6). Node isn't on the render path. An app's own data
service is the app's business: Interview's local backend is Node today.

**D10 — Portable static output.** `dist/` holds one `index.html` per rendered
route, `404.html`, `sitemap.xml`, `robots.txt`, content-hashed assets and
precompressed files. It deploys to any static host.

**D11 — HTTP caching from the start.**
- **Anonymous documents** answer `Cache-Control: public, max-age=0,
  s-maxage=<lifetime>, stale-while-revalidate=<window>`. They carry an `ETag`
  and surrogate keys naming the answers they used. When there is a CDN, it
  serves them, and a purge by key refreshes them.
- **The clock never keys a cache.** The checkpoint carries its render time, and
  relative times update after adoption.
- **A render that doesn't settle** answers 503 with `Retry-After`, carrying the
  same document with its placeholders:
  - a browser shows it, and the runtime finishes it;
  - a crawler comes back later;
  - no cache keeps it.
- **Personal documents** are `private, no-store` (D3).

**D12 — The rules still hold.**
- No app JavaScript runs before first pixel: the first pixels are HTML.
- The boot and adoption glue is host code, counted by `boot.mjs` and budgeted in
  bytes (LLP 1047 D9).
- The server's startup is counted the same way. Per render, it counts the module
  graph it loads, the source calls it makes, and the time to first byte.

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

The take was Messages as the Snapback4 consumer: Interview, built on Snapback4,
is the consumer now. Still refused: progressive forms and server-driven UI. No
Node on the render path.

r3 changes nothing there. Streaming stays on the doing-list, deferred until a
measurement asks for it.

## 6. Prerequisites

- **Landed on main** (web lane, 2026-09-23):
  - one URL allowlist for every navigating attribute (`871e2631`);
  - `h1`–`h6` from heading levels, and `<html lang>` (`44f8643c`);
  - same-origin links followed in place (`1e7016a0`);
  - font fallbacks (`e0a7a23d`).
- **Pending in the web lane:** compression and caching in `serve.mjs`.
- **LLP 1048.003:** the head, document scrolling and route fields (phase 1a);
  placeholders (phase 1b).
- **LLP 1047:** the runtime's size. `interaction` activation waits on it.

## 7. Phases

| Phase | Work | What users get |
|---|---|---|
| 1a | 1048.000 steps 1–4 + 1048.003 part A | Every in-repo app's routes as real HTML from `dist/`, on any static host: titles, real links, a correct 404 and a sitemap. The runtime loads when the page is idle and takes over with the same content |
| 1b | 1048.000 steps 5–10 + Interview's §10 work | Interview's public question, post and profile pages, public at each author's choice, as real HTML from a local server with cache-ready headers. The runtime adopts the page, and signed-in readers keep the public content until their own view is ready |
| 2 | 1048.001 (activation policies, semantic replay, navigation data) + 1048.002 (personal pages) | Pages rendered for signed-in web readers; instant navigation; a lazier runtime |
| Later | Hosting on a CDN or provider (§9.6); streaming, personal holes in cached pages and the edge, if a measurement or consumer asks; server-only artifacts | — |

**1a** needs no server and no Hermes. Its documents render with build-stage data.
Sources that can't answer at build render their placeholders, and the runtime
answers them after it takes over.

**1b** adds:
- the async render host;
- checkpoint answers and adoption;
- the server.

`client` stays every route's default until a route declares otherwise.

## 8. Verification

- **First acceptance case (1b).** An anonymous Interview question URL, from the
  local server, with deliberately delayed data. It must deliver:
  - useful HTML and metadata with JavaScript off;
  - real links;
  - status codes: 404 for a missing or non-public item, 503 with `Retry-After`
    for a render that didn't settle, 500 for a failed one;
  - cache headers that let a shared cache keep the page;
  - no content lost when the runtime starts.

  Then, in order:
  - a signed-in reader whose replica is cold keeps the public content until
    their own view is ready;
  - desktop layout;
  - navigation.
- **Parity.** The served HTML, parsed, equals the live host's document for every
  rendered route of every app, normalized. A difference names the element and
  attribute.
- **Counterexamples**, each a test:
  - delayed replies;
  - a network failure at boot, which changes nothing the server sent;
  - a store read after an await: an anonymous render shows the logged-out state;
  - user text containing `</script` and `<!--`, in the checkpoint and in
    structured data;
  - differing viewports;
  - repeated server requests in one process;
  - malformed URLs;
  - empty regions;
  - edits before activation.
- **Measurements**, at 150 ms RTT and 6 Mbit/s. Add the latency with a loopback
  delay proxy: in Chrome 153, CDP's latency emulation doesn't apply to requests
  and only throughput is throttled (the Interview lane, 2026-09-23).
  - server work and time to first byte, with the module graph and source calls
    per render;
  - meaningful content paint;
  - layout shift;
  - payload bytes;
  - activation cost.
- **Privacy:**
  - an anonymous render never receives a cookie or a store;
  - a private author's content answers 404 to a logged-out reader;
  - in phase 2, a personal response is `private, no-store` and never reaches a
    shared cache.

## 9. Rulings (Charlie, 2026-09-23)

1. **The NOT-DOING take (§5): Messages as the Snapback4 consumer.** It comes off
   the doing-list ("We don't need that on the list"), and Interview is the
   Snapback4 consumer. The game engine stays (Charlie is working on it).
2. **The Contract surface in LLP 1048.003: approved.** That was the `head`
   element, semantic elements, media variants, route policy fields and
   placeholders.
   - r3 keeps the head, document scrolling, route fields and placeholders.
   - It drops semantic tags with HTML defaults and media variants until a
     consumer needs them.
   - Why, per the three reviews: they are kernel work on four hosts, and
     Interview's anonymous readers never see the layout they were for.
3. **The website consumer: Interview** (`~/projects/interview`, §10).
4. **The server target: a native Rust binary.**
5. **Public content: "questions, answers, posts, profiles — but all at the
   preference of the user."**
   - Each person chooses, and the choice is off by default.
   - So nothing written while Interview was invitation-only becomes public
     unless its author turns it on.
   - The rules are Interview's (§10.2).
6. **Hosting: "for now just hosted locally, eventually on a CDN/hosting
   provider."**
   - Phase 1's server binds loopback.
   - Its headers are already what a CDN needs (D11).
   - Interview's NOT-DOING keeps production hosting out.
7. **The server's shape.** Charlie: "hmmm I'm not sure", then "make the best
   decisions you can".
   - **The author's decision:** render on request, from a serve mode of the
     Linux host binary. It reads Interview's public content from Interview's
     backend over loopback, and sends cache-ready headers.
   - **Two alternatives wait for real hosting:**
     - a read-only replica of Interview's public content on the server (Fable);
     - writing static files when content changes (Grok).
   - **Why not static files:** a new question's URL has to answer at once. A
     CDN's purge by key also keeps the index from records to pages, which
     rewriting on change would have to keep itself.
8. **Pages for signed-in readers.** Charlie: "hmmm why not server render the
   personal stuff?"
   - **The author's decision:** they are rendered on the server, in the simple
     form (D3, LLP 1048.002). A request carrying the reader's session gets the
     whole page rendered for that reader, `private, no-store`, never stored by a
     shared cache.
   - **What the three reviews objected to** was r2's form: personal parts mixed
     into cached pages, guarded by confidentiality labels and runtime read
     tracking, which is a proof obligation that never ends. That form stays
     out.
   - **Personal pages come in phase 2.** They need a web session cookie in
     Interview. Meanwhile, signed-in readers with a synced replica already see
     their own content from the device right after the runtime starts.
9. **Links and previews** (Charlie, 2026-09-23: "yes, yes").
   - Each answer's author links to their profile (Interview).
   - A preview image per page comes later: a card drawn by the headless Linux
     host from a Contract view.
   - The hosting provider and the `robots.txt` policy for AI crawlers are
     decided when hosting is (Charlie: "yes"). Until then, `robots.txt` is the
     neutral default.

## 10. The consumer: Interview

Interview (`~/projects/interview`) is a successor to Quora built on exact2 and
Snapback4: questions, AI and human answers, profiles, votes, comments, search, a
feed and Markdown posts, on iOS, macOS and the web. Its question pages are what
people share and what search engines should index — the case this LLP exists
for. Its routes (`app.contract`'s `routes nav`) map onto render policies:

| Route | Content | Phase 1b | Phase 2 |
|---|---|---|---|
| `/prompt/:question`, `/post/:post`, `/people/:person` | Public at the author's choice (§9.5) | `cached`, from the local server | a personal page for signed-in readers |
| `/`, `/prompts` | Feeds | `cached`, the anonymous view of public content | a personal page |
| `/search?q=` | Query results | `request`, `noindex` | — |
| `/prompts/new`, `/prompt/:question/write`, `/messages`, `/notifications`, `/profile`, `/settings` | Private | `client` | a personal page |

Interview's own work, in its repository — phase 1b:

1. **Public view gating.** Today content renders only for
   `data.ready and data.authenticated` (`app.contract` ~655, ~703), so logged-out
   readers get the welcome flow. Public routes must render public content to
   anonymous readers.
2. **A public content source**, split from the viewer. It is answered by
   bounded, anonymous backend projections, and returns the rows the replica's
   own queries return: one set of shapes, not a second public interface.
   - Today `app.ts` sends `replicaOnly: true` to `/state`, and the anonymous
     branch of that response returns no content (`app.ts` ~86;
     `scripts/backend.mjs` ~163, ~208).
   - The older full response's field names (`selected`, `answers`, `profile`)
     don't match what the adapter reads (`details`, `profiles`).
   - The viewer, drafts and accounts stay `client`.
   - Setting `authenticated=true` to fake public access is ruled out.
   - **What is public** (§9.5):
     - a question is public when its author is;
     - an answer is public when both its author and its question's author are;
     - a post is public when its author is;
     - a profile is public when its person is, and lists only that person's
       public content.
   - The feed and search show only public content.
3. **Initial data without the `started` timer.** `state started = false` holds
   the source empty until a 250 ms timer (`app.contract` ~175, ~241). Public
   routes read their data immediately.
4. **Real links.** Posts, questions and authors navigate through buttons with
   actions (`app.contract` ~1007, ~1016, ~1031). They become `link href=` with
   the same actions, so they are crawlable and open in new tabs.
5. **Search in the URL.** `/search?q=` drives the query instead of local state
   (`app.contract` ~187, ~206). Its pages are `noindex`.
6. **A document layout.** The screen scrolls in an inner scroller inside a
   height-constrained container (`app.contract` ~637). Public routes use
   explicit document scrolling (LLP 1048.003 D4).
7. **A `head` per route:**
   - the question's text as the title;
   - the leading answer's excerpt as the description;
   - the author and an image;
   - structured data describing a `QAPage`.
8. **Missing and non-public items.** Both render the not-found route with a 404
   status, identically.
9. **Never less than the server.** A signed-in reader's page keeps the public
   content until their own view is ready. It doesn't fall back to "Opening saved
   conversations…" (`app.ts` ~132) in place of content the server sent.
10. **Failure stays data** (LLP 1016 D4). Interview's sources already shape a
    failed fetch as an answer (`app.ts` ~186). Since the boot no longer asks
    checkpoint answers again, a failure can't replace what the server sent. A
    throw is for a source that is wrong, and it leaves the request pending.
11. **One switch.** Public reading sits behind one backend setting, off by
    default. With it off, nothing is public, whatever authors choose.

r2's item 9, the rail as width variants, is gone. The rail renders only
`when data.authenticated and wide` (`app.contract` ~413), so anonymous readers
never see it.

**Phase 2**, for personal pages: a web session cookie set at sign-in
(`HttpOnly`, `Secure`, `SameSite=Lax`). The viewer source reads it through the
request context on the server, and reads the store on the device.
