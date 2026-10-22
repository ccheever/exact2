# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), third narrow re-check (of the option A replay), 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `c79a253c27`.
- **Method:** one brief (sha256 `f38171b7f0afd247fc1859f799aafeb0efdf7637f876862d04023b4842a2c594`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** superseded by option B (6f2d000060), which leaves every non-popover row on HTML's path

VERDICT: WRONG

1. **should-fix — [host/web/menu-rows.js:51](/tmp/iosfix-rv-mu6/host/web/menu-rows.js:51):** Dialog replay omits the `:popover-open` guard. An open, nonmodal dialog can also be showing as a popover. HTML dispatches its command event but then ignores `close` and `request-close`; this code calls the methods, potentially changing `open`, `returnValue`, and firing closing events. Check popover state after command dispatch. [HTML dialog command steps](https://html.spec.whatwg.org/multipage/interactive-elements.html#the-dialog-element).

2. **should-fix — [host/web/menu-rows.js:25](/tmp/iosfix-rv-mu6/host/web/menu-rows.js:25):** Built-in popover commands also require an HTML target. A row with `commandfor="icon" command="show-popover"` targeting `<svg id="icon">` reaches event dispatch here; HTML rejects it without firing an event. Add the HTML-element check for built-ins while allowing custom commands on other elements. [HTML command validation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element).

3. **should-fix — [host/web/menu-rows.js:81](/tmp/iosfix-rv-mu6/host/web/menu-rows.js:81):** Intercepting every invocation now suppresses native form activation for previously unintercepted dialog/custom-command rows. For example, a form-associated `type="submit"` row naming a dialog submits natively; here submission is canceled and the dialog command executes instead. Replay must respect the button’s form/type rules before resolving commands. [HTML button activation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element).

The [retargeting test](/tmp/iosfix-rv-mu6/host/web/tests/menu-rows.test.mjs:169) meaningfully catches both previous failures. The other listed cases look correct statically, including unknown-command rejection and the earlier menu behavior. No additional `requestClose` availability blocker found. No tests run or files changed.
