# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 2, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `8117d6245f`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Both named blockers are fixed. The native parked-only table is sound on the inspected paths, and the extractions preserve runtime behavior. I found **one remaining wasm blocker in the whole change, already present at `b85eba31b9`**, rather than introduced by this delta.

**(c) Remaining blocker: replacing a wasm answer loses the old fetch’s abort.**

Repeat the same mutation/source/arguments while its POST is held, with the replacement source calling `previousController.abort()` before fetching again—the existing [holding pattern](js/tests/fixtures/castle.ts:158).

The replacement initially returns a continuation and sets `waiting[key] = true` ([js/web/src/lib.rs:362](js/web/src/lib.rs:362)). The runner replaces the old POST ticket before that JavaScript turn executes ([commit.rs:854](runner/src/runner/commit.rs:854), [glue.js:714](host/web/glue.js:714)).

When the source aborts the old fetch:

- If collection happens before the replacement turn finishes, `note_aborted` removes the mapping but discards the abort because `waiting[key]` is `true` ([js/web/src/lib.rs:180](js/web/src/lib.rs:180)).
- If the replacement fetch’s result arrives first, checkpoint parsing removes that waiting entry, and `step` prunes the old fetch mapping before recording the replacement ([js/web/src/lib.rs:296](js/web/src/lib.rs:296), [475](js/web/src/lib.rs:475)).

There is also no remaining runner mapping: `match_aborts` searches only current pending requests ([admission.rs:117](runner/src/runner/admission.rs:117)). Consequently no abort operation reaches the old POST’s controller. Ordinary retirement cancels only GET/HEAD, so the POST continues running ([glue.js:754](host/web/glue.js:754), [954](host/web/glue.js:954)). Fixing this requires preserving the original request identity across replacement, beyond merely removing the `waiting` check.

**(a) The two requested fixes and native lifetime cases check out.**

- **Held fetch / replacement:** `take_aborted` maps queued aborts **before** pruning, and retains every still-parked fetch without a cap ([abort.rs:13](js/src/abort.rs:13)). Replacement removes the old parked call without deleting its fetch mapping, then forgets it before returning ([lib.rs:1074](js/src/lib.rs:1074)). Thus an abort made in that replacing turn survives. Forgetting settles the old promise and removes its abort subscription, preventing a later door-22 abort from that fetch ([prelude.js:504](js/src/prelude.js:504), [1292](js/src/prelude.js:1292)).
- **Drain before turn completion:** not against the native `Module` table on these paths. Inline collection follows `answer_for` ([queue.rs:399](runner/src/runner/queue.rs:399)). Worker collection follows `turn`, or follows `forget` for retirement ([placed.rs:372](data/src/placed.rs:372), [416](data/src/placed.rs:416)). An earlier host drain reads the proxy’s separate queue ([placed.rs:563](data/src/placed.rs:563)). Wasm turns can span tasks; that distinction matters to the blocker above.
- **Unload:** clears parked calls, fetch mappings, unsent requests and queued aborts before ticket numbering can restart ([lib.rs:711](js/src/lib.rs:711)).
- **Continuation parking:** targeted continuations deliberately preserve the existing parked call until runner retirement decides ownership ([lib.rs:1074](js/src/lib.rs:1074)). Resumption removes the completed fetch’s mapping; a subsequent fetch gets its own mapping. Continuations and streams do not acquire fetch entries ([lib.rs:1233](js/src/lib.rs:1233), [1367](js/src/lib.rs:1367)).

The scratch-context wake defect is fixed: door 22 now wakes unconditionally ([module-glue.js:135](host/web/module-glue.js:135)), including storage delivery under `scratch` ([216](host/web/module-glue.js:216)). The page coalesces calls while one collection timer is pending ([glue.js:1373](host/web/glue.js:1373)).

**(b) Move audit**

I compared the extracted function bodies against `b85eba31b9` in memory.

| Move | Result |
|---|---|
| Linux and Apple pumps | Identical bodies: [Linux](host/linux/src/presenter/pump.rs:12), [Apple](host/apple/src/abi/pump.rs:6). Linux abort tests retain `cfg(all(test, unix))`. |
| Core cancellation methods | Identical bodies. Only `forget` and `cancel` become `pub(crate)`; `forget_with` remains private ([executor_forget.rs:13](host/apple/src/executor_forget.rs:13)). `Core` remains `pub(super)`, so no external API is exposed ([executor_core.rs:211](host/apple/src/executor_core.rs:211)). |
| Executor abort tests | All four tests and the helper match. Parent `cfg(test)` remains; Linux’s helper path is updated ([executor_core.rs:1384](host/apple/src/executor_core.rs:1384), [executor.rs:189](host/linux/src/executor.rs:189)). |
| Render requests | `hand_out` matches; `settle_aborts` preserves statement order, error propagation and loop behavior ([requests.rs:5](host/render/src/requests.rs:5), [lib.rs:795](host/render/src/lib.rs:795)). |
| Web Host methods | Identical bodies. `complete`’s `pub(super)` restores parent access after extraction ([complete.rs:10](host/web/src/host/complete.rs:10)). |
| Web Bridge methods | Identical bodies and public signatures; abort tests retain `cfg(test)` ([requests.rs:6](host/web/src/abi/requests.rs:6)). |
| Glue joins | Behavior equivalent, though not literally whitespace-only: dedupe state moves from `abortWoken` to `wake.due` ([glue.js:1373](host/web/glue.js:1373)). |

`collected_aborts()` is exactly the previous collect-then-test sequence ([admission.rs:151](runner/src/runner/admission.rs:151)). No lost runtime `cfg` or consequential visibility widening found.

Static review only; no builds, tests or file changes.

Verdict: blocker — wasm replacement can discard an explicit abort and leave the old POST running; already present in `b85eba31b9`.
