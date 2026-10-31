# Code review: Linux focus on a control a commit disables, round 3-recheck2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `28d6bc52c1`.
- **Method:** one brief (sha256 `95188dbb34449021275fe8ac172c3f95e6a3462b4386dbd63d30d58037a832b0`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll re-check the HEAD commit read-only: the blur-between-input-and-change fix, and whether any other path still fires two focus-relevant events before the fixup runs.The commit is the Linux disable-blur fix. I'll read the input/change paths and every other place that can dispatch two focus-relevant events before servicing.The range path now blurs between input and change. Next I’ll compare that with the checkbox and radio loops, and scan every other multi-event dispatch.The range loop now blurs after a dispatched input. I’ll check the other multi-event dispatch sites, including text fields and menus, for the same gap.The `set_control_value` fix holds. A focused control whose `input` disables it blurs before `change`, and no other value path still fires two of those events before that fixup.

`set_control_value` dispatches `input`, then `change`, and only then `after_commit`. It now calls `blur_disabled_now()` before `change` when `input` was actually dispatched, the same gate the checkbox and radio loops use. That helper sees the disable already applied by `dispatch_at`'s commit, and `blur_disabled_focus` moves the focus off, runs `blur` and `focusout`, then one `sync_commit`. `change` runs after that. If `change` enables the control again, `clear_removed_focus` finds no focus and does not put it back.

For `a_range_its_input_disables_blurs_before_its_change_runs` that is `input` (`"i"`, disabled), `blur` (`"b"`, focus `None`), `change` (`"c"`, enabled again). The log is `"ibc"` and the focus stays `None`. A missing `input` handler leaves `dispatched` false, so a lone `change` is unchanged and the fixup stays in `after_commit`. A select still cannot show the `"c"`: `control_payload` refuses a `change` whose option is disabled, but the blur still runs before that dispatch.

The other value deliveries do not have this pair. Checkbox and radio already blur between click, `input`, and `change`. A field edit, a hatch `input`, `commit_text`, and a native-module event each dispatch one event and then service. `deliver_picker` dispatches one `change` or `cancel` and then `after_commit`. Date, time, and shell picker answers go through `set_control_value`, so they get this fixup. A file or save picker does not.

APPROVE
