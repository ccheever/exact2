# The root lockfile pins a private Snapback repository every build resolves

**Status:** Open
**Systems:** Cargo workspace, Messages, snapback4, app builds outside the repo
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1036.001; rules/NOT-DOING.md (2026-09-23, Messages taken off as the Snapback4 consumer); issues/closed/20260913-snapback4-native-module-for-exact2-hosts.md

Seth's Crew port (report of 2026-09-24, D2) could not build offline: `cargo metadata --locked --offline` failed in the exact2 root because `Cargo.lock` pins `https://github.com/expo/snapback.git`, which is private, and `cargo fetch` over https says "repository not found". Crew never uses Snapback. The workaround was a `GIT_CONFIG_*` rewrite to SSH plus `CARGO_NET_GIT_FETCH_WITH_CLI=true` on every call.

Root `Cargo.toml` declares `snapback4-{core,device,lang}` as git dependencies. Only `snapback4/` and `apps/messages/{apple,linux,web}` use them; no core crate does, so an external app's own graph never contains them. What fails is every cargo command the scripts run in the exact2 root with `--locked --offline` (`scripts/app.mjs` `cargoReproducibilityFlags`): the web build's `exact-markdown-editor` and `exact-textflow` builds (`host/web/build.mjs`), the dev server (`host/web/dev.mjs`), deploy (`scripts/deploy.mjs`), and `cargo fetch`. Cargo resolves the whole root lockfile regardless of which package is asked for.

On 2026-09-23 NOT-DOING took Messages off as the Snapback4 consumer; Interview is. Interview (outside the repo) depends on `exact2/snapback4` by path and pins the Snapback repository in its own lockfile, so the root lockfile carries a private dependency for a consumer that is no longer on the list.

Fix without a cargo feature on a core crate (CLAUDE.md): give `snapback4/` its own workspace outside the root members, as `game/` already is, so its lockfile carries the pin and the root's does not. Then Messages' Snapback client (`apps/messages/{bake.rs,native.rs,snapback-*.ts}`, `apps/messages/apple/src/snapback_tests.rs`, …) either goes (the ruling implies it) or moves into that workspace. Charlie's call: remove or move. Either is removal of apparatus from the root.

Done when `cargo metadata --locked --offline` succeeds in a fresh clone's root with no GitHub credentials, Interview still builds against `snapback4/` by path, and `Cargo.lock` at the root names no git source.
