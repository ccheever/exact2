# Code review: the app-diary fixes, batch 1 (gaps/papercuts), 2026-10-04 (astra)

- **Family:** OpenAI — `gpt-6-astra` via `codex exec`, reasoning effort xhigh, read-only sandbox, `-C` the gaps/papercuts worktree.
- **Method:** five rounds (r1–r3 on the papercuts and early merges; r4 in two parts over the data, input and driver merges; r5 a delta over every fix). The author (Claude, Opus 5.5, orchestrating) is not a reviewer.
- **Transcription:** each round's final message, unedited, below.
- **Verdicts:** r1 DO NOT LAND · r2 DO NOT LAND · r3 LAND WITH FIXES · r4a DO NOT LAND · r4b DO NOT LAND · r5 LAND WITH FIXES.
- **Disposition:** every finding fixed in the commit after its round (f9bf13aa, 60a776ee, 325d8855, af670d6b, 6dc6d091c/e5570cb11 on fix/data, c9cb87bc1 on fix/input, da47e8e57), except: r5 #1 (two drives of one app on one simulator end each other) is pre-existing and queued; 4a8's browser-realm remnant is queued (QUEUE.md).

---

## papercuts

1. **must-fix — [scripts/agent-launch.mjs:156](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-launch.mjs:156): Real build inputs are now ignored.**  
   Editing `apps/native-fixture/modules/web/index.js` no longer makes the web build stale, although the builder copies that module into the output. The same regression affects game art, including `sprites-fixture/art/strip.png` and `skinned-fixture/art/fox.glb`. I reproduced both exclusions with the actual walker. Agents can consequently verify yesterday’s module or artwork. **Fix:** include the complete consumed module and game-art trees, preferably deriving coverage from build dependencies; add regression cases for both.

2. **must-fix — [game/new.mjs:284](/Users/ccheever/projects/exact2-wt-gaps/game/new.mjs:284): Automatic builds corrupt `--json` output.**  
   `bun exact.mjs agent web --json tree` now inherits the build’s stdout before printing the agent response. Build summaries make the result invalid JSON. This also breaks the existing dispatcher test at `game/new-app.test.mjs:37`: its fake builder and agent produce two JSON records, which its single `JSON.parse` rejects. I reproduced this using the generated dispatcher in memory. **Fix:** send automatic-build progress to stderr while preserving agent stdout and build failure propagation.

3. **should-fix — [scripts/agent.mjs:1276](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent.mjs:1276): Native tests permanently accumulate scratch stores.**  
   Every test creates a unique storage name, but `finally` only closes the carrier. Native storage lives under persistent cache directories—for example, macOS’s `Library/Caches/exact/<app-id>/agent/<name>`—and neither carrier shutdown nor storage-session destruction deletes it. Repeated storage tests accumulate databases and files indefinitely. **Fix:** remove each test-owned store after shutdown, including failed launches, while retaining persistence for ordinary named `--storage` drives.

4. **should-fix — [game/new.mjs:241](/Users/ccheever/projects/exact2-wt-gaps/game/new.mjs:241): The printed `cd` command is incorrectly shell-quoted.**  
   `JSON.stringify(dir)` produces double quotes, which still expand shell variables. Creating `field-log` beneath a directory literally named `$PROJECT` prints a command that changes to a different path. I confirmed the expansion in a shell with a controlled variable. **Fix:** use proper single-quote shell escaping, as the existing `ownWebBuild` helper does.

Validation passed: 486 date oracle rows, 107 number oracle rows, boundary checks, the existing compiled export test, `cargo fmt --check`, `caps`, and `boot`. Full builds and native launches were not run under this read-only review.

Verdict: DO NOT LAND
---

## papercuts-r2

Reviewed **`f9bf13aa`**, HEAD when this review began. The later unrelated commit is excluded.

