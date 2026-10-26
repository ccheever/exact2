# Code review: the agent's settle waits for a list's owed scroll events (conformance's startend boot race), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `9deb6cfc1b`.
- **Method:** one brief per round (sha256 `d73970a16846d29ce2497e5221d3a247d1754c75cee69118664bd48c86630a2b`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** SHOULD-FIX: descendant events; round 16; device holds before scroll on wasm; the diagnostic
- **Disposition:** taken in a2eff5013d and 614159b9e1

every `scroll` that reaches the capture listener. It never looks at `event.target`. Capture on `document` (the viewport port, `:468`) runs for every scroller in the document; capture on an overflow port runs for a descendant scroller too. Scroll events do not bubble, and they do go through capture.

That reopens the race fix 1 closed. `place`/`move` sets the mark, the port's own event runs, the reader's handler jumps and `move` sets the mark again, then a later `scroll` in the same rendering update hits this listener and clears it. `fireOwedScrolls` is awaiting `requestAnimationFrame`, so it observes the cleared mark, returns `{ fired: true, owed: 0 }`, and the next settle round does not wait. The jump's event is the next frame. The new test (`collection.test.mjs:1492`) uses one `own: true` port and no nested scroller, so it stays green. The ack should ignore an event whose target is not that port, and not `document` for a viewport port.

**`host/web-js/agent.js:387`** — `continue` on `fired || owed` skips the clock advance for that round. Rounds 0–14 recover on the next iteration. Round 15 (`round < 16`) leaves the loop. After the loop, only a mark that is still set becomes `reason: 'scroll'` (`:413`). `fired && !owed` on that last round — the handler ran, did not jump, and armed a timer or a `then` — falls through to `{ settled: !exact.inflight.n }` (`:414`) with the advance skipped. A request the handler sent synchronously still shows up as in flight; a timer does not. The wasm loop advances at the top of the next iteration, and its bound returns `reply(false)` (`glue.js:1253`), so the same chain fails closed there.

**`host/web/glue.js:1252`** — on the quiescent path the scroll check returns before device holds. The JS host does the opposite (`agent.js:411`, then `:413`). Rounds 0–14 `continue` while a scroll is owed, so this return is the round-15 skip, or a mark set during the `imageHold` await just above it. Either way a held device request loses its `tickets`. The line's own comment is that a hold is never waited on; this reply does not name the hold. A stuck mark makes every later wasm settle say `scroll` and the tickets stay hidden. JS would say `device` first.

**`scripts/agent.mjs:1232`** — `r.reason === 'scroll'` is mapped, for every host, to "waited 4 s for a scroll view a finger left moving (a fling's glide, a bounce)". That string matches `Agent.swift:815` (`awaitScrollRest`). The web reason is an owed list `scroll` event still outstanding at the round bound (16× up to 8 frames on JS, 15 on wasm), with no finger and no 4 s wait. Nothing else branches on the reason. An agent that follows the diagnostic settles again and looks for a fling. `layout` is the wrong next read; the mark is the list whose event never arrived. `landed()` does not see this reason: it copies `clock land`, and land returns before the scroll wait.

## NIT

No separate nits. The wasm round-15 skip of `fireOwedScrolls` matches the stated bound and fails closed.
