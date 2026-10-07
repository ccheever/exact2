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

## Update (2026-10-07)

Three of its failures were stale test scaffolding, fixed in js/web/tests/browser.rs:
- **The exports mismatch** was the storage fixture's grants: `js/tests/fixtures/storage.ts` (and `js/tests/it/storage.rs` `GRANTS`) declare `fs.read doc:/` and `fs.write doc:/`, which the probe's `storageIdentity` and its grant-set list lacked, so admission rightly refused the module. The probe now admits the fixture's own grants.
- **The agent-mode check** faked agent mode with `history.replaceState`, which the store has not read since 18d0dec29; it stands in the navigation entry, as f6072f214 did for the storage service test.
- **The scroll-height check** compared against the height at the guest's creation, so any growth of the page's own content tripped it (1 run in 4). It now compares the page with and without the private frames, at once.
- **The abandoned-stat step hung** (an answer queued behind an abandoned call's storage parks until a background round delivers it, LLP 1097 D7); the probe now runs background rounds as the runner does (`drained`).

**What remains: a decision on LLP 1097 D7.** The abandoned-open step still fails ("abandoned open kept its database locked"). Before D7 the host discarded an abandoned call's open and closed its handle. Under D7 the abandoned call's chain continues in the background: the fixture's `.then(db=>{store.set('session','orphan');return db.close();})` runs with the background's store context, which grants nothing, so the `store.set` throws, `db.close()` never runs, and the database stays locked for the realm's life. Either the host should close a let-go call's open handles when its chain fails (an app that throws while an open is pending would otherwise lock its database), or the probe's expectation is pre-D7 and should change. For the owner of LLP 1097.
