# exact-js-web: after activation, typing in a baked field dispatches no edit

**Status:** Closed
**Resolution:** A stale fixture, not a startup bug: the edit does dispatch, as a text field's commit (kind 41, selection then value, x2apps codeedit #2), not kind 1 (a non-text control's change); the next step expected the range to snap back to the app's 50, which LLP 1069.001 D4 amended (2026-10-04) replaced with keeping the person's value until the bound value changes. Both assertions updated; the test passes 3 of 3 on the mini.
**Systems:** js/web/tests/browser.rs, host/web (startup gating of a baked page)
**Author:** Claude (Opus 5.5), found fixing the module-guard test's storage step
**Date:** 2026-10-07

`browser_modules_guard_their_own_builtins_and_refuse_bad_candidates` gets past its storage step now (LLP 1097 D7: the abandoned open closes its database in a finally) and fails later, in its startup section, on every run (3 of 3):

    AssertionError: ready input dispatches edits (false !== true)

The probe opens `/startup`, checks that an unready app neither dispatches nor edits its baked input, calls `startup.release()`, then clicks `action`, clicks `editor` and types `ready`. The button's press dispatches (`event.id === 2, kind === 0`), but no input edit (`id === 3, kind === 1`) reaches `startup.dispatch`.

It reproduces with main's own runtime files (origin/main host/web-js and js/src, only the test's storage fixture changed), so it is on main, hidden until now behind the storage step that failed first. The async lane runs this test; the gate does not.

Run on the mini: `cargo test -p exact-js-web --test browser browser_modules_guard`.
