# Code review: the Linux and Android scrub trace, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max, sandbox read-only, a throwaway worktree.
- **Scope:** the trace commit as first written.
- **Method:** one brief (sha256 `74b2a53d85f63c445512506f46729c42d5813bed6b80e829ac8012f001c6db81`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (the tail never flushed, made-up times, dropped revisions, a queued stream, the instrument's own time, a panic under the lock).
- **Disposition:** fixed in round 1's commit (see its message).

---

1. **Should-fix — [preferences.rs:264](/private/tmp/rv-tr-1/host/linux/src/presenter/preferences.rs:264): final records never flush.** Linux never calls `scrub_frame_drawn`; four applied scrubs produce no output, and longer runs retain their final one to four records indefinitely. Android has the same problem for native-view-only changes or deduplicated canvas output. **Fix:** emit immediately on hosts using the commit-time fallback, and flush Android’s no-frame cases at an explicit turn boundary.

2. **Should-fix — [preferences.rs:266](/private/tmp/rv-tr-1/host/linux/src/presenter/preferences.rs:266): queue overflow fabricates `turned` timestamps.** If one development poll applies five scrubs before Android takes a frame, the fifth logs the preceding four with `turned = applied`, although their frame hand-off has not occurred. This falsely reports zero hand-off latency. **Fix:** distinguish hosts without frame callbacks explicitly; queue length must not select that fallback. Report overflow explicitly if storage fills.

3. **Should-fix — [preferences.rs:115](/private/tmp/rv-tr-1/host/linux/src/presenter/preferences.rs:115): ignored revisions are logged as applied.** The runner returns `Ok(nothing())` for duplicate or older scrub revisions ([live.rs:110](/private/tmp/rv-tr-1/runner/src/runner/live.rs:110)). Consequently, replayed batches or late snapshots pass this success check and generate duplicate or false application records. **Fix:** propagate whether the runner accepted the scrub revision and trace only accepted revisions.

4. **Should-fix — [jni.rs:447](/private/tmp/rv-tr-1/host/linux/src/canvas/jni.rs:447): an older queued stream can flush newer scrub records.** With the agent enabled, `Native_frame` can return an earlier stream from `canvas::agent::next_stream`. If a development poll has since applied a scrub, this call drains its record even though the returned stream predates that scrub. **Fix:** associate trace records with the generated stream or presentation generation, and drain only records covered by the stream being handed off.

5. **Should-fix — [preferences.rs:251](/private/tmp/rv-tr-1/host/linux/src/presenter/preferences.rs:251): `applied` includes instrumentation work.** The timestamp is captured after reparsing and allocating the entire edit solely for tracing. Large snapshots therefore inflate application duration and understate commit-to-hand-off time; Apple captures `applied` before extracting trace metadata. **Fix:** capture it immediately after `after_commit()` and pass it into `trace_scrub`.

6. **Should-fix — [preferences.rs:283](/private/tmp/rv-tr-1/host/linux/src/presenter/preferences.rs:283): logging can panic while holding `PENDING`.** A failing stderr sink, such as `/dev/full` on Linux, makes `eprintln!` panic. Across the JNI boundary this aborts; if unwinding is caught elsewhere, the poisoned mutex makes subsequent trace calls silently return. **Fix:** use non-panicking writes, perform I/O outside the queue lock, and handle poisoned state explicitly. The fallback print at line 266 needs the same treatment.

The disabled steady-state path adds only a cached flag check; the initial call reads the environment once. No added parsing, clock reads, mutex locking, or allocation occurs thereafter. `PENDING` stays bounded at four entries. Android serializes application and hand-off on the host thread. The public `$crate` path is valid by inspection. Apple’s field labels, ordering, and Unix-millisecond units match; Rust rounds to 0.1 ms.

Source review only; no builds or tests were run under the read-only constraint.
