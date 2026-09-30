# The JS target's agent reports no navigation URL: its copy of navigation.js has its own state

**Status:** Closed
**Resolution:** the agent observes the page's own navigation.js instance, which rt.js router() keeps (pageHistory); the copy stays for what is stateless; the router sweep passes and the web smoke is green
**Systems:** web JS target, agent API
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-30
**Related:** 419f027a, LLP 1038 D11, LLP 1035.002 D2

`bun scripts/smoke.mjs web` fails at `ef7dfedb` (origin/main, 2026-09-30) in its router sweep, `navigation::browser_session_history_on_the_js_target` (`host/web/tests/it/navigation.rs`): 11 cases, each `null !== '/…'`, because `state.navigation.url` is `null` on every JS-target page. It fails the same with every file of `90d967b3` set back to its `ef7dfedb` version, so it is not that change.

Cause: `419f027a` ("the agent adapter reads its own copies of navigation.js and names.js") has `host/web-js/build.mjs` copy `navigation.js` to `agent-navigation.js` and point `agent.js` at the copy. A copy is a second module instance with its own module state. `navigation.observation()` answers `url: last?.url ?? null` (`host/web/navigation.js:194`), and `last` is module-level (`:3`), written only by the page's instance. The agent's instance never sees a navigation, so its `url` is always `null`. The same holds for any other state the agent reads through the copy (`written`, `cursor`, `echo`, `pop`, `root`…); `names.js` is data, so copying it is safe.

Fix, one of:
- keep the copy for `names.js` only, and import the page's `navigation.js` again (gives back part of the ~1.2 KB the commit saved on each non-production entry);
- or keep the copy, but have `observation()` (and any other agent read) take its state from the page's instance, for example state published on `globalThis.exact` by the page's `navigation.js`.

Verify with `cargo test -p exact-web --test it navigation::browser_session_history_on_the_js_target -- --ignored` and `bun scripts/smoke.mjs web`.
