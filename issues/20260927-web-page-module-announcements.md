# A web page module's announcements connect only after a `native.later`, are applied once per announcement, and a failed load is cached until reload

**Status:** Open
**Systems:** Web host (`host/web/native-glue.js`, `host/web/glue.js`), runner announcements (`runner/src/runner/time.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1016.002, LLP 1067 D5

- **Announcements need a `native.later` first.** `connect` is called only inside `pageNative` (`host/web/native-glue.js:129-133`), and that is reached only from an `exact-native:` request (`glue.js:681`). A page module that exports `connect` to announce, say, connectivity is never connected unless something also calls `native.later`. A module that exports only `connect` is refused with "exports no later".
- **No coalescing on the web.** Every announcement calls `wasm.exact_changed` and then `applyBatch` synchronously (`glue.js:41`). Native hosts queue announcements, deduplicate them, and drain once (`runner/src/runner/time.rs:97-137`), as LLP 1016.002 describes. A burst of identical announcements re-asks the resource N times on the web and once on native.
- **A failed load is cached forever.** `globalThis.exact.nativeArtifact ??= new Promise(…)` (`native-glue.js:30`) and `pageNativeModule ??=` (`glue.js:41`) keep a rejected promise. After one slow or failed first load (the 10 s timeout), every later native view and every `native.later` fails until the page reloads.

**Fix:**
- Register announcements independently of long calls: after first paint, or at the first `native.watch`.
- Queue and deduplicate web announcements, and drain them at the same boundary native hosts use.
- Clear both caches when a load is rejected.

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Astra max (code and design), Opus 5.5 max. Verification: confirmed by reading.
