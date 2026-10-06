# Round 1
I'll review the newest commit against LLP 1069.001 D4 and the web text-field behavior, starting with the review guidance and the commit itself.This is a read-only review of `HEAD` against D4 and the web field behavior. I'll read the project notes and the commit, then check the surrounding paths.The commit stores typed text in the presenter's `chosen` map. I'll read how that map is used and how the web build writes `value`.`[P2] Forget a field's typed text once its bound value changes — host/linux/src/presenter/typing.rs:561`

`keep_typed` stores `(typed, bound-at-the-time)` and leaves that pair in place until a later edit of the same field. `choice` only hides it while the current `value` differs, so when the bound string comes back the draft is shown again, and committed again on the next blur. That happens on the path this commit is for: a field with `value=` and `change=` (no per-key write) types `"ab"` against `""`, Enter runs `commit_text` and returns at line 473 without reconciling, the action sets the slot to `"ab"`, and the pair `("ab", "")` stays. A later reset to `""` paints and reports `"ab"`. The same thing happens for an `input` handler that writes nothing (`("mine", "kept")` in the new test) if that slot moves away and returns. The web build does not do this: `applyProps` / `P` assign `el.value` on every change of the bound value (`host/web/glue.js`, `host/web-js/rt.js`), so the trip back overwrites the element. Apple does the same in `applyProps`. Drop pairs whose node is gone or whose current `value` is no longer the snapshot, in `service_commit` (it already runs after Enter, blur, and later slot writes). `keep_typed` still inserts after that when this edit did not change the bound value. Select, date, and range entries in the same map resurrect the same way; that sweep matches "shown again when it changes" and does not drop a choice whose bound value is still the snapshot.

The cases the commit names otherwise match the web build. A handler that writes the typed string, or a different one, changes the bound value, so `keep_typed` removes the pair and paint, commit, and `type` show the prop. A handler that writes nothing, an unbound field, and a literal `value="x"` keep the draft, because the bound string did not change. Hardware edits, `selectText` replacement, maxlength (longer than the shown text, not the prop), textarea Enter (a newline, no commit), password bullets, and the placeholder all read that shown text. Blur still commits it; moving focus does not clear it. Destroyed views cannot pick up a stale pair: runner view ids are not reused. Nothing else under `host/linux` paints or commits a text field from the `value` prop; the remaining reads are the bound snapshot and the fallback when no pair matches. `tree` overlays `props.value` from `field_text` for every `chosen` id, so it shows the same ghost, and it also shows a select, date, or range choice while its pair matches, which the new comment states.

The new test covers unbound text, a refused write, and a rewritten write. It does not cover a bound value that changes and then returns, nor a literal `value`. Content-sized fields (`field-sizing: content`, not the default) are still measured in the kernel from the `value` prop, so a draft that is not the prop does not resize the box.

# Round 2
I'll review the two newest commits on HEAD without changing files, starting with the diffs and the surrounding presenter code those fixes touch.The branch is two commits ahead of its base. I'll read both diffs and the presenter paths they change, then check whether the dirty flag, the `chosen` sweep, and the text-only overlay actually hold.The diffs are small. Next I’ll read how `chosen` is written and swept, and the order of dispatch, `after_commit`, and `keep_typed`.The sweep keeps a select, date, range, or typed entry whose node is still there and whose `value` string still equals the stored snapshot. `keep_typed` and `set_control_value` both write `chosen` after `after_commit` returns, so that sweep does not remove the entry they just stored. The tree overlay is limited to `TextInput` and reports `field_text`. Two holes remain.

**Enter commits a new bound value without ever sweeping it, so the typed text comes back.** `commit_text` dispatches `change` and returns (`host/linux/src/presenter/typing.rs:527-531`). The Enter path then returns, and calls `after_commit` only when the field also has a `submit` handler (`host/linux/src/presenter/typing.rs:466-474`, `host/linux/src/presenter/events.rs:176-188`). The sweep runs only inside `service_commit` (`host/linux/src/presenter.rs:668-672`).

