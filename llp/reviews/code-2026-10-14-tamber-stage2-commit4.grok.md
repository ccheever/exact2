# Code review: LLP 1121 stage 2, commit 4 (setMode and prefer mode), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `3af0ad820b53b295897a357c986c16685451268a5cfcc3a398229f2d37140612`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 1's commit

---

`prefer mode` commits pins before the rest of the request is known to be valid. A later refusal leaves the earlier mode applied and reports an error.

**[MAJOR] `prefer` applies mode pins one commit at a time, before the other facts are checked — `host/linux/src/agent.rs:536`**

`prefer` says nothing applies unless every fact is known. Media and page still follow that: they are parsed into locals and applied only after the whole group validates. The new mode loop does not. Each pin calls `set_mode`, which commits immediately, and only then does fold, media, and page run.

Two requests that the CLI accepts both leave a new mode in place and an error reply:

- `prefer mode dark-style:dim density:bogus` — the pin syntax check in `scripts/agent-prefer.mjs` does not know the design. `dark-style:dim` commits; `density:bogus` returns `prefer mode density: …` from `Runner::set_mode` before `set_window`. The window stays on dim.
- `prefer mode dark-style:dim color-gamut p3` (or `display-mode standalone`) — `preferGroups` allows both. Linux rejects `p3` and `display-mode` only at `host/linux/src/agent.rs:571` and the `_` arm, after dim has already committed. The reply is an error with no `mode` field; `modes()` is already dim.

The same loop is on the other hosts:

- `host/apple/Sources/ExactKit/Agent.swift:431` — `setMode` plus `session.apply(batch)` before `preferFold` and the media/page checks. Swift dictionary order is unordered, so which earlier pin remains is not stable.
- `host/web/glue.js:1151` — `pageChanged()` and `foldChanged()` always run, then each pin is `exact_set_mode` + `applyBatch`. `batch.error` returns with earlier pins, and any page or fold in that request, already applied. `scripts/agent-prefer.mjs:60` emulates media through CDP before `ask()`, so a mode error also leaves the browser's media changed.

A single bad pin still changes nothing: `set_mode` rejects an unknown axis or value before `set_window`. The Linux test only covers that case and one successful pin.

On a non-poison failure, `Runner::set_mode` (`runner/src/runner/mode.rs:109`) rewinds the window after `conclude` has already put back the checkpoint tuple. That checkpoint was taken after `set_window`, so the rewind is what restores the old tuple, and `moved` is set so the next visit re-themes to it. A poisoned update is different: `conclude` deliberately leaves the tuple with the mutated tree (`runner/src/runner/commit.rs:137`), and this rewind then points `modes()` at the old tuple on a dead runner. Minor next to the prefer bug; skip the rewind when `poisoned` is set.

What holds up:

- An action `setMode` whose commit is refused before `update` never touches the window. `take_set_modes` runs at `runner/src/runner/commit.rs:919`, after settle and `gate_step` return at line 892. The checkpoint truncate drops the command. An `update` failure poisons and does not restore the tuple, which matches the tree that was already written.
- Several `setMode`s in one action are removed without skipping, so later commands stay in order. `first_command` still bounds the journal, the `scrollIntoView` drain, and `apply_sounds`. The last valid pin wins. An invalid computed pin is journaled and does not undo an earlier valid one. An unchanged value does not set `moved`.
- The visit after a real change re-themes mounted rows and rows mounted later. `stale` includes `retheme`, so a collection does not take the early-out. `collection.around` is saved after `enter`, which has already stored the list node's new tuple. `create_row` and `rebind` install that `around`.
- A colour token does not lay out. `background_color` is not in `StyleMask::LAYOUT`; `padding_top` and `font_size` are. `changed_mask` marks only rows whose resolved value changed, and `txn.rs` lays out only on `StyleMask::LAYOUT`. That is D5: the new slice is in the action's own commit, and layout runs only where a selected value is in a layout row.
- The JS target refuses `prefer mode` before page or fold, and journals `setMode` without applying it. That matches commit 6's deferral.

No test covers a second bad pin, a mode pin followed by a fact the host rejects, a refused action commit, several `setMode`s beside another command, or a virtualized row mounted after `setMode`. Apple and web have no prefer-mode test.

VERDICT: fix (MAJOR host/linux/src/agent.rs:536, host/apple/Sources/ExactKit/Agent.swift:431, host/web/glue.js:1151; MINOR runner/src/runner/mode.rs:109)
