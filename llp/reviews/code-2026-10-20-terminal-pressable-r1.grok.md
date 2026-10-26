# Code review: the terminal taps a pressable box (QUEUE), round 1 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `cbaf1291a6`.
- **Method:** one brief (sha256 `4c018c5ead5014b8bac089bda2c1bb114386629a611e946a913f44b2a4d12bc7`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a host fix, the lead's 20:00Z task), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX).
- **Disposition:** both taken ("Round 1").

**Verdict: SHOULD-FIX.** A visible view with its own `press` is hittable in paint order, and a mouse click uses that geometry. `tap` / `type` / `wheel` and `press`'s focus path still treat every hit as a control.

1. **SHOULD-FIX — A clipped descendant of a pressable view is tappable.** `host/terminal/src/paint.rs:222`, `host/terminal/src/pointer.rs:237`. `on_screen` walks ancestors and succeeds if any hit has cells. That meant "inside a visible button or field." A `column press=` or card surface is now a hit for its clipped frame, so a button, field, or marker below the fold, or scrolled out of an inner scroller, passes `tap`, `type`, and `wheel` (`cli.rs:411`, `:425`, `:457`) and `press` runs it. `hit` is geometric: that descendant is not in `hits`. Accept the ancestor only when the node's frame meets that ancestor's visible hit rect. The new test keeps every target inside 20 rows, so it stays green.

2. **SHOULD-FIX — Pressing a non-control focuses it and scroll-into-views its full frame.** `host/terminal/src/host.rs:926`, `host/terminal/src/host.rs:761`. With no layer open, `press_with_inner` focuses every press, and `focus` always `reveal`s `nearest`. A box is not in `focusables` (`host.rs:724`), so `after_commit` (`host.rs:437`) clears that focus and sends no `blur`, after `reveal` has already run. A card taller than its scroller jumps so the card's bottom sits on the viewport bottom. Focus only when `focusables` contains the id. Tab order, the mouse wheel, and `scrollIntoView` do not read `hits`.

`pointerdown`, `hover`, `dblclick`, and `contextmenu` should stay off `hits`. This host dispatches Press, Focus, Blur, Key, Input, Submit, Navigate, and Cancel only. A hit whose press walk finds nothing swallows the click, including click-away blur. A `link` is already `Pressable`. Disabled and `visibility: hidden` stay hittable, same as a control. `display: none` and a box wholly outside the row clip never enter `hits`.

`handlers_of` runs only for painted non-controls (`||` skips Pressable, TextInput, and Control). It is a site lookup; an empty handler list does not allocate. Rows wholly outside the clip return first.

The new test is deterministic: exact counts, no clock. Clicks cover the box, the column, and the inner button (the button does not also press the column). The slot fill is tap-by-id only (`screen.rs:760`); a click on `fill` is the stacking check, and it is the same paint order as `inner`.

Removing `marker-toggle` is safe in this tree. `markedSlot` still requires `box testId="marker-…" press=toggle` and `text glyph`. `toggle` remains. No script or doc taps `marker-toggle`. The app comment names `scripts/stage12-gate.mjs`; that file is not in the repo. Tests were not executed.
