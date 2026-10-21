# Code review: LLP 1122 T2c step 1 (source maps by plan digest), round 2, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f1a88d26e`.
- **Method:** one brief (sha256 `d09d480ebe5bf75d5e121f51e41555bbc4924b95e7c4abce288a343c8cc366fb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (5 medium, 1 low).
- **Disposition:** taken.
  - **The card's channel.** Cards no longer go on stdout. The resident loop writes them, as JSON, to a pipe the JS loop gives it for that alone (fd 3, `EXACT_MAP_FD`), where nothing a request makes is written.
  - **When a card is said.** It is said only after the generation is published (`current` assigned), for every published plan, with `map: null` for a plan with none. It is also said when an edit changes the map but not the plan: that edit makes no new generation, and the fresh map is kept and said (the wasm loop keeps it too).
  - **Where the bytes come from.** The child serves them at `/__dev/map/<sha256>`, the only path `pullMap` asks.
  - **The JS loop's order.** Each card gets a sequence number. A pull is stored only if no later card of its plan has been seen. The 8 plans said last are kept, in the order they were said.
  - **Digest currency.** A selection whose `planDigest` is not the one its build says it runs now is not current: refused and asked again. The ask-again key includes the digest.
  - **The page's digest.** The page takes it from the plan card of a generation it has verified, and reads the runner only for the plan it booted with.
  - **Tests.** The pull test covers the pipe's card, the map path, a plan with no map, a card mismatch and another plan. The server test covers a selection from another plan than the build's.

---

- **Medium — Request text can impersonate a map card.** [host/web-js/dev.mjs:615](/tmp/rv-dt-maps-r2/host/web-js/dev.mjs:615) trusts matching stdout lines, but [host/web/dev.mjs:1364](/tmp/rv-dt-maps-r2/host/web/dev.mjs:1364) logs decoded `fetch` and `boot` query parameters verbatim. After an edit, another local process can request `/__dev/reloaded` with the known epoch/sequence and inject newline-delimited `source map …` text. It can replay an older map’s card or change `residentPlan` to an unavailable digest—even when the pull fails. **Fix:** carry cards over a dedicated IPC channel, separate from request-derived logging; test newline injection through the actual handler.

- **Medium — The same-digest refresh fix does not reach TypeScript formatting edits.** [host/web/dev.mjs:725](/tmp/rv-dt-maps-r2/host/web/dev.mjs:725) discards the candidate when its module and design structure are unchanged, including an empty token batch. Adding blank lines to `app.contract` therefore produces a new map but never reaches `captureGeneration()` or updates the map provider. Selections retain old locations and become read-only despite successful compilation. **Fix:** refresh and publish verified map metadata on this path even when no application generation is needed. The new test replaces the provider’s buffer directly and misses this.

- **Medium — Overlapping pulls can restore an obsolete map.** [host/web-js/dev.mjs:494](/tmp/rv-dt-maps-r2/host/web-js/dev.mjs:494) installs every completed pull unconditionally. If two cards name the same plan digest but different source locations, an older pull finishing last overwrites the newer map. Tracking `residentPlan` separately does not prevent this. The eight-entry limit also evicts by completion order, potentially evicting a newer generation before delayed older ones. **Fix:** track announcement order and the latest card per digest; reject superseded completions and retain the eight most recent announced maps.

- **Medium — Map cards do not reliably identify the published running generation.** [host/web/dev.mjs:472](/tmp/rv-dt-maps-r2/host/web/dev.mjs:472) announces before retention/publication succeeds, and emits nothing for a generation without a map. Thus a rejected candidate can become the parent’s running plan; conversely, a successfully published mapless plan leaves the previous plan selected. Builds omitting `planDigest` then receive the wrong fallback or lose outlines unnecessarily. **Fix:** announce every successfully published generation after assigning `current`, with a nullable map card.

- **Medium — Selections are not checked against the build’s recorded `planDigest`.** [scripts/tamber-server.mjs:218](/tmp/rv-dt-maps-r2/scripts/tamber-server.mjs:218) selects the report’s historical map, while `current()` checks only `plan`, `design`, and revision. A build that updates `planDigest` from P1 to P2 while leaving the optional `plan` unchanged can submit a delayed P1 selection that is accepted and potentially editable. **Fix:** reject a nonnull selection digest conflicting with the recorded digest, and include that digest in the refresh-deduplication key.

- **Low — Reading the page digest still performs unnecessary inspection work.** [host/web/dev.js:368](/tmp/rv-dt-maps-r2/host/web/dev.js:368) requests `sites`, serializing every view/site pair and initially encoding and hashing the plan, even with inspect mode off. Unknown results are now correctly cached, but this cost remains once per generation. **Fix:** retain the already verified plan-card digest when the generation is accepted.

Explicit unknown-digest refusal, empty-site clearing, click-token forwarding, and the `tamber/1.8` documentation look correct by inspection. The tests do not establish the production cases above or ninth-entry eviction. I did not run tests because this review permits only read-only commands.

Verdict: **Request changes.**