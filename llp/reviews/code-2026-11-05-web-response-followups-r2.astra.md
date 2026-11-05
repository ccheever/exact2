# Code review: web response follow-ups, round 2, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `c8cc6b3314` on `ea352093a8`.
- **Method:** one brief (sha256 `191edc6b3a7599de23334d9047e4081a72ee00836ea564d60973688c31d76fe9`), one reviewer, pointed at the full change and the delta from round 1, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; round 2 accepts every prior settlement.

No remaining findings. I completed all four passes over `c8cc6b3314` against `ea352093a8`, including the delta from `84eafb1a03`.

Every prior settlement is accepted. Saving and restoring the pending signal closes the round-1 finding in both directions. I also accept Retry-After forwarding, settlement after a stream’s first answer, browser resubscription, and preservation of pending semantics for unanswered streams and heartbeats. No further simplification or writing changes are required.

I independently ran read-only Bun probes covering both rollback directions, visible pending-state observers, and subsequent valid-answer recovery. Removing only the rollback correction in memory reproduced both defects. Passing the captured runtime states through the actual renderer produced the expected 410/settled and 503/unsettled responses, activation modes and checkpoints. These probes used controlled transports and an in-memory renderer entry; they were not compiled browser runs. `git diff --check` passed, and the worktree remains unchanged.

The new regressions assert observable behavior. The retained negative-control log shows all four fail at their behavioral assertions when pending rollback is removed.

Verification limits: I inspected, but did not independently rerun, the author’s 84 passing web/runtime tests, 13 passing build tests, or Cargo/boot checks. Build, Clippy, formatting, boot and workspace checks report success. The full Rust run still failed on Apple executor and SVG timing checks; isolated passes with both main and candidate do not establish a passing full suite. Caps retains two unchanged violations outside this diff. Firefox was excluded as instructed.