1. **must-fix — Finding 1 remains partially unfixed.** [scripts/agent-launch.mjs:192](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-launch.mjs:192) returns immediately for Git-ignored files, bypassing the new input-tree, manifest and crate protections. Ignored `modules/web/index.js`, `art/strip.png` and declared icons still escape staleness detection. Ignored `data/table.bin` used by `include_bytes!` is newly skipped compared with the parent. **Fix:** apply all mandatory-input protections before Git-ignore filtering; cover these cases in the regression test. The restored Git-ignore whitelist existed on `origin/main`, but this pre-existing problem is **MATERIAL**: it preserves finding 1 and introduces the crate-resource regression.

2. **must-fix — Finding 2 fixed.** [game/new.mjs:285](/Users/ccheever/projects/exact2-wt-gaps/game/new.mjs:285) redirects both automatic-build streams to stderr with `[0, 2, 2]`, preserving JSON stdout. The missing-Cargo setup in [game/new-app.test.mjs:117](/Users/ccheever/projects/exact2-wt-gaps/game/new-app.test.mjs:117) also correctly prevents PATH repair. Targeted subprocess checks passed; no further fix identified.

3. **should-fix — Finding 3 fixes accumulation but regresses isolation.** [scripts/agent.mjs:1276](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent.mjs:1276) gives concurrent runs of the same app identical stores, beginning with `test.t0`. The second native launch deletes the first run’s active storage, potentially breaking SQLite operations or contaminating assertions. **Fix:** retain unique run namespaces and clean them after shutdown, or serialize access to reusable stores.

4. **should-fix — Finding 4 fixed.** [game/new.mjs:241](/Users/ccheever/projects/exact2-wt-gaps/game/new.mjs:241) uses single quotes and escapes embedded apostrophes correctly. A shell round-trip preserved apostrophes, `$HOME` and command-substitution text literally. No further fix identified.

5. **should-fix — New staleness false positive for world saves.** [scripts/agent-launch.mjs:176](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-launch.mjs:176) omits `.world` from recognized outputs. Saving the documented `screenshot run.world world save` into the app now makes the next web drive reject an otherwise unchanged build; the parent skipped that output. **Fix:** recognize world-save outputs outside protected input locations.

6. **should-fix — Native reset failures silently reuse dirty storage.** [host/apple/src/host.rs:621](/Users/ccheever/projects/exact2-wt-gaps/host/apple/src/host.rs:621) and [host/linux/src/host.rs:585](/Users/ccheever/projects/exact2-wt-gaps/host/linux/src/host.rs:585) discard every deletion error. If permissions prevent cleanup, an authored test can start with previous-run data despite requesting fresh storage. **Fix:** tolerate `NotFound`, but propagate other errors before activation. The deletion is otherwise confined to validated agent scratch storage.

Validation used read-only Bun subprocess checks, in-memory reproductions against the pinned commit and parent, and one existing test. Native hosts and the full suite were not run.

Verdict: DO NOT LAND
---

## papercuts-r3

Reviewed **`f9bf13aa..052db894`**, HEAD when review began. The later `fix/data` merge is excluded.

Round-2 findings **1, 2, 4, 5, and 6 are fixed**. Finding **3 remains partially fixed**:

1. **should-fix — Stale-lock recovery can admit two owners.** [scripts/agent.mjs:1272](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent.mjs:1272)  
   Two contenders can read the same dead PID. One removes the stale lock and acquires it; the other then removes that newly acquired lock using its earlier observation. Both proceed, allowing the second launch to empty the first run’s active store. Either release callback can also delete the other owner’s lock. An in-memory interleaving of the actual function reproduced both failures.  
   **Fix:** use ownership-safe locking/recovery, or unique run stores cleaned after shutdown. Add a concurrent stale-lock recovery test.

2. **should-fix — Host aliases bypass serialization.** [scripts/agent.mjs:1288](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent.mjs:1288)  
   Concurrent `agent.mjs mac --test …` and `agent.mjs macos --test …` acquire different locks. `open()` maps both names to the same macOS carrier, where both use the same app’s `test.t0` storage. This reproduces round-2 finding 3 without needing a stale lock.  
   **Fix:** canonicalize the host before constructing the lock key. Cover simultaneous `mac`/`macos` acquisition.

