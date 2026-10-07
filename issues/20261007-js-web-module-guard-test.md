# exact-js-web: the module-guard browser test still fails

**Status:** Open
**Systems:** js/web/tests/browser.rs, host/web/module-glue.js
**Author:** Claude (Opus 5.5), triaging the async lane's first run on the mini
**Date:** 2026-10-07

`browser_modules_guard_their_own_builtins_and_refuse_bad_candidates` fails on main. The async lane runs it; the gate does not.

Two of its failures are fixed (87e38b939): its seeded-stream check, like the storage test's agent check, faked agent mode with `history.replaceState`, which the store and the seed have not read since 18d0dec29 (they read the launch URL's navigation entry). What remains:
- **`module exports mismatch the admitted client`**, thrown by `prepare` (`host/web/module-glue.js:171`) at the probe's line 362. One of the probe's fixtures declares grants or an app id its admitted identity does not match (`sameGrantDeclaration`). This happened on every one of three runs.
- **`private module grew document scroll height`** (the check at the probe's `pageHeight`): failed in one of four runs. It is flaky, and the private module iframe has no client rects, so something else grows the page.

Run on the mini: `cargo test -p exact-js-web --test browser browser_modules_guard` (Chrome; CHROME defaults to the app).
