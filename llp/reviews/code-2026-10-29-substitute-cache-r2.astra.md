# Code review: the runner's substitute cache, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, read-only sandbox, a throwaway worktree at `4025946c87`.
- **Method:** one brief (sha256 `85b44e35ecb5492bbfa21f0ec862fcd4691aba20e9bbb6d0d69d175c6e3bd694`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

Both round-1 findings are fixed and have regression coverage:

- [substitute_tests.rs:94](runner/src/runner/modules/substitute_tests.rs:94) asserts successful commits and handler execution, then checks zero additional parses across 38 scroll dispatches.
- [substitute_tests.rs:152](runner/src/runner/modules/substitute_tests.rs:152) checks bounded cache size across 50 targets and permission visibility only on `target-7`.

No new correctness or memory problems found. Every mutation of the target read by `this_target` goes through `set_module_target`. Clearing precedes announcements; exclusive mutable access prevents an intervening reader. The unchanged-target early return is safe.

The cache stores **target-filtered JSON**, with filtering before insertion. Identical substitute strings remain safe to share because filtering depends only on that JSON and the runner’s current target—not module-row identity. Mutating consumers clone the JSON.

Execution remains unverified here: `cargo test -p exact-runner --lib substitute_tests --no-fail-fast` failed before compilation because the read-only sandbox denied build-directory creation. I therefore cannot independently confirm passing tests or the reported cache-off failures.

VERDICT: APPROVE

No findings.
