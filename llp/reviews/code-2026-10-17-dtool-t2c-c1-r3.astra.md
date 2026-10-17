# Code review: LLP 1122 T2c step 1 (source maps by plan digest), round 3, 2026-10-17 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `fa0d64578`.
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

- **Medium — Live token edits announce a plan that never runs.** [host/web/dev.mjs:743](/tmp/rv-dt-maps-r3/host/web/dev.mjs:743) announces the candidate’s digest on every `live` edit. Changing a token value changes those plan bytes, but builds receive a batch and retain their existing plan. The JS loop therefore selects an unpublished plan as its fallback; after eight distinct edits, these candidates evict the running plan’s map. **Fix:** announce the published plan’s identity, and refresh metadata without a generation only when verified against that plan.

- **Medium — Formatting-only map refreshes leave stale metadata active.** [host/web/dev.mjs:390](/tmp/rv-dt-maps-r3/host/web/dev.mjs:390) returns `current.files` when no digest is supplied, bypassing the freshly updated `plannedMaps`. After adding blank lines, builds omitting `planDigest` still receive the old map and fail stale-source checks. Moreover, an asset edit recaptures the unchanged `currentModule` and [overwrites the fresh cache entry with its old map](/tmp/rv-dt-maps-r3/host/web/dev.mjs:436), breaking explicit-digest joins too. **Fix:** retain refreshed metadata for the accepted plan and use it for both fallback lookup and subsequent captures. Test formatting followed by an asset edit.

- **Medium — The initial map pull can permanently lose a startup race.** [host/web-js/dev.mjs:497](/tmp/rv-dt-maps-r3/host/web-js/dev.mjs:497) attempts each card once. The child starts its compiler before awaiting the filesystem reader and calling `server.listen`; a warm compiler can publish its card during that wait. The parent’s connection then fails, and starting the listener does not trigger another pull. That plan remains unmapped until another announcement. **Fix:** coordinate initial card delivery with listener readiness, or retry the still-current card after connection failure. Test a card arriving before the listener opens.

- **Low — The verified page-digest handoff never succeeds.** [host/web/dev.js:236](/tmp/rv-dt-maps-r3/host/web/dev.js:236) reads `candidate.plan.sha256`, but [`fetchGeneration` returns `plan.bytes`](/tmp/rv-dt-maps-r3/host/web/dev.js:142), a `Uint8Array` without that property. Every replacement generation still falls back to encoding/hashing the runner’s plan and serializing all view/site pairs. **Fix:** preserve the verified digest as a separate candidate field and pass it to `knownPlan`; assert that reloads require no `sites` query.

The dedicated pipe, public Host header, constrained map route, integrity checks, superseded-pull rejection, unknown-digest refusal, clearing, click-token forwarding, and `tamber/1.8` updates look correct by inspection.

Tests were inspected, not run under the read-only restriction. They do not exercise the failures above, the actual pipe/route, overlapping pulls, or ninth-entry eviction. The [bad-hash test](/tmp/rv-dt-maps-r3/scripts/tamber-inspect.test.mjs:149) receives 404, so it does not prove rejection of mismatched bytes returned with 200.

**Verdict: Request changes — three Medium correctness issues remain.**