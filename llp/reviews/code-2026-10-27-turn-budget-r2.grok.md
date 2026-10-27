# Code review: the data-module turn budget in the calling thread's CPU time, round 2 of 3, 2026-10-27 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `4aabdb2324`.
- **Method:** one brief (sha256 `cb6ba7bc5fc75d9720d99641685335ab1b07611bc840492cc50199a860c39b21`), blind to the other reviewer; 3 rounds (runtime, client-reachable), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Grok's progress narration before the report is left out.

**CLEAN.** Round 2 of `x2/turn-limit` (the four commits since `c540dae0`, the merge-base with `origin/main`; `HEAD` is `4aabdb23`) has no runtime or client-reachable defect. The round 1 fixes hold, and `judge` is consistent at all four sites.

**Round 1 fixes**

- A zero budget with a coarse CPU reading of 0 still uses the budget sentence when any wall time passed (`js/src/turn.rs:108`). `0.0` formats as `0`, so `pure.rs`, `caltrain.rs`, and `castle.rs` still match `over the 0 ms budget`. A snapshot deadline is decided first (`turn.rs:99`) and stays on the wall clock.
- The CPU-clock test works until the clock reads 20 ms, and stops at 30 s of wall (`turn.rs:223`). If that cap hits while the clock is live and still under 20 ms, the assertion fails closed rather than passing a clock that never moved.
- `js/src/lib.rs` ends at line 1487.
- The hold line is written in release on the slow-load channel: `TurnClock::judge` pushes into `Module.logs` (`turn.rs:71`), the same vec as the slow-load line (`lib.rs:505`). The runner takes those lines after an answer (`runner/src/runner/commit.rs:1306`) and stamps them into the agent journal. `DataSource::take_logs` prefixes both with `console:` (`js/src/background.rs:192`). Same channel, including release. Source names are plan identifiers, so the new line does not give a module a journal-injection path.

**`judge` at the four sites**

| Site | Clock / judge | On overrun |
|---|---|---|
| `begin` (answer) | `lib.rs:923`, `lib.rs:950` | `overruns`, `finish_let_go`, `over_budget` (yield hint) at `lib.rs:989` |
| `overlay` | `lib.rs:1112`, `lib.rs:1121` | `` `{source}`'s overlay {why} `` at `lib.rs:1128` |
| `resume` | `lib.rs:1207`, `lib.rs:1255` | same as `begin`, at `lib.rs:1271` |
| `message` | `lib.rs:1348`, `lib.rs:1385` | `` `{source}` {why} `` at `lib.rs:1400`, `overruns` incremented |

The clock starts immediately after the deadline remainder is taken from `started`, at every site. `deadline` is `snapshot_deadline.is_some()`. `main_thread` defaults to true (`lib.rs:373`) and is cleared only for a worker (`lib.rs:441`), which matches the 250 ms test. An interrupt is still taken before the budget error in `begin` and `resume`. The returned `Option<String>` is owned, so the `&mut self.logs` borrow ends inside `judge`. The hold line is pushed only when `over` is `None` (`turn.rs:71`), so a budget `Err` does not record a hold for a turn the budget rejected.

In-tree matchers still match the new sentences: `over the 0 ms budget`, `over the 100 ms budget for one turn`, and the yield hint on `begin` / `resume`. The message and overlay paths never carried that hint. Nothing in the tree matches the old deadline wording.

**Tests and a module**

The 250 ms sleep sits under the 1000 ms backstop with a wide margin, and `thread::sleep` does not return early or charge the calling thread 100 ms of CPU. The 1100 ms sleep is over the backstop on the wall clock, so the refusal is the backstop sentence. The 1000-iteration accumulator `496491` matches `7 × 499500 mod 1000003`. The 30M loop is the same shape as the existing 12M walk, which is already treated as several times a 100 ms turn, and the assertion accepts both the CPU sentence and the no-clock wall sentence. `busy` exists only on the storage fixture.

A module cannot set `budget_ms`. Yielding still starts a new turn with its own budget, as it did when the limit was wall time. Waiting is bounded by ten budgets (`turn.rs:121`); past that the turn is refused. The interrupt is still the only thing that stops a call that does not return.
