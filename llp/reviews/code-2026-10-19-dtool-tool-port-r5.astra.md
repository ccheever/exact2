# Code review: the design tool's port for tests (EXACT_TOOL_PORT), round 5, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `7b4db39f80`.
- **Method:** one brief (sha256 `53f233b9d82c68b6260697429f33aaad97ad368252b7b38b0ea457ff539c4fcc`), shared by both reviewers, blind to the other review; commit f5cee1c0e4 (round 4's fixes: two general web-build freshness changes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (2 P2).
- **Disposition:** taken in `29fd3f9982`: host/web/build.mjs records when the JS build read its inputs (`inputsAt`) and webChanges nominates every input changed since then (less two seconds of clock grain), not since the marker, so an edit during the compile, or a build published over another, is compared with the digest read (both, P2); only test files and globs (`….test.contract`) map into the stage (Astra P2). Not taken: the wasm path and the JS dev loop's markers (no `inputs`; the dev loop watches its sources) keep their own freshness, as before (Astra, Grok note it, no regression); Expose's build-written local.ts is deterministic, so it reads as a change at most once after it moves (Astra).

**Request changes: two P2 issues remain.**

1. **[P2] Edits during compilation still leave obsolete output “fresh.”** The snapshot now happens at [host/web/build.mjs:68](/tmp/rv-dt-port5/host/web/build.mjs:68), but [webChanges:834](/tmp/rv-dt-port5/scripts/agent-launch.mjs:834) still selects candidates using the completed marker’s timestamp. The new deletion loop checks existence, not content.

   Concrete interleaving: A captures S1; B stages S2 and publishes its build; A publishes S1 last. S2’s files are older than A’s marker and still exist, so their recorded S1 hashes are never compared. Subsequent staging leaves unchanged S2 files untouched, and [buildTool:1222](/tmp/rv-dt-port5/scripts/tamber-server.mjs:1222) accepts S1. `verify` and agent drives inherit the same false freshness; `serve.mjs` validates the output’s integrity, not its source freshness.

   A memory-only probe using the actual freshness functions returned `all: []` for this timing. Deletion and an edit newer than the marker were detected. The candidate boundary must include changes during compilation.

2. **[P2] Argument mapping still rewrites flags and agent operations.** [tamber-tool-port.mjs:82–83](/tmp/rv-dt-port5/scripts/tamber-tool-port.mjs:82) treats any non-option argument containing `*`, `?` or `[` as a test glob, without checking its argument role.

   From `apps/design-tool`, `bun exact.mjs agent web --url 'http://127.0.0.1:8765/?agent=1' tree` rewrites the URL to `…/target/design-tool-test-<port>/http:/127.0.0.1:8765/?agent=1`. Likewise, the quoted operation `"type name Ada?"` becomes a filesystem path. Both break the drive. These transformations reproduced with the actual mapping function and staging replaced by a memory-only stub. Remap selected test operands and `--test` values only.

The remaining checks:

- The absent `.exact/backend-origin` sentinel is correctly excluded from the deletion sweep through its `null` digest. Relative shared keys, absolute app keys, canonicalized app directories and game input exclusions revealed no new path mismatch or default-path rebuild loop.
- **There is another generated input:** Expose’s imported `local.ts`, written by [its build script](/tmp/rv-dt-port5/apps/expose/web/build.rs:9), which the [JS builder invokes](/tmp/rv-dt-port5/host/web-js/build.mjs:211). It can differ from the precompile snapshot. A freshness correction must account for generated inputs; `app.contract.d.ts` is already excluded, and shared Rust generation writes into `OUT_DIR`.
- The paths do not provide identical guarantees: [wasm records inputs after compilation](/tmp/rv-dt-port5/host/web/build.mjs:499); [JS dev markers contain no `inputs` field](/tmp/rv-dt-port5/host/web-js/dev.mjs:139). The dev loop instead uses watchers and supersession checks. The new deletion sweep cannot cover its inventory-free markers.
- Existing test files, relative paths and globs through `/tmp`’s symlink map correctly; outside paths remain unchanged. `--help`/`-h` returned successfully for all three verbs with port 8765.
- The additional existence scan is linear. Warm, read-only measurements were approximately **0.46 ms / 422 inputs** for the tool, **0.97 ms / 749** for Caltrain wasm, and **7.47 ms / 4,803** for Tennis. No performance blocker demonstrated.

No files or Git state changed. No builds or test suites ran.
