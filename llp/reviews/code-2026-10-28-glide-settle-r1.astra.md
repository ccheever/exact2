# Code review: web clock settle waits for a glide, round 1, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `28ab8d2a32`.
- **Method:** one brief (sha256 `fa3999aa80931259886620e810aa27346800e1757acb68f8f5c998aedee55ced`), blind to the other reviewer; 2 rounds (agent tooling on the web hosts, theme2's 03:42 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **BLOCKER — [host/web/input-glue.js:1102](host/web/input-glue.js:1102): The idle fast path can report settlement before scrolling starts.** After 500 ms idle, schedule the scrolling loop with `requestAnimationFrame(step)` and immediately settle. Scroll events run before rAF callbacks, so the first frame’s scroll write has no event yet. The helper returns true; both hosts can then answer `settled: true` while scrolling continues. I reproduced this using both hosts’ actual settle code with a controlled frame scheduler following [HTML’s rendering order](https://html.spec.whatwg.org/multipage/webappapis.html#update-the-rendering). Require sufficient frame observation after idle and cover this ordering; the fixture’s synchronous `step()` and preceding `home()` mask it.

- **BLOCKER — [host/web/input-glue.js:1097](host/web/input-glue.js:1097): The four-second deadline does not bound the frame waits.** It is checked only after two callbacks arrive. With simulated three-second frames, the actual helper returned false after nine seconds; with callbacks suspended, it remained pending past forty seconds. Race frame waits against the shared deadline. Add stalled-frame coverage and an upper-duration assertion—the current test only requires elapsed time **greater than** 3.5 seconds.

- **NIT — [scripts/agent.mjs:1152](scripts/agent.mjs:1152): The diagnostic describes a different condition.** “No `scrollend` yet” is inaccurate: this implementation ignores `scrollend`, and the continuous-scroll fixture can emit it every frame. Describe continuing scroll events instead.

Review remained read-only. Browser suites were not rerun because dependencies and built artifacts are absent; reproductions used controlled scheduling. The listener is agent-only and idempotent, and `glue.js` remains within the cap at 1,499 lines.
