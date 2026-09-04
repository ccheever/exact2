# A live publisher's stream lock can be stolen after 60 seconds

**Status:** Closed
**Resolution:** Permanent OS locks replace age/PID leases; token and inode checks guard staged conditional heads, old holders never unlink successors, and standalone head writes acquire the stream lock. Direct writers bypassing the adapter remain outside its serialized contract.
**Systems:** exact deploy
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1030.000 D3 item 5, `scripts/origin.mjs`

`putHead` is read-digest-then-rename, not an atomic compare-and-swap. Safety is the stream lock (`DirectoryOrigin.withLock`). `lockHolder` marks a lock stale when `age > 60s` *or* the pid is gone, and `withLock` deletes a stale lock on the first `EEXIST` even if `process.kill(pid, 0)` succeeded.

A publisher still inside `withLock` — large stream copies, a stuck NFS rename, `--slow-ms` ≥ 60s — can have its lock stolen. Two processes then both read digest D0 and both rename a seq-N head; last rename wins, both report success. Combined with overwriting `./app.plan` under the live head (`issues/20260904-stream-files-overwritten-before-head.md`), the loser's later put of the named files can replace the payloads the winner's head names. Clients then fail every check until a clean republish.

The first publisher's unconditional `finally { rmSync(lock) }` is a second
ownership bug (`scripts/origin.mjs:125`). After another process removes and
recreates the lock, the old owner deletes the successor's lock when it exits.
A third publisher can then enter while the successor is still live. The note
has no random ownership token and release checks no longer prove it owns the
inode it removes.

PID reuse inside 60s is the other side: a dead lock looks live and a second publisher refuses. O_EXCL itself is correct. Blob writes outside the lock are fine: blobs are immutable.

Do not treat a live pid as stale on age alone; a multi-host origin also cannot
interpret another host's pid without the recorded host identity. Give each
acquisition an unguessable token and remove only the lock still bearing that
token. Make `putHead` fail closed if the file changed between open and rename
(or write to a unique name and link). Test a live lock older than 60 seconds,
a stolen/replaced lock whose prior owner exits, and two publishers from
different host identities. Optionally fsync the temp file before rename.
