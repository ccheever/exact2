# Code review: the image error event on Linux, Android and Windows, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d788b85214`.
- **Method:** one brief (sha256 `194f1ec2ac61aa9f2861b14459579302f0e8716fd0b9b8926c50f6be71d313dc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

Round 1 is not fully resolved. Admission-refusal and off-screen decode settlement fixes look correct by inspection; these gaps remain:

1. **Signed queries still leak for accepted URL forms** — [host/linux/src/image/remote.rs:32](host/linux/src/image/remote.rs:32). `https:example.com/a.png?hm=secret` is accepted and normalized by `clean()`, but `without_queries()` leaves it unchanged. On failure, workers log the original source, including `hm`. Strip queries directly from known source strings, and cover these forms in the message sanitizer and its [Apple counterpart](host/apple/Sources/ExactKit/RasterRetry.swift:82).

2. **SVG diagnostic contents bypass redaction** — [host/linux/src/image/svg.rs:198](host/linux/src/image/svg.rs:198). Only `name` is sanitized. Parser warnings quote rejected attribute values and stylesheet selectors verbatim. For example, `fill="url(https://h/x?hm=secret)"` produces a warning containing the signed query. Redact diagnostic contents before logging, including parse errors, rather than sanitizing only the image’s source.

3. **Reassigning an overlong source after a symbol suppresses its error** — [host/linux/src/image.rs:475](host/linux/src/image.rs:475). With separate syncs for `long URL → symbol:… → same long URL`, the symbol branch preserves `view.rejected`. The second URL assignment therefore neither advances `assignment` nor queues an error. Package-source transitions have the same gap. Clear the rejection identity on intervening source assignments, including those early-return branches.

**Non-blocking**

none.

Read-only review; no network, tests, or builds.
