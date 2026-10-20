# Code review: LLP 1122 T2c step 1 (source maps by plan digest), round 2, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `f1a88d26e`.
- **Method:** one brief (sha256 `d09d480ebe5bf75d5e121f51e41555bbc4924b95e7c4abce288a343c8cc366fb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 1 medium, 1 low).
- **Disposition:** taken.
  - **The card's channel.** Cards no longer go on stdout. The resident loop writes them, as JSON, to a pipe the JS loop gives it for that alone (fd 3, `EXACT_MAP_FD`), where nothing a request makes is written.
  - **When a card is said.** It is said only after the generation is published (`current` assigned), for every published plan, with `map: null` for a plan with none. It is also said when an edit changes the map but not the plan: that edit makes no new generation, and the fresh map is kept and said (the wasm loop keeps it too).
  - **Where the bytes come from.** The child serves them at `/__dev/map/<sha256>`, the only path `pullMap` asks.
  - **The JS loop's order.** Each card gets a sequence number. A pull is stored only if no later card of its plan has been seen. The 8 plans said last are kept, in the order they were said.
  - **Digest currency.** A selection whose `planDigest` is not the one its build says it runs now is not current: refused and asked again. The ask-again key includes the digest.
  - **The page's digest.** The page takes it from the plan card of a generation it has verified, and reads the runner only for the plan it booted with.
  - **Tests.** The pull test covers the pipe's card, the map path, a plan with no map, a card mismatch and another plan. The server test covers a selection from another plan than the build's.

---

The round-1 fixes hold for per-plan join, the host gate, empty-site clears, the click token, and an unknown digest. The resident loop still adopts a map from a card that the generation may never publish, and an older pull can replace a newer one.

**High** — The stdout card is logged before the generation is admitted, and the JS loop makes that plan the running map before a pull succeeds. `host/web/dev.mjs:472` and `host/web-js/dev.mjs:490`.

`captureGeneration` prints `source map …` and only then checks the 64 KiB envelope (`:479`), retains the generation (`:481`), and assigns `current` (`:485`). Any of those can throw, and the caller treats that as a refused generation: clients stay on the previous plan. The parent sets `residentPlan = card.plan` as soon as it reads the line. If the throw is the envelope check, the fetch 404s and the retry 404s, so `sourceMap()` with no digest is null even though the previous plan's bytes are still in `residentMaps`. If retain succeeded and `classifyGeneration` then throws, the fetch hits the disk cache and the unpublished plan becomes the running map while devices are still on the old one. A native that has not reported `planDigest` is outlined from that pointer, so the tool gets "no source map for the running plan yet" or sites for a plan that was never served, until a later card succeeds.

Log the card only after `current = revision`. In `residentMap`, keep the previous running plan until this card's pull succeeds, and ignore the result if a later card has already been seen.

**Medium** — A slower pull of the same plan digest overwrites the newer map and refreshes the 8-entry cache. `host/web-js/dev.mjs:492-496`.

`residentPlan` moves when the card is seen, but the bytes are stored whenever the pull finishes, including the 500 ms retry. A location-only edit keeps the plan digest and changes the map. The first generation's body can still be streaming (the cap is 64 MiB, and `res.end` yields above the socket watermark), or its retry can still be in flight, when the second pull stores the new bytes. The late `residentMaps.set(got.digest, got.bytes)` puts the old bytes back and moves that digest to the newest slot, so the eviction can drop a map a build is still on. Joins and outlines then use the stale locations. The wasm loop does not have this race: `keepMap` is synchronous.

Stamp each card with a monotonic id and store bytes only when that id is still the latest for that plan.

**Low** — The new tests never reach those paths. `scripts/tamber-server.test.mjs:143` and `scripts/tamber-inspect.test.mjs:135`.

The server test covers three digests, a missing running map, a same-digest replacement, an empty site list, and a selection with no plan. `pullMap` checks the public host, a bad hash, a non-generation URL, and another plan's body. Nothing publishes a card and then refuses the generation, overlaps two pulls of one digest, fills a ninth entry, or 409s one stale map while another build is outlined.

Verdict: Request changes — a refused or overlapping resident generation can drop or regress the running map that builds without a `planDigest` are outlined from.
