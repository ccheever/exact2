# Apps made by exact new: web-build builds the retiring wasm target, and there is no verb to test or drive the app

**Status:** Closed
**Resolution:** Fixed by 28d617918.
**Systems:** game/new.mjs (exact new), web build, agent
**Severity:** P3
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-02
**Related:** game/new.mjs commandsFor, host/web/build.mjs, LLP 1071

The `exact.mjs` that `exact new` writes (`game/new.mjs`, `commandsFor`) has these verbs:
`web`, `web-build`, `ios`, `mac` and `update`. Two things don't fit the rest of the
toolchain:

1. **`web-build` passes `--wasm`.** `host/web/build.mjs` calls `--wasm` internal: the
   web build is the JS target (LLP 1071), and the wasm target remains for games, delivery
   bakes, conformance's oracle and native clients on the dev URL. So
   `bun exact.mjs web-build` makes a different artifact than the one the dev loop
   (`bun exact.mjs web`), the README and `agent.mjs web` work with. It also needs the
   pinned nightly, which an ordinary app otherwise doesn't need. The verb should build
   the JS target, as `bun host/web/build.mjs <name>-web` does.
2. **There's no way to test or drive the app from its own directory.** Running its
   `app.test.contract` takes, from the exact2 checkout:
   `EXACT_APP_DIR=../todo bun scripts/agent.mjs <host> --app todo --test ../todo/app.test.contract`,
   plus `EXACT_SIM` for iOS. Since the agent loop is how an app made with `exact new` gets
   verified, a `test <host>` verb (and an `agent <host> …` passthrough) would cut that to
   `bun exact.mjs test web`. `exact.mjs` already knows the app directory and the checkout.

Found when a fresh Claude Code session followed the README to build a todo app,
2026-10-02. It used the README's command instead of `web-build`, and drove the tests from
the exact2 checkout by hand.
