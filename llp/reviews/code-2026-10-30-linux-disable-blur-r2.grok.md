# Code review: Linux focus on a control a commit disables, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `c82b5ee79c`.
- **Method:** one brief (sha256 `1cc57928902c0cd6802de2f74dacef6931402f96b9549ad0ec09f5ede7ea97bc`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

## Round-1 items

**Event order.** `set_focus` (`events.rs:406`) commits a typed field, then stages `blur`, then the bubbling `focusout`. `stopPropagation` sticks only on stages 1 and 3, so a stopped blur still runs `focusout`. Each event calls `after_commit`, and a change with no focus handlers still gets the final `after_commit` when `dispatched` is set. A subtree removed during blur uses the existing detached `focusout` path. The change commit is serviced with the blur’s `after_commit`, which is how every blur on this host already works.

**Re-entrancy.** Focus is cleared before the handlers run, so the nested `service_commit` inside `set_focus` sees `focus == None` and does not blur again. `autofocus` and `lands` both skip `disabled_control`, so they will not focus a control that is still disabled. The removed-focus path is unchanged: `sync_commit`, and `focus_sent` makes the outer `service_commit` return. This path returns false on purpose, so the disabling commit keeps being serviced. Nested `finish_commit` is the same shape autofocus already causes by calling `set_focus` from inside `service_commit`.

**Eligibility.** `disabled_control` (`surface_controls.rs:14`) is a disabled `Pressable` with no `href`, or a `TextInput`, or a `Control`. That is the web’s `:disabled` set: `button`, `input`, `select`, `textarea`. An `<a>` does not match, and both progress kinds are subtracted. Indeterminate progress is a `div` (`host/web/src/element.rs:573`); a determinate one is `<progress>` (`:574`). Neither matches `:disabled` in `glue.js:461` or `rt.js:703`. A control inside a disabled box is not itself `disabled_control`, and the box test keeps its focus. There is no fieldset node in this host.

**Re-enable.** `dispatch_press` clears `disabled_blur` on entry and drops the fallback when this press’s commit blurred that target (`surface_controls.rs:982`). The predicate does not look at the control’s final enabled state, and it applies to keyboard activation as well as pointer activation. Both go through `dispatch_press`. The re-enable test uses `tap()`, so it hits this path.

**Route plus disabled.** The new branch requires `!route_visibility(id).1`. A control that is disabled and route-hidden in one commit falls through to `release_focus`, which stores `route_focus.inert`.

**Remote, Android, Windows.** `remote()` is `preferences.pointer == Pointer::None` (`remote.rs:149`, viewport bit 6). Phones are coarse (bit 5) and take the blur. Windows is fine and takes the blur. Android TV and `tv: true` keep the focus. The commit message states that three-host scope. `the_remote_leaves_a_control_disabled_under_the_focus` still encodes the exception. The macOS/iOS probe stays out of this commit, as agreed.

**Other focus owners.** `focus()` is a queued command (`commands.rs:101`); it is not applied inside the handler. `take_focus_commands` keeps it. Window restore (`events.rs:39`) requires `focusable`, so a control that is still disabled is not restored. A page under a sheet is route-inert, so this blur is skipped and the route path runs. Surface focus is a separate index. `reader_field_focused` will not put focus back on a field that is still disabled, and the reader statement marks a disabled field `INERT`. `field_reveal` is set even when blurring to nothing; `reveal_field_cursor` returns immediately when `focus` is `None` (`field_scroll.rs:61`). With no focused node, no ring is drawn.

**IME.** The author’s account is right about the drop, and only half-right about Chrome. `composing_field` (`ime.rs:83`) already treats a disabled field as having no composition, so `ime_commit` / `ime_end` already discarded the platform string and cleared `self.ime` while the field was disabled. That predates this commit. What is new is `commit_text` at this blur: `ime_owns` is then false, so `change` can fire. On desktop the value is `field_text`, and `ime_edit` has already stored the preedit in `chosen`, so `change` carries the preedit snapshot. The IME’s later converted `Commit` is still dropped if it arrives while the field is disabled. Chrome commits the finished composition string. On the hosted reader, composition text lives in `hosted.composing`, which `field_text` does not read, and `reader_field_edited` already returns when the field is not editable, so a `change` here can carry the pre-composition value and run save or validation that did not run before. That is a real edge, and it belongs on the separate IME item. The fieldnotes bug is buttons.

**`focus()` during blur.** The same queue exists for every blur. `type_text` runs commands after the initial `set_focus` and not after `apply_field_edit`, so the new field test never flushes `focus("next")`. A later `run_commands` on a real turn would still run it, and the press fallback filter does not block that later command. The comment at `focus_disabled_tests.rs:148` overstates the gap. Leaving it as its own item is right.

## Tests and the fixture

The tests that should be red without the fix are red for the right reason: a press and a tap leave `focused` and a log of `"+"`, a re-enable leaves focus on the button (and round 1’s fallback restored it after a second blur), `stopPropagation` on the hand-rolled blur dropped `focusout`, and a self-disabling field never reached `change` then `blur`. The box-ancestor test and the removed-control test pass on the parent; they are regression guards. The link test also passes on the parent. It is red only against round 1’s predicate, which blurred `<a>`. The commit message’s “Red before” list includes that link test; against `HEAD~1` it is green.

`disable-blur.contract` / `.steps` compare wasm, JS, and Linux after each step’s `clock settle`. Settle drains live timers (`agent.js` moves the clock to the next timer due), so `after(100)` on `tap self` runs before that step is compared, on every host that shares settle. The following `clock +200` is then idle. The fixture does not cover links, progress, `stopPropagation`, or a field’s `change`.

I could not run the suite. `cargo test -p exact-linux` fails in `js/build.rs:166` because `rolldown` is not at `node_modules/.bin/rolldown` and `EXACT_ROLLDOWN` is unset. I did not install it or edit the tree. The `remote` filter was not started. Conform was not re-run. The author’s “10/10 with `--linux`” and “exact-linux’s suite passes” are unchecked here.

## Non-blocking

1. `surface_controls.rs:982`. After the filter turns this press’s fallback into `None`, the existing pointer clause `fallback != Some(target)` is true. If the blur’s nested `service_commit` has put focus back on that same target, a pointer press assigns `self.focus = None` with no `blur` or `focusout`. Keyboard activation does not enter that clause, so it keeps the refocus. Concrete case: an earlier `autofocus` control is already processed, a later button also has `autofocus`, a pointer press sets `busy`, and the blur handler clears `busy`. Nested autofocus focuses the button (`focus` / `focusin`), then the assignment drops it. A button that was itself the first autofocus at boot is already in `autofocus_processed`, and the committed re-enable test has no `autofocus`, so that test stays green. A `set_focus(None)` at that assignment, or skipping the assignment when `disabled_blur` already blurred the target, would close it.

2. No unit test that a disabled progress keeps focus. No `pointer: false` test of the fallback filter (same line as the pointer path). No test whose blur handler removes the node, and no test for disabled plus route-inert. The synthetic fixture omits those too.

3. A disable-blur inside `service_commit` runs a full nested `finish_commit`, then the outer `service_commit` continues into `autofocus`. The nested pass can focus the next unprocessed `autofocus` candidate; the outer pass can mark one further candidate processed and leave it unfocused, because focus is already set and is not `autofocus_from`. One extra candidate burned. Same family as autofocus already calling `set_focus` from inside `service_commit`.

4. Desktop IME `change` at this blur carries the preedit snapshot, and the platform’s later `Commit` is dropped while the field stays disabled. Hosted `change` can carry the pre-composition text because `field_text` ignores `hosted.composing`. Separate item, as above.

5. `focus("next")` from `change` is queued until `run_commands`. The field test does not flush commands, and its comment says the command is not honored.

6. The allowed tests did not run here (`rolldown` missing). Conform was not re-run.

APPROVE
