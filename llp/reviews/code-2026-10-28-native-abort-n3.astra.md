# Code review: native abort (a fetch the app aborts is cancelled on every target), narrow re-check 3, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `3a0115739e`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two blockers in `8117d6245f..3a0115739e`.

1. **[P1] The new wasm poll can discard an abort for the fetch being recorded.** `hear_aborted()` runs before insertion into `fetches` ([lib.rs:484](js/web/src/lib.rs:484)), while `note_aborted` discards unmapped tickets ([lib.rs:177](js/web/src/lib.rs:177)).

   Client-reachable sequence: one action starts two sources. A awaits a POST; B aborts A’s controller. The realm’s promise chain lets B begin after A’s fetch is claimed but before A’s result reaches wasm ([module-glue.js:344](host/web/module-glue.js:344), [glue.js:714](host/web/glue.js:714)). A’s `step` then drains and drops that abort, records the fetch, and sends the POST. An in-memory promise-order trace confirmed this ordering. Preserve unmatched aborts, or install the incoming mapping before draining and pruning.

2. **[P1] An abort matched during a refused commit can strand restored pending work.** A mutation assignment temporarily removes and retires its request **before settlement** ([commit.rs:580](runner/src/runner/commit.rs:580)). A native source queried during settlement can abort that POST; the runner immediately hears it ([query.rs:104](runner/src/runner/query.rs:104)) and moves the retired match directly into `explicit_aborts` ([admission.rs:133](runner/src/runner/admission.rs:133)).

   If a subsequent derive or gate refuses the commit, rollback restores the pending request ([commit.rs:180](runner/src/runner/commit.rs:180)). Its abort has already been consumed without queuing an answer. Apple/Linux cancel its transport and discard its completion ([executor_forget.rs:75](host/apple/src/executor_forget.rs:75), [executor_core.rs:951](host/apple/src/executor_core.rs:951)), leaving it pending indefinitely. Pending-first matching only protects aborts heard **after** restoration. Defer the “no answer owed” decision until the pump checks final pending state.

For the requested checks:

- **(a)** The specific already-recorded, same-key replacement is repaired: realm door 22 retains the ticket ([module-glue.js:135](host/web/module-glue.js:135)); the adapter maps it without consulting waiting state ([lib.rs:180](js/web/src/lib.rs:180)); the retired match produces cancellation; web completion emits `batch.abort` ([complete.rs:58](host/web/src/host/complete.rs:58)); glue aborts that ticket’s controller ([glue.js:713](host/web/glue.js:713)). Finding 1 is another ordering that still loses cancellation.
- **(b)** No different-request cancellation found on supported reload paths. Runner tickets advance without rollback ([commit.rs:866](runner/src/runner/commit.rs:866)); fresh runners clear all abort/retired queues ([runner.rs:865](runner/src/runner.rs:865)); module unload clears fetch mappings and aborts ([lib.rs:711](js/src/lib.rs:711)). A request present in both tables takes the pending branch. Finding 2 concerns restoration after matching.
- **(c)** No wake loop found. Explicit aborts are drained by Apple emit ([abi.rs:257](host/apple/src/abi.rs:257)), Linux’s normal and empty-pump paths ([presenter.rs:812](host/linux/src/presenter.rs:812), [pump.rs:44](host/linux/src/presenter/pump.rs:44)), web completion, and render settlement ([requests.rs:50](host/render/src/requests.rs:50)). Render repeats only when an answer was fulfilled.
- **(d)** Both findings above arise from the incremental changes.

No files changed; no builds or repository tests run.

Verdict: blockers — wasm abort discarded before ticket recording; rollback restores pending work whose abort answer was lost.
