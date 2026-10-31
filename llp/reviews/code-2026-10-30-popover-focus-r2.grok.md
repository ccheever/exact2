# Code review: Linux non-modal popover follows HTML, round 2 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `/tmp/dt3-rv-b2`.
- **Method:** one brief (sha256 `fd9744e48664e21c48e33879d710f5a8f03db1c7c0dd1613d13e0c422c4cc05a`), shared by both reviewers, blind to the other review. The author (Claude, dtool3) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review the full diff read-only, starting with the review criteria and the change from `d16c84ee03` to HEAD.This is a read-only review of `d16c84ee03...HEAD`. I'll map the commits and the full diff before checking HTML parity and the round-1 fixes.The presenter changes are in. Next I’ll read the layout, top-layer report, and geometry paths those changes call into.Auto-compacting conversation (80% full)...
Conversation compacted.
The two defects from the last pass still need exact line citations and a pass over the dialog tests before the review is written. I'll confirm those against the current tree, then give the findings.**MAJOR** — Structural reads treat the popover's painted border box as its block rectangle. `host/linux/src/presenter/alert.rs:537` stores `placed` as `[paintedOrigin.x, paintedOrigin.y, borderWidth, borderHeight]`. `alert_popover_frame` (`alert.rs:746`) and `report_top_layer` (`host/linux/src/presenter/top.rs:1322`) pass that rectangle to `top_layer_frames` with `modal: None`.

An open authored popover is the root of the viewport block the kernel implies. `shown_at` (`runner/src/geometry.rs:719`) treats a reported rectangle as that block whenever `host_block_size` is `Some`: scale is `rect.size / used`, then a shift by the root's `frame.xy`. The contract (`runner/src/runner/shown.rs:40`) is that a block root is reported by its block rectangle, with the border box sitting inside it at `frame.xy`. `used_in` (`kernel/src/kernel/block.rs:749`) is `location + border + end margin`, and the static position folds the start margin into `location` (`a_measure_answers_its_resolved_margins`: margin-left 10% of 250 → location 25, border 100, used width 125).

On the neutral look, which both Linux `run_config` and the Windows overlay set with `show_neutral_look`, every non-menu popover gets the default margin-top and margin-bottom of 4px (`kernel/src/arena/alert.rs:99`). Paint already places the border box with `area::origin` (that margin is the anchor gap) and subtracts `frame.xy`. The read path adds `frame.xy` back and scales. For a border height `H`, used height is `H + 8`, so `frame().y` is `painted.y + 4·H/(H+8)` and `frame().height` is `H²/(H+8)`. A 60px box is read about 3.5px lower and about 53px tall, and `elementFromPoint` misses the bottom strip. An authored margin, the position-area gap, does the same on both axes. Real clicks still hit the presenter's painted boxes.

`reads_see_an_open_popovers_box_where_it_is_drawn` (`alert_popover_tests.rs:236`) compares `frame()` with `rect_of` for a popover with no authored margin, booted without `show_neutral_look`, plus a `translateX` the runner applies itself through `own_transform`. That is the path where `frame.xy` is zero and the used size equals the border box.

The rectangle that matches paint is block origin `paintedOrigin − frame.xy` and size `host_block_size`, so the scale is 1 and the border box lands on the painted origin before `own_transform`.

`report_top_layer` only writes runner state. `alert_layout` calls it when `placed` changes, before focus; `alert_present` calls it again after focus; `alert_close` calls it after `take`, so a closing popover clears the frames. It does not re-enter layout.

**MAJOR** — Escape under a manual popover never reaches the page. `alert.rs:646` lets every key except Escape fall through while a popover is open, then still takes Escape: `key_event`, `alert_close_request`, `return true` (`alert.rs:667`). `alert_close_request` (`alert.rs:357`) returns immediately for `manual` — its comment says Back and Escape are the page's — and that `return true` skips `shortcut()` and `key_default` (`typing.rs:627`). An `aria-keyshortcuts="Escape"` button on the page does not run while a manual popover is open. `surface_takes_keys` (`top.rs:707`) stays true for any open box, so the text-field Escape blur is skipped too (`surface_controls.rs:506`). The focused node's keydown does run, inside `alert_key`. An auto popover consuming Escape is its close watcher. Back is already the page's for a manual popover (`alert_takes_back` is `!manual`). `back_under_a_manual_popover_is_the_pages` only calls `system_back`, and `a_page_shortcut_works_under_a_popover` uses Alt+P, which takes the early return.

For a manual popover, `alert_key` should return false for every key, Escape included, so the page path handles that key once.

**MINOR** — A repeated remote Select is still swallowed under a popover. `typing.rs:623` returns before the page path when the key is a repeated `Select` and `surface_takes_keys()` is true. That flag (`top.rs:707`) is true for any open box, including a non-modal popover. The first Select reaches the page; the repeat does not.

Modal dialogs and alerts still take focus and keep the page inert (`alert_inert`, `page_inert`, `alert_allows`). The tab splice puts a popover's stops after its invoker; `tabindex > 0` stays in the positive pass, and a popover with no invoker stays in tree order. Light-dismiss timing and nested presentation stay the declared pre-existing items. The new tests lock the round-1 rules they name.

VERDICT: Changes requested — two majors (read geometry, manual Escape) and one minor, no blocker.
