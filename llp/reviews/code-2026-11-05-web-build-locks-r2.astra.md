# Code review: web build locks, round 2, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `3cd5164e85` (one source commit on `70365218ab`).
- **Method:** one brief (sha256 `5cd61991ca68c24c330651be237797ae31525aed121a04cc5bafd13aeb7bf352`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review worktree and verification files.

R2 raises **one P2 finding**. Settlement 1 is incomplete; settlement 2 is accepted.

1. **P2 — Read-only probes still request an exclusive lock, breaking Linux NFSv4.** In `host/web/build-lock.mjs:52–57`, the probe opens with `'r'` but calls `flock(fd, 6)` (`LOCK_EX | LOCK_NB`), through `lockHeld` at line 92. Linux requires write access for exclusive locks over NFS; its NFSv4 implementation explicitly returns `EBADF` for this combination. Consequently, probing an existing, unlocked `dist.lock` throws, and generation lookups calling `swapping()` can reject instead of returning their normal result. The base used a shared probe. Use `LOCK_SH | LOCK_NB` for POSIX probes while keeping acquisition exclusive. This finding follows from the code and native contract; I did not execute Linux/NFS. [flock documentation](https://man7.org/linux/man-pages/man2/flock.2.html), [Linux NFSv4 implementation, `nfs4_proc_lock`](https://raw.githubusercontent.com/torvalds/linux/master/fs/nfs/nfs4proc.c).

The R1 settlements:

- **Settlement 1: reject as complete.** The access-flag correction repairs the original macOS regression. My independent, read-only comparison returned `false/true/false` for base and R2 around a real held OS lock; R1 threw `EPERM`. The chmod negative and positive logs also support that repair. The exclusive probe issue above remains.
- **Settlement 2: accept.** `replace-build.test.mjs:75–76` now checks both persistence and released ownership. The [negative log](/tmp/exact-build-lock-swap-negative.log) isolates the obsolete assertion, and the [positive log](/tmp/exact-build-lock-swap-positive.log) passes the entire four-test suite. I found no remaining affected deletion assertions.

All four passes are complete:

- **Adversarial correctness:** the finding above; no additional concrete binding, ABI, ownership or cleanup defect found.
- **Simplification:** no additional finding. Removing mixed PID-file compatibility follows James’s instruction.
- **Writing:** no additional finding. The actual final commit message accurately reports completed checks and explicitly limits native platform qualification.
- **Verification:** inspected logs support all six [required checks passing](/tmp/exact-build-lock-r2-checks.json), [18 cache/generation tests](/tmp/exact-build-lock-r2-tests.log), the repaired swap suite and the unchanged dev-build fixture. The complete [caps output](/tmp/exact-build-lock-final-caps.log) contains only the foldables violation; that file is identical to base. No supplied check remains pending. Native Linux and Windows execution remain unverified.

This round **does not raise nothing**, and **not every settlement is accepted**.