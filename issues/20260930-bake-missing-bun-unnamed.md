# The bake reports a missing Bun as a bare 'No such file or directory'

**Status:** Open
**Systems:** bake, build
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1012.001 F9

Without Bun on `PATH`, `cargo build --release -p caltrain-linux` fails in the app's build script with:

```
panicked at apps/caltrain/linux/build.rs:29:29:
compatibility id: No such file or directory (os error 2)
```

The shader inventory in `bake/src/compat.rs` spawns Bun (`crate::bun()…output().map_err(|e| e.to_string())`), and the spawn error loses the program's name. The message names neither what is missing nor the fix, and reads as a missing source file. Found 2026-09-30 when a shell without `~/.bun/bin` on `PATH` rebuilt Caltrain for Linux; the same build passed with Bun on `PATH`.

Fix: name the program and the remedy in the error (for example `bun: not found on PATH (the bake runs Bun for the shader inventory; install it or put ~/.bun/bin on PATH)`), wherever `crate::bun()` is spawned.
