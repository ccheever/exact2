# Code review: the data-module turn budget in the calling thread's CPU time, round 1 of 3, 2026-10-27 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `174a8c3b6b`.
- **Method:** one brief (sha256 `55700ed545cf6f6bf449d2b1ac334dac555ff29c43815f414b43b6af26872245`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

No runtime **BLOCKER** found in `HEAD~3..HEAD`. Four **SHOULD** findings:

- **SHOULD — [js/src/turn.rs:82](js/src/turn.rs:82): Zero-budget messages break existing Windows tests.** A short turn can record `own_ms == 0` with positive wall time. With budget zero, it reaches the backstop message, which lacks `"over the 0 ms budget"` required by the caltrain, pure, and castle tests. Preserve that wording and cover the zero-CPU case.

- **SHOULD — [js/src/turn.rs:178](js/src/turn.rs:178): The CPU-clock test can fail under scheduling load.** Stopping after 40 ms of wall time does not guarantee the asserted 5 ms of CPU work. Descheduling can exhaust the window before meaningful work runs. Use CPU progress or bounded work, with a separate generous wall timeout.

- **SHOULD — [js/src/lib.rs:698](js/src/lib.rs:698): The approved DEV diagnostic also runs in production.** Neither `held` nor its callers check development mode; the log-draining and journal paths are also unconditional. Release turns therefore allocate and retain these messages. Gate their emission on development mode.

- **SHOULD — [js/src/lib.rs:1527](js/src/lib.rs:1527): The source-size check fails.** `caps` reports 1,559 lines against the 1,500-line limit. Extract helpers or test modules.

All four existing budgeted paths use the new clock. JS execution and resumed continuations remain on their owner thread; snapshot enforcement retains an absolute wall-deadline check; infinity remains unlimited; the interrupt guard is unchanged. The inspected Windows bindings and Unix dependency declarations match the implementation.

Tree unchanged. Rust tests were not run in the read-only environment. `caps` ran with its compilation step disabled and reported the size violation above.
