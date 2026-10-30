# Code review: the web's interaction, phase 2, adversarial (sol)

- **Family:** OpenAI. GPT-6.1 Sol (`gpt-6.1-sol`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `ea23290bba` (8 commits on `a7918080ba`).
- **Method:** one brief (sha256 `323a60d05ff7ed3f26814e90a7e4bc9f8f8f3a73e05048e5493a1717cc45d9b1`), one reviewer. The author (Claude, Opus 5.5) is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths.

**Blocking**

- **32f7a73851 — The sheet’s scrim allows clicks through.** [neutral.css:431](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/neutral.css:431)  
  In a narrow Chromium, Firefox or WebKit window, open an unplaced popover, then tap a background action beneath the scrim. The action receives the click while the sheet dismisses. The change paints a modal presentation but adds no input blocking or focus containment. **Established by reading and reasoning:** HTML requires popover backdrops to have `pointer-events: none !important`. The geometry test manually supplies the sheet marker and never exercises dismissal. [HTML rendering rules](https://html.spec.whatwg.org/multipage/rendering.html#the-css-user-agent-style-sheet-and-presentational-hints).

- **5756526e9e — A suppressed click can still open another popover.** [input-glue.js:907](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/input-glue.js:907)  
  On the JS target, hold menu row A, programmatically scroll row B under the finger, then lift; B invokes a popover. `menuRows` registered its window capture listener before this guard, so it already queued `act()` before suppression runs. That callback never checks the cancelled event. **Established by reading registration order and running the extracted listeners with DOM stubs:** the click was suppressed, yet one popover opened.

- **038a86666e — One wheel event can indefinitely misclassify autonomous scrolling.** [input-glue.js:814](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/input-glue.js:814)  
  On a touchscreen laptop, wheel one panel while a sibling chat updates `scrollTop` every 100 ms. Every chat scroll extends the global deadline, so the chat remains classified as user scrolling indefinitely. Its buttons lose touch feedback, including an already held press. **Established by a source replay:** after one second, the sibling chat still reported movement and cancelled a held press; the control without the initial wheel did neither.

- **81608f62a1 — The new hover selector outranks a row’s press.** [neutral.css:449](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/neutral.css:449)  
  A menu button matching `:hover[data-pressed]:not(:focus)`—notably a touch-activated Safari button—gets `background-image: none`. The hover rule has specificity `(1,5,1)`; the supposedly winning press rule below has `(1,4,1)`. Its later position cannot help. **Established by CSS cascade reasoning.** The comment claiming equal specificity is false, and the added highlight test never presses an unfocused hovered row.

- **32f7a73851 — Reused popovers retain an obsolete sheet marker.** [menu-rows.js:82](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/menu-rows.js:82)  
  At phone width, open a generic popover, close it, then reopen the same element as a context menu or after changing its role to `menu`. The exclusion returns before removing `data-exact-sheet`. The CSS still matches that marker, so the excluded presentation becomes a bottom sheet with a scrim; context coordinates are overridden. **Established by reading and a source replay:** the marker survived the excluded reopening. Tests use fresh, separate elements.

**Non-blocking**

- **5756526e9e — Click suppression is not associated with the originating pointer.** [input-glue.js:907](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/input-glue.js:907)  
  If an outside lift produces no browser click, the guard instead consumes the next unrelated script `.click()` or assistive-technology activation within one second. Neither necessarily has an intervening `pointerdown` or `keydown`. It checks no pointer identity, trust, target or activation type. **Established by reading and reasoning.**

- **1528e9d74a — Removing an open popover leaves its invoker permanently highlighted.** [menu-rows.js:96](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/menu-rows.js:96)  
  Conditionally remove an open popover while retaining its toolbar button. Cleanup depends entirely on a window-captured `toggle`; removal closes the popover without firing that event, leaving `data-exact-open` on the button. **Established by reading and reasoning from HTML’s removal and hide algorithms.** [HTML removal steps](https://html.spec.whatwg.org/multipage/infrastructure.html#html-element-removing-steps).

- **1528e9d74a — Two popovers sharing an invoker clear each other’s highlight.** [menu-rows.js:98](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/menu-rows.js:98)  
  Open two manual popovers from the same button, then close either one. Its handler removes the shared attribute although the other remains open. **Established by running the extracted handler with DOM stubs:** one popover remained open and the invoker was unmarked.

- **81608f62a1 — Leaving a menu onto another button retains the old row’s focus.** [input-glue.js:730](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/input-glue.js:730)  
  Hover a menu row, then move directly onto an outside button or link. The `relatedTarget.closest(ROW)` condition treats any such control as another menu row, so focus and highlight remain behind; Down continues from that stale row instead of entering from the top. **Established by a listener replay:** exiting onto an ordinary box restored the invoker; exiting onto an outside button retained the row.

- **6a106f94bd — Hover exit adds another wash beneath an open button’s retained highlight.** [input-glue.js:712](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt/host/web/input-glue.js:712)  
  With a bordered invoker’s popover open, move the mouse from that button into the popover. `data-exact-open` keeps its background-image wash, but this guard checks only `data-pressed`. The fade adds a second wash as `background-color` underneath it, briefly darkening the retained highlight. **Established by reading and CSS compositing reasoning.** The fade tests cover buttons without open popovers.

All eight diffs were read; no files were edited. Browser tests could not run: the checkout lacks `playwright-core`, and launching the machine’s installed copy failed with `EPERM` creating its temporary directory. The replays above exercised extracted source with stubs, not browser rendering. I could not verify physical iOS/Android behavior or compare UIKit/AppKit presentation.