# LLP 1048: Rendering across the curve — one renderer, from static pages to per-request HTML

**Type:** RFC
**Status:** Draft — direction approved by Charlie 2026-09-23 ("ok let's do pre-rendering etc. … we might as well have a great solution for almost every point on that curve"); the NOT-DOING take (§5) and the Contract surface (LLP 1048.003) await his ruling
**Systems:** Web host (`host/web`: a page serializer beside `Host::create`, adoption in `glue.js`, the page shell); build (`contract::bake`, `host/web/build.mjs`); Runner (carried state as a page payload, provenance, placeholders); Contract (head, semantic elements, media variants, route policies — LLP 1048.003); serving (`host/web/serve.mjs`, a request renderer — LLP 1048.002); Router (route enumeration, navigation payloads); Delivery (web releases carry rendered pages)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Implementer:** assigned by Charlie per phase (§7)
**Date:** 2026-09-23
**Related:** LLP 1007 (web host), 1023 (one URL per app), 1038 (router), 1039 (viewport facts), 1016 (async settlement), 1018 (the store), 1027 (data sources), 1030 (delivery), 1047 (pay for what you use), 1005 §8 (the dependency table); `rules/NOT-DOING.md` §Features ("Server generation in every form"); sub-LLPs 1048.000 (pages at build), 1048.001 (adopt, don't hydrate), 1048.002 (pages per request), 1048.003 (documents in Contract)

## Summary

Websites are a goal (Charlie, 2026-09-23). Today an exact2 page is an empty
`#exact-root` until about 390 KB (brotli) of wasm arrives and boots. Crawlers,
link unfurlers and readers without JavaScript see nothing, and phones wait: the
2026-09-22 website trial painted first at 2.26 s on a 6 Mbit/s, 150 ms profile.

The web's answer has split into a dozen named strategies — SSG, SSR, ISR,
streaming, partial prerendering, islands, resumability, server components.
Most are local optima around one cost: a component framework cannot prove that
the server's HTML equals what the client will render, so it re-runs components
on the client to attach behavior (hydration), and it cannot see which parts of
a page depend on what without the author marking boundaries.

exact2 does not have that cost. The runner is deterministic, the web host's DOM
is one Rust function of a batch, runner state already serializes, every binding
knows its inputs, and all data enters through declared sources. So most of the
curve collapses into **one renderer with three settings**:

- **when** a page renders — a render policy per route: `client`, `build`,
  `cached` or `request`;
- **how** it arrives — complete, or streamed as its slower sources answer;
- **when** it becomes interactive — an activation policy: `eager`, `idle`,
  `visible`, `interaction` or `never`.

Partial prerendering is inferred from where each input comes from (its
provenance), not authored. Hydration is replaced by **adoption**: the client
resumes from the page's carried state and binds to the existing DOM after a
digest proves the two agree.

## 1. The curve today

Roughly ten rendering strategies that people choose for real reasons, plus two
axes that cut across all of them:

| # | Strategy | What it is | Why people choose it | Examples (2026) |
|---|---|---|---|---|
| 1 | Client rendering (SPA) | An empty shell; JavaScript renders everything | Apps behind a login, no SEO need, static hosting, simple mental model | Vite SPA; React Router SPA mode |
| 2 | Static generation (SSG) | Every page rendered at build | Content known at build; CDN speed and cost; SEO | Astro, Eleventy, Hugo; Next `output: export` |
| 3 | Incremental / on-demand regeneration (ISR, DPR, `use cache`) | Render on first request, cache, revalidate by TTL or tag | Too many pages to build; content that changes occasionally | Next revalidate/`revalidateTag`; Nuxt route rules |
| 4 | Server rendering per request (SSR) | Render on every request | Personalized or fresh data; auth; SEO for dynamic pages | Next dynamic routes; React Router framework mode; SvelteKit; Rails |
| 5 | Streaming SSR | Flush the shell, stream slow parts out of order | First paint not blocked by the slowest query | React Suspense streaming; SolidStart; Marko |
| 6 | Partial prerendering / server islands | A static shell from the CDN, dynamic holes per request | CDN speed and personalization in one page | Next 16 Cache Components; React 19.2 `prerender`/`resume`; Astro 6 `server:defer` |
| 7 | Edge rendering | 3–6, run near the user | Latency; geography | Cloudflare Workers; Deno Deploy |
| 8 | Server-driven UI (HTML over the wire) | The server renders fragments on each interaction | Server-owned logic, minimal client code, realtime | htmx; Hotwire; Phoenix LiveView; Blazor Server |
| 9 | Progressive enhancement / server actions | Forms work without JavaScript; mutations run on the server | Resilience; accessibility | React Router forms; SvelteKit actions; Next server actions |
| 10 | Server components (RSC) | Server-only components ship no client code; data is fetched on the server | Smaller bundles; data colocated with UI; secrets stay server-side | Next App Router |

**Becoming interactive:** full hydration (React); progressive or incremental
hydration with event replay (Angular 21, Nuxt); partial hydration or islands
(Astro, Fresh); resumability (Qwik 2); none (pure static).

**Navigating:** client routing with data payloads and prefetch (Next, React
Router, TanStack); or multi-page navigation made instant with Speculation Rules,
the back/forward cache and cross-document view transitions (Chromium; Safari
18.2 has the transitions).

Every serious framework lets one site mix these per route (Astro's per-page
`prerender`; TanStack Start's `ssr: true | 'data-only' | false`).

## 2. What is different about exact2

1. **A deterministic first frame.** The runner is a function of the plan, the
   launch location, the sources' answers, the clock, the store and the
   viewport; view ids come from a counter (`Ids::fresh`,
   `runner/src/instance.rs`). Equal inputs give byte-equal batches.
2. **One DOM mapping.** The web host turns a batch into DOM through
   `Host::create`, `tag_for`, `props_for` and `css.rs` (`host/web/src`) —
   Rust that also compiles natively. A page rendered by the same code agrees
   with the live DOM by construction, and a digest can prove it.
3. **Serializable state.** `Runner::carry()` produces `Carried` (slots,
   resource answers with their arguments, router, clock, store —
   `runner/src/runner/carry.rs`), and `Runner::boot_carrying` restarts a runner
   in that state; the dev loop uses both (LLP 1007 §6). That is a resume
   payload.
4. **Known inputs.** The dependency table (LLP 1005 §8, 2026-09-22) records
   what every binding and site reads — slots, derives, resources, pending
   flags, the clock — and settlement tracks store dependence. What a subtree
   depends on is data, not a convention.
5. **Declared data.** Resources enter through named sources that answer now or
   later, and the host does all I/O (LLP 1016). The same sources run at build,
   on a server, or in the browser.
6. **A fixed runtime cost.** Interactivity costs one wasm and a boot
   proportional to the tree, not per-component JavaScript. There is nothing to
   split into islands — only "load the runtime" or "don't".

So hydration mismatches cannot pass silently, partial prerendering needs no
authored boundaries, and the main benefits of server components — no client
code for server data, secrets that stay on the server — come from sources: an
answer computed on the server travels in the payload as data, and its source's
code never ships.

## 3. The synthesis

| We build | Covers (§1) | What it collapses |
|---|---|---|
| **A render policy per route** — `client`, `build`, `cached`, `request` — for one renderer, deployed to a static `dist/`, a server, or the edge | 1, 2, 3, 4, 7; SPA shells; static export | SSG, ISR and SSR as separate systems |
| **Provenance-driven partial prerendering** — each input is available at `build`, `request` or `client`; a subtree renders at the earliest stage its inputs allow and leaves a hole for a later stage to fill | 6; "SPA mode" shells | Suspense boundaries; `use cache` annotations; server islands |
| **Streaming** — a request page flushes what is ready and streams holes as their sources answer | 5 | — |
| **Adoption with an activation policy** — resume from the page's carried state, bind the existing DOM after a digest check; `interaction` replays the triggering event; `never` ships no runtime | full, progressive and incremental hydration; resumability; islands; zero-JS pages | hydration and its mismatch class |
| **Navigation with route payloads** — client navigation fetches the destination's carried answers, prefetched on hover or visibility; every route is also a real document | client routing with prefetch; multi-page navigation; Speculation Rules | — |
| **Documents in Contract** — head, semantic elements, media variants, document scrolling | table stakes for every row | — |

Server components (10) collapse into provenance: a source answered at build or
per request is data in the payload, never client code.

**Deferred, not specified** (no implementer; each returns with a consumer):
server actions that work without the runtime (9) and server-driven UI (8) —
the runner on a server with batches over the wire makes 8 cheap later; offline
service workers; bot-only rendering (never: every page is real HTML).

What an author chooses is small: a render policy per route, rarely an
activation policy, and where each source can run. Everything else — which parts
of a page are static, which are holes, whether a page ships a runtime — is
inferred.

## 4. Decisions

**D1 — The batch is the page.** A route's HTML is the serialization of the
first batch the web host emits for it, through the same Rust mapping
(`tag_for`, `props_for`, `css.rs`). DOM effects `glue.js` applies outside ops
at first paint — router projection (`navigation.project`), symbol images
(`refreshSymbols`), `prepareContexts`, list sync (`glue.js` `apply`, ~579–830)
— move into the batch or into Rust. An async parity check compares rendered
HTML with the live DOM for every route of every app (§8).

**D2 — A render policy per route.** `client` (today: the shell only), `build`
(rendered at build for every enumerated location), `cached` (rendered on first
request, cached, revalidated), `request` (rendered per request, private). The
default is inferred: `build` when a route's content is available at build, with
client holes for the rest; `cached`/`request` only where a server target
exists. Authors override per route (LLP 1048.003 D5).

**D3 — Provenance on sources, inferred everywhere else.** Each source declares
where it can answer: `build`, `request` or `client`. The runner's own inputs
have fixed provenance: the store is `client`; the viewport is `client`, or
`request` when client hints supply it; the clock is `request`; the router
location is `build` for enumerated routes and `request` otherwise. A binding's
provenance is the latest of its inputs (the dependency table). Request and
client values never enter a `build` or `cached` page — the renderer refuses by
name (LLP 1048.002 D6).

**D4 — Adopt, don't hydrate.** A rendered page embeds its carried state and a
digest of the plan, launch location, payload and first batch. The client boots
with that state, compares digests and binds the existing elements by
`data-view` id. On a mismatch it builds fresh DOM off-document and swaps once,
logging the first differing op. No code re-runs "to match".

**D5 — Activation is a policy, inferred by default.** `never` when a page has
no handlers, timers, pending sources or client holes (no runtime is shipped);
otherwise `eager`. Authors may choose `idle`, `visible` or `interaction`;
events captured before adoption are replayed after it.

**D6 — Placeholders, never refusals.** A resource whose source cannot answer at
the current stage renders its placeholder with `pending(x)` true. Boot no
longer refuses an unanswerable resource (the website trial's deep-link crash,
2026-09-22). The compiler requires a placeholder where a shape has no empty
value (LLP 1048.003 D6).

**D7 — Route payloads.** Each rendered route also emits its carried answers as
a sibling file. Client navigation to it settles build-provenance resources from
that file, prefetched on hover or visibility; request-provenance resources ask
the request renderer's data endpoint.

**D8 — The head is part of the view.** A `head` element in the active route's
subtree sets title, description, canonical URL, image and robots; the innermost
active one wins. The renderer writes it into `<head>`; the web host keeps
`document.title` and the meta tags in sync; native hosts map `title` to the
window or scene title (LLP 1048.003 D1).

**D9 — One renderer, several targets.** Rendering is a Rust function over the
runner and the web host's mapping. Targets: the build (a static `dist/`), a
native server binary (`cached` and `request`), and an edge worker (the same
code in wasm). Node is not on the render path.

**D10 — Portable static output.** `dist/` holds one `index.html` per rendered
route, `404.html`, `sitemap.xml`, `robots.txt`, content-hashed assets and
precompressed files. It deploys to any static host, and `serve.mjs` serves it
with correct caching.

**D11 — The rules still hold.** No app JavaScript runs before first pixel — the
first pixels are HTML. The adoption and replay glue is host code, counted by
`boot.mjs` and budgeted in bytes (LLP 1047 D9).

## 5. NOT-DOING (proposed; Charlie rules the take)

Today `rules/NOT-DOING.md` refuses "Server generation in every form: SSR,
streaming, static export, progressive forms, hydration, route payloads,
response caching." Proposed entry:

> **Expanded (Charlie, 2026-09-23: "ok let's do pre-rendering etc."):** web
> rendering across the curve (LLP 1048) — build-time pages, adoption instead of
> hydration, route payloads, request-time rendering with cache policies,
> inferred partial prerendering and streaming. Unblocks websites that crawlers,
> link previews and readers without JavaScript can read, and that paint before
> the runtime loads. Take: *(Charlie's choice; proposed — the game-engine
> add-on, LLP 1046, waits behind websites)*. Still refused: progressive forms
> (server actions without the runtime) and server-driven UI; no Node on the
> render path.

## 6. Prerequisites

- **In flight** (the web lane, 2026-09-23): `h1`–`h6`, `lang`, same-origin link
  interception, brotli/caching/preload in `serve.mjs`, the `javascript:` href
  allowlist, font fallbacks.
- **LLP 1048.003:** `head`, semantic elements, media variants, document
  scrolling, route policy fields, required placeholders.
- **LLP 1047:** the runtime's size, which becomes the cost of interactivity once
  the HTML paints first.

## 7. Phases

| Phase | Sub-LLPs | What users get |
|---|---|---|
| 1 | 1048.003 part A (head, document scrolling, semantic elements) + **1048.000** (build renderer, enumeration, `dist/`, parity; the runtime replaces the page on boot) | Crawlable pages, link previews, per-route titles, first paint at HTML arrival |
| 2 | **1048.001** (adoption, activation, payloads, navigation) + 1048.003 part B (media variants, placeholders) | No swap, zero-JS pages, a lazy runtime, instant navigation |
| 3 | **1048.002** (provenance holes, the request renderer, cache policies, streaming, edge) | Personalized and fresh pages at CDN speed |

`client` stays the default until phase 1 ships; then `build` becomes the default
for routes whose content is available at build.

## 8. Verification

- **Parity** (async lane): for every route of every app, the rendered
  `#exact-root` equals the live DOM after the first batch at the same URL in
  headless Chrome (normalized; a difference names the element and attribute).
- **Adoption:** digest equality; the agent's `tree` identical before and after
  adoption; replayed events produce the same state as live ones.
- **Web vitals:** first contentful and largest contentful paint at 150 ms RTT and
  6 Mbit/s, before → after, for Caltrain, the Markdown reader and the trial
  site. Target: first paint at HTML arrival.
- **Accessibility and SEO:** the CDP accessibility tree (headings, landmarks,
  links), per-route title and meta, a valid sitemap.
- **Privacy:** no request- or client-provenance value in any `build` or
  `cached` page — a renderer refusal, with a test.

## 9. Questions for Charlie

1. The NOT-DOING take (§5).
2. The Contract surface in LLP 1048.003 (the `head` element, semantic elements,
   media variants, route policy fields, placeholders).
3. The v1 bar: add a website consumer? Proposed: exact2's own site — landing
   page, docs from the LLP reader, a blog — built in Contract and served as
   rendered pages.
4. The server target: a native Rust binary (proposed) or Bun with the wasm.
