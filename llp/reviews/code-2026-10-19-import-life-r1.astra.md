# Code review: Tamber imports' lifecycle leftovers, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `4fe3a693be`.
- **Method:** one brief (sha256 `62b056046f0b18c7b5b209e2f6aac936af723aeb1b82df3dc7955f0022cfa67f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking (correctness or security)**

- [scripts/tamber-isolate.mjs:82](/tmp/rv-il-1/scripts/tamber-isolate.mjs:82): Cancellation releases admission counts, but the cancelled callback and request payload remain in `last` until preceding work finishes; cleanup waits until line 99. A client can keep one costly planner running and repeatedly enqueue/abort large requests, bypassing the queue’s memory bound. Remove cancelled entries and their payloads immediately.
- [scripts/tamber-server.mjs:1136](/tmp/rv-il-1/scripts/tamber-server.mjs:1136): Cancellation stops working once planning finishes. Revocation or disconnect while `hold()` awaits the shared server can still create a proposal and return 200. Carry cancellation through the hold stage and check it before insertion.
- [contract/cli/src/tamber_export.rs:182](/tmp/rv-il-1/contract/cli/src/tamber_export.rs:182): Failed reads consume no budget. Invalid UTF-8 makes `read_to_string` return `Err` after reading, leaving `left` unchanged; sixteen malformed files can consume roughly 1 GiB instead of 64 MiB. Charge consumed bytes on errors too.

**Should-fix**

- [scripts/tamber-isolate.test.mjs:113](/tmp/rv-il-1/scripts/tamber-isolate.test.mjs:113), [scripts/tamber-reimport.test.mjs:190](/tmp/rv-il-1/scripts/tamber-reimport.test.mjs:190): Fixed sleeps do not establish that the planner started before cancellation. Await its spawn/start and assert its exit. The new assertions meaningfully catch basic regressions, but short-read/growth/deadline behavior and the baseline byte cap remain untested.

**Nits:** None.

The release guard, client-specific callbacks, normal-completion `writableEnded` check, and directory symlink checks otherwise look sound.

Both permitted test commands were blocked by sandbox `EPERM` errors before test execution; these findings are from static review.
