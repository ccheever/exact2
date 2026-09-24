# An iOS TypeScript app needs Hermes provisioned by hand from an RFC paragraph

**Status:** Open
**Systems:** Apple host build, exact-js, Hermes provisioning
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1027 D6 (iOS execution); LLP 1036.001; js/build.rs

Seth's Crew port (report of 2026-09-24, D3) spent about 30 minutes before its first TypeScript app built for iOS:

- a sibling `ibex` checkout (`js/build.rs` looks for `../ibex` beside the exact2 checkout), mentioned nowhere near `host/apple/build.mjs` or in CLAUDE.md's build lines;
- `./scripts/build-hermes.sh --vanilla` in ibex, about 20 minutes, which builds full frameworks for device, simulator and macOS plus the host compiler, although exact2 needs only the headers, the macOS VM and `hermesc` from it (`js/build.rs`, `ibex/ios/Frameworks-vanilla`);
- a hand-run CMake lean build per iOS platform, copied out of LLP 1027's "iOS execution" paragraph into `target/hermes-ios/{ios,ios-simulator}`.

The refusal in `js/build.rs` names the pieces, but there is no command that does them. Two further costs:

- The lean archives default to `<checkout>/target/hermes-ios`, so every new exact2 worktree provisions again. `EXACT_HERMES_IOS_DIR` does not rescue it: pointed at another checkout's `target/hermes-ios`, the build refuses with "compiler input has no captured source identity" (seen 2026-09-24 reproducing the port's F4/F6), so the archives were copied into the new worktree's `target/` by hand.
- The pin disagrees with itself. The `js/build.rs` header pins facebook/hermes `6badada7…`, as does the local ibex receipt (`ios/Frameworks-vanilla/hermes-input-receipt.json`, ibex `639de62d`); LLP 1027's recipe says `e3371863…` "(updated 2026-09-19), matching the sibling ibex vanilla headers/compiler receipt". Following the RFC by hand can mix headers, compiler and VM from two commits.

Fix, without a new script: when `host/apple/build.mjs --ios` builds an app with an `app.ts` and the target platform's three archives are missing, build only that platform's three lean targets (`hermesvmlean_a jsi boost_context`) from ibex's pristine source cache (`~/.cache/exact/hermes/hermes-src`) with ibex's `ImportHostCompilers.cmake`, into a location shared across worktrees (a per-user cache keyed by the Hermes commit, not `target/`), and say so on one line. Reconcile the pin first: one commit, named in `js/build.rs` and LLP 1027, checked against the ibex receipt at build time with a refusal naming both when they differ.

Done when a fresh worktree with a provisioned ibex runs `bun host/apple/build.mjs --ios --run` for a TypeScript app without any manual Hermes step, a second worktree reuses the archives, and a pin mismatch between ibex and `js/build.rs` is a named refusal.
