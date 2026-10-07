# Code review: per-subtree `color-scheme` (LLP 1034 §8), round 4 (confirmation of the landed code), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a fresh worktree at origin/main 887f53d5d.
- **Method:** one brief (sha256 `f5637bd61b4fbd094d41eaf15a254c1c9d62e8398acb362b92a7105ee5df712e`), shared with grok, asked by the coordinator after landing. Reviewed 26ae6e071 and the code around it on main, including the never-reviewed round-3 withdrawal and the gaps §8 records. Blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** NOT READY.
- **Disposition:** 1 taken (blocking): a batch's preamble that lands slices published ahead of it now holds the appearance-report drain (`holdingReports`) until that batch, the outermost apply, drains it, so no batch made later applies before it; destruction is rechecked after landing. Covered by the existing session/fill XCTests; no new integration test of an in-flight tick with queued reports (the harness has no hook to hold a tick's publication). 2 taken (blocking): `Session.scheme` reports every view whose style carries a `color-scheme` after each session scheme change, so a subtree fixed to the scheme the session leaves starts later animations in its own; test `ColorSchemeMacTests.testASessionSchemeChangeReportsAFixedSubtree`. Viewless inline/SVG nodes cannot set a scheme and take the session's for keyframes; declared. 3-7 taken as §8 wording: the absent-entry recolouring, viewless nodes, clearing an authored scheme through the old report, "the next batch that passes through apply", Linux keyframes taking the session appearance at start, the content region's full list of captured paints (and its LLP corrected to 1043.000), and the flow-damage rule as the code has it.

---

NOT READY

1. **[Session.swift:963](/tmp/x2cs4/host/apple/Sources/ExactKit/Session.swift:963), [Session.swift:1008](/tmp/x2cs4/host/apple/Sources/ExactKit/Session.swift:1008) — must-fix; blocks. The withdrawal leaves an out-of-order application path.**  
   Let async fill/tick **A** be published, and synchronous batch **B** be produced afterward. `apply(B)` first lands A. Because `applying` is still false, A becomes an outermost application; its appearance drain produces and applies **C**, then execution returns to apply B. Presentation order is **A → C → B**, allowing B’s older paint samples or frame flags to overwrite C. New subtree trait callbacks can supply these reports. This drain flaw predates the withdrawal; removing `flushViewDarkSoon` does not eliminate it.

   **Fix:** defer appearance-report production until all already-produced batches, including the waiting synchronous batch, have been presented. Recheck destruction/generation after reentrant landing. Add a session integration test with an outstanding tick/fill and multiple appearance reports.

2. **[Session.swift:1290](/tmp/x2cs4/host/apple/Sources/ExactKit/Session.swift:1290), [host.rs:1270](/tmp/x2cs4/host/apple/src/host.rs:1270) — must-fix; blocks. A fixed subtree can initialize future animations in the wrong scheme indefinitely.**  
   A light subtree initially matching a light session has no report entry. Changing the session to dark leaves that subtree’s effective appearance unchanged, so no trait callback is required. A background-only keyframe started afterward takes the engine’s dark session scheme. Waiting for another batch does not repair the missing report.

   I **do not accept this gap**: an ordinary session-scheme change invalidates subsequently started work in a stable, explicitly light subtree. §8 identifies the missing callback, but should state the persistent consequence for future animations. Its “only when … next change appearance” wording also overlooks reports triggered by changed `text_color`.

   **Fix:** establish each animation node’s computed scheme before adopting new animations, including viewless inline/SVG nodes. Alternatively, reconcile fixed-scheme reports after every session change, with ordering handled as in finding 1.

3. **[paint.rs:260](/tmp/x2cs4/kernel/src/motion/paint.rs:260), [§8:404](/tmp/x2cs4/llp/1034-scheme-aware-colour.rfc.md:404) — should-fix; nonblocking. The declared Apple keyframe limitation is inaccurate.**  
   Retaining already-playing colours agrees with LLP 1062 D9; the explicitly deferred same-commit initialization case is acceptable for this slice, separately from finding 2. However, “only its first” is false: `first` means the map entry is currently absent. In a light session, reports **dark → light → dark** produce recolouring **yes → no → yes**, because returning to light removes the entry. Inline/SVG animation nodes also receive no report merely because their containing view reports.

   **Fix:** document the repeatable absent-entry correction and viewless-node limitation, or track initialization separately from current overrides. Qualify “Transitions have none of this”: clearing an authored scheme can initially resolve through the old report until its removal reaches the engine.

4. **[Session.swift:1317](/tmp/x2cs4/host/apple/Sources/ExactKit/Session.swift:1317), [§8:409](/tmp/x2cs4/llp/1034-scheme-aware-colour.rfc.md:409) — should-fix; nonblocking. Waiting outside a batch is acceptable, but “next batch” needs qualification.**  
   This restores the pre-change scheduling policy and avoids introducing another asynchronous flush. However, `applyUnlessEmpty` and `landFill` can skip presentation; those batches do not drain reports. An idle session can therefore retain a report indefinitely.

   **Fix:** say “the next batch actually passed through `apply`,” including the idle/skipped-batch case. No independent main-queue flush is required to accept this limitation.

5. **[host.rs:1252](/tmp/x2cs4/host/linux/src/host.rs:1252), [§8:412](/tmp/x2cs4/llp/1034-scheme-aware-colour.rfc.md:412) — should-fix; nonblocking. Linux keyframes are an acceptable declared platform gap.**  
   The engine receives no production per-view appearance reports, so subtree overrides do not establish keyframe appearance. The declaration identifies that limitation, but “keeps resolving in the app’s scheme” suggests continuous resolution.

   **Fix:** specify that keyframes use the session appearance at animation start, subject to the first-report boot correction; later session flips preserve existing plays.

6. **[region.rs:210](/tmp/x2cs4/host/linux/src/paint/region.rs:210), [region.rs:254](/tmp/x2cs4/host/linux/src/paint/region.rs:254) — should-fix; nonblocking. The Linux content-region exclusion is broader than §8 states.**  
   Deferring this separate capture path is acceptable, but it resolves more than text runs using `painter.dark`: box fills, borders, gradients, shadows, material tints and image tint also bypass subtree appearance.

   **Fix:** expand §8’s exclusion to those captured paints and correct its unrelated LLP 1093 reference. Alternatively, capture each node using its computed scheme.

7. **[paint.rs:870](/tmp/x2cs4/host/linux/src/paint.rs:870), [§8:416](/tmp/x2cs4/llp/1034-scheme-aware-colour.rfc.md:416) — nit; nonblocking. Full repaint is an acceptable correctness fallback.**  
   The implementation disables flow damage when an encountered node’s appearance differs from the carried appearance. Merely mounting a same-scheme subtree does not trigger this condition.

   **Fix:** narrow §8’s wording accordingly; no repaint optimization is required for acceptance.

The normal first-scheme replay order is sound, and `forgetAppearances` clears all four caches before applying a replacement tree. I found no additional lifetime regression from the withdrawal: nested applications leave draining to the outer application, and destroyed sessions are rejected by `scheme`/`apply`. These observations do not resolve finding 1.

Reviewed HEAD `887f53d5d`. No files changed. The ordering trace was checked with an executable control-flow model; native tests were unavailable because this checkout has no build artifacts and Swift’s module-cache writes were blocked by the read-only sandbox.
