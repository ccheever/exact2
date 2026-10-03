# Code review: the Signal clone diary's fixes (f6ea6c40), 2026-10-02 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` the session worktree at `f6ea6c40`.
- **Method:** one brief (sha256 `917f31971adb6fd603f1fb41ddf5e21dc20a96aae88b53b36913dfacf23a2c2a`); one round; requested by Charlie. The author (Claude) is not a reviewer.
- **Transcription:** the run's final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition:** every finding checked in the source and fixed in the commit after `f6ea6c40`: (1) `--unsigned` refuses an app with GPU modules (`GpuModule.swift` checks each module's baked digest at load); (2) the staleness walk keeps ignored Rust, TOML and WGSL files and everything under the declared shader roots and preludes (`shaderWatchRoots`), with both of the review's cases in the caps test; (3) a custom action's title is shown only when its text child is visible (`display: none`, hidden or transparent text stays off), `testANativeSwipeActionIsDrawnFromItsFace` covers it; (4) Charlie's waiver recorded in `rules/DEFERRED.md`.

---

1. **Must-fix — re-signing breaks GPU archives.**  
   [build.mjs:888](host/apple/build.mjs:888) selects ad-hoc signing, and [build.mjs:940](host/apple/build.mjs:940) bakes the signed GPU dylibs’ hashes into the executable. [GpuModule.swift:67](host/apple/Sources/ExactKit/GpuModule.swift:67) verifies the entire file before loading it.

   **Failure:** archive an app with a primary or declared GPU module, then have the consumer re-sign its embedded libraries. The signature changes the bytes; loading the module fails with `digest mismatch`, including under development trust.

   **Fix:** reject `--unsigned` for these apps until final signing and baked identity can be coordinated. Re-signing support must preserve verification, rather than bypass it.

2. **Must-fix — the ignore filter suppresses real build inputs.**  
   [agent-launch.mjs:145](scripts/agent-launch.mjs:145) preserves only the TypeScript capture’s extensions and three directories, but [agent-launch.mjs:182](scripts/agent-launch.mjs:182) applies this to every web app. Actual inputs also include Rust files and manifest-declared [shader roots and preludes](scripts/app.mjs:40).

   **Failure:** modify an ignored `data/src/generated.rs` included by the app, or ignored `shader-gen/paint.wgsl` under a declared shader root. The driver now accepts the old web build. I reproduced both omissions by executing the changed functions against an in-memory filesystem.

   **Fix:** retain dependencies from the actual build and declared shader inventory regardless of Git status. Where that inventory is unavailable, preserve the conservative scan. Add both cases to the test.

3. **Should-fix — hidden text becomes a visible swipe title.**  
   [SwipeActionsIOS.swift:320](host/apple/Sources/ExactKit/IOS/SwipeActionsIOS.swift:320) treats any nonempty face title as visible. However, [control.rs:301](kernel/src/control.rs:301) derives that title from text children without checking their display or opacity.

   **Failure:** a custom action containing a symbol and a direct text child with `display="none"` previously projected as an icon alone. It now visibly displays that hidden text beneath the icon.

   **Fix:** distinguish visibly authored titles in the shared face/projection data and test a custom action with hidden text.

4. **Should-fix — reconcile the new archive workflow with binding scope.**  
   [build.mjs:31](host/apple/build.mjs:31) explicitly introduces the re-signing workflow, while [DEFERRED.md:535](rules/DEFERRED.md:535) still defers distributable IPAs and EAS/AppDrop adapters. The signed archive support already predates this branch, but this extends that unresolved scope conflict.

   **Fix:** record the narrow admission and required trade or Charlie’s waiver alongside the deferred entry.

Read-only review; no files modified. Native builds and UIKit tests were not run.

**Verdict: LAND WITH FIXES.**