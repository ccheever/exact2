# Code review: LLP 1121 stage 1 (theme and its reach), round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `045867d097115785aeeed45234b02df07073217127abe1068357f9f2b5b62bc7`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix from both (Astra 4 MAJOR, 4 MINOR; Grok 2 MAJOR, 2 MINOR).
- **Disposition:** fixed in 81bcf1f10 but the Linux painter honouring an authored accent for text, recorded in QUEUE as pre-existing

---

**MAJOR** — Linux still paints a tint role as the theme colour. `kernel/src/style.rs:729`

`resolve` now goes through `painted_override`, which rewrites every tint role (`AccentColor`, `LinkText`, `ButtonText`, `link`, `symbol`) to the theme's `AccentColor` and returns that colour. The comment says an authored accent stays the host's to apply. The Linux painter was not changed, and it calls `resolve` for text, SVG fills, shadows, and symbol tint (`host/linux/src/paint/svg.rs:118`). `color="AccentColor"` under `accent-color="#1083fe"` therefore paints the theme colour. Control chrome is the exception: `control::accents` still prefers the authored accent. Apple and the web still keep the role, so an authored accent wins there.

**MAJOR** — `AccentColor = "LinkText"` recurses without bound. `kernel/src/style/roles.rs:367`

Lowering allows that value. `AccentColor` is the one tint role a theme may set, and the value may be another role (`contract/lower/src/theme.rs:171`). `set_role_overrides` stores `ColorValue::Role(LinkText)`. `resolve(AccentColor)` returns that role; `resolve(LinkText)` is a tint role whose accent override is present, so it maps back to `AccentColor` and returns `Role(LinkText)` again. The same loop holds for `ButtonText`, `symbol`, and `link`. `Highlight` does not: it is `@tint/0.2`, not `@tint`. Linux paint, kernel SVG, gradients, and motion all call `resolve`, so the first of those on a legal theme does not return. On the web the same value makes `:root`'s `--exact-accent` a custom-property cycle, so the theme tint is dropped.

**MINOR** — Placeholders outside the iOS field. `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:855`

The iOS field now reads `ThemeRoles.color("placeholder", …)`. A Mac field still uses `NSColor.placeholderTextColor` for a native control. Text areas still draw the platform placeholder (`host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:222`, `host/apple/Sources/ExactKit/Mac/TextAreaMac.swift:187`), and the iOS date pill does too (`host/apple/Sources/ExactKit/IOS/DatesIOS.swift:128`). A `placeholder` override misses those.

**MINOR** — A refused boot still replaces the thread's overrides. `runner/src/runner.rs:809`

The early refusals (clock, schema, app id, root) now run first, and clearing is journaled. `settle` (`runner.rs:1002`), `init_late_slots` (`1004`), `gate_step` (`1026`), `Tree::create` (`1033`), and `apply` (`1041`) can still return an error after `set_role_overrides`. The table is thread-local, so it survives the dropped runner, and the journal line is only on the success path (`1064`). A settlement cycle leaves the rejected plan's theme in place.

VERDICT: fix (MAJOR kernel/src/style.rs:729, MAJOR kernel/src/style/roles.rs:367, MINOR host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:855, MINOR host/apple/Sources/ExactKit/IOS/TextAreaIOS.swift:222, MINOR runner/src/runner.rs:809)