Sequence: the field shows typed `"mine"` against bound `"kept"`, so `chosen` holds `("mine", "kept")`. Enter's `change` sets the bound value to `"other"`. Nothing sweeps, and nothing sets `dirty` (`after_commit` is what does, at `host/linux/src/presenter.rs:631`), so the last frame keeps painting `"mine"`. The next `service_commit` drops the entry only while the prop is still `"other"`. A later action that sets the prop back to `"kept"` is often that first sweep: `"kept"` still matches the snapshot, the pair stays, and `"mine"` is shown again. A paint in between does not help. `frame` does not sweep.

The same sampling misses a round trip inside one turn. `set_focus` dispatches `change`, then `blur`, then sweeps once (`host/linux/src/presenter/events.rs:82-101`). If `change` moves the bound value and `blur` puts the old value back, the sweep sees the original string and keeps the typed text. `submit` after `change` on Enter does the same when the submit handler restores the old value.

**A `field-sizing: content` field lays out the bound prop, so unbound typed text does not change its size.** Content sizing measures `PropId::Value` (`kernel/src/arena.rs:630-634`). Typed text lives only in the presenter's `chosen` map (`host/linux/src/presenter/typing.rs:558-570`). The default `field-sizing` is `fixed`, whose box ignores the live string. With `content`, the kernel keeps measuring the prop (often empty) while paint draws the typed text, so a textarea does not grow and the hit box stays the old size.

# Round 3
I'll review the three newest commits on HEAD, with the latest change as the focus, and check the surrounding code for the defects you named.The branch is three commits ahead of origin/main. I'll read those diffs and the surrounding presenter and host code before judging the HEAD fix.The HEAD change records every commit's bound value and drains that log before a choice is kept. I'll read the host, presenter, and control paths that produce and consume it.The separate-commit paths from round 2 are fixed. One hole is left: a batch of receipts is snapshotted after every receipt has already been applied, so a value that changes and comes back inside that batch never shows up in the log.

`Host::commit_effects` walks each receipt and reads the live `value` prop:

```1214:1225:host/linux/src/host.rs
            for node in r
                .touched
                .iter()
                .filter_map(|k| self.runner.kernel().node_by_key(*k))
            {
                if matches!(
                    node.node_type,
                    exact_kernel::NodeType::TextInput | exact_kernel::NodeType::Control
                ) {
                    let value = node.props.str(exact_kernel::PropId::Value).unwrap_or("");
                    self.bound_values.push((node.id, Some(value.to_owned())));
                }
            }
```

The runner applies every action before it returns the slice. `advance_within` pushes a receipt only after `run_action` (`runner/src/runner/commit.rs:455-457`), then `Host::advanced` (`host/linux/src/host.rs:972-975`) hands the whole vec to `commit_effects`. The same shape is `fulfill_all` (`host/linux/src/host.rs:782-804`) and `collection_feedback_filled` (`host/linux/src/host.rs:517-529`). By the time the loop runs, every touched field reads the value after the last receipt. A timer that sets `kept` to `"other"` and a second timer or `then` in that same advance that sets it back to `"kept"` are both stored as `Some("kept")`. `forget_replaced_choices` (`host/linux/src/presenter/typing.rs:537-546`) only drops when a recorded value differs from the snapshot, so the typed text stays.

That is the round-2 resurrection, on the paths that batch. `dispatch_at` still commits one receipt at a time (`host/linux/src/host.rs:852-858`), which is why Enter, blur in `set_focus`, and the new test (`events_tests.rs:691-699`, two presses then `after_commit`) are fine. Destroyed and renewed nodes are unaffected: those push `None` from the receipt (`host.rs:1206-1211` and `1234-1239`) and do not read the prop.

Drain-then-insert is in the right order. `service_commit` only drains (`presenter.rs:664`). `keep_typed` (`typing.rs:576`) and `set_control_value` (`control.rs:172`) drain before they insert, so the history cannot delete the entry just stored. `bound_values` is not unbounded: `take_bound_values` empties it, and that runs on advance, frame, pump, and the input paths. Entries from a commit after that take last until the next drain. `Presenter::replaced` clears `chosen` (`presenter.rs:1328-1331`) and the new host starts with an empty log.

