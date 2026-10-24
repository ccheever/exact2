# Code review: the painted popover's default placement and AppKit live margins, narrow re-check 1, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `9deccf3a62`.
- **Method:** one brief (sha256 `7c2f577252278d8071f6577cbf236d666aa082daf88b6dd68364973c0b82c3b8`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in a665e28161 (dialog popover margins)

**WRONG — the fix handles ordinary popovers but remains incomplete for a `dialog` used as a popover.**

At [style.rs:1055](/tmp/iosfix-rv-flip3/host/apple/src/style.rs:1055), the dialog branch selects only inset rows before the new popover condition can select margins. This leaves the same missing-margin defect for that supported combination.

Concrete case: a painted Mac `dialog popover="auto"` with no `position-area`, `margin=0 margin-top=12`, and a 120×60 border box, anchored at `(100,500,80,30)` in a 400×600 viewport. [MenusMac.swift:573](/tmp/iosfix-rv-flip3/host/apple/Sources/ExactKit/Mac/MenusMac.swift:573) receives zero margins and places it below at `y=530`. With its margin, that candidate overflows; it should flip above to `y=428`. Authored `position-try-fallbacks="flip-block"` forces this painted path. Preserve dialog insets while also sending its popover margins.

The other consumers look consistent with the ordinary-popover fix:

- [PopoversMac.swift:187](/tmp/iosfix-rv-flip3/host/apple/Sources/ExactKit/Mac/PopoversMac.swift:187) now subtracts margins from the block’s used size and applies the invoker-facing gap. Nonzero margins also disable toolbar anchoring at line 155; the existing [header test](/tmp/iosfix-rv-flip3/host/apple/tests/ExactKitTests/HeaderMacTests.swift:575) explicitly expects that behavior.
- Painted Mac/iOS placement replaces the kernel origin, so margins are not added twice. The [iOS keyboard clamp](/tmp/iosfix-rv-flip3/host/apple/Sources/ExactKit/IOS/MenusIOS.swift:743) clamps the resulting border box without rereading margins.
- [Native Mac menu placement](/tmp/iosfix-rv-flip3/host/apple/Sources/ExactKit/Mac/MenusMac.swift:443) now honors margins while retaining `PositionArea.of` and its existing placement policy.
- Ordinary frames remain kernel-owned. Flat paint treats margin rows as inert; scroll anchoring and iOS control-bar detection inspect them without applying another frame offset. Agent state does not independently apply margins; reported geometry follows the positioned views.

The [new assertion](/tmp/iosfix-rv-flip3/host/apple/src/spacing_tests.rs:249) **would fail without the fix**: the old encoder omits `margin_top`, yielding JSON null rather than `12`. It tests only a `column`; an otherwise identical `dialog` fixture would still fail.

This is a source-traced verdict. No files changed; tests were not run because this read-only checkout has no build artifacts.
