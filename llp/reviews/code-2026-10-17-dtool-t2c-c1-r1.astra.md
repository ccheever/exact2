# Code review: LLP 1122 T2c step 1 (source maps by plan digest), round 1, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `a27b46995`.
- **Method:** one brief (sha256 `12e549c8c8b6cc5b5013ae70cd763b58f953ae0a33926a6b62e932c065b7c004`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 4 medium, 1 low).
- **Disposition:** taken.
  - The resident loop says each map's card (plan, SHA-256, length, generation path) on its stdout, the pipe the JS loop spawned it with. The JS loop fetches those bytes from the child's port, naming the public host the child's gate answers, so it is not refused 421. Only a generation path is fetched, with no redirect, and the bytes are kept only when they match the card. A process that took the child's port cannot make bytes match a card it did not write. The running map is the plan the child said last, not the pull that ended last. A not-yet-published map is asked once more.
  - The server no longer caches maps by digest. It asks the dev loop each time (a map can change under one plan digest) and parses each byte buffer once.
  - A selection that names no plan joins with no map, and is read-only.
  - `outline` resolves each build's own map. A missing or stale running map refuses only when no build could be outlined.
  - The page's digest is asked once per generation, unknown included. A status saying `planDigest: null` makes it unknown.
  - The docs' selection and `outline` sections say each build is sent its own plan's outline.
  - Tests:
    - the server test: a missing running map with builds on their own, a map moved under the same digest, and a selection with no plan;
    - the pull test: the card from the stdout line, the public host, a card mismatch, an absolute URL, and another plan.

---

- **High — Resident map pulls are rejected by the child’s Host gate.** [host/web-js/dev.mjs:488](/tmp/rv-dt-maps/host/web-js/dev.mjs:488) fetches using the internal port, but the child constructs its gate with `--serve-as`’s public port. For public `8765` and internal `8766`, the request receives 421; `pullMap` silently returns null. The resident-map cache therefore stays empty. **Fix:** preserve the public Host header on both fetches while connecting to the internal port. Test against the actual `--serve-as` gate.

- **Medium — A plan digest does not make its source map immutable.** [scripts/tamber-server.mjs:197](/tmp/rv-dt-maps/scripts/tamber-server.mjs:197) returns cached maps without consulting the provider. A formatting-only edit can preserve plan bytes while changing source locations and file hashes; the compiler deliberately stores locations outside the plan. Subsequent selections keep obsolete locations and remain read-only despite a successful rebuild. **Fix:** refresh cached entries when the map’s content/card changes, including when its plan digest stays unchanged.

- **Medium — A selection without a digest now uses the running map.** [scripts/tamber-server.mjs:217](/tmp/rv-dt-maps/scripts/tamber-server.mjs:217) passes an absent/null `selected.planDigest` to `mapOf`, which means “running generation.” A report whose other status fields match can consequently acquire source and editability from an unrelated plan’s site. Previously, `mapFor` refused this join. **Fix:** require a valid selection digest before resolving a map; otherwise keep the selection read-only. Also check it against the build’s recorded digest when known.

- **Medium — One unavailable running map blocks other builds’ valid outlines.** [scripts/tamber-server.mjs:557](/tmp/rv-dt-maps/scripts/tamber-server.mjs:557) returns 503/409 before resolving individual builds. If the current generation lacks a map—or its map is stale—a connected build with a known, current map receives nothing. **Fix:** resolve and validate maps per build; use the running map only for builds lacking a digest. Test a missing current map alongside an available older map.

- **Medium — The pull checks integrity, but does not authenticate the supplying process.** [scripts/tamber-inspect.mjs:34](/tmp/rv-dt-maps/scripts/tamber-inspect.mjs:34) trusts the envelope returned from a port that the parent releases before spawning the child. A process answering there can supply both forged map bytes and matching hashes. Absolute map URLs and redirects are also accepted. No request handler directly ingests maps, but that alone does not establish the claimed isolation. **Fix:** bind the expected map card to the child through its inherited channel, and enforce same-origin, nonredirecting fetches.

- **Low — Unavailable page digests are repeatedly queried.** [host/web/dev.js:366](/tmp/rv-dt-maps/host/web/dev.js:366) retries whenever the cached result is null. A build exposing `exact_agent` but never returning `planDigest` runs this query after every commit, rather than once per generation. Successful reads also serialize every view/site pair just to obtain the digest. **Fix:** cache unavailable results distinctly from “not queried,” and preferably use a digest-only read.

The protocol constant and docs consistently say `tamber/1.8`. Explicit unknown-digest refusal, empty-site clearing, click-token forwarding, and each cache’s eight-entry limit look correct by inspection. The tests do not cover real resident pulling, same-digest map replacement, missing selection identity, ninth-entry eviction, or page-query counts. I inspected tests without running them because they write files.

Verdict: **Request changes.**