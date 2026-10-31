# Code review: Linux focus on a control a commit disables, round 3-recheck (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `87c9ce06fb`.
- **Method:** one brief (sha256 `f091be7e22d9a0bb1b3e9887da903a2b222f34a99609382e95abe0710e112b8d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll re-check this commit against the three round-3 blockers and the unchanged `service` callers, read-only, then run the Linux focus tests if the sandbox allows.The three round-3 fixes are in place and match the web fixup they were written for. `set_focus` still services every event (`service` true). Only the disabled-blur path passes `service` false.

**Ancestor press.** `tap` on the child focuses the child, then `dispatch_press` runs on the column (`press=go`). At that start, focus is not the press target, so `fallback` is `None`. The commit disables the child, `blur_disabled_focus` clears focus and records the id even when the child has no blur handler, and `autofocus=off` then focuses the column. The old pointer clause treated that as “focus became the target, fallback is not” and wrote `focus = None`. `disabled_blur` non-empty now skips that write, in `surface_controls.rs` around the fallback. Without the guard, `a_press_its_ancestor_handles_keeps_the_focus_its_commit_moved` ends on `None`.

**Checkbox and radio.** `toggle_control_with_press` and `activate_radio` still fire input and change before the trailing `after_commit`. `blur_disabled_now` runs after each of those commits has landed and before the next event. A checkbox whose `input` sets `disabled` and whose `change` clears it therefore blurs in between (`"ibc"`, focus stays `None`). Without that call, `change` re-enables first and `clear_removed_focus` never blurs, so the log is `"ic"`. A switch uses the checkbox loop. A programmatic checkbox or radio set goes through those same functions.

**Blur, then focusout, then one service.** `blur_disabled_focus` stages the field `change`, the `blur`, and the bubbling `focusout` with `move_focus_serviced(..., false)`, then one `sync_commit`, the same shape as a removed focus. Stopping the blur does not stop `focusout`. Because `sent` is true, the outer `service_commit` returns before its own `autofocus`/`arrive`, so those run once inside that `sync_commit`, after `focusout`. That is the `"-oF"` order. The old per-event `after_commit` autofocused in the gap and produced `"-Fo"`. When nothing listened, `sent` is false, `sync_commit` is skipped, and the outer `service_commit` autofocuses after the focus is already `None` (the ancestor case).

`set_focus` is the only caller that passes `service` true. `move_focus_serviced` is private.

**What this presenter covers.** Android app crates and `host/windows` both use this `Presenter`. `remote()` is `pointer == None`, so a phone or a desktop blurs and a TV does not. `the_remote_leaves_a_control_disabled_under_the_focus` still keeps the disabled button and moves it with an arrow. Shipping macOS/iOS hosts are separate; the pending Apple probe is still outside this commit. The painted Apple benchmark carrier links the same presenter and is marked non-shipping.

**Controls versus the web.** `disabled_control` is a `button` (`Pressable` with no `href`), a text field, or a `Control`. A link is an `<a>` and is excluded. Indeterminate progress is a `div` on the web, and a determinate `progress` does not match `:disabled`; both are excluded in `focus_fixup`. That matches `glue.js` and `rt.js`, which blur only when the focused element itself matches `:disabled`.

**IME, left as proposed.** A disabled field already fails `composing_field`, so the platform session is gone before this blur, and a later `ime_commit` drops the text. That predates the commit. The new part is `commit_text` on the way out: `ime_owns` is false once `disabled` is set, so `change` fires with `field_text` (the `chosen` value `edit_field` stored during preedit), then `blur`. That is the same text a normal blur would commit after composition had been applied, not a second compositionend. Headless Chrome still cannot show the platform event. I would not block the focus fix on it.

`cargo test -p exact-linux --lib focus_disabled` did not run: the `exact-js` build script panics because `node_modules/.bin/rolldown` is absent. The red/green claims above are from the control flow, not from a run.

APPROVE

Notes, not blockers:

- `set_control_value` (`control.rs` around the input/change loop, used by a range press and a select menu choice) still fires input and change and only then `after_commit`. A select whose `input` disables it and whose `change` re-enables it still keeps focus. Checkbox and radio sets do not use that loop.
- The conform fixture is a virtual `clock +200` over `after(100)` with the log in the tree. I did not re-run `conform.mjs --linux`.
- Radio has the same hook as the checkbox and no `"ibc"` twin. The ancestor case is covered with a child that has no blur handler, which is the `sent == false` path.
