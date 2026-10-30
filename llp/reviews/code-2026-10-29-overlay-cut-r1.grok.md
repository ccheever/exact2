# Code review: the exact-js overlay cut, round 1 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `0dd528d127`.
- **Method:** one brief (sha256 `3c1ceb6d3c692a806892749f7153eda41a7351dfedbe2b8eb9f3f6cda1039316`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The native no-overlay path now returns the same `Ok(None)` that `{tag:4}` used to mean, and the Hermes tests go red if that early return is removed. The wasm probe does not.

`VERDICT: APPROVE`

1. **MINOR** — A failed `has-overlay` probe is treated as “no overlay”. `js/web/src/lib.rs:561`. `call` yields the glue JSON as success, and `moduleCall` turns a throw into `{"error":...}` (`host/web/glue.js:157`). `has["has"] != true` then returns `Ok(None)`, so a missing realm, a missing `__exact_has_overlay`, or a throwing `exact` / `overlay` getter never becomes the `Unavailable` the old overlay call produced. Native still does (`js/src/lib.rs:1139`). `Runner::shown` shows the answer and retires unkept writes for both `Ok(None)` and `Err`, so the screen matches; the journal line is dropped, and a probe that throws on a module that does have an overlay skips it. Treat an `error` field, or a missing `has` boolean, as a failure. Nothing in `host/web` or `js/web` covers this path.

2. **NIT** — A getter-backed `exact.overlay` runs twice whenever it returns a function. `js/src/prelude.js:1393` and `js/src/prelude.js:1397`. The first read is outside the overlay turn clock and does not clear `initializing`; the second is the existing `typeof` inside `__exact_overlay`. A pure getter is fine. One with side effects, or one that does not return the same value twice, is not the old single read.
