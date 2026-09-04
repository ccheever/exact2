# DirectoryOrigin follows symlinks outside its root

**Status:** Closed
**Resolution:** LLP 1030.002 routes every origin verb through no-follow directory handles; atomically exchanged symlink race tests keep outside bytes private and untouched.
**Systems:** Delivery, exact deploy, Security
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** scripts/origin.mjs; LLP 1030.000 D3

`DirectoryOrigin.path()` checks only that `resolve(root, rel)` has the root as
a lexical prefix (`scripts/origin.mjs:60-65`). `get`, `list`, `put`, lock
creation, and rename then follow filesystem symlinks. A symlinked component
inside an otherwise valid origin path escapes the declared origin.

This was reproduced with a temporary origin containing `escape -> ../outside`:
`DirectoryOrigin.put("escape/proof.txt", bytes)` created
`outside/proof.txt`. The temporary tree was removed after the reproduction.
A stale or malicious symlink under `.exact/<channel>/<id>`, `releases`, or a
root asset path can therefore make deploy overwrite files outside the origin;
reads and listings can disclose them to the classifier as well. `serve.mjs`
already rejects this class with real-path containment, so publish and serve
currently disagree about the same tree.

Done when every origin operation refuses a symlink in any traversed component
and cannot be raced between validation and open/rename (for example via
directory-relative no-follow operations or an equivalently owned directory
handle). Test file and directory symlinks for get/list/put/putHead/withLock,
including a non-existent final path and a replacement race.
