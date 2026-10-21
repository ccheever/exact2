# Code review: image spools in a per-process, lock-held directory, swept at launch (Apple), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/bsky-rs-rv`.
- **Method:** one brief (sha256 `4125618ef87b5df8d9efcdcffecf9e2c4eeb111e19fa8cf1ff819b6e4cf7b066`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit cb94d1a35 in a detached worktree. The author (Claude, for the Bluesky clone) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition (r2, b3a36408b):** All five taken. 1: only ENOENT sweeps; any other open failure keeps the directory (test: an unreadable lock that a live descriptor holds). 2: the directory is published only once made and locked; a failure throws through the throwing spool paths and the next spool tries again (test: a root that cannot be made). 3: creating and sweeping hold `.namespace.lock`, which closes the create-before-lock window outright, so the 60 s grace period is gone. 4: `O_CLOEXEC` on every descriptor (test: the flag is set); fork without exec is not something the host does. 5: every test uses a private root.

# Round 1

1. **P2 — Failed lock-file opens can cause deletion of a live owner’s directory.** [RasterSpool.swift:54](/tmp/bsky-rs-rv/host/apple/Sources/ExactKit/RasterSpool.swift:54) treats every `open` failure as a missing lock. `EACCES`, `EMFILE`, and other errors do not establish that the owner is dead; advisory locking does not prevent unlinking its files. **Fix:** delete on confirmed `ENOENT` only; preserve the directory on other errors. Add a regression covering an inaccessible but held lock.

2. **P2 — Initialization failures permanently publish an invalid or unprotected directory.** [RasterSpool.swift:24](/tmp/bsky-rs-rv/host/apple/Sources/ExactKit/RasterSpool.swift:24) ignores directory creation, `open`, and `flock` failures. Failed creation breaks every subsequent spool until restart, even after the underlying failure clears. Failed locking can allow spooling but leaves those files eligible for deletion. **Fix:** publish the directory only after successful creation and locking, close failed descriptors, and allow initialization to retry. Propagate failure through the existing throwing image-loading path.

3. **P2 — The grace period reduces the creation race but does not close it.** [RasterSpool.swift:26](/tmp/bsky-rs-rv/host/apple/Sources/ExactKit/RasterSpool.swift:26) permits this sequence: an owner opens `.lock`, pauses before `flock` for over 60 seconds, and another process locks and removes the directory. The owner can subsequently acquire its already-unlinked lock and cache a nonexistent directory—even if every return value were checked. **Fix:** coordinate directory creation and sweeping with a common namespace lock. Test a creator paused before locking, without real-time sleeps.

4. **P2 — The owner descriptor survives exec.** [RasterSpool.swift:26](/tmp/bsky-rs-rv/host/apple/Sources/ExactKit/RasterSpool.swift:26) omits `O_CLOEXEC`. An inheriting helper can retain the lock after the app dies, preventing reclamation until that helper exits. I confirmed this with a read-only probe on this Mac’s APFS volume. **Fix:** use `O_CLOEXEC`, including for sweep descriptors. For fork-without-exec, explicitly account for the inherited descriptor; unlocking it in the child also unlocks the parent’s shared lock. [Darwin flock semantics](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flock.2.html).

5. **P2 — The test advances time against the real shared spool root.** [RasterSpoolTests.swift:64](/tmp/bsky-rs-rv/host/apple/tests/ExactKitTests/RasterSpoolTests.swift:64) sweeps every application’s directory with `now + 3600`, bypassing the grace period for another process currently creating its directory. **Fix:** inject an isolated root for initialization tests. The existing separate-open test correctly exercises lock contention, but the suite needs failure, initialization-race, and descriptor-inheritance coverage.

The basic design otherwise fits: established locks protect idle directories regardless of mtime, and simultaneous sweepers of abandoned directories are harmless when directory names are never reused. The local integer going out of scope does not close its descriptor. Swift’s static initialization is thread-safe. [Swift type properties](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/properties/#Type-Properties).

App extensions have separate containers; App Groups introduce an explicitly shared container rather than automatically sharing ordinary temporary storage. The iOS/tvOS legacy cleanup restriction is therefore reasonable, and avoiding it on unsandboxed macOS is appropriate. [Apple container guidance](https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/ExtensionScenarios.html#//apple_ref/doc/uid/TP40014214-CH21-SW1). I found no repository dependency on the old spool filename. XCTest was not rerun because it writes files; the checkout remains unchanged.

**Verdict: READY WITH CHANGES.**

# Round 2

- **Method:** one brief (sha256 `890596ad68ab6ccfcd02db9f2ea5e07608743509cb86ba8f39aa0d8ed8abf8bc`), blind to the other review, on b3a36408b with round 1's artifacts.
- **Verdict:** NOT READY.
- **Disposition (r3, HEAD):** All four taken. 1: a symlinked root is `unlink`ed (it cannot remove a directory, so a real root another launch made survives) and the root is checked a real directory under the namespace lock before anything is swept or made (test: a second establish keeps the first's live spool). 2: the namespace lock is asked for 20 times, 25 ms apart, then the spool throws and the next one asks again (test: a held lock fails fast, without sweeping, and recovers when released). 3: the test reads the held descriptor's own FD_CLOEXEC. 4: the spool is an instance; a test fails one on an unwritable root and the same instance succeeds once it is writable.

1. **P2 — Concurrent root repair can delete a live launch’s spools.** [RasterSpool.swift:42](/tmp/bsky-rs-rv2/host/apple/Sources/ExactKit/RasterSpool.swift:42). Both launches can observe the same symlink. Launch A replaces it, establishes its directory and starts spooling; launch B then executes `removeItem` against A’s newly created root. This happens before acquiring the namespace lock. Foundation’s removal [resolves the path again](https://github.com/swiftlang/swift-foundation/blob/main/Sources/FoundationEssentials/FileManager/FileManager%2BFiles.swift#L269), so the earlier symlink observation does not protect the replacement directory. **Fix:** use nonrecursive `unlink` for symlink removal, revalidate concurrent replacements, and anchor sweeping to a root opened with `O_DIRECTORY | O_NOFOLLOW`. Add a deterministic two-launch repair test.

2. **P2 — Namespace contention can indefinitely stop all image work.** [RasterSpool.swift:44](/tmp/bsky-rs-rv2/host/apple/Sources/ExactKit/RasterSpool.swift:44). `wait: true` performs a [blocking flock](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flock.2.html) while `directory()` holds its `NSLock`. If another process pauses while holding the namespace lock, one image worker waits in `flock` and the other waits on `NSLock`. These are the process’s only two raster workers, so bundled-image work also stalls. Cancellation and HTTP timeouts cannot release these waits. **Fix:** use bounded, cancellation-aware acquisition, throwing on expiry without publishing a directory. Test a held namespace lock and subsequent recovery.

3. **P3 — The close-on-exec assertion tests its own descriptor.** [RasterSpoolTests.swift:73](/tmp/bsky-rs-rv2/host/apple/tests/ExactKitTests/RasterSpoolTests.swift:73). The test opens a new descriptor with `O_CLOEXEC`, then checks that descriptor’s flag. It still passes if every production `O_CLOEXEC` is removed. A read-only descriptor probe confirmed this independence. **Fix:** inspect the actual production descriptor or exercise inheritance through an exec helper.

4. **P3 — The failure test does not prove retry or cache recovery.** [RasterSpoolTests.swift:103](/tmp/bsky-rs-rv2/host/apple/tests/ExactKitTests/RasterSpoolTests.swift:103). It calls the uncached `establish` helper once with an invalid parent. It never exercises `directory()`, restores access, or attempts another spool. **Fix:** test the cached path against an isolated, unwritable root, restore permissions, and verify the next attempt successfully creates and holds its lock.

Round-1 audit: the lock-open error handling, publication-after-success, descriptor flags, and test-root isolation are corrected. The namespace lock closes the original creation race for a stable root; symlink repair remains incomplete as finding 1 describes. `NSLock` protects all accesses to `established`; unwritable-root setup failures throw without caching success and release acquired descriptors.

XCTest was not run because it writes files. No files were edited.

**NOT READY**

# Round 3

- **Method:** one brief (sha256 `f38a4a655f46ff2e3cd12fb2235c45925468c41431d58da62084984cf08d536a`), blind to the other review, on 335aa6214 with rounds 1 and 2's artifacts.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** Both taken (29d34558c): a test pauses one launch between seeing the symlinked root and unlinking it, through a test-only hook, while another repairs and spools; it fails with the old `removeItem`. The first spool's own sweep is tested inside the spool root, with no explicit sweep.

1. **P3 — Root-repair test does not reproduce the earlier race.** [RasterSpoolTests.swift:139](/tmp/bsky-rs-rv3/host/apple/tests/ExactKitTests/RasterSpoolTests.swift:139). The second `establish` starts after the first finishes, so it observes a real directory. This test also passes with the previous, unsafe `removeItem` implementation. **Suggested fix:** deterministically pause one creator after observing the symlink, let another establish and write a spool, then resume the first and assert that the live spool survives.

2. **P3 — Establishment test does not prove abandoned directories are swept.** [RasterSpoolTests.swift:68](/tmp/bsky-rs-rv3/host/apple/tests/ExactKitTests/RasterSpoolTests.swift:68). Its “dead” directory sits outside `exact-raster`; the subsequent sweep is called directly by the test. Removing the production `sweep(root)` call from `establish` would leave this suite passing, despite disabling launch cleanup. **Suggested fix:** seed an abandoned directory inside `exact-raster`, call `directory()`, and assert its removal without an additional explicit sweep.

The earlier production defects are resolved for cooperating launches: conservative lock-open failure handling, publication only after successful setup, retry after failure, namespace serialization, bounded contention, and descriptor inheritance protection. `NSLock` guards the cached state consistently. Unwritable-root setup throws without caching success. The revised descriptor and retry tests address their earlier findings, and all test roots are isolated.

XCTest was not run because the review is read-only. `git diff --check` passed; no files were edited.

READY WITH CHANGES
