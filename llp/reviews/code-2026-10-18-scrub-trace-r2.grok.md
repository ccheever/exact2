# Code review: the Linux and Android scrub trace, round 2, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, no plan, a throwaway worktree.
- **Scope:** the trace commit and round 1.
- **Method:** one brief (sha256 `ab10f2b1fa09983dd3361b9a0da8399a01363622f29c839f957823673194e941`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT (should-fix: a line's age, the display frame's flush; a nit).
- **Disposition:** fixed in round 2's commit (see its message).

---

1. **should-fix** — A queued scrub line can be dropped, or stamped long after it was drawn. `host/linux/src/presenter/preferences.rs:292` and `:308`.
   `turned none` is only decided when a later scrub arrives, and only for entries already older than 34 ms. `scrub_frame_drawn` then stamps whatever is left with that callback's time, with no age check. Stop the activity mid-drag (poll still applies; `produce` returns before `Native_frame`, and the vsync callback is removed): lines older than 34 ms become `none` only if another scrub shows up, and the newest line stays in `PENDING` until the next callback. Resume a second later and that line is printed with `turned` equal to the resume time. If nothing calls `scrub_frame_drawn` again and no later scrub arrives, it is never printed.
   **Fix:** keep the time of the last `scrub_frame_drawn`. In `trace_scrub`, if that time is missing or more than 34 ms behind `applied`, write the line as `turned none` immediately. In `scrub_frame_drawn`, split the queue and write the same `none` for any entry with `turned - applied > 34`, and stamp only the fresh ones.

2. **should-fix** — `Native_displayFrame` attributes a draw before this turn is known to be the host's own. `host/linux/src/canvas/jni.rs:845`.
   The callback always flushes, and the reader calls it before `Native_frame`. `Native_frame` then prefers a primed or agent stream (`own` is false) and does not paint. A scrub applied on the previous poll stays queued because that `Native_frame` skipped the flush; the next vsync's `displayFrame` prints it with `turned` set, and the reader still takes the old agent stream. That is the "stream queued before" case this commit says it does not stamp.
   **Fix:** have `displayFrame` only set `FRAMES`. Flush from `Native_frame` when `own` is true (including when the stream is null). The same turn's `produce` / `pump` still calls `Native_frame` after `displayFrame`, so a view-only change is still stamped.

3. **nit** — Every traced frame locks stderr even when there is nothing to print. `host/linux/src/presenter/preferences.rs:265` (called from `:308`).
   With the variable set, each display frame and each own `Native_frame` takes the stderr lock and returns. That lock is the logcat pipe.
   **Fix:** return before `stderr().lock()` when `lines` is empty.

What holds:

- Unset, the steady path is `OnceLock::get` (an atomic load). No parse, no `design()` clone, no mutex, and `Native_frame` still returns the same buffer. The env read happens once.
- `apply_live` and `Native_frame` run on the thread that called `start` (the owner thread, or the main thread when `owner=0`). The presenter is that thread's `HOST`. `PENDING` is uncontended there. A poisoned mutex is recovered with `into_inner`. The queue stays about 34 ms deep while scrubs keep arriving. A line is either printed immediately or queued, not both.
- A dropped older revision does not change `Runner::design`, so it produces no line. `applied` is sampled after `after_commit` and before `Edit::parse`.
- `$crate::presenter::scrub_frame_drawn` is the public re-export. The macro expands in the app crate and resolves back to this crate on Android.
- No `unwrap` on the trace path. `Edit::parse` returns `Option`. `unix_ms` maps a pre-epoch clock to `0`. `writeln` errors are ignored. Stderr's lock is reentrant.
- The line matches `PlanURL.swift`: `exact scrub: rev … received … applied … turned … value …`, value last. Apple's `print` uses Swift's default `Double` text; this forces one decimal place, which a float parse accepts. `turned none` is the extra token Apple does not emit.
