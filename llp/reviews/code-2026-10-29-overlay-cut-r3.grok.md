# Code review: the exact-js overlay cut, round 3 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `66ede89624`.
- **Method:** one brief (sha256 `8dcab46b62f640eb3d71d571ec22dcfff57a6048509a22f4edb0ed8978eaa5c6`), shared by both reviewers, blind to the other review. The author (Claude, x2-perf4's workers) is not a reviewer.
- **Transcription:** the final message, unedited.

The load-time decision is taken once, cached on the realm that evaluated the module, and consulted before any encode on the paths that can run an overlay. Late `exact.overlay` writes stay invisible until the next load, on both targets.

**When.** The read is a plain `exact.overlay` get after synchronous top-level evaluation, still accounted as init:

- Native `Module::load` calls `__exact_has_overlay` only after `load_engine` (prelude plus bytecode) and the appId/grants checks, and stores the bool before the engine is installed (`js/src/lib.rs:560-564`, `js/src/prelude.js:1394-1396`). The bake entry `export const overlay = app.overlay` is part of that evaluation (`js/bake/src/lib.rs:1192-1193`). The bundle is an es2016 IIFE, so this evaluation is the whole module body.
- The wasm page realm reads it after the helper, prelude, and app scripts have been appended and the export check has passed (`host/web/module-glue.js:185-199`).
- The JS web target reads `source.overlay` once inside `install` (`host/web-js/ts-data.js:459`), after the ESM graph. A top-level await in `app` has settled before that read.
- A data module activated after first pixel decides inside that instance's own `load` or `prepare`, not at bake.

Anything other than `undefined`, and any throw, is stored as having an overlay (`prelude.js:1395`; a failed native call keeps the per-call path via `map_or(true)`). A worker placement never runs the function: native returns before `Module::overlay` (`data/src/placed.rs:783-791`, `data/src/mixed.rs:660-666`); the wasm worker `prepare` returns before the capture (`host/web/module-glue.js:108`) and `has-overlay` falls back to `{has: true}` (`module-glue.js:525-526`). That is the wasm rule under test: only an explicit `{has: false}` is none (`js/web/src/lib.rs:111-121`, `1224-1231`).

**Where it lives.** Native stores `has_overlay` on the `Module`. The page realm closes over `overlayAtLoad` (`module-glue.js:391`). A new `Module::loaded` or a new `prepare` decides again. `unload` drops the engine; `overlay` returns before the flag when the engine is gone (`js/src/lib.rs:1130-1138`), and a successful `load` overwrites the flag. A failed wasm replacement is disposed with its realm.

**Every overlay call.** Native and the wasm page both check the cached bit before encoding arguments, the answer, or writes (`js/src/lib.rs:1137-1140`, `js/web/src/lib.rs:566-572`). A module loaded without an overlay encodes nothing on those paths. A module loaded with one still encodes and calls, including after a later delete (the prelude then returns `{tag: 4}` and the answer shows). The wasm worker fallback still encodes, then returns `{tag: 4, worker: true}` without calling the function. That matches the explicit `{has: false}` rule and the LLP sentence above it (a worker placement shows answers only).

**Proxy at commit.** The decision is not read again. Commit uses the bool or the closed-over boolean. The overlay call still does `typeof exact.overlay` and the call (`prelude.js:1399`, `1413`); the proxy fixture counts those as the two reads per overlay (`js/tests/it/overlay.rs:216-235`). There is no descriptor walk on the commit path.

**LLP 1054.** The line at `llp/1054.000.000-declared-refreshes-and-kept-requests.rfc.md:61-64` matches the code: once, at install on the JS web target and at load on the native and wasm executors, and a later set or delete is not followed until the next load.

The tests pin that matrix: top-level overlay value and encode count, none encoded three times, reload add and remove, runtime set and delete ignored, proxy read once at load, a throwing load-time read still asked (`js/tests/it/overlay.rs:151-245`), the same install-time pin on web-js including a delete that still calls the installed wrapper (`host/web/tests/js-runtime.test.mjs:1113-1130`), and the wasm `{has: false}` rule.

VERDICT: APPROVE
