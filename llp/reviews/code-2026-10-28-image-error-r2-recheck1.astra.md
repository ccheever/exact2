# Code review: the image error event on Linux, Android and Windows, round 2, re-check 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `d79b7210ab`.
- **Method:** one brief (sha256 `dff19173615cd456558f8fc23f7aa3aa334c20ff8720d437981b1dee8d274f2a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Signed queries still leak from SVG warnings** — [host/linux/src/image/remote.rs:42](host/linux/src/image/remote.rs:42). An SVG attribute such as `fill="url(https:h/a?hm=secret)"` produces a warning that this sanitizer leaves unchanged: there is no `://`, leading `/`, or leading scheme. [svg.rs:199](host/linux/src/image/svg.rs:199) then logs the signature. Wrapped protocol-relative URLs have the same problem. Recognize URLs inside diagnostic wrappers and add regression cases for both forms. Astra 2 remains unresolved.

**Non-blocking**

none.

The standalone bare-scheme fix, symbol/package resets, and unchanged-source `renew` fix look correct by inspection. No tests or builds run.
