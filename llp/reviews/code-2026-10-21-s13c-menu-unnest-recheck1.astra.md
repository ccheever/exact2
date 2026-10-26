# Code review: exact2's menu rule (a popover a menu row opens, opened from the menu's own invoker), narrow re-check after round 3 (theme2 02:15), 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `f57839b257`.
- **Method:** one brief (sha256 `22ba769942731068981b38b462b489f314972bedea3fb5991d811ab77b41e429`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG (one web blocker)
- **Disposition:** taken in 78b17fb2f7 and fecbf16a85 (replay after the press)

VERDICT: WRONG

1. **blocker — [host/web/menu-rows.js:54](/tmp/iosfix-rv-mu4/host/web/menu-rows.js:54): invocation is classified before the press.** This capture listener runs before the row’s handler. If a dialog row’s press changes it to invoke a popover, the guard skips interception; HTML opens the new popover with the row as source, keeping it nested. Conversely, a popover row retargeted to `show-modal` has already had its activation cancelled, and `act()` returns at line 35 without opening the dialog. Fix 1 therefore handles the static case but fails both retarget directions; the added test covers neither. The decision must honor the post-press invocation while preserving non-popover commands. See [DOM dispatch ordering](https://dom.spec.whatwg.org/#concept-event-dispatch) and [HTML button activation](https://html.spec.whatwg.org/multipage/form-elements.html#the-button-element).

Fixes 2–4 look correct statically. Both native walks visit each entry at most once, terminate on cycles, and reach the outermost opener on acyclic chains beyond eight menus. A static dialog `commandfor` correctly takes precedence over `popovertarget`. I found no additional regression in the other named invariants.

Static review only; no tests run or files changed.
