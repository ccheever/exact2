# Round 1
Two defects in `2e0ad2a9f`:

1. **[P2] `spawn()` returning is not a sufficient Linux barrier** — [support.rs:250](/Users/admin/projects/exact2-wt-upd/update/tests/it/support.rs:250). Glibc’s `posix_spawn` uses `CLONE_VFORK`. On Linux 6.12, `exec_mm_release` wakes the parent **before** `do_close_on_exec` runs. Consequently, `spawned` can release its write guard while the child still holds `owner.lock`, allowing another reopen to fail. This follows from the [glibc implementation](https://raw.githubusercontent.com/bminor/glibc/master/sysdeps/unix/sysv/linux/spawni.c), [Linux exec ordering](https://raw.githubusercontent.com/torvalds/linux/v6.12/fs/exec.c), and [vfork completion code](https://raw.githubusercontent.com/torvalds/linux/v6.6/kernel/fork.c). For these two short commands, keep the write guard through `wait()`/`wait_with_output()`.

2. **[P2] Windows lint regression: unused `Store` import** — [windows.rs:3](/Users/admin/projects/exact2-wt-upd/update/tests/it/windows.rs:3). Replacing its sole `Store::open` call leaves `Store` unused. Windows checks with `-D warnings` will fail. Remove that import.

The descriptor-inheritance diagnosis is right on both platforms. macOS’s kernel `posix_spawn` also copies descriptors before closing CLOEXEC descriptors during image activation; its synchronous path supports this guard’s intended boundary. Linux’s return timing is the problem above. Rust’s fork/exec fallback waits for its CLOEXEC error channel, which should likewise not be described as a universal guarantee that every inherited descriptor has finished closing. [XNU implementation](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/kern/kern_exec.c), [Rust spawn implementation](https://doc.rust-lang.org/src/std/sys/process/unix/unix.rs.html).

**Coverage is complete for the current harness:** all `Store::open` calls reach `open_store`, including durability through `support::open`, Windows refusal tests, and the owner child. Both process launches use `spawned`.

**I found no deadlock in the current call paths.** Read guards end before callers receive their stores; waits occur outside the write guard; poisoning recovers the guard around stateless `()`; and the executed child gets a fresh static lock. Holding the parent’s write guard through these child waits would also be safe.

**Yes, retain a product QUEUE item.** [store.rs:484](/Users/admin/projects/exact2-wt-upd/update/src/store.rs:484) still immediately refuses a reopen while an unrelated spawned child retains the previous owner’s descriptor. An application concurrently spawning and dropping/reopening its store can encounter exactly this availability failure. The harness synchronization does not resolve that existing product limitation.

Source review only; no tests run or files changed.
# Disposition

Fixed: the write guard is now held to the child's exit (`finished`), since on Linux glibc's vfork wakes the parent before the child's CLOEXEC descriptors close; the unused Windows `Store` import is gone. A QUEUE line records the product-side window for an embedder that reopens while spawning (the hosts open once at launch). The probe, rerun with `finished`: 0 failures in 3,000 cycles beside 444 spawns.