3. **should-fix — Select settling misses replacement options with identical values.** [host/web-js/rt.js:203](/Users/ccheever/projects/exact2-wt-gaps/host/web-js/rt.js:203)  
   Consider `select value="b"` whose `when` branches each contain options valued `"a"` and `"b"`. Switching branches replaces the option nodes, and the browser selects the new first option. Because the value sequence remains `"a\0b"` and `$set` is false, settlement leaves `"a"` displayed despite the committed value `"b"`. The wasm host reapplies the value after its children update. The new fixture changes the value sequence, so it misses this case.  
   **Fix:** detect option-node replacement as well as value changes, then reapply the committed selection. Add this branch-replacement case.

The nested-list anchor fix and whole-source-map detection passed focused in-memory probes. Caps, boot, and the targeted Apple document-type test passed. Browser/native app runs and the full suite were not run. No files were modified.

Verdict: LAND WITH FIXES
---

## r4a

1. **Must-fix — shared-fetch continuations can strand subsequent requests.** [js/src/prelude.js:648](/Users/ccheever/projects/exact2-wt-gaps/js/src/prelude.js:648). If A starts a shared fetch and B awaits it before starting another fetch, fulfilling A runs B’s continuation with `currentCall` still set to A. B’s second request attaches to A, which completes without dispatching it. B then fails as “pending on nothing,” leaving the request behind. Reproduced with the prelude. **Fix:** preserve ownership across shared continuations or schedule outstanding requests at module scope before declaring quiescence.

2. **Must-fix — capture recursively copies an output directory inside the app.** [host/web-js/build.mjs:155](/Users/ccheever/projects/exact2-wt-gaps/host/web-js/build.mjs:155). With `--out <app>/web-out`, capture enters `web-out/.gen/typescript`, its own destination, generating repeated `web-out/.gen/typescript/...` paths until failure. The newly added producer test uses exactly this output layout. Reproduced using the capture function against an in-memory filesystem. **Fix:** exclude the resolved output/staging trees from capture.

3. **Must-fix — `preventDefault()` does not prevent web submission.** [host/web-js/rt.js:783](/Users/ccheever/projects/exact2-wt-gaps/host/web-js/rt.js:783), [host/web/glue.js:597](/Users/ccheever/projects/exact2-wt-gaps/host/web/glue.js:597). An input’s Enter handler can call `preventDefault()`, but its separate `submit` listener still executes. The probe produced `key, submit` with `defaultPrevented === true`. Ancestor handlers also run too late to stop this submission. **Fix:** execute implicit submission after key propagation finishes, only when the event remains unprevented.

4. **Must-fix — releasing outside a nested-button pan leaves a stale contact.** [host/web/input-glue.js:200](/Users/ccheever/projects/exact2-wt-gaps/host/web/input-glue.js:200). Nested presses now defer capture, but move/up listeners remain local to the pan element. Press near a card’s edge, move outside before capture begins, then release: the card misses pointerup. Returning the mouse can pan the card with no button held; the event probe reproduced this. **Fix:** observe movement and termination on the document while capture is deferred, with cancellation and listener cleanup.

5. **Must-fix — cloning `__proto__` changes the result’s prototype.** [js/src/standard.js:62](/Users/ccheever/projects/exact2-wt-gaps/js/src/standard.js:62). Cloning `JSON.parse('{"__proto__":{"admin":true},"ok":1}')` loses the own `__proto__` property and produces an inherited `admin === true`. The built-in clone preserves the property and ordinary prototype. **Fix:** create own data properties with `Object.defineProperty`, bypassing inherited setters.

6. **Should-fix — macOS dialog defaults run before key handlers.** [host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:155](/Users/ccheever/projects/exact2-wt-gaps/host/apple/Sources/ExactKit/Mac/ExactViewMac.swift:155). The preceding `dialogs.key(event)` consumes Tab and Escape, moving focus or closing the dialog before the new dispatcher runs. Thus a focused editor cannot hear or prevent those keys as it can on the web. The agent route has the same ordering. **Fix:** dispatch authored key handlers before menu/dialog defaults and honor prevention in both routes.

