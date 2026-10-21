# Code review: LLP 1121 stage 2, commit 4 (setMode and prefer mode), 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, a throwaway worktree at the reviewed commit.
- **Method:** one brief (sha256 `3af0ad820b53b295897a357c986c16685451268a5cfcc3a398229f2d37140612`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** fix (both).
- **Disposition:** fixed in round 1's commit

---

- **MAJOR — Firefox/WebKit silently ignore mode requests.** [scripts/agent.mjs:1281](/tmp/rv-s2c4/scripts/agent.mjs:1281) passes four arguments, but [agent-playwright.mjs:328](/tmp/rv-s2c4/scripts/agent-playwright.mjs:328) accepts only media/page. `prefer mode dark-style:dim` succeeds without sending the mode to either target. Confirmed with the actual carrier method and mocked browser calls.

- **MAJOR — Error-bearing batches are discarded.** [host/web/glue.js:1151](/tmp/rv-s2c4/host/web/glue.js:1151) and [Agent.swift:433](/tmp/rv-s2c4/host/apple/Sources/ExactKit/Agent.swift:433) return before applying the batch when it contains an error. A successful mode commit can be followed by a failing host step—Apple’s resize handlers, for example—so a batch may contain both accepted operations and an error. Dropping those operations leaves presentation behind the runner and host mirror. Apply the batch before reporting its error.

- **MAJOR — Rejected preference requests can partially change modes.** [host/linux/src/agent.rs:536](/tmp/rv-s2c4/host/linux/src/agent.rs:536), and the corresponding Apple/wasm loops, commit each entry before validating the rest. A valid `dark-style:dim` followed by an unknown axis returns an error after applying Dim. Native hosts also apply modes before validating media/page facts. Validate the entire request before mutating it. Confirmed the wasm partial application with an in-memory probe.

- **MINOR — `setMode` accepts multiple pins inside one argument.** [runner/src/runner/mode.rs:28](/tmp/rv-s2c4/runner/src/runner/mode.rs:28) passes concatenated arguments to the pin-list parser; [contract/lower/src/theme.rs:564](/tmp/rv-s2c4/contract/lower/src/theme.rs:564) validates the same way. With both axes declared, `setMode("dark-style", "dim,density:compact")` passes validation and changes two axes, although that string is no declared value. Validate one axis and one value directly.

- **MINOR — Known-invalid literal axes escape compilation checks.** [contract/lower/src/theme.rs:561](/tmp/rv-s2c4/contract/lower/src/theme.rs:561) checks only when both arguments are literal. `setMode("undeclared-axis", choice)` therefore compiles despite the axis being statically invalid. Check a literal axis independently.

- **MINOR — Apple/web replies echo requests rather than report current modes.** [Agent.swift:507](/tmp/rv-s2c4/host/apple/Sources/ExactKit/Agent.swift:507) and [glue.js:1152](/tmp/rv-s2c4/host/web/glue.js:1152) omit untouched axes; an empty mode group returns `{}`. Linux correctly reports `Runner::modes()`. Return the actual window tuple consistently.

The runner sequencing otherwise looks sound by inspection: recoverable action refusals precede mode extraction; later failures follow the existing poison policy. Direct `set_mode` restores the tuple and forces another theme visit. Command removal preserves `first_command`, and collection updates refresh `around` before later mounts.

Missing regression coverage includes rollback, multiple/interleaved commands, unchanged action values, late virtualized mounts, and `setMode`’s paint-only layout receipt.

The four Bun preference tests passed. Rust/Swift tests were not run: the filesystem is read-only and this checkout has no build artifacts. No files were edited.

VERDICT: fix
