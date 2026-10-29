# Code review: LLP 1146 D2, an aborted kept write in a module turn, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `c193d3dc84`.
- **Method:** one brief (sha256 `9da4555bae08a16be3e13ff2900a8ab5348a5a0cf16028d5a3ec10c491a101bb`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

1. `js/src/adopt_tests.rs:120-127` — `an_adopted_fetch_ends_with_its_instance` can pass without an abort ever being recorded. `unload` (`js/src/lib.rs:712-735`) always drops the engine, clears `host.aborted`, zeroes `module_outstanding`, and clears parked work, so the asserts after it hold for any open call. `calls_open() > 0` is the still-pending `cancelAwait`, and `s.get(KEY) == None` only catches an at-once rejection that wrote the key. Before `unload`, assert that the store is still unset, `host.module_outstanding > 0`, and `host.aborted == [ticket]`. The empty `take_aborted()` after unload then means a recorded abort was cleared.

2. `runner/src/runner.rs:371-372` — The comment says `module_aborts` stays outside rollback because the module has already rejected the fetch. For an adopted fetch the promise is still pending when the ticket is recorded. The exclusion is what lets a refused aborting commit deliver the abort on the next pump. Say that, so a later edit does not move the list into the checkpoint and drop the abort while the server is still holding the request.
