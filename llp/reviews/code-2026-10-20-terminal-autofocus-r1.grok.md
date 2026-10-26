# Code review: the terminal's autofocus taken once (QUEUE), round 1 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok 4.7 via the grok CLI, xhigh, `--max-turns 400 --no-plan --no-subagents --deny Edit --deny Write`, a throwaway worktree at `bf9b4b1b18`.
- **Method:** one brief (sha256 `57704b8a4d700ac25736284119ebf51e40d681cf52e9708e78ee1d640d9a270a`), blind to the other reviewer; the commit as amended after the previous pass; 2 rounds (a small host fix, the lead's 21:00Z task). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve (no findings).
- **Disposition:** none needed.

**No findings.**

`Host::autofocus` offers each control once, by `NodeKey`, and only assigns it when `focus` is empty. Anything focusable and not yet in the set is spent on that commit, including one that arrives while another control holds the focus. Hidden, disabled, and closed-dialog controls stay out of `focusables()`, so they are not spent and can still take an empty focus once they qualify. `retain` drops a key only when `node_by_key` fails, and that lookup checks the slot generation, so a reused slot cannot alias a spent control. The set stays bounded by live nodes.

Boot still focuses the first field. A dialog or popover still focuses through `focus_open_layer`, which ignores the set, and `close_layer` still restores the focus it saved. A new route or list row is a new key and gets its own turn. A re-render keeps the key, so it does not run again.

`screen::autofocus_takes_the_focus_once_and_never_refills_it` passed. It is a fixed layout and a synchronous `Vt`: boot `'a'` reaches `field`, the first card press mounts `late` and `'b'` reaches it, and the second press leaves `'c'` nowhere. Either field accepting the last key would fail the text check.
