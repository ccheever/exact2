# Code review: long-poll streams (LLP 1016.000 D8), round 3's re-check 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `0da59c229a`.
- **Method:** one brief (sha256 `10cf7a9522794e5646b8d446b8ac71d5ffc323e4daccbc6953bd3e3f559c1d4f`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

- [host/apple/src/executor_core.rs:456](host/apple/src/executor_core.rs:456): Admission still omits `long_poll_refusal()`. An invalid ordered poll enters the normal queue without raising the refusal barrier. Later ordered work can execute before its failure is parsed; completion order alone does not provide the deadline-refusal fence.

- [runner/src/runner/commit.rs:1111](runner/src/runner/commit.rs:1111): The consumed-refusal rollback blocker remains. Executor completions—and web batch refusals—leave `p.refused` false. If parsing returns a valid fallback whose derive traps, rollback restores the resource ticket, cleanup is skipped, and no completion remains to settle it.

- [host/apple/src/executor_core.rs:1306](host/apple/src/executor_core.rs:1306), [executor_async.rs:1027](host/apple/src/executor_async.rs:1027): Non-stream continuation/storage polls bypass long-poll validation and return `Unsupported`, including background continuations. The renderer consequently fulfills them instead of treating them as refused placeholders ([lib.rs:798](host/render/src/lib.rs:798)).

**Should-fix:** None additional.  
**Nits:** None.

Both permitted tests stopped before assertions because the read-only sandbox denied Cargo writes. No files changed.
