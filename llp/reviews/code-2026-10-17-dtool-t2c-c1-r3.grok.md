# Code review: LLP 1122 T2c step 1 (source maps by plan digest), round 3, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `fa0d64578`.
- **Method:** one brief (sha256 `bd4b8f7b59ce668080cfcd4eeafa30fa7b5df69d4302b3d9c7744b855daa35ef`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium, 1 low).
- **Disposition:** taken (the last round), but the tests in part.
  - A fresh map with no generation (an edit that moves only locations) is kept and said only for the plan the builds run. A value edit's candidate plan is no one's. It also replaces the current module's map, so a later recapture keeps it.
  - A build that says no plan is served the published plan's kept map, the fresh one included.
  - A generation's map is kept only once it is published. A refused candidate changes neither cache.
  - The JS loop asks a card's map again (up to 20 times, 250 ms apart) while it is still the latest, so a card said before the child listens is not lost.
  - `outline` refuses (503) when builds could not be outlined because their plans have no map here, and no build was outlined.
  - The page takes a verified generation's plan digest from the candidate (`planSha256`).
  - The pull test serves bytes for any map path, so a card whose hash is not the bytes' is refused on its bytes, not by a 404.
  - In part: no test runs the real pipe, a ninth plan, or overlapping pulls end to end. Those paths are exercised by the gate through the JS loop's natives.

---

**Medium — A refused generation is already in the eight-map cache.** `host/web/dev.mjs:436`

`keepMap` runs as soon as the map’s digest matches the plan, and only then does `captureGeneration` check the module receipt (`:439`), the payload budget (`:474`), the 64 KiB envelope (`:491`), retention (`:493`), and classification (`:496`). `current` is assigned at `:497`, and `announcePlan` follows that. Any of those throws leaves the previous generation published, but the new bytes are already stored and the oldest of the eight may already be gone.

Scenario: eight plans are cached, a phone is still on the oldest, and the next candidate’s dev-generation cache is full (the throw `retainDevGeneration` already documents). That phone’s digest now misses. `sourceMap` falls through to `current.files`, the digests differ, and `mapOf` returns null, so the phone is outlined from nothing. The same happens when the refused candidate reuses a plan a build already reports: `plannedMaps` then holds a map that was never served.

Fix: call `keepMap` only after `current = revision`, beside `announcePlan`. On throw, leave both caches as they were.

**Medium — A build that says no plan still sees the pre-edit map.** `host/web/dev.mjs:390`

A location-only edit correctly stores and announces the fresh map (`:743`) and does not publish a generation. `sourceMap(digest)` sees that map. `sourceMap()` with no digest never looks in `plannedMaps`; it returns `current.files`’s `app.plan.map.json`, which that path does not replace, or the map still on disk.

Scenario: blank lines are added to `app.contract`. The plan digest is unchanged, so there is no new generation. A build whose `planDigest` is null (a native that never sends one, or a page whose `sites` read came back null) is outlined from the old bytes. Those source hashes no longer match disk, `staleMap` 409s the outline, and the fresh map sits unused under that same digest. The web page itself is fine, because it sends the digest and hits `plannedMaps`.

Fix: for a missing digest, resolve `current.envelope.plan.sha256` in `plannedMaps` before `current.files` and `readMap()`.

**Medium — A missing per-build map still answers success.** `scripts/tamber-server.mjs:564`

Refusal happens only when nothing was outlined and a map was stale or the running map is absent. A build whose digest is unknown is skipped without recording a reason. If the running map exists, the handler returns 200 `{sites, builds: []}`.

Scenario: the ninth published plan evicts the only connected phone’s map. Inspect is on, and the running map is fine. The phone is sent nothing. The design tool treats an empty `builds` list as “No build can inspect” (`apps/design-tool/app.ts:1440`). The documented 503 never happens. The server test does not hit this: its “running map missing” case still has another build’s map, so `builds` is non-empty.

Fix: if any inspecting build was skipped because its map was missing or stale and none were outlined, return 409 when a map was stale and 503 otherwise. Keep 200 with `builds: []` for the case where no build can inspect.

**Low — The page never reads the plan digest it already verified.** `host/web/dev.js:236`

`knownPlan(candidate?.plan?.sha256)` is meant to run once a generation is accepted. `fetchGeneration` verifies `plan.sha256` and then returns `plan: plan.bytes` (`:142`), a `Uint8Array`. `.sha256` is missing, so `knownPlan` returns immediately. Every later generation still calls `ask({ op: "sites" })`, which walks every view, once per generation (`:370`). The value is right when that call succeeds; the card digest is simply unused. The boot generation still has to ask the runner.

Fix: return the verified `plan.sha256` on the candidate and pass that to `knownPlan`.

The server tests stub `sourceMap`, and the pull test stubs the HTTP server. Neither runs `keepMap`, the no-digest callback, a ninth plan, or the page’s card path, so they pass while these three production paths fail.

Verdict: Request changes — a refused candidate can drop a live plan’s map, a no-plan build misses a location-only map, and a missing map is reported as success.
