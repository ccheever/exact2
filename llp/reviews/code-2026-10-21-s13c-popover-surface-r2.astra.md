# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), round 2, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `b53b3a3389`.
- **Method:** one brief (sha256 `0c822fe9bd20d0b24d00260e6552ce4ec880ad479618eab5390d51ede1705766`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 683d4064e5

VERDICT: WRONG

1. **blocker — [contract/lower/src/popover_surface.rs:15](/tmp/iosfix-rv-ps2/contract/lower/src/popover_surface.rs:15): An authored press background still receives the default surface.** The supported `-exact-press-background-color` attribute matches none of this predicate’s branches. A popover using it, directly or through a class, gets `defaultSurface=true`, although the equivalent `:active background-color` correctly suppresses it. The glass remains when the authored background paints. Recognize both spellings and test their equivalence.

2. **should-fix — [host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1109](/tmp/iosfix-rv-ps2/host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1109): The scroller’s default corner is never cleared.** After setting its radius to 30, clearing `popover` removes the material but leaves the scroll view rounded. Changing an authored radius from unset to `0` likewise leaves children clipped at 30 while the material becomes square. The retained scroller can also carry this into pooled reuse. Synchronize clipping when the default stops applying, including material removal.

3. **should-fix — [host/apple/Sources/ExactKit/Backdrop.swift:168](/tmp/iosfix-rv-ps2/host/apple/Sources/ExactKit/Backdrop.swift:168): Context-menu exclusion lacks invalidation.** Eligibility now depends on another node’s `contextPopover`, but changing that relation updates only the invoker. The target’s `updateMaterial()` is never called. Consequently, adding the relation can leave glass on an agent-painted context menu; removing it can leave an ordinary popover without its default surface. Reconcile affected targets when these references change.

4. **should-fix — [host/apple/tests/ExactKitTests/PopoverSurfaceIOSTests.swift:61](/tmp/iosfix-rv-ps2/host/apple/tests/ExactKitTests/PopoverSurfaceIOSTests.swift:61): The pixel cache can return another screenshot’s pixels.** It keys decoded bytes by `ObjectIdentifier(image)` without retaining the image. Images released between loop iterations can have their identifiers reused, making the dark or scrolling case read a previous capture. Retain the image alongside its bytes or scope decoding to each snapshot.

Read-only review; no files changed. Tests and the reported 48-app plan comparison were not rerun.
