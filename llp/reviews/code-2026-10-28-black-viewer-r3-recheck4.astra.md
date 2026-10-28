# Code review: the black viewer: thumbnail under the picture, picture retries and priority, round 3, re-check 4 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `1421c1821b`.
- **Method:** one brief (sha256 `993b9134f5562eee072f5bdbf8fe01f748f4f3100d4c8043af17ea8d94c0ed7a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/presenter/images.rs:51](host/linux/src/presenter/images.rs:51) — **A→B→A within one advance still preserves a stale load.** `advance_timed` can execute multiple timer/`then` commits before `Presenter::advance` synchronizes images. The final sync sees A unchanged, so neither `assignment` nor queue retention changes. The old event passes validation. Track source revisions per committed change, or synchronize between commits. The forged-assignment test does not cover this case.

2. [host/linux/src/presenter/images.rs:72](host/linux/src/presenter/images.rs:72) — **`dispatch_at` can still commit before the handler despite the deadline check.** On the seekable clock, `gates.rs:129` excludes module-poll deadlines, but `advance.rs:790` still runs polls. For example, a refusing timer can return before polling, leaving an overdue poll behind after that timer is consumed. Validation then passes, and dispatch’s advance can change the image source before delivering its old `Load`. Perform the advance before validation and dispatch without another advance, or validate inside dispatch after advancing.

3. [host/linux/src/presenter/images.rs:54](host/linux/src/presenter/images.rs:54) — **The moving cutoff can starve loads indefinitely.** Two recurring timers whose callbacks each exceed their interval can continually make each other overdue at the newly sampled time. Every turn exhausts eight rounds and defers the load again; an immediate wake does not ensure progress. Drain against a fixed cutoff, then validate and dispatch without another implicit advance.

**Non-blocking**

none. The host/runner maximum addresses the reported clock-lag case, and the `cool` change fixes expired-entry protection.

Static review only; no tests or builds run.
