# Code review: @exact/actions ComposeFrame (stage 13 C piece 3), round 1, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fd9ab119cd`.
- **Method:** one brief (sha256 `62a75fb91d27d7deb172b25ec553a5190512dd7775cb1942327cdc757e11cf92`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** all three taken in 5648b6471b (Drafts left out by if; the slot at the view's root, no wrapper; the ring's limit guarded)

VERDICT: WRONG

1. **blocker — [packages/actions/compose-frame.contract:87](/tmp/iosfix-rv-cf1/packages/actions/compose-frame.contract:87): Drafts remains in the native iOS bar when hidden.** The clone previously omitted it with `if`. This change keeps the button mounted with `display="none"`, but the [header walker](/tmp/iosfix-rv-cf1/kernel/src/bars.rs:196) ignores hidden status, and [UIKit’s projection](/tmp/iosfix-rv-cf1/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:239) includes those buttons. Thus `reply=true` or `drafts=false` still produces a Drafts bar item. Fix hidden-button projection while retaining D5’s variant conditions.

2. **should-fix — [packages/actions/compose-frame.contract:95](/tmp/iosfix-rv-cf1/packages/actions/compose-frame.contract:95): The body wrapper changes iOS scrolling behavior.** [Native bar integration](/tmp/iosfix-rv-cf1/host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:1038) requires the first in-flow child after the header to be the scroller. The clone satisfied that; this column interrupts it. The body loses scrolling beneath the navigation bar and its automatic inset behavior. Preserve that relationship or extend host recognition of this wrapper.

3. **should-fix — [packages/actions/compose-frame.contract:184](/tmp/iosfix-rv-cf1/packages/actions/compose-frame.contract:184): Nonpositive limits are unhandled.** Both ring branches divide by `limit`; zero produces `NaN` dash values. This occurs in the generated gallery, which [supplies zero for numeric props](/tmp/iosfix-rv-cf1/contract/cli/src/gallery.rs:1035). Negative limits also produce misleading saturated pies. Define nonpositive-limit behavior and guard both divisions.

Static review only; no files changed or tests run.
