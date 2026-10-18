# Code review: LLP 1121 stage 2, commit 4, round 2, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `87aa414c9d1aab99ff9c514637b326953ca08e0952d081abe626b61c250bc5a2`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 2's commit

---

- **MAJOR — Mode refusal still leaves other facts changed.** The native ordering now applies fold changes before validating modes; Apple also writes `agentDisplayScale` during validation. A request combining `segments 2x1` with an unknown mode returns an error after changing the fold. On web, only page facts are restored: the fold substitute remains changed while `foldChanged()` is skipped, leaving browser-side facts and the runner inconsistent. Confirmed with an in-memory probe. See [Linux:611](/tmp/rv-s2c4/host/linux/src/agent.rs:611), [Apple:475](/tmp/rv-s2c4/host/apple/Sources/ExactKit/Agent.swift:475), and [web:1151](/tmp/rv-s2c4/host/web/glue.js:1151).

- **MAJOR — Linux still commits modes before fallible preference setters.** At [agent.rs:611](/tmp/rv-s2c4/host/linux/src/agent.rs:611), modes commit before `set_preferences`, `set_page`, and the size setters; subsequent errors do not undo them. Concrete case: valid `dark-style:dim` plus `root-font-size:1e300` passes the agent’s finite-positive `f64` validation, then fails the runner’s `f32` validation after Dim has committed.

- **MINOR — An already-poisoned runner can change its tuple.** [mode.rs:143](/tmp/rv-s2c4/runner/src/runner/mode.rs:143) skips restoration whenever `self.poisoned` is true. A subsequent valid mode request calls `set_window` before `commit_again` returns `Poisoned`, so the tuple changes without updating the tree. Reject before mutation when already poisoned; retain the tuple only when this commit caused poisoning.

- **MINOR — Empty mode requests still omit the actual tuple.** `{"op":"prefer","mode":{}}` now returns no `mode` field on [Apple:475](/tmp/rv-s2c4/host/apple/Sources/ExactKit/Agent.swift:475) or [web:1151](/tmp/rv-s2c4/host/web/glue.js:1151). Linux reports the current tuple. This part of round 1’s reply finding remains unresolved.

No splice defect found: zero-operation batches still contain fields, error objects splice correctly, and neither batch writer returns an empty string or `{}`. Page-fact restoration itself passed, including root font size.

Seven targeted Bun tests passed; carrier probes confirmed Firefox/WebKit forward modes and preserve returned axes. Rust/Swift tests weren’t run: this read-only checkout has no build artifacts. No edits made.

VERDICT: fix (MAJOR: host/linux/src/agent.rs:611, host/apple/Sources/ExactKit/Agent.swift:475, host/web/glue.js:1151; MINOR: runner/src/runner/mode.rs:143, host/apple/Sources/ExactKit/Agent.swift:475, host/web/glue.js:1151)
