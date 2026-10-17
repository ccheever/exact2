# Code review: LLP 1122 T2c step 1 (source maps by plan digest), round 1, 2026-10-17 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `a27b46995`.
- **Method:** one brief (sha256 `12e549c8c8b6cc5b5013ae70cd763b58f953ae0a33926a6b62e932c065b7c004`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium, 1 low).
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

Per-plan join works when each build names a digest the server already holds. The tool outline and the JS loop's notion of the running map do not.

**High** — `scripts/tamber-server.mjs:557-560`. `outline` still loads `mapOf()` (no digest) and returns 503 or 409 before the per-build loop. That callback is the running generation only (`host/web/dev.mjs:374` does not consult `plannedMaps` unless a digest is passed). A Contract generation that published no map, or whose sources no longer match disk, refuses the request for every build.

Scenario: generation P2 is current and has no `app.plan.map.json` (the Rust bake path drops it), while a page is still on P1 and P1's map is in the last eight. `uses` names a span that exists in P1. The tool gets "no source map for the running plan yet" and that page is sent nothing. Same if the file was reverted to P1's bytes and P2's map is now stale: `staleMap` 409s the whole call even though P1's map matches disk. A stale or missing older map is only skipped later (`:564-566`), so this gate is stricter than the per-plan rule.

Fix: drop the early return. Resolve each build from `mapOf(b.planDigest)`, and use the running map only for a build that omitted `planDigest`. Return 503/409 only when that running map was actually required and failed. Add a test whose `sourceMap(digest)` returns null for the running digest and a real map for an older one.

**Medium** — `host/web-js/dev.mjs:487-494` and `:611`. Each `/ready/` line starts `pullMap` and does not wait. The map stored last is treated as the running generation (`[...residentMaps.values()].at(-1)`). `/ready/` also matches `edit → batch ready`, which is not a new plan.

Scenario: P1's pull is still awaiting a large map when P2's pull finishes; P1 then completes and becomes the running map. `mapOf()` with no digest now parses P1. If P1's sources have moved, the High gate 409s every tool outline while P2's map is sitting in the cache. A build that never sent `planDigest` is outlined from P1. Two ready lines in one stdout chunk both read the current `/exact.json`, so the earlier generation is never stored.

Fix: start a pull only for `plan ready` / `module generation ready` / `Rust generation … ready`, and serialize them. Apply a result only when no later pull has started, and key "running" to that latest success. Put the generation epoch and seq on the log line and fetch that retained `/__dev/generation/…/exact.json`, so a slow pull cannot observe a newer envelope and drop the one that just became ready.

**Medium** — `docs/tamber-protocol.md:772-778`. `hello` is `tamber/1.8` in the Rust constant, the hello tests, and the version list, and `shown()` does carry `planDigest`. The selection section still says every inspecting build is sent that selection's outline, and the `outline` request is still "the dev loop's current map" plus one 409. The frame text at `:745-748` says the opposite: each build gets its own plan, and a build that names none gets the running generation.

Scenario: a client written from `:772` waits for an outline on a build whose `planDigest` is a different plan. The server never sends it (`scripts/tamber-server.mjs:223`).

Fix: say a selection outline goes only to builds on that map's digest, or that omitted `planDigest`; say `ok.sites` is the running plan's sites; say a 409 applies to the map that build is actually using.

**Low** — `host/web/dev.js:366-368` and `scripts/tamber-builds.mjs:264`. The page does refresh when the runner is replaced (`exact.generation` is the boot incarnation, and a new plan boots). The read is `ask({ op: "sites" })`, which builds `{"planDigest", "sites":[…every view…]}` and throws away the list. While the result is null, every later status repeats that walk. A status that sends `planDigest: null` does not clear a digest already stored, because only a string is copied.

Scenario: the status for the new boot throws once, so the record stays on the previous plan. The next click's node carries the new digest, the outline filter requires the record to match, and this page is not sent its own copies. Nothing else calls `status()` until a fact changes, so an idle page stays wrong.

Fix: cache only a 64-hex digest, and if the status names `planDigest`, store that value including null. Prefer the plan SHA-256 the page already verified on the generation card over a full site dump.

What holds: an unknown digest is not filled from another plan (`mapOf` rejects a digest mismatch and does not cache the miss; the JS callback returns null for an unknown key). The eight-entry caps evict the oldest insert. An empty `at` sends `{clear:true}` to every inspecting build; a plan with no sites at those lines gets `{sites:[]}`, which clears; a plan with no map is left untouched. The selection outline still passes `click` through, and the tool outline still does not. No request handler accepts map bytes. `pullMap` keeps a body only when the length, the card SHA-256, and the map's own digest all match the envelope.

The new tests miss the failures above. `scripts/tamber-server.test.mjs:148` always substitutes P1's map for an unknown digest, so "no map" is only the mismatch check. Nothing fills eight maps, marks a file stale, omits `planDigest`, or overlaps two pulls.

Verdict: request changes — a build is not outlined from its own map when the running map is missing or stale, and the JS loop can treat an older pull as that running map.
