# Code review: the painted popover's default placement and AppKit live margins, narrow re-check 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `a665e28161`.
- **Method:** one brief (sha256 `e5defef54a264f8c2968db80b2c2dded9f536d771a1718bc387f5c6c41b7ba31`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 7476e92c85 (resize reads margins)

**WRONG — the encoder fix is correct, but it exposes a macOS resize regression.**

[style.rs:1056](/tmp/iosfix-rv-flip4/host/apple/src/style.rs:1056) now forwards margins without an authored area, including alongside dialog insets. However, [PopoversMac.swift:187](/tmp/iosfix-rv-flip4/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:187) captures those margins once when opening. Its retained resize callback subtracts that snapshot from subsequent block sizes.

**Concrete case:** open a native, non-menu popover with no area or fallbacks, a 100-point border-box height, and `margin-top=12` (other margins zero). Change the margin to zero while it remains open. The block height becomes 100, but the callback still subtracts 12: native content height becomes **88**, clipping the 100-point subtree. Before these commits, the omitted margin read as zero, so the resulting content height was 100. [BlockHostMac.swift:171](/tmp/iosfix-rv-flip4/host/apple/Sources/ExactKit/Mac/BlockHostMac.swift:171) invokes that retained callback. Read current margins during resizing; the native anchor gap also needs updating when margins change.

Other consumers:

- **Native Mac presentation:** margins now affect the initial gap and content size, plus toolbar eligibility ([PopoversMac.swift:154](/tmp/iosfix-rv-flip4/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:154)). A form with nonzero margins can keep its header painted.
- **Menus and painted layout:** the Mac’s native menu launch point also changes ([MenusMac.swift:443](/tmp/iosfix-rv-flip4/host/apple/Sources/ExactKit/Mac/MenusMac.swift:443)); painted layouts on both hosts correctly receive margin boxes for flipping.
- **iOS keyboard:** [MenusIOS.swift:743](/tmp/iosfix-rv-flip4/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:743) reduces placement bounds before choosing placement. Its final clamp uses the border box and does not add margins again.
- **Other layout/state:** ordinary frames remain kernel-supplied; flat paint ignores margins. Scroll anchoring tracks margin changes; control-bar detection reads vertical margins but excludes popovers from in-flow candidates. Agent state does not independently apply these rows.

**Test sensitivity:** by inspection, removing both fixes fails [spacing_tests.rs:249](/tmp/iosfix-rv-flip4/host/apple/src/spacing_tests.rs:249); removing only the dialog fix fails the assertion at line 263. These assertions check serialization, not live native resizing.

I replayed the resize arithmetic in memory. Cargo/AppKit tests were not run: this read-only checkout has no built test artifacts.
