# Code review: LLP 1121 stage 3, commit 3, round 3, 2026-10-14 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort max, sandbox read-only, the landing worktree.
- **Method:** one brief (sha256 `3b9cf8131a7a27422d0b24124f3fb9d1b1fbe0ee39ff93e7a10cf19b7b74986a`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** Astra: one high, three medium; Grok: three medium.
- **Disposition:** applied without re-review (before digests in the journal and withdrawal; the held lock file's identity rather than a directory descriptor)

---

Found **one high and three medium issues**.

1. **High — Restart can apply a transaction that detected a conflict.** [tamber_serve.rs:833](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:833)  
   If the final digest check detects an editor’s change but journal unlink fails, `stop()` leaves the journal and valid temporaries. On restart, [recovery:351](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:351) checks only the temporary’s new digest and overwrites the editor’s file. Crashing after journal publication but before the final check has the same problem. This extends beyond the documented read-to-rename window. **Fix:** record the original digests in the journal and validate every pending destination during recovery; retain and refuse the journal on a mismatch.

2. **Medium — The recorded directory identity can belong to a directory whose lock is not held.** [tamber_serve.rs:299](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:299)  
   The lock is acquired before `state_id` is sampled. Replacing `.exact` between those operations leaves the server holding the old directory’s lock while recording the replacement’s identity. Every later `same_state()` passes, and another server can hold the replacement’s lock concurrently. **Fix:** retain an opened directory descriptor, derive its identity with `fstat`, open the lock relative to it, and verify the pathname before recovery. Use that descriptor for state operations.

3. **Medium — Restart skips a failed parent-directory durability barrier.** [tamber_serve.rs:265](/tmp/rv-s3c3/contract/cli/src/tamber_serve.rs:265)  
   If `create_dir` succeeds but flushing the app directory fails, startup exits while leaving `.exact` present. The next start takes the existing-directory branch and never retries that barrier. Flushing `.exact` itself does not guarantee persistence of its entry in the parent. A subsequent crash during a commit can therefore lose reachable recovery data on filesystems without implicit parent persistence. [fsync documentation](https://man7.org/linux/man-pages/man2/fsync.2.html)  
   **Fix:** flush the app directory on startup even when `.exact` already exists, before recovery or commits.

4. **Medium — The Unix restriction breaks the integration tests on Windows.** [tests/it/tamber_serve.rs:101](/tmp/rv-s3c3/contract/cli/tests/it/tamber_serve.rs:101)  
   The tests remain unconditional and expect successful startup, while `Server::start` now always rejects non-Unix platforms. They consequently fail or encounter broken pipes when run on Windows. **Fix:** gate the success-path tests with `cfg(unix)` and separately test refusal on non-Unix platforms.

No low-severity findings. Read-only review; integration tests were not run because they create files.
