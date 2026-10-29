# Code review: the image error event on Linux, Android and Windows, round 2, re-check 5 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `75086ffa79`.
- **Method:** one brief (sha256 `237606eed6793c2eb24af32c7cd79f4115990a3e31db237a0487a4a89d9d338a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Query contents still terminate redaction early** — [host/linux/src/image/remote.rs:61](host/linux/src/image/remote.rs:61), [host/apple/Sources/ExactKit/RasterRetry.swift:100](host/apple/Sources/ExactKit/RasterRetry.swift:100). `https://h/a.svg?x=),&hm=secret` becomes `https://h/a.svg),&hm=secret`. Likewise, `?x=))&hm=secret` leaks the signature. Both punctuation sequences are valid inside queries; one-character lookahead cannot establish the token’s end. Drop the entire remaining token, preserving only verified trailing wrappers, and add regressions on both hosts.

2. **A closing mark immediately after `?` bypasses redaction entirely** — [host/linux/src/image/remote.rs:52](host/linux/src/image/remote.rs:52), [host/apple/Sources/ExactKit/RasterRetry.swift:94](host/apple/Sources/ExactKit/RasterRetry.swift:94). The retained opening guard leaves `https://h/a.svg?)&hm=secret` unchanged. For recognized URLs, start redaction at `?` regardless of the following character; cover this on both hosts.

**Non-blocking**

none. The quadratic rescanning is resolved by inspection: both scans advance monotonically with constant lookahead. Time and memory are O(input size); no nonterminating loop found. No tests or builds run.
