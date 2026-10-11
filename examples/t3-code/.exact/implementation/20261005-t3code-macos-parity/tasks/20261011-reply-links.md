---
name: 20261011-reply-links
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# Reply links that are not links: other schemes, fragments, and a code span inside a link

## Outcome

STATUS "Known differences" rows 4 and 5 (the user asked on 2026-10-11 to fix what can be fixed now). Both were seen while
building [realinput-1010g-followups](closed/20261010-realinput-1010g-followups.md) (#419, "Seen while building").

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| RL-1 | A reply's `mailto:`, `irc:` or `xmpp:` link is plain text: no press, no menu, not a link to accessibility (#407 round 2 left it without a press) | `ChatMarkdown`'s `<a>`: find what a click does in the desktop app (`shell.openExternal` for these schemes, or nothing) and what its context menu shows |
| RL-2 | A reply's `#fragment` link is plain text too | Find what the reference does with an in-page fragment in a reply (scroll to a heading, open nothing, or the link as text) |
| RL-3 | A code span inside a web link (``[`x` docs](https://…)``) is a plain code box (no press, no menu), and a link that starts with code has no link node, so its words read as text, not as a link. Drawing them as the link (two more `FlowRuns` branches) measured +763 KB of plan in #419 | The whole `<a>` is one link, its code part included |

## Steps

1. Reference first (CDP on the reference lane, and its source under `target/t3-ref/src-1e2ecbd975`): RL-1 and RL-2's
   click, menu and accessibility; RL-3's link with a code span inside and one that starts with a code span.
2. Match each row in the clone (`FlowRuns`, `macos/src/markdown_links.rs`, the reply renderer). A scheme the reference
   opens goes out through the same system handler (`NSWorkspace`) the web links use; keep #419's link node and labels.
3. RL-3: find a way that does not cost hundreds of KB of plan (for example one shared branch, or labelling the code run as
   part of the link node instead of a new branch). Report the plan size before and after; if no way stays under about
   100 KB, build it anyway only if the cost is under 300 KB, otherwise record the measurement and leave the row open.
4. Tests (Bun; the Rust test for `markdown_links.rs` if it changes); one agent drive with a fixture reply that holds all
   four kinds of link; `tree --ax` read for the link roles; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RL-1..RL-3 | reference comparison first; tests; agent drive; `tree --ax` | before / after / reference images |
| A click on a mailto link opening the mail app, under a real pointer | a real-input step for the final session | — |

## Next action

Start now.
