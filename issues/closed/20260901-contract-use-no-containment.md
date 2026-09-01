# `use` resolves any `.contract` path; fonts already refuse `..`

**Status:** Closed
**Resolution:** Contract use resolution now canonicalizes and confines every transitive import below the app directory.
**Systems:** Contract compiler
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1017 P8, LLP 1019 (font sources must be portable relative paths under `assets/`)

The only path rule is `ends_with(".contract")`. Load is `dir.join(&u.path)` then `read_to_string` (`contract/cli/src/lib.rs`). `../`, extra `..` segments, and absolute paths (`/tmp/x.contract`) are read if they exist. Cycle detection canonicalizes, but it does not require the target to stay under the entry file's directory.

Font sources must be portable relative paths under `assets/` with `ParentDir` refused. `use` never got that check. For a trusted local author this is `#include`. For a hosted or untrusted compile it is path traversal. Merge also drops the used file's directory, so a font declared in `lib/row.contract` is resolved against the *entry* file's parent.

Fix: resolve against the using file, then require the canonical target to be a `.contract` file under the app root. Refuse `..` and absolute paths with `contract-use-path`. Keep relative `./foo.contract` as the only spelling. Do not follow a symlink out of the app dir.
