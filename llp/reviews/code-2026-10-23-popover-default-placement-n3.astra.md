# Code review: the painted popover's default placement and AppKit live margins, narrow re-check 3, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `7476e92c85`.
- **Method:** one brief (sha256 `c15a72b7862b8ede79c1aa766b0ad73bab3606e1c95cbcb60cd9bbff0e8de019`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** theme2 21:59: both fixed in f4525ec737

**WRONG — the margin forwarding is correct, but two AppKit update cases remain incomplete.**

1. **Resizing can still be skipped.** [PopoversMac.swift:190](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:190) reads current margins inside the callback, but [BlockHostMac.swift:169](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/BlockHostMac.swift:169) suppresses that callback when the block’s total size stays equal. Concrete case: a manual popover with no area changes from **100-point content + 12-point top margin** to **112-point content + zero margin**. Both blocks measure 112 points, so the native content remains 100 points and clips the enlarged content.

2. **The anchor gap still uses opening margins.** [PopoversMac.swift:199](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:199) expands the anchor only during presentation. In the new test’s own `margin-top: 12 → 0` case, the original 12-point gap remains after the margin disappears. [MenusMac.swift:553](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/MenusMac.swift:553) excludes native popovers from subsequent placement. The height assertion cannot detect this.

Other consumers checked:

- Painted placement consumes margins once in [MenusIOS.swift:766](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:766) and [MenusMac.swift:573](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/MenusMac.swift:573). The iOS keyboard clamp still uses border-box bounds.
- Chooser launch points ([MenusMac.swift:443](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/MenusMac.swift:443)) and toolbar-anchor selection ([PopoversMac.swift:155](/tmp/iosfix-rv-flip5/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:155)) now receive the margins too.
- Other direct reads are control-bar edge detection and scroll-anchor invalidation; FlatPaint treats margins as inert. Ordinary frames remain kernel-owned; agent state does not apply these rows as another layout.

**The regression assertions would fail without their fixes:** [spacing_tests.rs:249](/tmp/iosfix-rv-flip5/host/apple/src/spacing_tests.rs:249) gets missing/null instead of `12`; its dialog assertion likewise loses the margin. Reverting only the live-margin callback makes [PopoverNativeMacTests.swift:469](/tmp/iosfix-rv-flip5/host/apple/tests/ExactKitTests/PopoverNativeMacTests.swift:469) compare a height shortened by 12 points against its opening height. Neither test covers the two cases above.

These are source-traced results; tests were not executed because this read-only checkout has no build artifacts.
