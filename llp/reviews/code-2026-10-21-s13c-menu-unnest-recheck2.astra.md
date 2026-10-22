# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), second narrow re-check, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `fecbf16a85`.
- **Method:** one brief (sha256 `883420d8a741aead95aea2ff45454a782b5974c0a596e44f4695712d466f545f`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** four of five taken in c79a253c27; the fifth (a dialog's toggle source) unfixable; theme2 chose option B (02:54)

**VERDICT: WRONG**

1. **blocker — [host/web/menu-rows.js:44](/tmp/iosfix-rv-mu5/host/web/menu-rows.js:44): Invalid command/target pairs still fire an event.** `command="show-modal"` targeting a `<div>` dispatches `command` before the dialog check at line 47. HTML validates the command against the target **before dispatch**; dialog commands on non-dialogs produce no event. The second commit correctly filters unknown command names, but misses this validation. [HTML button activation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element)

2. **should-fix — [host/web/menu-rows.js:23](/tmp/iosfix-rv-mu5/host/web/menu-rows.js:23): Popover eligibility is captured before the command listener runs.** If a `show-popover` target initially lacks `popover`, and its command listener adds that attribute, HTML subsequently opens it. Here, `action` remains `null`, so replay stops. Determine eligibility after dispatch; retain the command independently of the target’s initial state. [HTML command processing](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element)

3. **should-fix — [host/web/menu-rows.js:49](/tmp/iosfix-rv-mu5/host/web/menu-rows.js:49): `close` lacks the post-event connection check.** A command listener can remove an open dialog. HTML then stops because the target is disconnected; this code still calls `close()`, changing its state and emitting closing events. Recheck connection after dispatch. [HTML button activation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element)

4. **should-fix — [host/web/menu-rows.js:48](/tmp/iosfix-rv-mu5/host/web/menu-rows.js:48): Dialog replay loses the invoker on toggle events.** Native dialog commands pass the row through to `beforetoggle` and `toggle` as `source`. Calling `showModal()`, `close()`, or `requestClose()` directly supplies `null`. The synthetic **command** event’s correct source does not preserve those subsequent events’ sources. [HTML dialog algorithms](https://html.spec.whatwg.org/multipage/interactive-elements.html#the-dialog-element)

5. **should-fix — [host/web/menu-rows.js:39](/tmp/iosfix-rv-mu5/host/web/menu-rows.js:39): The disabled check misses disabled fieldsets.** If the press disables an enclosing fieldset, `row.disabled` remains false and replay proceeds. HTML considers that row disabled. This existing guard now also governs the newly intercepted dialog commands; use effective disabled state, such as `:disabled`. [HTML disabled controls](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#concept-fe-disabled)

The [new test](/tmp/iosfix-rv-mu5/host/web/tests/menu-rows.test.mjs:166) meaningfully catches both original failures: missing modal opening and a picker retaining the menu as its ancestor/source. Static inspection finds no regression in outermost-opener selection, submenu nesting, toggle closing, missing-command-target fallback, or hiding the menu.

The ordinary open/connected guards, optional button value, command cancellation, and custom-command casing are correct. `requestClose?.()` avoids an exception when unavailable, but then performs no close. No tests were run or files edited.
