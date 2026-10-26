# Code review: the terminal taps a pressable box (QUEUE), narrow re-check 1, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `09c17cc9c6`.
- **Method:** one brief (sha256 `51d0657e02a6deda4a6f2dc361c893f8505e888155e9853d936181c29265e5bb`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a host fix, the lead's 20:00Z task), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX).
- **Disposition:** both taken, as Astra's.

**Verdict: RIGHT WITH CHANGES.**

Controls stay on the old path: paint (`paint.rs:227`) still records every `Pressable`, `TextInput`, and `Control` without calling `Host::presses` (`host.rs:683`), and `invoke` (`host.rs:975`) still skips a disabled `commandfor`. `presses` does omit a disabled view and any view under `inert`. `press_at` (`pointer.rs:84`) takes a run only when a `press` or a non-empty `href` lies strictly between the run and the hit, so the bold label keeps the button's press and `commandfor`. `screen::a_disabled_or_inert_box_takes_no_press_and_a_buttons_label_is_its_own` passed and matches that fixture. The refill at `host.rs:440` is from `abcae3b0e8`. Before round 1, a box press focused the box and the following commit assigned `autofocus()` with no focus event.

1. **SHOULD-FIX — A disabled or inert box still presses when it is nested, or when the click is an inline run.** `pointer.rs:78`, `pointer.rs:112`, `cli.rs:408`. `press_run` follows the nearest `Press` and never reads `disabled` or `inert`. A child `text` is a run (`paint.rs:618`); with no hit, that walk runs the box, and `tap` on the run calls `press_run` first. An ancestor hit makes `on_screen` (`pointer.rs:268`) true, so `tap` of the box presses it and a plain click presses the ancestor. The new test has neither an ancestor hit nor a child run.

2. **SHOULD-FIX — A click on no hit leaves focus empty.** `QUEUE.md:1592`. `click` calls `focus(None)` (`pointer.rs:169`), and `focus` then stores `None` (`host.rs:779`) over the refill the blur commit just did. The next key does not type there. A box that cannot hold focus does refill, on this commit and on the old `focus(Some(box))` path. Linux `autofocus_processed` (`arrive.rs:90`) matches the note's remedy.
