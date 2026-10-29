# Code review: web clock settle waits for a glide, narrow re-check 2, 2026-10-28 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `8240a92223`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (agent tooling on the web hosts, theme2's 03:42 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's narration before its verdict is left out.

**CLEAN.** The extra probe does not create a new wrong settle.

The second `glidesSettled` runs only on a round that was about to return settled: the clock is at rest and nothing is pending, and on the JS host the epoch is unchanged too. A quiet pair still returns true before the deadline check, including when `budget.ms` is already 0, and only time inside that call is subtracted. No new scroll costs two frames of the 4 s and the round ends. A scroll seen there is waited out once, until two quiet frames or the budget — the wait the next round's opening call would have done — and then that next round runs. A resting scroller is not turned into `reason: 'scroll'`, and a glide is not charged twice.

On the JS host, `continue` at `host/web-js/agent.js:501` stays inside the `for` and comes after this round's `advance`. The next iteration waits for the glide, then snapshots `scrollsFired` into `lastRound`, then advances, so handlers that ran during the probe are the baseline and what they armed runs in the new round. Falling out of round 15 still hits the existing `scroll-event` check at `host/web-js/agent.js:512` before the new `moved` return at `host/web-js/agent.js:514`. A later round that breaks with `quiet` still answers settled. `seek` is skipped only when this round did not commit; a commit during the probe already seeked from `After`.

On the wasm host, `moved` at `host/web/glue.js:1252` is per iteration. The helper runs only in the quiet arm; `?? true` applies when it is missing, and a real `false` still answers `reason: 'scroll'`. The fall-through is the old loop. `rounds >= 15` at `host/web/glue.js:1253` uses that iteration's `moved`, so an earlier scroll does not relabel a later non-scroll exhaustion. Rounds that used to answer `settled: true` while a scroll was still one frame out now take another round; that is the miss this fix closes. A probe that sees no new event still returns on that round.
