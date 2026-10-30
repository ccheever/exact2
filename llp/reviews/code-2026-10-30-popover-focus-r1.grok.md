# Code review: Linux non-modal popover follows HTML, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `/tmp/dt3-rv-b1`.
- **Method:** one brief (sha256 `9e7167733f27492b5198750ddd4c2c2bb1d7d1c8a4bb918ecaf40a2ad4465cc6`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

## BLOCKER

**The authored popover's top-layer frame is empty on an attached display and is never updated after it moves — `host/linux/src/presenter/alert.rs:710`**

`alert_popover_frame` publishes whatever `box_of` returns, and that is called only from `alert_present` (`alert.rs:265`) and from `alert_close` (to clear it). `box_of` reads the acknowledged picture. On the Linux host, `display_frame` sets the display active before the first user gesture, and `boxes()` will not repaint while it is attached. A popover that was closed is `HostCover::Whole`, so it is not in that picture: the report is an empty list, and `querySelectorAll` / `elementsFromPoint` still treat the subtree as closed. `alert_layout` (`alert.rs:515`) later moves the anchor, resizes, and sets `dirty` without reporting again. `sync_surface` (`host/linux/src/presenter/top.rs:1230`) returns immediately when only an authored popover is open, so the per-commit report that shape surfaces get never runs. Headless `frame()` paints inside `box_of`, which is why `reads_see_an_open_popovers_subtree` and synthetic-reads pass. There is no `box_of` cycle: the inner `alert_layout` returns at the measured-inputs check. The failure is a missing second report after the picture that actually contains the popover, and after every later layout.

## MAJOR

**A wheel over the popover, or over empty viewport, still dies as if the page were inert — `host/linux/src/presenter/wheel.rs:171`**

The page-scroll gate at the bottom of `wheel_at` was switched to `alert_inert()`, but the chain stops on `alert_root() == Some(id)` for every box, and the miss gate at `wheel.rs:118` still drops the tick when `alert.is_some()` and `hit` is none. `scroll_parent` walks DOM parents, so a wheel whose hit is inside the popover reaches the popover root and returns. Chrome scrolls the page when the popover does not consume the wheel (`overflow: auto` that is not overflowing, no `overscroll-behavior` containment). Wheeling the page beside the popover works. Wheeling on the popover, or in viewport that hits no box, does not.

**Right-click, the edit menu, and long-press still treat a non-modal popover as modal — `host/linux/src/presenter/edit_menu.rs:50`**

`edit_menu_at` and `edit_menu_key` (`edit_menu.rs:71`) return immediately on `top_open()`, and `long_press` does the same (`host/linux/src/presenter/top.rs:1171`). `top_open()` is still `alert.is_some()`. The secondary button never enters `alert_pointer`: Windows `MouseButton::Right` calls `pointer_aux`, and `aux_pointer` dispatches `pointerdown` with no light dismiss. An auto popover therefore stays open, and the page gets neither the edit menu nor a context menu. HTML light-dismisses on that `pointerdown` and still shows the menu. A long-press on the live page under a manual popover is refused the same way. Primary click is fine: `pointer_down` light-dismisses, then the click reaches the page.

**`alert_key` still consumes every key while a popover is open — `host/linux/src/presenter/alert.rs:637`**

For any non-alert box the function always returns true, so `key_pressed` never reaches `shortcut()` or `back_key_down`. Tab, typing, Enter, and Escape do run, because they go through `key_default` inside `alert_key` (the new Tab test covers that). A page `aria-keyshortcuts` chord does not, and Alt+Left / `BrowserBack` neither navigates nor closes the popover. HTML leaves those to the page. Escape on an auto popover is correct, and a manual popover correctly refuses `alert_close_request`.

## MINOR

**Sticky clearance is skipped for the whole time any popover is open — `host/linux/src/presenter/reveal.rs:106`**

`reveal` now scrolls a page node under a popover (`alert_allows`), but `clear_of_sticky` still returns false on `alert.is_some()`. A sticky page header is not scrolled clear while an unrelated popover is up. The popover itself cannot be scrolled out of the way. This check does not distinguish those.

## Checked, holding

Popover focusing matches HTML: focus stays put unless something inside autofocuses. A modal dialog or alert still blurs, moves in, and falls back to the box. Light dismiss on primary `pointerdown` closes an auto popover and the same press hits the page (`pointer_cancel` runs before the contact exists). A press on the stored invoker is left to the toggle, so one click closes once. Hide restores focus only when it is inside the popover. Manual popovers do not light-dismiss. Tab order uses `alert_allows`, so a modal box stays trapped and a popover continues onto the page. `alert_allows` for a modal is still "inside the box." The access tree is the box alone under `alert_inert`, and the page plus the popover otherwise. Windows MenuFlyout is the `surface_plan` path (`present` calls `alert_present` only when that plan is absent). Android still refuses authored boxes in `invoke` / `popover_command` when it has neither windows nor neutral alerts. One slot remains: a second popover, or a popover opened from a dialog, replaces the open box. That limit predates this diff.

The new tests cover those holding rules in a headless presenter. They do not open a display frame, move the anchor and read `elementsFromPoint`, wheel over the popover, or right-click.

VERDICT: Not ready — the top-layer frame the structural reads depend on is not published on the attached Linux display and goes stale after layout.
