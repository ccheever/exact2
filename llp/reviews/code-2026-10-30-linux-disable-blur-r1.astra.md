# Code review: Linux focus on a control a commit disables, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `59346a6d0e`.
- **Method:** one brief (sha256 `88f0edfbb04da83426eb8764eb1d135e87fb7bfbc08efa3b0d73921863fa014d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I found five blocking correctness issues. These are code-derived repros; both requested Cargo commands failed before running tests because the sandbox refused the `/tmp/rv-rt1-db1-target…` directory.

1. **A text-field `change` commit can leave effects unserviced.** [events.rs:219](host/linux/src/presenter/events.rs:219) discards whether `commit_text` dispatched, then initializes `sent = false`. With no blur/focusout listeners, `sync_commit()` never runs. Concrete input: a field disables itself in `input`, and its `change` handler calls `focus("next")`. The command remains in the runner rather than reaching `self.commands`; `run_commands()` cannot execute it. Requests started by `change` have the same problem. Preserve the dispatch flag as `set_focus` does.

2. **The new predicate blurs elements that the web leaves focused.** [events.rs:193](host/linux/src/presenter/events.rs:193) includes every `Pressable` and `Control`. A pressable with `href` renders as `<a>`; progress controls render as `<div>` or `<progress>`. None acquires HTML disabled semantics here. Focus `link href="/docs" disabled=off` or `progress tabindex=0 disabled=off`, then set `off = true`: Linux now blurs it, while the web retains focus. The existing helper in `surface_controls.rs` already excludes links; the new duplicate loses that exception. Use consistent eligibility that also excludes progress.

3. **Re-enabling in blur defeats the fallback guard.** [surface_controls.rs:981](host/linux/src/surface_controls.rs:981) checks the final disabled value, after blur handlers have run. Change the fixture’s blur handler to append `"-"` and set `busy = false`. Pressing the button disables and blurs it, but the filter then accepts the newly enabled button and line 985 restores focus without a focus event. This affects keyboard activation too, which uses this path. Suppress fallback based on the focus loss that occurred during dispatch, not only the final property.

4. **Stopping `blur` incorrectly suppresses `focusout`.** [events.rs:225](host/linux/src/presenter/events.rs:225) breaks the entire event sequence when any handler calls `stopPropagation()`. A self-disabling button whose blur calls `stopPropagation()`, inside a wrapper with `focusout`, therefore never notifies the wrapper. `set_focus` correctly resets propagation between event stages: stopping blur must not cancel the separate focusout event.

5. **Removing a subtree during blur dispatches focusout to the wrong ancestors.** [events.rs:222](host/linux/src/presenter/events.rs:222) skips removed nodes and dispatches to surviving ancestors from the original path. Concrete input: an outer wrapper contains a conditional inner wrapper and self-disabling button; both wrappers observe focusout, and the button’s blur removes the inner subtree. The established behavior sends focusout through the detached inner subtree, excluding the surviving outer wrapper. This loop does the reverse. Preserve `set_focus`’s detached-path handling and the runner’s retained handler scopes.

Non-blocking observations:

- Clearing `self.focus` before dispatch prevents an immediate repeat blur of the same node. Using `sync_commit` follows the removal path and avoids recursive collection refinement; finding 1 concerns the missing invocation.
- Ordinary focus commands, autofocus, restoration and hosted-field focus callbacks check eligibility. The remote exception preserves the existing `Pointer::None` behavior. I found no separate regression in ring or `field_reveal` handling.
- Native shell surfaces maintain separate row-focus state; this commit does not establish parity there. Android hosted-field composition and Windows IME behavior remain unqualified by the added tests.
- The two self-disable tests should fail on the parent commit by inspection; ancestor-disabled and removal tests protect existing behavior. None exercises text commits, focusout ordering, detached handlers or synchronous re-enabling.
- The synthetic fixture uses virtual time, so its timer is deterministic. Conformance settles after each operation, meaning the first comparison can already follow re-enabling; the log still detects the original missing blur.
- The commit acknowledges Android/Windows impact and reports Linux, Fieldnotes and synthetic results. Full-conformance, caps and native-adapter coverage remain unverified here. The Apple probe is explicitly pending. `git diff --check` passed, and the checkout remained unchanged.

CHANGES
