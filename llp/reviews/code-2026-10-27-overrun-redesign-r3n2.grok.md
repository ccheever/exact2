# Code review: the Overrun re-ask redesign, narrow re-check 2, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `ebb8c89664`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

CLEAN.

The re-check 1 stall is fixed, and the new gate does not let a later `then`, `next`, or answer pass a re-ask that will actually dispatch.

`order` at `runner/src/runner/advance.rs:326` is `earliest` while timers run and `reask` while they are held. `ready` (`advance.rs:327`) is what admits `next` (`:348`), answer (`:350`), and `then` (`:351`). The hold still fires only when `timers` is set (`:484`).

With timers held, `timer_at` and the transport wake are both absent, so `reask` is `earliest` kept only when its time is strictly before a due hold (`:325`). That is the value `order` uses. If `reask` is `Some`, a candidate later than it fails `at <= reask` and is not selected; the re-ask block at `:509` is what runs. A candidate earlier than it runs first, which is deadline order. A tie still runs in the old loop order (answer, then `next`, then `then`, then the re-ask) because the comparison is `<=`, the same comparison the timers-running path uses. Nothing in the held path dispatches outside that gate.

The behavior changes only when the hold is due and the earliest re-ask is at or after it. Then `reask` is `None`, so it does not dispatch, and `order` is `None`, so it no longer blocks the hidden invocation's `then`. The hold itself also waits. That is the stall from re-check 1 (hold 1000, re-ask 1250, `then` 1300).

With timers running, `order` stays `earliest`, so round 3 is unchanged: the hold is `ready` because it is not after the re-ask, the `then` after the re-ask is not, and the loop fires the hold, then the re-ask, then the later work.

With timers held and no due hold, the hold filter keeps every `earliest`, so `order == earliest` and the gate matches the previous one. `land_then`, an unseen wake, and a hidden call all enter `advance_steps` with `timers` false (`:207`, `:220`, `:230`); there is no second ordering site.
