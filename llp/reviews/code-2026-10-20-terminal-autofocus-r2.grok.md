# Code review: the terminal's autofocus taken once (QUEUE), round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `eb75ec69c6`.
- **Method:** one brief (sha256 `6491d2e4a92efdb4628eb48eedec8da4cac360b8207ab21d42285c5d34003166`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a small host fix, the lead's 21:00Z task). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**Verdict: no BLOCKING or SHOULD-FIX findings.**

Round 1’s nits are in the tree. The field comment at `host/terminal/src/host.rs:198` says a disabled, hidden, or closed-layer control is offered once it can take focus, as on Linux. The test at `host/terminal/tests/it/screen.rs:1021` mounts `third` while the `more` button holds the focus, blurs with a card click that commits while the focus is empty, then types `d`. `third` staying empty is the spend check: that commit would have focused it, and `d` would have landed there, had it still been eligible. `screen::autofocus_takes_the_focus_once_and_never_refills_it` passed.

`autofocus` (`host.rs:786`) walks `focusables` in order, records every new autofocus `NodeKey`, and returns the first. `after_commit` (`host.rs:449`) assigns that id only when `focus` is empty, so a control that arrives while something else is focused is still spent. That is one offer per element, on arrival, into an empty focus — the ordinary-app reading of HTML’s candidates and of Linux’s `autofocus_processed`.

Boot still assigns in the first `after_commit` (`host.rs:299`). `focus_open_layer` (`host.rs:1077`) still focuses a dialog or popover’s first autofocus on open, and `close_layer` (`host.rs:1110`) still restores the focus it saved. A new route or list row is a new key and gets its own turn when nothing is focused. A keyed update keeps the key, so it is not offered again. Hidden and disabled controls stay out of `focusables` until they qualify.

`retain` (`host.rs:798`) drops a key when `node_by_key` fails, and `resolve` (`kernel/src/arena.rs:357`) requires the slot generation. A reused slot fails that check, the old key is dropped, and the new node can be offered. The set stays within the live nodes.

The test is a fixed 40×20 layout and a synchronous `Vt`: boot `a`, card click then `b`, second click then `c`, button then blur then `d`. No timers.
