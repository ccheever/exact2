# Code review: Linux focus on a control a commit disables, round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `59346a6d0e`.
- **Method:** one brief (sha256 `88f0edfbb04da83426eb8764eb1d135e87fb7bfbc08efa3b0d73921863fa014d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review this commit read-only against the focus-parity conditions you listed, starting with the diff and the host paths that move or keep focus.The diff is small. I'll read the focus paths around it — commit servicing, press fallback, and the web fixup — before judging event order and re-entrancy.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I'll re-check the four candidate defects against the current lines before writing the verdict.CHANGES

1. `host/linux/src/presenter/events.rs:191` — `disabled_control` blurs every disabled `Pressable` and every disabled `Control`. HTML's focus fixup (`host/web/glue.js:461`, `host/web-js/rt.js:703`) blurs only when `el.matches(":disabled")`. A pressable with `href` is an `<a>` (`host/web/src/element.rs:488`), and both progress kinds miss `:disabled` too: an indeterminate `progress` is a `div` (`element.rs:573`), a determinate one is `<progress>` (`element.rs:574`). The predicate the rest of this presenter already uses (`host/linux/src/surface_controls.rs:14`) excludes the link, so `focusable` still treats that link as focusable after this path has blurred it. `ControlKind::Progress` and `ProgressBar` need the same exclusion.

   Failing input: focus `link href="/docs" disabled=off focus=note("+") blur=note("-")`, then set `off` from another control. The web keeps the focus and runs no blur. This path clears the focus and runs the blur. The same split happens for `progress disabled=off tabindex=0` (and for `progress value=0.4 max=1 tabindex=0`).

2. `host/linux/src/presenter/events.rs:225` — `if self.take_focus_commands() { break }` aborts the whole list. That break is right for the removed-node path, whose list is only `focusout`. Here the list is `blur`, then `focusout`. `stopPropagation` on `blur` therefore drops every later `focusout`. `set_focus` does not: a stage change clears `stopped` (`events.rs:461`), and `stopPropagation` is honored only for `focusout` and `focusin` (`events.rs:491`).

   Failing input: a column with `focusout=note("o")` around a button whose `blur` action calls `stopPropagation()`, pressed so the button's own commit sets `disabled`. `set_focus` and `el.blur()` run the column `focusout`. This path runs `blur` and then stops, so the column never hears it.

3. `host/linux/src/presenter/events.rs:219` — `commit_text` runs, then `sync_commit` runs only when `sent` is set by a `blur` or `focusout` dispatch (`events.rs:231`). `commit_text` returns `Some` when it dispatched `change` (`typing.rs:746`), and `set_focus` treats that as a dispatch and always `after_commit`s (`events.rs:425`, `events.rs:494`). This call sits inside `service_commit` after `take_commands` (`presenter.rs:878`) and `take_requests` (`presenter.rs:818`). With no `blur` and no `focusout` listener, `sent` stays false, so `focus()`, `send`, and anything else that action queued stays on the runner. `run_commands` only drains `self.commands`, and the next `pump` does not call `service_commit` unless the executor already has a reply (`pump.rs:42`).

   Failing input: `input id="q" disabled=locked input=lock change=save`, where `lock` sets `locked = true` and `save` calls `focus("next")` (or `send`). Type one character. `input` disables the focused field, `change` runs, the focus becomes `None`, and `next` never takes the focus until some later, unrelated commit.

4. `host/linux/src/presenter/events.rs:216` together with `host/linux/src/presenter/ime.rs:47` and `host/windows/src/ime.rs:41` — the new path sets `focus = None` and then `commit_text`s while the field is already disabled. `composing_field` treats disabled as "no composition" (`ime.rs:83`), so `ime_owns` is false and `change` commits `chosen`, which is the unconverted preedit (`field.rs:960`). On the next Windows frame `ime_target()` is `None` (it requires `self.focus`), so `InputMethod::sync` calls `complete()` (`CPS_COMPLETE`). The resulting `Ime::Commit` goes through `ime_begin`, which wipes `self.ime` because `composing_field()` is `None` and then returns false because the focus is gone (`ime.rs:47`). The converted string is dropped. A normal `set_focus` blur leaves the field enabled, so `ime_owns` holds `commit_text` back and that same `Commit` still applies. This presenter is the Windows host's presenter. Android is the same early commit: `blur_disabled_focus` fires `change` and `blur` before the reader applies the statement, `Fields.kt:520` then sets `text.isEnabled = false`, and `reader_field_edited` drops the composition-end edit because the node is already disabled (`hosted.rs:526`). The drop-on-disabled check is older; emitting `change` on the pre-confirm string in this turn is this commit.

   Failing input (Windows): focus an `input`, `ime_begin` / `ime_preedit("かな")`, then disable that field from a task or an `input` handler. `change` observes `かな`. The IME's confirmed reading never arrives. On Android, disable the hosted field while Gboard still has an underlined word: `change` observes the underlined text, and the word the keyboard then commits is ignored.

Non-blocking notes

The TV exception is in the right place. `remote()` is pointer-none (`remote.rs:149`), checked before the blur, and `the_remote_leaves_a_control_disabled_under_the_focus` still expects Select to leave the focus on the disabled button. Phones and Windows have a pointer, so they take this blur. That is the shared-presenter impact the approver asked to be named; these unit tests only exercise the shared logic.

`sync_commit` matches the removed-focus path: it re-enters `service_commit` without refining collections. Keyboard activation goes through `dispatch_press`, and the fallback filter there drops a control this commit disabled, so the button is not focused again and `blur` does not run twice. `autofocus` still runs in the nested commit when an event was sent, and it will not focus a disabled control. A `focus()` command from the `blur` handler is queued rather than applied before `focusout`, as in `set_focus`. `field_reveal` and the focus ring follow `set_focus(None)` closely enough. `aria-disabled` is left alone, which matches `:disabled`.

A control that one commit both disables and route-hides takes the blur path and is not recorded as `route_focus.inert`, so a later return to that route will not restore it.

The two positive tests (`a_focused_control_its_commit_disables_loses_the_focus_and_runs_its_blur`, and the tap test) fail on the old presenter: the focus stays and the log is `+` rather than `+-`. The tap also fails if the press fallback puts the focus back. The ancestor-box test and the removed-node test pass without this fix; they are guards. Nothing covers a link, a progress, `stopPropagation`, a `change`-only field, or an open composition.

`disable-blur.steps` advances with `clock +200`, which is what fires `after(100, done)`. A bare `clock settle` would not. The compare includes `focused` and the log text, and the script is deterministic on hosts that share the runner clock. I did not re-run the conform harness or fieldnotes.

The macOS/iOS probe is outside this commit, as agreed.

`cargo test -p exact-linux --lib focus_disabled` and `cargo test -p exact-linux --lib remote` both died at compile time here (`EXIT 101`): `js/build.rs:166` panics because `rolldown` is missing at `node_modules/.bin/rolldown`. I did not install it. The assertions were not executed.

Verdict: CHANGES
