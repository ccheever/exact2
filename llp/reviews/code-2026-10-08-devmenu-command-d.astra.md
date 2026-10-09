# Code review: ⌘D opens the iOS dev menu, round 1 (15b841e85, not landed), 2026-10-08 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a detached worktree at `15b841e85`.
- **Method:** one brief (sha256 `e6d500e7ba249ea66a465b1f1912db9baca58791eb769f720025457770cdcd44`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** the change is dropped: the iOS adapter's app delegate already registers ⌘D ("Exact Menu", `host/apple/Sources/ExactIOS/main.swift`), which is what opened the menu on the simulator. Only the author guide's mention of ⌘D lands. The delegate's command does not yield to an app's own `Meta+d` shortcut under a sheet (Grok 1); that is pre-existing and left as is.

---

LAND WITH CHANGES

1. **MATERIAL — Installation leaks across sessions and outlives its owner.** [DevMenuIOS.swift:122](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:122) becomes permanently true; [ExactViewIOS.swift:171](host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:171) consequently advertises ⌘D in every session. Installing on window A makes focused session B open A’s menu; after A disappears, B still consumes the chord. **Fix:** remove this duplicate binding and flag—the standalone adapter already registers ⌘D at [main.swift:195](host/apple/Sources/ExactIOS/main.swift:195). Alternatively, scope eligibility and dispatch to the live installing window/session.

The remaining assessment:

- **Responder chain:** empty focus is handled by [ExactViewIOS.swift:207](host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:207); descendant fields can reach its commands. Presented sheets bypass this view—the viewport moves at [ModalIOS.swift:473](host/apple/Sources/ExactKit/IOS/ModalIOS.swift:473)—but the existing adapter command covers them. The new binding adds no standalone coverage.
- **Conflicts:** the matching app command is preserved at [ExactViewIOS.swift:170](host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:170). The dev command retains [UIKit’s default text-input priority](https://developer.apple.com/documentation/UIKit/UIKeyCommand/wantsPriorityOverSystemBehavior), so it does not override system editing commands.
- **Sample host:** correctly receives no dev command: its [window setup](host/apple/Sources/ExactHostIOS/main.swift:220) never installs the menu.
- **Disabled/production:** `EXACT_DEV_MENU=0` prevents installation at [DevMenuIOS.swift:132](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:132). Production trust alone does **not** disable this menu; that is pre-existing—the [enabled predicate](host/apple/Sources/ExactKit/IOS/DevMenuIOS.swift:115) checks only the environment.
- **Platforms/quality:** the [iOS guard](host/apple/Sources/ExactKit/IOS/ExactViewIOS.swift:157) excludes tvOS and native macOS; the APIs support Catalyst. Idiom/comments are acceptable. Changed Swift files are 400/336 lines; caps and diff whitespace checks pass.

Static review; UIKit builds and runtime dispatch were not exercised in this read-only checkout.
