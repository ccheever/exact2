# Code review: Linux and Android store writes on a thread of their own, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `407dfa914a`.
- **Method:** one brief (sha256 `1785d5f1cdfb1866e64f8c67db1e489bd039a1f0ea819f852787c2e797205f3c`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

1. **Empty commits can lose required retries** — [store_writer.rs:95](host/linux/src/store_writer.rs:95). If commit A is queued or being written, `queue.pending` can still be false. An empty commit B is then discarded; if A fails, B never retries it. Coalescing consecutive empty commits also removes retry opportunities. This violates condition (2). Preserve each commit boundary, including empty commits, and decide whether it needs a flush on the writer thread.

2. **Boot can read before earlier writes finish** — [system_support.rs:25](host/linux/src/system_support.rs:25). After five seconds, the snapshot proceeds against potentially stale or partially updated files. The new runner can retain those stale values even after the writer catches up. Also, the carried-launch return bypasses the barrier, although [host.rs:256](host/linux/src/host.rs:256) still constructs the presence backend, which reads and reconciles secret files. A carried reload adding a presence policy can therefore race an earlier plaintext write. Establish the barrier before every boot-time file consumer and fail or defer boot when it cannot complete.

3. **Queued writes can change destination or persistence policy** — [store_writer.rs:72](host/linux/src/store_writer.rs:72). The queue captures only the app ID and values; `flush` resolves the environment later. Changing `HOME`, XDG variables, or `EXACT_AGENT*` between submission and execution can redirect writes, silently discard them, or persist a nameless agent drive’s supposedly memory-only secrets into the real store. Capture each commit’s storage context, including disabled-storage and secret-storage policy, when submitting it.

4. **Windows inherits asynchronous persistence without an exit drain** — [host.rs:1275](host/linux/src/host.rs:1275). Windows uses this same `Host`; its [window-close path](host/windows/src/window.rs:713) ends hatches/packages and exits without draining. Excord then calls `process::exit`. Closing the window can now discard successfully committed writes during an orderly exit. Add the bounded drain to Windows’ event-loop exit path.

5. **One app can consume another app’s warnings** — [store_writer.rs:89](host/linux/src/store_writer.rs:89). Warnings are process-wide and untagged. After app A’s write fails, app B’s next submission takes the warning and logs it in B’s runner. A never receives it, and the pending write’s `warned` flag suppresses repetition. Keep warnings associated with their app and return only that app’s warnings; cover two app IDs in a test.

6. **The retry test leaves another retry running while changing its fixture** — [host_tests.rs:107](host/linux/src/host_tests.rs:107). The final `advance` retrieves warnings but also queues an empty retry. After step 7, that retry can observe the gap between removing the blocking directory and creating `"old"` at lines 139–140. `forget` then succeeds against an absent file, clears the pending deletion, and step 8 leaves `"old"` behind. Drain after the final `advance` before modifying files or environment. Separately, add a gated test for finding 1; the existing drain-before-next-commit sequence masks it.

**Non-blocking**

- **Conditions (1) and (2), otherwise:** ordinary runner reads see writes immediately in memory. Nonempty submissions remain FIFO and use the unchanged name-ordered `StoreWrites::flush`. Sets retain temp-file/write/fsync/rename behavior; deletes retain unlink behavior. Directory fsync remains best effort in both vendored stores, as before.

- **Condition (3), durability wording:** a process death can now lose an entire queued tail of already-returned commits. Previously, successful writes had completed before their commit returned. A drain timeout leaves that risk outstanding. Moreover, `drain(true)` means queued attempts finished: it excludes retained failures and cannot cover later submissions. The “all on disk” claims in the JNI/Kotlin comments should reflect those limits.

- **Deadlocks: none found in the reviewed paths.** `Owner.drainStore` runs behind earlier immediate exact-thread posts and shares the two-second budget with the native wait. The writer releases the queue mutex during I/O, and `busy` prevents a mid-write drain from succeeding. Boot and teardown introduce no reverse dependency on the calling thread.

- **Tests:** `GATE` usefully checks asynchronous handoff and timeout, but the final-state assertions would also pass an implementation that merged commits. The abort child validly checks process death after the Rust drain; it does not exercise Android’s posting barrier, intermediate crash states, or power-loss durability. Add explicit intermediate-order and barrier coverage.

- **Condition (4):** Apple’s synchronous persistence and the web’s handoff to the page remain unchanged. Windows is the shared-code exception identified above.

Static review only; no files edited, tests or builds run, or network access used.
