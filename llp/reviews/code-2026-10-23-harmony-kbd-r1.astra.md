# Code review: Harmony transcript keyboard (on-drag removed), quick review, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `2ecf80136d`.
- **Method:** one brief (sha256 `c4c4a8da22a12440b3f798452ebfffa487275642c036e87802d55849fbda46af`), blind to the other reviewer; one quick round (the lead's 00:58Z: a one-line Harmony change). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** landed as f7e0fce73f (the same change rebased); both NONE

**NONE — No material or minor findings in HEAD (`2ecf801`).**

1. **Contract:** [app.contract:743](apps/harmony/app/app.contract:743) correctly continues the list’s attributes; `padding-bottom=8` remains attached to the list. Parsing and lowering passed using a cached compiler whose recorded source hashes match this checkout.

2. **iOS:** The list receives `overflow_y="auto"` from [style.rs:722](host/apple/src/style.rs:722), satisfying `scrollsVertically`. With the prop absent, [KeyboardAvoidanceIOS.swift:262](host/apple/Sources/ExactKit/IOS/KeyboardAvoidanceIOS.swift:262) returns `.interactive`. `scroll-start="end"` affects the initial offset, not the axis. This virtualized list uses `node.scroll` ([CollectionIOS.swift:43](host/apple/Sources/ExactKit/IOS/CollectionIOS.swift:43)); `syncScroll()` applies `updateKeyboardDismissal()` at [NodeViewIOS.swift:1328](host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1328). Native collection mounting also calls `syncScroll()` ([ScrollViewIOS.swift:226](host/apple/Sources/ExactKit/IOS/ScrollViewIOS.swift:226)).

3. **Other hosts:** No behavioral regression found. Web treats the prop as an inert attribute ([rt.js:710](host/web-js/rt.js:710)); Android explicitly leaves it unimplemented ([README.md:305](host/android/README.md:305)); macOS’s scroll setup does not consume it ([NodeViewMac.swift:1066](host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1066)).

No files changed. Native gesture behavior was reviewed in source; device tests were not run.
