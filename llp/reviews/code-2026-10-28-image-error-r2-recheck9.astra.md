# Code review: the image error event on Linux, Android and Windows, round 2, re-check 9 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `8518133e85`.
- **Method:** one brief (sha256 `2a89d7f1040e9ddc90cadd14a94054cef1da87fd76917d1bda2c69ebe25910dc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. [host/linux/src/image/svg.rs:222](host/linux/src/image/svg.rs:222) — Bundled SVG warnings can still leak query tails. A bundled SVG with `@import url("https://h/a?x=a &hm=secret");` inside a CDATA `<style>` produces a warning containing that URL. `remote::without_queries` stops redacting at the ASCII space, leaving `&hm=secret` in the image log. The source-name replacement cannot remove this embedded URL. Suppress content-bearing diagnostics for bundled SVGs too, or redact complete quoted URLs before logging.

**Non-blocking**

none.

The reported HTTP(S), `data:`, and `app:/` diagnostic leaks are resolved, and Apple’s branch removal is sound. The broader “no query in image logs” guarantee remains incomplete. Static review only; no tests or builds run.
