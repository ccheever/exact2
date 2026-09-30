# The JS web build ignores a failed or missing wasm-opt and dies on an unrelated ENOENT

**Status:** Open
**Systems:** web JS target, build
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** LLP 1012.001 F9

`host/web-js/module.mjs` `buildFlow` runs `wasm-opt` with `spawnSync` and never checks its result. When binaryen is not on `PATH`, no `textflow.wasm` is written, and `host/web-js/build.mjs:245` `cpSync` fails with `ENOENT: … lstat …/target/web-js-modules/flow/textflow.wasm`. Through `bun host/web/build.mjs textflow-web`, all that prints is one stack frame (`at …/host/web-js/build.mjs:245:44`) and "the web build (the JS target) failed". Found 2026-09-30 building Textflow in a shell without `/opt/homebrew/bin` on `PATH`.

Fix: check `wasm-opt`'s spawn error and status in `buildFlow` (and its siblings in `module.mjs` that call it the same way), and name the tool and the fix. `host/web/build.mjs` could also print the error message, not only the last frame.
