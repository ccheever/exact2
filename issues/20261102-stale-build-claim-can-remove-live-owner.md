# Recovering a stale build claim can remove a new live owner's claim

**Status:** Closed
**Severity:** P2
**Systems:** Apple build output ownership (`scripts/app.mjs`)
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

Two builds starting after a killed build can both acquire the same output claim. The stale-claim recovery at `scripts/app.mjs:1188` checks the file's contents and then unlinks its pathname. Another process can replace the stale file with its live claim between those operations. The first process removes that new claim and creates its own, leaving both callers inside the protected build. The release callback at line 1192 also unlinks by pathname without checking ownership.

This helper protects Apple Cargo products and the shared Swift scratch directory. Concurrent access can therefore overwrite build products or interfere with another build's output. A second content comparison before unlinking leaves the same race.

**Verification.** `/tmp/exact-review-build-lock-race.mjs` ran two processes against a private temporary claim. Process A executes the unchanged `claimBuildOutput` function extracted from the current source, with only its filesystem `rmSync` dependency wrapped by a barrier. A pauses after its final stale-content check. Process B calls the actual exported helper and acquires the stale claim. Resuming A lets it delete B's live claim and report success too. B's release then removes A's active claim. Both processes exited successfully; the fixture was removed. No app build was run, so product corruption is the consequence of the broken exclusion rather than a measured result.

**Fix.** Use an ownership mechanism whose acquisition and stale recovery cannot unlink another live holder, preferably an OS lock released when its process exits. Release must relinquish only the caller's ownership.

**Acceptance.** With two contenders recovering one stale claim, at most one enters the protected operation. Releasing either contender cannot remove another active claim. Ordinary contention and recovery after process death still work.

No matching ticket was found in this checkout. The older closed origin-lock ticket concerns a different lock expiring while its owner is alive.

**Resolution.** Hold a nonblocking OS flock on a permanent companion inode for the operation’s entire lifetime. Create and remove the JSON diagnostic record only while holding that lock; never unlink the OS lock file. Closing the descriptor or process death releases ownership. Repeated release calls do nothing.

**Verification after the fix.** The focused Bun regression started two child processes against one stale claim: exactly one acquired it. A waiting contender and a released owner could not remove another owner’s claim. After SIGKILL of the holder, the waiter acquired it. Normal release, repeated release, malformed stale metadata and stable lock inode checks passed. This exercised macOS process locking, without building an app; Linux uses the same flock operation and its libc errno accessor.
