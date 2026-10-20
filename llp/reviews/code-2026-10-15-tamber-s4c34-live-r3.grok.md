# Code review: stage 4 commits 3 and 4: the live table, round 3, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree, read-only by instruction.
- **Method:** one brief (sha256 `536e3c1bf3dce6c28fb827bdafb3cea95b9594a8b2c64fe4a027390fa78f4aaa`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (Linux keeping a snapshot through a 204; the web page not journaling a refused colour).
- **Disposition:** applied without re-review (the last round) in the third-round commit; the natives' snapshot is commit 5's one-source relay.

---

I'll review the last three commits as round 3: the value-only live-table change plus the fixes from the first two review rounds. I'll read the review criteria, then the commits and the design, and check each prior fix before hunting for anything still blocking.The three commits are the original live-table change plus two review-fix rounds. Next I’ll read the design and the full diff so I can check each prior fix against the current code.The structure digest, batch wire, and runner apply path are in place. I’ll read the hosts, the JS target, and the server next, then check each prior fix against the code.The batch path and the dev loops are in view. I’ll check token application, the JS design emit, and the host and server changes against the earlier fixes.The structure walk and the runner’s token edits look deliberate. I’ll check the places a wrong value-only decision or a stale snapshot would actually land: the server, the hosts, and the tests.Auto-compacting conversation (85% full)...
Conversation compacted.
The review is still open on the Linux snapshot path and the tests named in the commits. I'll read the review rules, then check those against the current code.[P1] Drop a stored design snapshot when `/__dev/design` answers 204 — `host/linux/src/fetch.rs:134`

A poll stores a snapshot only when the body is present, so a 204 leaves the previous one in place. That snapshot is also held while a generation sits in `pending_dev` (`host/linux/src/presenter/delivery.rs:38`), which is how long a Rust module's `preload` keeps returning false. A later plan of the same structure — a token edit beside a Contract edit, the case round 2 fixed on the web page — publishes, clears the server snapshot, and comes back as 204. The poll still has the old snapshot. Once that generation boots, `poll_development` applies it (`host/linux/src/presenter/delivery.rs:44`). `apply_live` accepts it: a snapshot has no base, and the structure matches, so the new plan's baked colours are replaced and the digest sticks.

[P3] Journal a wide-table refusal on the web page — `host/web/dev.js:290`

Linux and Apple both print a line when `unsupported` is set. The wasm host returns that flag beside `live: "applied"` (`host/web/src/host/live.rs:32`), and the page logs only `stale` and `refused…`. A colour the wide table refuses keeps its old row, and the wasm loop says nothing.

The round 1 and round 2 fixes are in the tree and hold: the bytecode walk keeps code-referenced strings, snapshots skip role overrides, the web page and Apple drop or cancel a snapshot from an older generation, a later Apple session fetches again, the TypeScript path compares module executables, the JS loop reloads on an accent-flag change and on a deleted asset, a dev build with no axes emits the empty tuple, and a held propose refuses clashing `requires` and `files` digests and sends `requires` through to the server on accept. The value-only check fails closed (Contract text, assets, token names, slices, role and profile values, axes). `set_tokens` lands every edit before it reparses rows, and boot clears that plan's edits. Release emits still call `every(false)` under a guard that restores the previous flag. Swift, `exact.h`, and both `exact_design_rows` exports agree.

The Linux finding is blocking. The web journal line is not.

The Linux live test calls `apply_live` on a booted presenter and never polls, so it does not cover a 204 after a stored snapshot. The web generation check has no harness, and Apple's colour report is only checked by compiling.
