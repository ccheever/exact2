# Code review: Linux and Android store writes on a thread of their own, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a6e4ba1244`.
- **Method:** one brief (sha256 `391b56ac01a6aecb86ae36e763b376d13e57c74ca450048ff5ef8c9e64d0e178`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **[P2] Some Windows exits bypass persistence draining** — [host/windows/src/window.rs:719](host/windows/src/window.rs:719). The drain runs only for `CloseRequested`. The bounded frame run (line 495) and `App::fail` exit without it; `run()` then returns to `process::exit`. Writes queued by the final commit can therefore disappear on these controlled exits. Move the bounded drain and warning collection into common cleanup after `run_app`, or an `exiting` callback.

**Non-blocking**

1. **The roots regression test assumes Unix storage paths** — [store_writer_tests.rs:301](host/linux/src/store_writer_tests.rs:301). Windows ignores the changed `HOME`/XDG variables and uses LocalAppData, so the empty commits have identical roots, collapse legitimately, and `written(3)` times out. Gate this test with `#[cfg(unix)]` or adapt its root selection for Windows.

All three round-2 blockers are resolved by inspection. No tests or builds were run.
