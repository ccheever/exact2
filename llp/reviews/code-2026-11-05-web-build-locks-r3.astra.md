# Code review: web build locks, round 3, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `ee9d96c8d0` (one source commit on `70365218ab`).
- **Method:** one brief (sha256 `76243070185adfe193a17f66c4d0115d030e69b20408b29a61f0cef299a5d5ab`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

**R3 raises nothing. Every R1/R2 settlement is accepted.** I reviewed `ee9d96c8` against the base and R2, including all changed code, tests, prose and lock callers.

- **R1 settlement 1 — accepted.** Both backends retain read-only probe handles. The chmod regression remains covered, and R3’s shared probes complete the correction.
- **R1 settlement 2 — accepted.** The build-swap test checks persistence and released ownership. Its complete suite passes, and the sweep found no remaining affected deletion assertions.
- **R2 settlement — accepted.** Builds request exclusive locks; probes request shared locks: POSIX flags `6/5`, Windows flags `3/1`. This resolves the read-only NFS access-mode defect and shared-reader false positive according to the native contracts. [flock](https://man7.org/linux/man-pages/man2/flock.2.html), [LockFileEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex).

My independent macOS comparison confirmed that base and R3 report `false` with a real shared reader, while R2 reports `true`. All three detect an exclusive holder and report unlocked after release. The supplied [negative regression](/tmp/exact-build-lock-shared-negative.log) independently demonstrates the R2 failure.

The four passes found no further concrete issues:

- **Adversarial correctness:** no additional ownership, persistent-file, cleanup, caller, FFI or ABI defect found.
- **Simplification:** the shared/exclusive distinction remains small and native. Removing PID-file compatibility is authorized; no fallback or governance mechanism was added.
- **Writing:** the actual commit message accurately describes shared read-only probes, the latest Apple test failure and native qualification limits.
- **Verification:** [23 tests pass](/tmp/exact-build-lock-r3-tests.log), including the preserved regressions, ten rounds of eight contenders and eight contenders following holder death. The earlier [real dev-build test](/tmp/exact-build-lock-dev-bin.log) remains applicable to its unchanged fixture and acquisition behavior.

The [final gate JSON](/tmp/exact-build-lock-r3-checks.json) is complete: build, clippy, formatting, workspace and boot pass; Rust tests exit `101`. The [completed test log](/tmp/exact-build-lock-r3-test.log:3693) contains only the Apple worker-start failure reproduced by both preserved controls. The Apple source tree is identical to base, supporting baseline attribution. The complete [caps output](/tmp/exact-build-lock-r3-caps.log) reports only the unchanged foldables violation.

No required run or control remains pending. **Native Linux, NFS and Windows execution remain unverified.** The macOS shared-reader result establishes the local counterexample and correction, without claiming qualification on those platforms.