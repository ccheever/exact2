# Code review: web build locks, round 1, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `3f8ddbf0a9` (one source commit on `70365218ab`).
- **Method:** one brief (sha256 `e109c4d37b72e00353cfffaf2061c6c1104bca3ffb4aa390e917d5f71617de4c`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

**This round raises two P2 findings; it does not raise nothing.** I reviewed all four changed files, their callers, the governing instructions, and the supplied evidence.

1. **[P2] Ownership probes now require write permission.**  
   `host/web/build-lock.mjs:52` opens an existing file with `r+`; the Windows branch at line 24 likewise requests read/write access for probes. `lockHeld` therefore throws when the lock file is readable but not writable. Generation readers call it at `host/web/generations.mjs:180`, including when deciding whether a missing generation is absent or reaped. A read-only serving process can now throw instead of returning that result.

   I reproduced this without editing files, loading both exact revisions into Bun 1.4.2. For the same readable file in the read-only sandbox, the base returned `true` while a real OS lock was held and `false` after release; the candidate threw `EPERM` in both cases.

   Give probes read-only access and an appropriate native probe operation. Shared `flock` supports read-only descriptors, and Windows locking permits a handle opened with read access. [flock documentation](https://man7.org/linux/man-pages/man2/flock.2.html), [LockFileEx documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex).

2. **[P2] An affected build-swap test still requires lock-file deletion.**  
   `host/web/replace-build.test.mjs:74` asserts that `${dist}.lock` does not exist after `replaceBuild`. The candidate’s release at `host/web/build-lock.mjs:83` deliberately leaves it present. That assertion necessarily fails after a successful first build.

   Update it to check persistence and released ownership, then run `bun test host/web/replace-build.test.mjs`. The supplied passing GPU-cache/generation run does not include this suite. This finding follows directly from the code; I did not execute the file-writing suite under the review restrictions.

The **simplification pass** raises nothing additional. Removing the mixed PID protocol is authorized and simplifies ownership coherently. The **writing pass** raises nothing additional: the commit explicitly identifies unfinished checks rather than claiming verification is complete. Its promised final-results update should incorporate the results now available.

Native API inspection found no additional HANDLE, `OVERLAPPED`, or library-lifetime defect. The `u64` HANDLE representation matches [Bun’s requirement](https://bun.com/docs/runtime/ffi); the byte-64 lock and structure layout agree with Microsoft’s [LockFileEx](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex) and [OVERLAPPED](https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-overlapped) definitions. A separate Bun 1.4.2 probe confirmed retained symbols kept their library alive across forced garbage collections.

The **verification pass** establishes these limits:

- The inspected logs record passing [GPU-cache/generation tests](/tmp/exact-build-lock-final.log), [dev-build regression](/tmp/exact-build-lock-dev-bin.log), and [timeout assertions](/tmp/exact-build-lock-timeout.log). The [negative control](/tmp/exact-build-lock-negative.log) fails the persistent-file assertion with the base implementation.
- [Gate results](/tmp/exact-build-lock-checks.json) now show build, clippy, format, workspace, and boot passing.
- The necessary Apple control has finished. Both [base](/tmp/exact-build-lock-main-apple.log:633) and [candidate](/tmp/exact-build-lock-candidate-apple.log:630) reproduce the same worker-start failure. Baseline attribution is now supported.
- [Caps](/tmp/exact-build-lock-caps.log) reports only the foldables recipe violation; that file is 3,254 bytes in both commits.
- Linux and Windows execution remain unverified. That qualification gap is distinct from the two findings above. Verification is not complete while the affected build-swap test remains unresolved.