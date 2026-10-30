# Code review: Linux focus on a control a commit disables, round 3 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `37ec2cea19`.
- **Method:** one brief (sha256 `834a7b01323ba96e112929bdb81e20b711689a17119dcf490a81508c93dbc88a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

## Blocking

1. **[P1] A disabled-blur re-enters `autofocus` and route arrival before `focusout` — `host/linux/src/presenter/events.rs:211`**

`blur_disabled_focus` calls `set_focus(None)` and returns `sent = false`. `set_focus` sets focus to nothing, then services each event with a full `after_commit` (`events.rs:472`), which is `finish_commit`: another `service_commit`, collection refinement, and geometry. That nested `service_commit` runs `autofocus` (`presenter.rs:909`) and `arrive` (`presenter.rs:910`) while the outer `set_focus` still has `focusout` queued. Focus is already `None`, and on a pointer press `autofocus_from` is the button, so a laid-out `autofocus` candidate is taken immediately. `box_of` paints when the display is not attached (`presenter.rs:1039`), which is the unit-test and agent presenter, so a newly mounted field counts too.

The removal path does the opposite: it dispatches every `focusout`, then one `sync_commit`, and returns `sent = true` so the outer pass does not focus anything early (`events.rs:181`).

Failing input: a focused button `press=go disabled=busy blur=note("-")`, a column `focusout=note("o")`, and an already-shown `input autofocus=busy focus=note("F")`. `go` sets `busy = true`. The web and a finished `set_focus` log `-oF` (blur, focusout, then the field's focus). This logs `-Fo`. The same window is the nested test: A's `blur`, then B's `focus` and disabled-blur, then A's `focusout`. That test only checks that A is not focused at the end, so it stays green. A second `autofocus` field is also marked handled by the outer `autofocus()` after the first one has already taken focus, so it never gets a turn.

Same shape for a route push in that press: `arrive` focuses the new heading from inside the blur's `after_commit`, and the button's `focusout` runs after that.

2. **[P2] Disabling a field mid-composition commits the unconfirmed preedit and drops the platform result — `host/linux/src/presenter/events.rs:216`**

`set_focus` commits a typed field before `blur` (`events.rs:406`). `commit_text` skips that when `ime_owns` (`typing.rs:747`). `ime_owns` is `composing_field`, which returns nothing once `disabled` is set (`ime.rs:83`), and the disabling commit is already in the tree. A preedit has set `edited` (`field.rs:960`), so `change` fires with that preedit (`paint/control.rs` `choice` still holds it). The window's later `ime_commit` / `ime_end` then hits `composing_field() == None` and clears `self.ime` without applying or reverting (`ime.rs:201`).

An enabled field blurred the ordinary way still waits: `ime_owns` is true, so there is no early `change`, and `finish_ime` commits after the platform answers. Failing input: focus a field, `ime_preedit("ni")`, then a commit sets `disabled=true` on it. Expected is no `change` until the IME confirms or cancels. Actual is `change` of `ni`, then `blur`, and `ime_commit("に")` is discarded. Linux and Windows share this IME. Android's hosted reader is the other path: `commit_text` does not look at `hosted.composing`, and `reader_field_edited` ignores a confirm once the field is disabled (`hosted.rs:526`).

## Non-blocking

- The press fallback matches the round-2 note. `disabled_blur` is a list, cleared when the press starts and taken when it ends (`surface_controls.rs:949` and `:983`). Nested blurs stay in the list, and a press whose own control was blurred this way does not write `fallback` back, so a nested `autofocus` focus is left in place. Keyboard activation goes through `dispatch_press` (`top.rs:1029`), so it gets the same guard. Checkbox and radio changes that are not presses have no fallback; `clear_removed_focus` still blurs them.
- Route-hidden focus still takes the removal path (`events.rs:154` requires the route not be inert). `remote()` (pointer none) still keeps the focus; `the_remote_leaves_a_control_disabled_under_the_focus` is that pin. Nothing here is `cfg`'d to Linux, so Android phones and Windows take the blur and Android TV keeps the remote exception. I could not compile those tests in this sandbox.
- `focus_fixup` is the control itself, via `disabled_control`, minus `Progress` and `ProgressBar`. An indeterminate progress is a `div` on the web and a determinate one is `progress` (`host/web/src/element.rs:573`); neither matches `:disabled`. Other `Control` nodes are `input`, `select`, or `button`, which do. A link (`href`) stays focused. A disabled box does not blur its child. A removed node still runs no `blur`. A same-commit disable-and-unmount takes the removal path, because the node is already gone.
- `field_reveal` is set by `set_focus` and then ignored when focus is empty. The ring is painted for the focused node; `ringless` is refreshed on the next focus because the previous focus is empty.
- `follow_invocation` already refuses a disabled invoker. This patch does not put focus back on that button.
- Tests that assert "focus is empty and the blur ran" (press, tap, re-enable, stopped blur still `focusout`s, field `change` then `blur`) fail without the fix. The box, link, and removal tests pass without it; they are guards. The nested test fails on the round-2 single slot and does not lock order. `disable-blur.steps` is a fixed `tap` / `clock +200` script (`after(100)` then a 200 ms virtual advance), so the three hosts see one timer fire. It does not cover re-enable, nested order, the link, or a field.

Verdict: CHANGES
