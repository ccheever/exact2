# Concurrent HTTP reads are opt-in, and the guides never say how

**Status:** Closed
**Resolution:** docs/reference.md (What a data module can use) now says how a read leaves the ordered queue (exactIndependentHttp / independent_http), what that promises, the six-at-once lane and its budget, the ceiling and its refusals; a recipe line was over the recipe cap, so the reference holds it alone.
**Systems:** docs (reference.md, contract-for-agents.md, recipes), runner `Request::independent_http`, TypeScript `RequestInit.exactIndependentHttp`
**Severity:** P3
**Author:** Claude (Opus 5.5), from LLP 1054 D6 (the Bluesky port)
**Date:** 2026-10-19
**Related:** LLP 1054 D6; LLP 1054.000 C2 (ordered by default kept: "Recommend no", both reviews); LLP 1041 §8.4; LLP 1120

## What happens

A data source's HTTP requests run one at a time, in order, unless each request
opts out. In Rust that is `.independent_http(max_response_bytes)`
(`runner/src/request.rs`). In TypeScript it is
`fetch(url, { exactIndependentHttp: { maxResponseBytes } })`, which is in the
generated `app.contract.d.ts`. For a feed client almost every read wants it.
The Bluesky port found the Rust form only by reading `runner/src/request.rs`.

LLP 1054.000 C2 kept ordered as the default: a `GET` does not promise that its
reply can be parsed out of order. That ruling stands. What is still missing is
any mention in the guides:

- `docs/reference.md` explains how ordered reads queue on native hosts (around
  line 814) but never says that reads can opt out.
- `exactIndependentHttp` appears once in `docs/`, as the size limit in
  `docs/contract-grammar.md`'s failure table. `independent_http` appears
  nowhere in `docs/`.
- No recipe loads several things at once (`docs/recipes/fetch-loading-error.md`
  and `rest-backend.md` are the natural places).

## What would fix it

A paragraph in `docs/reference.md` beside the ordered-queue text: what
independence promises (no FIFO need, for the effect or for parsing), the
`maxResponseBytes` ceiling and what exceeding it does (`refused`), and both
spellings. A line in the fetch recipe pointing to it.
