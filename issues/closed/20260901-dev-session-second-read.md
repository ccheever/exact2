# Dev `Session` compiles a second read of the file, not the bytes it just hashed

**Status:** Closed
**Resolution:** Dev compilation now consumes the exact watched source snapshot while retaining its path for relative assets and uses.
**Systems:** Web host
**Severity:** P3
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** uncommitted `host/web/src/dev.rs` (failed-stamp re-read)

The WIP `Session::poll` re-reads after a failed compile even when `(mtime, length)` is unchanged — the right idea for a torn save. It then sets `self.last` from the first `read_to_string` and calls `build`, which ignores those bytes (`let _ = src`) and runs `contract::compile_path(&self.source)` — a second read.

If the first read is good, `last` is good, and `compile_path` sees torn bytes and fails, the next poll with now-good bytes of the same stamp hits `last == src` and returns `None` without compiling. The failed-stamp test only covers last=bad then src=good.

Fix: compile the bytes already in hand (`contract::compile(&src)`), so `last` and the compile agree. Keep the failed-stamp bypass.
