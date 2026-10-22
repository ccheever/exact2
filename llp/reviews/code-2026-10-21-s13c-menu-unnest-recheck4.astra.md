# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), narrow re-check of option B, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `6f2d000060`.
- **Method:** one brief (sha256 `7924bcd972005fbf498b6a444ed4c7ee9b7070c07fce7ed536b51cce704729ee`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG, no blockers
- **Disposition:** all four should-fixes and the nit taken in 73fb97808b; theme2 signed off 03:45 (nothing blocking)

VERDICT: **WRONG**

1. **should-fix — [host/web/menu-rows.js:17](/tmp/iosfix-rv-mu7/host/web/menu-rows.js:17).** The form-owner guard is incomplete. A `type="reset"` row naming a popover gets intercepted: its reset is canceled and the popover opens. A form-owned command button with missing/invalid `type` also passes because its `.type` getter returns `"button"`, although HTML’s Auto-state activation returns without invoking anything. Both must retain HTML’s behavior. [HTML button activation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element)

2. **should-fix — [host/web/menu-rows.js:21](/tmp/iosfix-rv-mu7/host/web/menu-rows.js:21).** The command lookup accepts inherited properties. `command="constructor"` or `"__proto__"` produces a truthy `action`, so an invalid command targeting a closed popover dispatches an event and opens it. HTML rejects these unknown commands. Use an explicit membership check or a prototype-free mapping. This defect predates the comparison base. [HTML command validation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element)

3. **should-fix — [host/web/menu-rows.js:47](/tmp/iosfix-rv-mu7/host/web/menu-rows.js:47).** Closing loses the invoking row’s identity. Native `hide-popover` and closing `toggle-popover` supply the row as the closing events’ `source`; `hidePopover()` supplies `null`. The popover closes, but `beforetoggle`/`toggle` listeners receive different information. This also predates the base. [HTML closing algorithm](https://html.spec.whatwg.org/multipage/popover.html#dom-hidepopover), [command activation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element)

4. **should-fix — [host/web/tests/menu-rows.test.mjs:106](/tmp/iosfix-rv-mu7/host/web/tests/menu-rows.test.mjs:106).** The command-row test checks opening and its source, but never observes `command` on `picker`. By inspection, removing the newly added command-event dispatch would leave these assertions satisfied. Add coverage for cancellation, adding/removing `popover` during the event, disconnection, and disabled fieldsets. The existing nesting and retargeting assertions are meaningful.

5. **nit — [host/web/tests/menu-rows.test.mjs:180](/tmp/iosfix-rv-mu7/host/web/tests/menu-rows.test.mjs:180).** The fixed 100 ms sleep does not establish completion before asserting that nothing happened. Use a completion boundary, consistent with `docs/testing.md`.

Otherwise, the cycle guard, submenu nesting, post-press popover retargeting, and Enter/Space click path look sound statically; Space and cycles lack explicit tests. The docs and QUEUE accurately record Option B’s accepted limitation.

Static review only; no files changed or tests run.
