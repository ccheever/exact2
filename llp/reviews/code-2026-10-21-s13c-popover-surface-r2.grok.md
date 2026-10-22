# Code review: a popover whose author wrote no surface gets the platform's (iOS's glass), round 2, 2026-10-21 (grok)

- **Family:** xAI. Grok 4.7 via `grok -p`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b53b3a3389`.
- **Method:** one brief (sha256 `0c822fe9bd20d0b24d00260e6552ce4ec880ad479618eab5390d51ede1705766`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH NITS
- **Disposition:** the scroll corner taken in 683d4064e5; the web's data attribute left (the web host's catch-all)

**VERDICT: RIGHT WITH NITS**

The round-1 fixes hold. The compiler strips a state prefix with `state_of` before the surface test, so `:hover background-color` counts as authored and the hovered popover is unmarked. A class border, a `background` or `border` shorthand (seen by name before expansion), an inset `box-shadow`, and `background-color="transparent"` all count. A component surface is an authored row on the body, `written` included. A literal `role="menu"` is excluded, and a role expression still reaches `accessibilityRole`, which the host refuses. An `alertdialog` popover is hidden and presented natively. The host requires a live `popover`, skips a node named as a `contextPopover`, and `writesCorner` keeps an authored radius, including 0. Non-scrolling children are rehomed into the glass `contentView` (`GlassGroup.swift:226`) and the glass shape is `cornerConfiguration`. The light-only shadow is recast on an appearance change: `traitCollectionDidChange` calls `applyStyle`, and the next display runs `applyBoxLayer` → `applyShadow` against `drawsDark`. The Mac column is unset, so no corner or shadow is sent, and the Mac assertions are `cfg`-guarded. The pre-26 ultra-thin stand-in is stated in the schema and in `Backdrop.swift`, and the metrics rows are `length`/`number` in the six-cell section. The two limitations called out as unchanged still match the code.

1. **should-fix** — A scrolling popover’s corner is applied only for the default radius, and never taken off. `host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1109`

   The scroll view owns the children (`GlassGroup.swift:226` prefers `scroll` over the glass content view). Line 1109 writes `cornerRadius`, `.continuous`, and `clipsToBounds` only inside `if let corner`, and `corner` is nil both when any radius row is set (`Backdrop.swift:174`) and when the surface no longer applies. A popover the compiler still marks because its only surface row is a radius (the `round` case) therefore never clips that scroll view, so scrolled children paint through the square corners of a glass shaped to the author’s radius. The same assignment is one-way: `updateMaterial` returns at `NodeViewIOS.swift:1097` once the material view is gone, and nothing else writes that layer’s corner. Clearing `popover`, a role becoming `menu`, or a radius row arriving later leaves a 30-point continuous clip on a node that no longer has the default surface. The hosted clear-popover and authored-0 checks are not scrollers.

2. **nit** — The web puts the flag on the element as `data-defaultsurface="true"`. `host/web/src/element.rs:1163`

   `DefaultSurface` has no arm in `props_for`, so the catch-all emits `data-defaultsurface`. The JS target’s `prop_name` (`host/web-js/src/style.rs:542`) projects that same map, so a bound flag is emitted the same way. Nothing in CSS or script reads it, and the UA popover style keys off `popover`, so the paint is unchanged. The attribute is still in the DOM.
