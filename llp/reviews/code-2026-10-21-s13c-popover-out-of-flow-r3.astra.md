# Code review: An open popover is out of its parent's flow (kernel, landed under the main-red permission), round 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `093f537452`.
- **Method:** one brief (sha256 `4b2169a01b1b502381cdc2432d462056dd91f586e94610237bcad35447de671c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in bc911f22f5 (exclusion settling, content regions, the Apple scroll extent, Linux anchoring); iOS anchoring's prop change QUEUE'd

VERDICT: WRONG

1. **blocker — [kernel/src/flow.rs:113](/tmp/iosfix-rv-oofr3/kernel/src/flow.rs:113).** `in_flow` still admits a popover whose authored position is static/relative. An auto-height, bottom-aligned popover containing text can therefore enter exclusion settling: wrapping increases its height, moves its top, and changes the wrapping again. This can oscillate and reach the assertion at [layout.rs:1475](/tmp/iosfix-rv-oofr3/kernel/src/layout.rs:1475). Apply the popover out-of-flow predicate here too.

2. **blocker — [kernel/src/region/state.rs:898](/tmp/iosfix-rv-oofr3/kernel/src/region/state.rs:898).** `flex_height_independent` still accepts a popover owner as an in-flow `height:auto; flex:1` item. Its actual absolute height depends on its children, which the shell pass cuts away, so registration can produce an incorrectly collapsed owner. [state.rs:759](/tmp/iosfix-rv-oofr3/kernel/src/region/state.rs:759) likewise misses popover descendants when rejecting static region owners. These existing admission guards need the effective out-of-flow classification independently of the deferred viewport work.

3. **should-fix — [host/apple/src/layout.rs:649](/tmp/iosfix-rv-oofr3/host/apple/src/layout.rs:649).** Scroll-extent padding still distinguishes only authored `Absolute`. A direct popover child receives the scroller’s trailing padding in addition to its border-box extent, while an equivalent authored absolute child does not. This leaves excess scrollable space on iOS/Mac.

4. **should-fix — [ScrollAnchoring.swift:67](/tmp/iosfix-rv-oofr3/host/apple/Sources/ExactKit/ScrollAnchoring.swift:67), [host/linux/src/scroll_anchor.rs:125](/tmp/iosfix-rv-oofr3/host/linux/src/scroll_anchor.rs:125).** Anchoring suppression still tracks authored position only. Setting/clearing `popover` now changes flow participation without registering that transition, allowing sibling movement to produce an unwanted scroll correction.

The ordinary prop/position invalidation paths and new cover regression test look sound statically. No files changed or tests run.
