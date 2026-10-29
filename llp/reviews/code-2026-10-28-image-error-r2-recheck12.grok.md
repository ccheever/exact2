# Code review: the image error event on Linux, Android and Windows, round 2, re-check 12 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `14c8880ace`.
- **Method:** one brief (sha256 `7bd8acbf21c6df4ee2baa2892b0c883a807534eb9360cc3218541125162ca7cf`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

The unknown-symbol logs (`NodeSymbolMac.swift:29`, `NodeViewIOS.swift:556`) and the package-refusal log (`RasterLoader.swift:622`) now pass the source through `RasterRetry.withoutQuery`, which drops the whole tail at the first `?` or `#`. A space in the query no longer leaves a remainder. The new test covers `photos:https://cdn.example/a.png?x=a &hm=secret`. The refusal text beside that source is the runner's fixed reason (`grant-missing`, `no implementation bound`, `permission …`), and the `error` event is that reason alone.

`RasterFailure.svg` is thrown only inside `decodeSVG` (`RasterImage.swift:465`, `468`, `469`, `483`, `484`). The only production caller is `RasterBackend.decode`, whose catch (`RasterLoader.swift:331`) discards the error and fails the permit. The view then reports the core name `decode failed` (`RasterLoader.swift:1000` and `1351`). `RasterMetadata.read` never throws `.svg`. The test reads `String(describing:)` off a direct `decode` throw and does not log it.
