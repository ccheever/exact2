# Code review: Linux focus on a control a commit disables, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `37ec2cea19`.
- **Method:** one brief (sha256 `834a7b01323ba96e112929bdb81e20b711689a17119dcf490a81508c93dbc88a`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **[P2] Ancestor press handlers still discard focus acquired during the commit** — [surface_controls.rs:983](host/linux/src/surface_controls.rs:983).

   With `off=false` and `go` setting `off=true`, tap `child`:

   ```text
   column press=go tabindex=0 autofocus=off testId="parent"
     button disabled=off testId="child"
       text "Go"
   ```

   The handler target is `parent`, but the focused control recorded in `disabled_blur` is `child`. After the child blurs, autofocus focuses the parent. `contains(&target)` is false, so the pointer clause overwrites that focus with `fallback=None`, without dispatching blur. Both web autofocus implementations retain the parent’s focus.

   Preserve the focus resulting from the disabled blur even when the press handler belongs to an ancestor, and add this tap case.

2. **[P2] Multi-event controls can miss the disabled state entirely** — [events.rs:154](host/linux/src/presenter/events.rs:154), reached after the event loop in [control.rs:152](host/linux/src/presenter/control.rs:152).

   A focused checkbox dispatches `input` and `change` before calling `after_commit()`. If `input` disables it, `blur` appends `"b"`, and `change` appends `"c"`, Linux produces `"cb"`; the web’s immediate fixup produces `"bc"`. If `change` also reenables it, Linux never observes the disabled state and retains focus without any blur. Radio activation has the same batching.

   Service the focus fixup between these event commits and cover both event order and disable/reenable.

Non-blocking notes:

- The ordinary commit → blur → focusout sequence, progress/link/ancestor exclusions, and TV remote exception look correct.
- Re-entrancy still differs from removal: `set_focus()` enters full `after_commit()` processing, including collection refinement. I found no additional concrete failure, but that deserves a collection regression case.
- The synthetic fixture uses controlled time. Neither blocking case is covered by the added tests.
- These findings are source-traced, not executed: both permitted Cargo commands failed because the sandbox denied creating the target directory. Full conformance, caps, and native adapter qualification were not independently verified here; the Apple probe remains pending as specified.

CHANGES
