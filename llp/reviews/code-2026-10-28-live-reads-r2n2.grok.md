# Code review: the clone's live reads on a local server, narrow re-check 2, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `558e9e41d3`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (test only), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

CLEAN

The waiting fetches in these tests all sit inside one `s.op({ op: 'clock' })`: `clock data`, `clock settle` and its follow-up clocks, a jump that stops on an in-flight send, reload's storage `clock data`, and the harness's mutation `clock +N`. An input's land goes through `carrier.ask({ op: 'clock', land: true })` and returns after settled `then`s; it does not block on the network, and reload's new page only waits for boot, so those paths neither release an answer nor hang. `done()` at `scripts/live-reads-fixture.mjs:265` clears every timer in one synchronous loop, so a timer queued for the same moment never runs; a callback that already ran has already dropped its answer from `releasing`. A short clock op puts the answer back in `withheld`, and the next clock op starts a fresh 300 ms timer. `clock data` and `clock settle` are each a single `s.op` that stays open until the fetch returns, so the timer fires inside that op. The split `clock + 61000` either waits in the step that fires the 60 s poll or leaves the answer for the following `clock data`. A clock op that returns before 300 ms ends with the answers still withheld, including the state read `s.clock` does after `s.op` returns.