7. **Should-fix — abort listeners retain completed fetches.** [js/src/prelude.js:472](/Users/ccheever/projects/exact2-wt-gaps/js/src/prelude.js:472). Every fetch registers a closure retaining its reject function, but fulfillment and forgetting never remove it. A reused, un-aborted controller accumulates settled promises and their response data. Four completed fetches produced four registrations and zero removals. **Fix:** retain an unsubscribe function with each pending ticket and invoke it on fulfillment, rejection, and forgetting.

8. **Should-fix — another abort listener can disable fetch cancellation.** [js/src/prelude.js:472](/Users/ccheever/projects/exact2-wt-gaps/js/src/prelude.js:472). Registering `e => e.stopImmediatePropagation()` before calling fetch prevents this listener from running. The reproduced fetch remained pending after abort and subsequently returned success. **Fix:** use an internal abort subscription, such as Ibex’s existing abort hooks, rather than a suppressible event listener.

9. **Should-fix — typed-array cloning invokes application constructors.** [js/src/standard.js:37](/Users/ccheever/projects/exact2-wt-gaps/js/src/standard.js:37). An overridden `constructor` can alter the clone’s bytes, throw, or return the original object. The probe cloned `[1,2]` into `[99,99]`; the built-in clone preserved `[1,2]` and returned an ordinary `Uint8Array`. **Fix:** select captured intrinsic constructors by the validated typed-array brand.

10. **Should-fix — `toSorted()` incorrectly preserves holes.** [js/src/standard.js:88](/Users/ccheever/projects/exact2-wt-gaps/js/src/standard.js:88). `[2,,1].toSorted()` returns a sparse array here, whereas the standard result is `[1,2,undefined]`. Consequently, a following `.map()` runs twice natively versus three times on the web. **Fix:** read every index into a fresh ordinary array before sorting; do not use species-sensitive `slice()`.

JavaScript findings were checked with in-memory Bun probes. Native dispatch was inspected but not launched; full builds were not run. No files changed.

Verdict: DO NOT LAND
---

## r4b

1. **Must-fix — Lock acquisition still allows two owners.** [scripts/agent-test.mjs:31](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-test.mjs:31)  
   If A pauses between `mkdirSync(lock)` and writing `owner`, B reclaims the unsigned directory after one second and acquires it. A then resumes, overwrites B’s owner file, and also returns successfully. Both runs can empty the same test stores. Reproduced with the actual lock function and a shared-memory filesystem probe. **Fix:** publish the directory and owner atomically; replace the same-process caps race with a separate-process test covering interrupted acquisition.

2. **Must-fix — Unsupported authored drags pass silently.** [scripts/agent-test.mjs:86](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-test.mjs:86)  
   An iOS drag on an ordinary scroller or reorder target can return `delivery: "unsupported"`. `dragTap` returns that reply normally, and `runTests` discards it. A test containing that drag reports success without performing it; subsequent expectations can also pass against unchanged state. The in-memory carrier probe returned **1 passed, 0 failed**. **Fix:** treat unsupported delivery and error replies as failed steps, preserving their reason.

3. **Should-fix — Reveal invalidates explicitly positioned gestures.** [scripts/agent.mjs:987](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent.mjs:987)  
   Reveal always targets the node’s center, ignoring `drag.from` and `down.at`. On a 2,000-point canvas in a 900-point viewport, `tap canvas drag 100 0 from 20 20` initially has a visible path. Reveal scrolls the page by 550 points, moving that start to `(20, -530)`; the previously valid drag now fails viewport validation. **Fix:** reveal the requested contact point when an explicit offset exists, and leave scrolling unchanged when that point is visible.

