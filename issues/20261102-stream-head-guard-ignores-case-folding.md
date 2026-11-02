# A case-variant `put` replaces a stream head or its lock

**Status:** Closed
**Systems:** filesystem tool (`filesystem/src/main.rs`, `filesystem/src/directory_windows.rs`), directory origin (`scripts/origin.mjs`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** The tool refuses `put` of `.exact/<channel>/<id>/exact.json` and of that directory's `.lock`, so a head moves only through `head` under the held lock and a compare of `previousDigest`. On a case-insensitive volume the same `put` with `Exact.json` or `.Lock` replaces the real file. The head moves with no lock and no digest check. Replacing `.lock` drops the name off the inode the current publisher holds; `Lock::verify` then fails that publisher, and the next `withLock` takes the new inode.

**Cause.** The guard compares the final segment as the exact strings `exact.json` and `.lock` (`filesystem/src/main.rs`, the `op == "put"` check, four segments under `.exact`). `DirectoryOrigin.put` forwards `safeRelative`, which rejects `..` and does not fold case (`scripts/origin.mjs`). A non-immutable `put` commits with `renameat`. Windows opens with `OBJ_CASE_INSENSITIVE` (`filesystem/src/directory_windows.rs`). Default APFS folds case the same way. A case-sensitive volume keeps the variant as a different name, which readers of `exact.json` do not open.

**Reproduction.** On a case-insensitive origin directory, with a stream already published:

```
origin.put(`.exact/${channel}/${id}/Exact.json`, Buffer.from('{"seq":1}\n'))
```

The bytes at `exact.json` are the new buffer. The call is `put`, not `putHead`, and it does not hold the stream lock. The same with `.Lock` replaces `.exact/${channel}/${id}/.lock`.

**Fix.** Compare the guarded segments case-insensitively, and reject a final segment that folds to `exact.json` or `.lock` anywhere the tool would replace an existing file. Do the same for the directory origin before it calls the tool. A case-sensitive volume should refuse the folded name as well, so a publisher cannot plant a twin that a later copy onto APFS collapses.

**Acceptance.** `put` of `Exact.json`, `EXACT.JSON`, `.Lock` and `.LOCK` under `.exact/<channel>/<id>/` is refused with the existing head/lock error. `putHead` under the held lock still publishes. A blob put at `.exact/blobs/<sha256>` is unchanged.

**Resolution.** The native filesystem helper and directory-origin adapter reject case-insensitive aliases of guarded stream heads and lock names.

**Verification after the fix.** All 10 filesystem tests passed, including .EXACT, Exact.json, .LOCK and ordinary blob writes. Adapter regressions also verified refusal before invoking the helper.
