# Code review: LLP 1121 stage 2, commit 4, round 2, 2026-10-14 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree at the reviewed commit, read-only by instruction.
- **Method:** one brief (sha256 `87aa414c9d1aab99ff9c514637b326953ca08e0952d081abe626b61c250bc5a2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 2's commit

---

**[MAJOR] Linux still commits modes before the rest of the request, and commits the fold before the pins are checked — `host/linux/src/agent.rs:610`**

`prefer` says nothing applies unless every fact is known. Media and page are parsed into locals first, but `prefer_fold` at line 534 both checks and commits, and `set_modes` at line 610 commits before `set_preferences`, `set_page`, and `set_root_font_size`.

Two requests that pass the scan still stick:

- `prefer` with a valid fold and `mode dark-style:grey`. The fold's `set_segments` has already committed. The bad pin returns at line 612. The segments stay.
- `prefer mode dark-style:dim` with `root-font-size` `1e20`. The scan accepts any finite number above 0. `set_modes` commits Dim. `Runner::set_root_font_size` then refuses it, because `1e20` is not finite as the `f32` the kernel stores. The error returns at line 623 with Dim left in place.

A preference or page commit that rolls itself back does the same: that group returns to what it was, and the pins do not.

**[MAJOR] Apple applies the fold and the display scale before `setModes`, and a mode error does not undo them — `host/apple/Sources/ExactKit/Agent.swift:478`**

`preferFold` at line 430 calls `session.segments`, which keeps the fold when the batch is accepted. The page scan at line 467 assigns `PageFacts.agentDisplayScale`, whose `didSet` posts the page-facts notification. The session observer calls `tellPage()`, which commits that scale. Both run before `setModes`. A bad pin returns at line 478. The fold and the scale stay; only the later `PageFacts.agent = facts` assignment is skipped.

**[MAJOR] A web mode refusal does not put the page back as it was — `host/web/glue.js:1151`**

On `modes.error` the glue calls `pageFacts.prefer(before)`. `before` is `pageFacts.read()`, which always includes `root-font-size`. `prefer` treats that key as a request to set `documentElement.style.fontSize` and `--exact-host-font-size`. A mode-only refusal, which never touched the page group, still writes those. An author `font-size` on the document element, and a percentage root size that was using the `100%` fallback, both change.

The same path leaves the fold. `preferFold` on line 1150 stores `foldSubstitute` before the pin is known, and the error return does not clear it or call `foldChanged`. The next resize sends the refused fold.

The batch splice is not part of this. `batch_for` / Apple `finish` always emit a non-empty object starting with `{"ops":`. A bare `{"error":…}` returns before the splice. `batch[1..]` stays on an ASCII brace.

**[MINOR] A comma inside one mode value becomes another pin — `host/web/src/abi/mode.rs:14`**

The glue joins entries with commas, and the web and Apple buffers split on the first comma and the first colon. `{"dark-style":"dim,density:compact"}` is one invalid value of `dark-style` on Linux, which refuses it and changes nothing. On the web and Apple it is two pins. With both axes declared, both are set. The same split is in `host/apple/src/abi/preferences.rs:15`.

**[MINOR] An already-poisoned runner still takes the new tuple — `runner/src/runner/mode.rs:143`**

Skipping the rewind when `poisoned` is set is right for a commit that poisons while writing the tree: `conclude` keeps that tuple. It is wrong when the runner was already poisoned. `set_window` runs first, `commit_again` returns `Poisoned` without `update`, and `conclude` restores the checkpoint taken after `set_window`. The rewind is skipped, so `modes()` names a tuple this call never wrote.

What holds: one axis and one value in `setMode`, a literal axis checked on its own, `window_with` matching `pinned`, pins checked before any of them is set, Playwright sending the mode group, `modes_json` read by `BatchShapes`, and a collection saving the surrounding tuple with `retheme: false` so a later row is not re-evaluated whole.

VERDICT: fix (MAJOR host/linux/src/agent.rs:610, host/apple/Sources/ExactKit/Agent.swift:478, host/web/glue.js:1151; MINOR host/web/src/abi/mode.rs:14, runner/src/runner/mode.rs:143)
