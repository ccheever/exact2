# Code review: Linux non-modal popover follows HTML (round 3 narrow pass), narrow re-check (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `/tmp/dt3-rv-b4`.
- **Method:** one brief (sha256 `60fb9557db45667d5ee81889e4eabdd3267931c2b77b755f6eca9301a58392d5`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

- **MAJOR — [typing.rs:532](/tmp/dt3-rv-b4/host/linux/src/presenter/typing.rs:532):** The virtual focus’s default rank still breaks mixed tabindex navigation. With `autofocus tabindex=-1` followed by `tabindex=1` and then `tabindex=0`, Tab skips the positive stop; Shift+Tab selects that following stop. HTML requires navigation from a non-stop to use tree order, independently of positive tabindex ordering. [Sequential navigation algorithm](https://html.spec.whatwg.org/multipage/interaction.html#sequential-navigation-search-algorithm).

- **MINOR — [alert.rs:208](/tmp/dt3-rv-b4/host/linux/src/presenter/alert.rs:208):** `i.target != key` classifies a source that invokes another popover as invoking nothing. Open A programmatically with source S, where S declares `commandfor=B command="hide-popover"` and B is closed. Clicking S exempts A from light-dismiss, then hiding B does nothing: A stays open. S’s current target is B, so this should dismiss A. [Target-popover rules](https://html.spec.whatwg.org/multipage/popover.html#popover-light-dismiss).

- **MINOR — [alert.rs:343](/tmp/dt3-rv-b4/host/linux/src/presenter/alert.rs:343):** The live manual check happens after `set_focus(None)` dispatches blur handlers. If a focused child’s blur handler removes its manual popover during `hidePopover`, `manual_popover(&open)` now returns false because the node is gone. Execution restores `open.restore`, incorrectly returning focus to the page. Read the live mode before dispatching blur and retain that decision through cleanup.

Read-only review; Rust tests were not rerun because this checkout has no build artifacts.

VERDICT: 1 MAJOR and 2 MINOR correctness issues remain.
