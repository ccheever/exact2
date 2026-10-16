# Code review: LLP 1121 stage 2, commit 4, round 3 (last), 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `bc0f9b16c9a07f32c2ab99de5896edeb8df9fea3400a3f4d9d89faae0670f1e6`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** applied without re-review in the round 3 commit

---

The round-2 mode checks, the empty-mode reply, and the comma split are in place. A root font size the runner refuses still leaves a mode committed on the web and on Apple.

**[MAJOR] A root font size the runner refuses is still followed by a mode commit — `host/web/glue.js:1151`**

`pageFacts.prefer` accepts any finite `f64` above 0, including `1e300`, and writes it onto the document element. `pageChanged()` then calls `exact_set_root_font_size`. The runner rejects that size because it is not finite as the `f32` it stores, and the glue drops that batch. `exact_set_mode` runs anyway. With a live tree, Dim is committed, the runner’s font size stays where it was, and the reply reports success with `root-font-size: 1e300`. Linux returns at `set_root_font_size` before `set_modes`.

**[MAJOR] Apple commits modes after `tellPage` has already swallowed that refusal — `host/apple/Sources/ExactKit/Agent.swift:493`**

`PageFacts.agent = facts` posts on the main queue, and that delivery runs `tellPage` before `prefer` continues. `tellPage` applies `setRootFontSize` and `setDisplayScale` and ignores a batch error. `1e300` passes the `Double` scan and fails in the runner, then `setModes` still runs. The reply’s page group is the local `PageFacts` value, so it reports the refused size and the new mode. The same path applies to `display-scale`.

**[MINOR] A poisoned runner rejects every Linux `prefer`, including one with no mode group — `host/linux/src/agent.rs:541`**

`check_modes` refuses when `poisoned` is set, and Linux calls it for an empty pin list. A `prefer` that only names media, or names nothing new, used to come back with the current facts. It now returns `prefer mode: Poisoned` before those facts are read. Web and Apple ask only when the request has a mode group.

What holds: a bad pin returns before the fold, page, or preferences are set; an empty mode group on a live runner carries the window’s modes; a comma or colon in a web or Apple axis or value is refused before the pin split; an already-poisoned runner does not take a new tuple in `set_modes`.

VERDICT: fix (MAJOR host/web/glue.js:1151, host/apple/Sources/ExactKit/Agent.swift:493; MINOR host/linux/src/agent.rs:541)
