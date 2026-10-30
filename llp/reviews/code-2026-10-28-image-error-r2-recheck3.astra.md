# Code review: the image error event on Linux, Android and Windows, round 2, re-check 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `eab63da37b`.
- **Method:** one brief (sha256 `70ba80b029a3cf1d62becd98d68e9a1105407b816e52546c3c5ae35876366814`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

The two reported examples are fixed, but query leakage remains:

1. [remote.rs:32](host/linux/src/image/remote.rs:32), [RasterRetry.swift:84](host/apple/Sources/ExactKit/RasterRetry.swift:84): Replacing a source that prefixes another URL removes its query delimiter prematurely. With source `https://h/a.svg?x=1`, `url(https://h/a.svg?x=1&hm=secret)` becomes `url(https://h/a.svg&hm=secret)`. The second pass preserves `hm=secret`; an SVG stylesheet warning can log this. Match complete references or consume the remaining query before replacement, and cover this on both hosts.

2. [remote.rs:43](host/linux/src/image/remote.rs:43), [RasterRetry.swift:95](host/apple/Sources/ExactKit/RasterRetry.swift:95): `@import url( a.css?hm=secret);` still survives unchanged. Whitespace separates `url(` from the query, and SVG import warnings retain that text. Recognize the complete URL span across whitespace before splitting words; add regression coverage on both hosts.

**Non-blocking**

none. Static review only; no tests or builds run.
