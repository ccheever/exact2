# Code review: Linux non-modal popover follows HTML, round 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `/tmp/dt3-rv-b3`.
- **Method:** one brief (sha256 `5a862743d4aab38439eb618eda3296e3af4a15324c3b571b77bb9ab08c49eca0`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — Page scrollbars can take presses through a popover.** [alert.rs:79](/tmp/dt3-rv-b3/host/linux/src/presenter/alert.rs:79) now permits every page node. [scrollbar.rs:75](/tmp/dt3-rv-b3/host/linux/src/presenter/scrollbar.rs:75) uses that permission without checking occlusion, and `pointer_down` handles scrollbars before `alert_pointer`. With desktop scrollbars enabled, place a popover button over a page scroller’s track: pressing that area scrolls or drags the underlying page instead of activating the button. Page scrollbar input needs the same top-layer hit protection as ordinary controls. The retained scrollbar tests cover modal boxes only.

- **MAJOR — Tab from a negative-tabindex autofocus target jumps onto the page.** [typing.rs:543](/tmp/dt3-rv-b3/host/linux/src/presenter/typing.rs:543) treats focused nodes absent from the stop list as “start at the first stop.” In the new `APP` fixture, add `autofocus=true tabindex=-1` to `save`: opening focuses Save, but Tab selects Probe instead of Other. The newly live page exposes this fallback across the popover boundary. Navigation must retain the focused node’s position within its scope even when that node is not a Tab stop. [HTML sequential focus navigation](https://html.spec.whatwg.org/multipage/interaction.html#sequential-focus-navigation).

- **MAJOR — Disabling the invoker incorrectly discards its popover’s focus scope.** [alert.rs:777](/tmp/dt3-rv-b3/host/linux/src/presenter/alert.rs:777) requires the invoker to appear among the Tab stops. Using the existing “After” fixture, focus Save and then disable Open: Shift-Tab goes to After through raw tree order; it should skip disabled Open and reach Probe. A non-focusable invoker still owns the scope; [WPT explicitly checks this case](https://github.com/web-platform-tests/wpt/blob/master/html/semantics/popovers/popover-focus-2.html#L68). Preserve the scope’s insertion position independently of the invoker’s eligibility.

- **MINOR — A former invoker remains exempt from light dismiss after its target changes.** [alert.rs:803](/tmp/dt3-rv-b3/host/linux/src/presenter/alert.rs:803) accepts `open.invoker` unconditionally, bypassing the current invocation checks. Open the popover, remove the original button’s `popovertarget` through a state update, then click that button: dismissal is skipped and there is no toggle to close it. The exemption must follow the button’s current valid popover target. [HTML target-popover rules](https://html.spec.whatwg.org/multipage/popover.html#target-popover). The new non-popover-command test uses a different button and misses this branch.

No additional defect found in the block-rectangle calculation, reporting recursion, or modal/alert changes. No edits made; `git diff --check` passed. Tests were not rerun: this read-only checkout has no build artifacts.

**VERDICT: NOT READY — three MAJOR defects and one MINOR remain.**
