# Code review: web list item roles, round 3, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `f809badfb8` (one source commit on `99fd8ca92c`).
- **Method:** one brief (sha256 `a6b882ac97a44fb0dd3464123e0d66da38ad6d908b92b145f38f7b9b751db6cb`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

Reviewed `f809badfb82e941106648d99b9392084c615dc0d` against `99fd8ca92c`. Four findings remain.

**Adversarial correctness:** R2 roles settlement **accepted**: the vocabulary audit, native per-owner assertions, Rust assertions and removal controls address the reported precedence errors. R2 navigation settlement **rejected as complete**: route admission and arrival are repaired, but navigation consumers below still miss wrappers. Remaining R1 settlement 1 **rejected** because the CSS sweep is incomplete.

1. **P2 — Native header and control CSS still requires physical children.** [index.html:249](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r3/host/web/index.html:249) and [neutral.css:327](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r3/host/web/neutral.css:327). The missed selectors include segment resets, sidebar layout and `[navigationkey] > header` styling. Production-wrapper/HTML-selector probes change their matches from one to zero. Wrapped headers consequently lose sticky positioning, padding and native paint; sidebar rules also miss wrapped relationships. Sweep both stylesheets, including modal/header rules. Add computed-style regressions for wrapped headers, List-rooted sidebars and unselected segments, with removal controls.

2. **P2 — Hidden Back controls lose their visible proxy.** [input-glue.js:1211](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r3/host/web/input-glue.js:1211) still queries `:scope > header`. The production-function probe resolves the visible Back before wrapping and returns null afterward. Agent geometry/reveal then uses the hidden authored control. Use logical header traversal. Put `id=back` inside the fixture’s header and test named Back tapping and geometry; the current direct shadow-button click bypasses this lookup.

3. **P2 — Clearing a keyed List can bypass sheet exit.** [rt.js:1282](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r3/host/web-js/rt.js:1282). The bulk-clear optimization checks only direct modal children. Wrapping hides the modal from that guard, allowing `parent.textContent = ""` to bypass `sheetLeaves`. A probe executing production `each` and `sheetLeaves` retains and animates the unwrapped sheet, but immediately removes the wrapped sheet without animation. Inspect logical children and extend the existing sheet-leave regression to a List parent with whole-region removal/replacement.

**Simplification:** Accepted. The shared logical walks and `listRow` remain appropriate; these fixes need no additional abstraction.

**Writing:** The documentation and commit-message structure follow the writing rules. Refresh the Test Plan after settlement: final Cargo results include the new guard failure below and the held-write timeout, now reproduced on main.

**Verification:**

4. **P2 — Required Cargo testing fails a stale exact-line allowlist.** [icon_path_readers.rs:131](/var/folders/6c/18fpzdj567n_bctdx5hd49th0000gn/T/exact-web-list-review-dq_1xdqs/checkout-r3/contract/cli/tests/it/icon_path_readers.rs:131) still allows `list?.parentElement`, while navigation now uses `viewParent(list)`. [Final Cargo output](/tmp/exact-list-r3-cargo-tests.log:2930) identifies that changed line; the [main scanner control](/tmp/exact-list-r3-main-scanner.log) passes. Update the narrowly justified allowlist and rerun its existing regression.

Completed evidence: [88 browser tests pass](/tmp/exact-list-r3-complete-web.log), the native collector passes, ordinary-list conformance passes, and both WebKit comparisons pass. Other required checks completed successfully. Structure conformance retains the documented main-reproduced unopened-tab differences. Caps’ foldables violation and both Apple timeout classes are reproduced on main, including the [new full-package controls](/tmp/exact-list-r3-main-controls.json).

I ran read-only production-function/selector probes. Builds and browser reruns were unavailable in this snapshot without `target`/`node_modules`; supplied logs provide that evidence. No files were changed, and no browsers or agents were started. VoiceOver evidence remains prototype-only.

**Not ready. Round 3 is not clean.**