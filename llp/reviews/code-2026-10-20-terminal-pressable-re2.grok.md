# Code review: the terminal taps a pressable box (QUEUE), narrow re-check 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `fa537bd733`.
- **Method:** one brief (sha256 `6854818ddbe4ea9415a2eb0e633644f0f21a72cedcdb87a9bdb4de646b7b6256`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a host fix, the lead's 20:00Z task), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**Verdict: RIGHT.**

A disabled or inert pressable box stays in `hits`: paint records every visible node with a `press` handler, and `Host::handler` returns `None` there, so the walk stops. An agent `tap` (`on_screen`, then `press`), a mouse click (`hit` is the box, then `press`), and a click or tap on a styled run of its text (`press_at` leaves the run alone when the box is the hit; `press_run` uses that same `handler`) run neither that press nor an ancestor’s. A `Pressable`, `TextInput`, or `Control` is still returned, and `invoke` still skips a disabled `commandfor`.

The rewritten test nests `off` under pressable `card`, taps `off` and `numb`, and clicks the runs “disabled” and “card”. Without the handler stop, `presses` becomes 1. Without the box remaining the hit, that run click falls through to `card` and `outer` becomes 1. Without `numb` in `hits`, its tap fails `on_screen`. `screen::a_disabled_or_inert_box_takes_no_press_and_a_buttons_label_is_its_own` passed.

The QUEUE note matches the code. A press on a box that cannot hold focus calls `focus(None)`, and the press commit refills the first `autofocus` control with no focus event. A click on no hit assigns `None` after the blur commit, so the focus stays empty until a later commit.

No findings.
