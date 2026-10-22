# Code review: An open popover is out of its parent's flow (kernel, landed under the main-red permission), round 3, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `093f537452`.
- **Method:** one brief (sha256 `4b2169a01b1b502381cdc2432d462056dd91f586e94610237bcad35447de671c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in bc911f22f5 and 89155b1d22 (regions, the scroll extent, Linux's anchor exclusion, the decoration walks)

**VERDICT: WRONG.** Nothing here blocks the out-of-flow rule on the main path. Setting or clearing `popover`, and changing `position`, restyles the box and the bottom-edge cover chain. Multi-column layout and iOS `inFlow` treat a popover as out of flow, and a sticky one stays in flow only while laid out. `a_popovers_insets_follow_its_parents_cover` matches the authored-absolute cover contract: `set_host_cover` restyles positioned children, so `top: 0` moves from 50 to 80. The viewport containing block stays the queued gap. Two consumers still key off authored `position: absolute` and get the new engine absolute wrong.

1. **should-fix** — `kernel/src/region/state.rs:759`. `holds_absolute` only sees authored `PositionType::Absolute`, so a static content-region owner that contains a popover is accepted. The trial lays that owner out as the root of its own tree and, through `taffy_style`, the popover is absolute, so the trial contains it. In the ordinary tree the containing block is the nearest positioned ancestor above that owner. Region publication then overwrites the popover’s frame with the trial’s. An authored absolute in the same place is refused for this disagreement.

2. **should-fix** — `host/apple/src/layout.rs:649`. A scroller’s kernel content already includes the popover the way it includes any absolute child (Taffy’s scrollable overflow, border box only). The host floor then adds the scroller’s end padding unless `position_type` is authored `Absolute`. A static popover that defines the scroll edge grows the iOS and Mac extent by that padding. Linux `content_size` adds padding for every child, so there a popover matches an absolute child. Terminal and Android take the kernel extent and match too.

3. **nit** — `host/linux/src/scroll_anchor.rs:146`. `excluded` and `out_of_flow` (`scroll_anchor.rs:124`) still mean authored `Absolute`. A popover in a static scroller is hoisted to an outside containing block, same as an absolute, but it can still be chosen as the anchor, and setting the prop does not flip the out-of-flow bit that suppresses adjustment. This is the path where the shell is not covering popovers whole (`navigation.top_layer`).

4. **nit** — `kernel/src/flow.rs:113` and `host/apple/Sources/ExactKit/NodeText.swift:36`. Float admission still treats a static popover as in flow, so text inside one can be admitted around an ancestor exclusion. Decoration walks stop at authored `absolute` or `fixed`, not at a popover, so an ancestor underline still paints through its text (`content_region/wire.rs:78` is the same check).
