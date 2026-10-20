# Code review: LLP 1121 stage 9 F1, AccentColor under an authored accent on the painters and iOS, round 1, 2026-10-18 (grok)

- **Family:** xAI. `grok-4.7` via Grok CLI, reasoning effort xhigh, edits denied, a throwaway worktree.
- **Scope:** the F1 commit.
- **Method:** one brief (sha256 `dcd021d4b128e2a6084a957856edfde9d07e56aefac4f3ede5cb445f79bf846f`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (iOS across presentations; the ancestor's appearance; the role test by string).
- **Disposition:** fixed in round 1's commit.

---

I'll review this commit read-only: the accent-scope wiring for painters, iOS spinner tint, and whether every `authored_accent` caller actually runs inside its node's scope.The diff is small. Next I’ll trace every `authored_accent` caller and how the accent scope is opened, including iOS and Mac.Nothing here is blocking. The in-tree case this commit is for works on the painters and on iOS: a spinner whose accent is the `AccentColor` role, under an ancestor that set a real accent, keeps that accent. The holes are off that path.

1. **should-fix** — iOS looks up the kept accent on the view hierarchy, so a sheet or popover drops it. `host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:76`

`authoredAbove` walks `superview`. A dialog presented as a sheet is moved onto the sheet scroller (`DialogSheetIOS.swift:261`), and an open popover is moved into its layer (`MenusIOS.swift:755`). Neither leaves the logical parent in the superview chain. The rest of the host already jumps back with `listenerParent` (sheet `authoredParent`, popover `logicalParent`).

Failure: a page sets `accent-color: #00c800`, and a dialog or popover sets `accent-color: AccentColor` around a spinner. The kernel still sends the role on the control (the concrete green was replaced in the cascade), the walk never reaches the green view, and the spinner falls through to the design `AccentColor`. The web and the Linux painter keep green, because they follow the logical tree. A concrete accent inherited onto the spinner with no role in between still works, because that color is copied into the control's style and the walk is skipped. Placed nodes are fine: the canvas stays above the overlay in the superview chain.

Fix: walk `presenter.listenerParent(of:)` (falling back to `superview`), not `superview` alone.

2. **should-fix** — the kept accent is resolved in the ancestor's appearance, not the spinner's. `host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:53`

`authoredAbove(owner)?.channels("accent_color")` uses the ancestor's `drawsDark`. The painters resolve the same `ColorValue` with the spinner's scheme (`self.dark` is switched to the node's appearance before `control::progress`). `color-scheme` is a per-view trait (`ColorScheme.swift:13`), so they can differ.

Failure: an ancestor in a light scheme sets `accent-color: light-dark(#00c800, #0000c8)`, and the spinner sits in a dark `color-scheme` subtree with `accent-color: AccentColor`. Linux paints `#0000c8`. iOS paints `#00c800`. A fixed color has no scheme and is unaffected.

Fix: `channels("accent_color", dark: owner.drawsDark)`.

3. **nit** — the role test is a search of `BatchValue`'s debug dump. `host/apple/Sources/ExactKit/IOS/ProgressIOS.swift:73`

`authorsAccent` treats a value as the role when `"\($0)"` contains `@tint` but not `@tint/`. `BatchValue` has no `CustomStringConvertible`, so this depends on the dump still containing the payload. On today's wire it does (`{"sys":"@tint"}`), which is why the new test passes, and `@tint/0.2` (`Highlight`) correctly counts as authored. It also cannot tell `AccentColor` from `LinkText`, `ButtonText`, `-exact-link`, or `-exact-symbol`, because iOS sends all of those as `@tint`. The painters only keep the outer accent for the `AccentColor` role id; a `LinkText` accent is stored and used. An ancestor `accent-color: LinkText` under a green accent therefore stays green on iOS and becomes the link color on the painters.

Fix: `sys == "@tint"` on the object, not a string search. Distinguishing `LinkText` needs the role name on the wire; the platform name alone cannot.

Checked and not findings:

- Both painter callers (`control::progress` at `control.rs:529`, `neutral::spinner` at `neutral.rs:216`) run only from `Painter::node` after `accent_scope` (`paint.rs:966`). Hosted fields, region capture, replay, the Android reader, bars, and window chrome open scopes but do not call `authored_accent`. Placed nodes re-enter `node()`, which opens the scope. Replay never paints a progress. No stale scope on these paths.
- Role detection uses the computed inherited accent, which is the right value: the node's own row wins when it has one, and an inherited `AccentColor` role must still keep the outer accent. `accent_scope` is fed by the same computed row. `auto` is `None`, not the role, so it resets to the design accent, as on the web.
- The scope never stores the `AccentColor` role: `roles::authored_accent` (`roles.rs:476`) substitutes the outer value. Other roles can be stored. The new filter only drops `AccentColor`, and `resolve` clears the scope before resolving a tint role, so it does not recurse. `light-dark()` in the scope is resolved with the spinner's `dark`.
- The determinate bar and the other iOS controls read `channels("accent_color")`, which resolves `@tint` through the inherited `tintColor` set by `applyAccentTint`. In-tree they already keep an authored accent. The brief named the spinners. The bar was not given this walk.
- Mac `ProgressMac` still never reads `accent-color`. The file states AppKit draws the spinner and the bar in the system accent. That is unchanged prior behavior, not a regression from this commit.
- Tests cover the in-tree case only. The iOS test nests three views and checks the second spinner is not green, so a wrong non-green color still passes. Neither test covers a sheet, a popover, or a `light-dark()` accent across two schemes.
