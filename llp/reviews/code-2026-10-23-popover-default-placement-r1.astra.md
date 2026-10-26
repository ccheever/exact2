# Code review: the painted popover's default placement and AppKit live margins, round 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9d62a0ae9f`.
- **Method:** one brief (sha256 `870d97d5ee1374e6d639aa956ecd3cdf0147ce28d5449272ac8090c2814e923e`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in afd265f7b2 (PositionArea.painted; origin and the chooser as main)

Two regressions and one documentation error:

1. **[P2] Authored `position-try-fallbacks: none` is overridden.** [PositionArea.swift:77](/tmp/iosfix-rv-flip1/host/apple/Sources/ExactKit/PositionArea.swift:77) treats an explicit `"none"` exactly like an absent declaration. With bounds `400×600`, anchor `(12,560,30,30)`, popover `300×200`, zero margins, and authored fallback `none`, the base should clamp to `(12,400)`. HEAD instead flips to `(12,360)`. CSS `none` leaves only the base option. Preserve declaration presence when selecting defaults. [CSS definition](https://drafts.csswg.org/css-anchor-position-1/#propdef-position-try-fallbacks).

2. **[P2] The shared change alters native NSMenu placement.** [MenusMac.swift:443](/tmp/iosfix-rv-flip1/host/apple/Sources/ExactKit/Mac/MenusMac.swift:443) calls this helper with `.platform`, so it now receives default flips and loses the former no-area clamp. Concrete no-fit case, in pane coordinates: bounds `400×400`, anchor `(180,180,40,30)`, menu `300×250`. All four options overflow. Previously the launch point was `(100,150)`; HEAD returns `(180,210)` through [PositionArea.swift:124](/tmp/iosfix-rv-flip1/host/apple/Sources/ExactKit/PositionArea.swift:124). AppKit fits against the screen, so a window comfortably inside a larger screen no longer constrains this menu to its pane. Fitting cases also change from clamping to flipping. Both violate the ruling that native menus remain unchanged.

3. **[P3] QUEUE attributes placement to the wrong owner.** [QUEUE.md:3](/tmp/iosfix-rv-flip1/QUEUE.md:3) says placement, stacking and dismissal come from kernel frames. The new iOS fixture supplies the popover frame at `(0,0)`, yet [MenusIOS.swift:765](/tmp/iosfix-rv-flip1/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:765) computes its displayed origin `(12,500)`. The host maintains the stack and reports displayed geometry through [OwnersIOS.swift:395](/tmp/iosfix-rv-flip1/host/apple/Sources/ExactKit/IOS/OwnersIOS.swift:395). The kernel supplies content layout; the compatibility explanation should identify the host’s placement and dismissal machinery. The reported blur measurements cannot be verified from this diff.

There is also a **pre-existing CSS history limitation newly exercised by defaults**: [PositionArea.swift:65](/tmp/iosfix-rv-flip1/host/apple/Sources/ExactKit/PositionArea.swift:65) resets history on any measured size change. In `400×600` bounds, anchor `(12,300,30,30)`, an auto-height `300×280` popover selects above at `(12,20)`. Removing content to make it `300×260` resets the choice and jumps below to `(12,330)`; CSS retains the still-fitting above option at `(12,40)` when computed styles are unchanged. The existing LLP explicitly documents size resets, so this needs distinguishing from the two regressions above. [CSS history rules](https://drafts.csswg.org/css-anchor-position-1/#fallback-remember).

Otherwise, option order, margin swaps, RTL default axes, and retaining the current option when all alternatives fail are correct. A nonempty authored fallback list correctly transforms the UA base represented by `axes("none")`. The fixed/absolute overflow distinction and iOS’s keyboard-bound reduction plus final clamp remain intact; NSPopover placement is unaffected.

The new test functions’ primary placement assertions would fail against the old implementation, and their coordinates are correct. The oversized/base assertions intentionally also pass before the change. However, [PositionAreaTests.swift:55](/tmp/iosfix-rv-flip1/host/apple/tests/ExactKitTests/PositionAreaTests.swift:55) does not detect removing RTL handling: its final point equals LTR’s. A fitting RTL case—anchor `(200,100,30,30)`, popover `120×60`—should assert `(110,130)`.

Read-only review with arithmetic replay; native tests were not run.
