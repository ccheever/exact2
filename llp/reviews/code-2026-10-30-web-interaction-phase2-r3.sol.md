# Code review: the web's interaction, phase 2, the whole branch, round 3, adversarial (sol)

- **Family:** OpenAI. GPT-6.1 Sol (`gpt-6.1-sol`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `e39e930cd4` (18 commits on `a7918080ba`).
- **Method:** one brief (sha256 `dd02adeec347108b338cb03088b592e1d1628d3f2f9111d088a2afe02065125c`), one reviewer, pointed at the fixes of the two earlier rounds and the commits no review had covered. The author (Claude, Opus 5.5) is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree's paths, and its commit shas are of that worktree's history.

**Blocking**

- **5c1ff9812d — Route switches do not close their popovers.** [menu-rows.js:96](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/menu-rows.js:96)  
  Open a root-declared popover from tab A, then switch to tab B through an app action or shortcut. Navigation hides retained panels and routes with `visibility: hidden` (`navigation.js:428,445`). Bare `checkVisibility()` ignores that property, so the popover remains logically open and its invoker remains marked. It can reappear when the old screen returns. The regression test substitutes `display: none`, missing the actual navigation behavior. **Established by reading and reasoning against [CSSOM View’s visibility checks](https://drafts.csswg.org/cssom-view/#dom-element-checkvisibility).**

- **b8ecae0d9f — A keyboard-opened dialog can inherit the previous pointer’s suppression.** [input-glue.js:964](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/input-glue.js:964)  
  Click within a form, then within two seconds use an Escape handler to open a discard-changes dialog. Only Enter and Space update `pressed.key`; Escape leaves the previous pointer record intact. The dialog’s focused control consequently receives `data-exact-no-ring`, hiding its keyboard focus indication until another key. The new test exercises Enter only. **Established by running the actual handlers with DOM stubs:** an Escape-triggered arrival received the suppression marker.

**Non-blocking**

- **f1379d415b — Click suppression still lacks pointer association.** [input-glue.js:914](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/input-glue.js:914)  
  If an outside lift produces no click, a trusted accessibility activation within the following second, without an intervening DOM key or pointer down, is swallowed instead. Checking `isTrusted` fixes script `.click()` but leaves this part of the earlier finding unresolved. **Established by a source replay:** a subsequent trusted click with `detail:0`, `pointerId:-1` and empty pointer type was prevented and stopped.

- **f23cef8cc6, retained by 5c1ff9812d — Popover sweep loops multiply.** [menu-rows.js:105](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/menu-rows.js:105)  
  Hide and reopen the sole popover in one task to change its source. Its open-to-open `toggle` first empties `opened`, then schedules another sweep although the previous sweep is still pending. Both subsequently reschedule themselves while the popover remains open. **Established by running the handler:** ten reopening notifications left ten visibility checks and ten callbacks on every subsequent frame.

- **b8ecae0d9f — Removing a focused dialog target leaves its global listener installed.** [input-glue.js:968](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/input-glue.js:968)  
  Conditionally remove the dialog or its focused button after a touch action. Cleanup depends on `blur` or a later key, but removal does not require a `blur` event. Repeated pointer-only use retains cleanup closures and detached elements until a key arrives. **Established by source replay and [HTML’s removal rules](https://html.spec.whatwg.org/multipage/infrastructure.html#html-element-removing-steps):** ten such arrivals without blur retained ten window listeners.

- **fb3e1f4955 — Autonomous scrolling still latches after a wheel in the same container.** [input-glue.js:822](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/input-glue.js:822)  
  Wheel a chat, then let streaming updates advance that same scroller every 100 ms after the wheel stops. Each autonomous scroll extends `person.until`; subsequent touches keep being classified as catching user scrolling and show no feedback. The earlier sibling-container defect is fixed, but this case remains. **Established by source replay:** movement remained classified as personal after 1.1 seconds of autonomous scrolling.

- **53f5fcc684 — Keyboard entry loses the focus origin used by pointer exit.** [input-glue.js:730](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/input-glue.js:730)  
  Open a menu, enter it with ArrowDown, move the pointer onto another row, then leave the rows. Because focus was already inside the menu on the first pointer entry, `focusBefore` was never recorded. Exit blurs the row to the body instead of returning focus to the invoker. **Established by running the extracted listeners.** The new regression manually focuses the outside button before pointer entry, avoiding this sequence.

- **bb32dfca9c — The edge regression never exercises an RTL window.** [neutral-look.test.mjs:507](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/tests/neutral-look.test.mjs:507)  
  Both cases retain an LTR document and change only the popover’s direction. Deleting the new `:root:dir(rtl)` margin rule would leave this regression passing. It also lacks a left-edge clearance assertion for a flipped placement. **Established by reading:** the mixed-content-direction fix is exercised; the RTL-window branch remains unqualified.

- **be6df784c1 — Glide comments still point to the old module.** [glue.js:1090](/private/tmp/claude-501/-Users-admin-Developer-exact-web/7293d1ff-f5b0-4cd1-9f8c-45442e119495/scratchpad/review-wt3/host/web/glue.js:1090)  
  The glide-settlement references here, at `glue.js:1232`, `glue-fixture.mjs:46` and `host/web-js/agent.js:466` still name `input-glue.js`; their implementation moved to `agent-input.js`. **Established by reading.** The moved helper bodies themselves are byte-identical, and the declaration-test move differs only in formatting.

Fifteen tests passed across the helper, shortcut, agent-loading and build-inventory suites. Browser-facing tests failed during import because `rolldown/utils` is missing; `playwright-core` is also absent. The replays used DOM stubs, not real engines. I could not check browser rendering, physical touch, assistive technology, native comparisons or the Cargo suite. No files were edited.