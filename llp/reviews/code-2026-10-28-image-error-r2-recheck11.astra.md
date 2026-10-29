# Code review: the image error event on Linux, Android and Windows, round 2, re-check 11 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `ba008c577f`.
- **Method:** one brief (sha256 `2a08382eca35efcfa7f128ee501a4554e90396888f874d61f6ef9ed47bad8f45`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

The SVG parser leak is resolved. Two pre-existing Apple paths still violate the broader guarantee:

1. [NodeSymbolMac.swift:29](host/apple/Sources/ExactKit/Mac/NodeSymbolMac.swift:29) and [NodeViewIOS.swift:556](host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:556) log unknown-symbol sources verbatim. A computed source such as `symbol:https://cdn.example/a.svg?hm=secret` exposes the query in the journal. Cut the source at its first `?` or `#` before logging.

2. [RasterLoader.swift:622](host/apple/Sources/ExactKit/RasterLoader.swift:622) applies the word-based sanitizer to a complete package-image source. When admission refuses `photos:https://cdn.example/a.png?x=a &hm=secret`, sanitization stops at the space and logs `&hm=secret`. Cut the complete source at its first query/fragment delimiter, or use `redacted(source, source: source)`.

No remaining query leak found in the reviewed Linux/Android/Windows image paths.

**Non-blocking**

none.

Static review only; no tests, builds, network access, or edits.
