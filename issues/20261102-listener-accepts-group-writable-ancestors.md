# Listener path checks accept directories other group members can replace

**Status:** Closed
**Severity:** P2
**Systems:** exact-listen, local socket security
**Author:** Codex
**Date:** 2026-11-02

**Problem.** A socket under a non-sticky, group-writable ancestor passes the
listener's private-path checks. Another member of that group can rename the
private child directory and substitute a directory and socket of their own.
The socket's `0600` mode and its immediate directory's `0700` mode do not
prevent renaming that directory through its writable parent.

**Cause.** `host/listen/src/bind.rs:114` checks `mode & 0o002` but ignores
`0o020`. This contradicts the function's documented rule that ancestors may
not be writable by other users unless sticky.

**Reproduction.** Using the built listener ABI at revision
`5b94c216a6fee8fca42026f5cb2e9a80063da03a`, created a private socket directory
under a `0770` ancestor and opened a listener. It emitted `Listening`, rather
than `Refused`. The mode acceptance was exercised locally; an attack by a
second OS account was not run. Exploitation requires another member of the
directory's group and traversable ancestors.

**Fix.** Include group write permission in the ancestor restriction, preserving
the intended sticky-directory exception. Cover a `0770` ancestor as well as a
world-writable ancestor in the path tests.

**Resolution.** Socket-directory preparation rejects ancestors writable by the group or others unless they have the sticky bit.

**Verification after the fix.** All 19 listener tests passed, including the real-socket ignored suite and 0770/0777 ancestor cases with and without the sticky bit.