4. **Should-fix — The new JS control path edits readonly fields.** [host/web-js/agent.js:252](/Users/ccheever/projects/exact2-wt-gaps/host/web-js/agent.js:252)  
   `type` now routes readonly date/time controls through `typeControl`, whose guard checks disabled and inert but ignores `readOnly`. It assigns the value and fires `input` and `change`, allowing application mutations that a user cannot trigger through that field. The function probe confirmed both events on a readonly date input. **Fix:** refuse readonly date, time, and datetime-local edits before assigning or dispatching events.

Verification used the existing compiler and in-memory probes. Fresh browser/native runs were blocked because the sandbox also denies temporary-directory writes. No repository files were changed.

Verdict: DO NOT LAND
---

## r5

- **4a.1 — fixed:** Module-wide claiming dispatches the follow-up fetch after the original owner settles.
- **4a.2 — fixed:** Capture excludes the resolved output directory, preventing recursive self-copying.
- **4a.3 — fixed:** Submission follows target and ancestor key handlers and honors prevention.
- **4a.4 — fixed:** Window listeners handle deferred contacts; termination removes listeners and stale contacts.
- **4a.5 — fixed:** Cloning defines own data properties, preserving `__proto__` without changing the prototype.
- **4a.6 — fixed:** Both macOS routes dispatch authored handlers before menu/dialog defaults.
- **4a.7 — fixed:** Fulfillment, rejection and forgetting release abort subscriptions.
- **4a.8 — partly:** Native hooks resist suppression; the [browser fallback](/Users/ccheever/projects/exact2-wt-gaps/js/src/prelude.js:413) still remains pending after a suppressed abort until the host replies.
- **4a.9 — fixed:** Typed-array cloning selects captured intrinsic constructors instead of application constructors.
- **4a.10 — fixed:** `toSorted()` reads every index into an ordinary array, including holes.
- **4b.1 — fixed:** Per-run store names eliminate the shared-store acquisition race.
- **4b.2 — fixed:** Unsupported delivery and error replies fail input steps with their reasons.
- **4b.3 — fixed:** Explicit contact offsets bypass reveal, preserving the reported visible gesture.
- **4b.4 — fixed:** Readonly date/time controls refuse edits before assignment or event dispatch.

New defects introduced by the fixes:

1. **Should-fix — Concurrent simulator suites terminate each other.** [scripts/agent-test.mjs:59](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-test.mjs:59). Removing serialization allows two suites for the same app and simulator to launch concurrently. The second launch uses `--terminate-running-process`, killing the first suite’s app. Unique storage names cannot isolate that process. **Fix:** serialize simulator sessions by device/bundle, or allocate separate simulators.

2. **Should-fix — Simulator test stores accumulate indefinitely.** [scripts/agent-test.mjs:24](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-test.mjs:24). iOS returns no cleanup base, while every run now creates new store names. Ordinary simulator installation preserves the app’s data container; closing the session does not uninstall it. Repeated suites therefore retain every run’s databases and files. **Fix:** resolve the simulator container and clean completed/abandoned test stores, or provide host-side cleanup.

3. **Should-fix — Cleanup ignores the host’s environment overrides.** [scripts/agent-test.mjs:53](/Users/ccheever/projects/exact2-wt-gaps/scripts/agent-test.mjs:53). With `env: {XDG_CACHE_HOME: "/tmp/suite-cache"}`, Linux creates stores there, but cleanup and sweeping use the driver’s cache directory. The new unique stores remain behind. **Fix:** derive the cleanup base from the effective launch environment, including its home directory.

4. **Should-fix — Readonly guard rejects editable checkbox/range controls.** [host/web/navigation.js:822](/Users/ccheever/projects/exact2-wt-gaps/host/web/navigation.js:822). Accepted controls such as `input type="range" readonly=true` remain interactive in HTML: `readOnly` reflects the attribute, but it has no effect on these types. The new guard nevertheless refuses agent edits. **Fix:** apply the guard only to input types that support readonly.

Verification: six focused tests passed; in-memory probes checked the fixes and cleanup paths. Native routing and simulator lifecycle were inspected, not launched. No files changed.

Verdict: LAND WITH FIXES