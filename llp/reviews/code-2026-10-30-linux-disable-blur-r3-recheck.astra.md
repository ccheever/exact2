# Code review: Linux focus on a control a commit disables, round 3-recheck (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `87c9ce06fb`.
- **Method:** one brief (sha256 `f091be7e22d9a0bb1b3e9887da903a2b222f34a99609382e95abe0710e112b8d`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

1. **[P2] The input/change fix misses `set_control_value`** — [control.rs:282](host/linux/src/presenter/control.rs:282). Selects, ranges, and picker answers still dispatch both events before `after_commit`. Concrete case: focus a select whose `input` sets `disabled=true` and logs `i`, whose `change` re-enables it and logs `c`, and whose blur logs `b`; choose another option. Source tracing yields **`ic`, focus retained**, instead of **`ibc`, focus cleared**. This is the same round-3 blocker addressed for checkboxes/radios. Apply the between-event fixup here too and cover this path.

Non-blocking notes:

- The ancestor fallback and deferred servicing fixes look correct. Focus clears before recursion; blur precedes focusout; ordinary `set_focus` callers retain `service=true`.
- By source tracing, reverting the respective fixes breaks the three regression assertions: ancestor focus, `ibc`, and `-oF`. I could not execute that verification.
- Links, progress controls, disabled ancestors, and the remote exception are handled explicitly. Android and Windows inherit the shared presenter change.
- I would leave IME qualification as a follow-up: disabled composition rejection predates this commit, but the new early `change` remains unverified against native composition completion.
- The synthetic fixture uses virtual time and appears deterministic. It does not cover the missed value-setting path.

Both permitted Cargo filters failed before running because the sandbox denied target-directory creation. `git diff --check` passed; no files changed. Full conformance, caps, and native platform qualification were not rerun.

**CHANGES**
