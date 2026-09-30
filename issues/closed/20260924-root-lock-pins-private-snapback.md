# The root lockfile pins a private Snapback repository every build resolves

**Status:** Closed
**Resolution:** Moved Messages and the Snapback adapter into their own Cargo workspace under Charlie’s explicit ruling; root resolution has no private git source and ordinary Messages build/drive commands select the nested workspace.
**Systems:** Cargo workspace, Messages, snapback4, app builds outside the repo
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1036.001; rules/DEFERRED.md (2026-09-23, Messages taken off as the Snapback4 consumer); issues/closed/20260913-snapback4-native-module-for-exact2-hosts.md

Seth's Crew port (report of 2026-09-24, D2) could not build offline: `cargo metadata --locked --offline` failed in the exact2 root because `Cargo.lock` pins `https://github.com/expo/snapback.git`, which is private, and `cargo fetch` over https says "repository not found". Crew never uses Snapback. The workaround was a `GIT_CONFIG_*` rewrite to SSH plus `CARGO_NET_GIT_FETCH_WITH_CLI=true` on every call.

Root `Cargo.toml` declares `snapback4-{core,device,lang}` as git dependencies. Only `snapback4/` and `apps/messages/{apple,linux,web}` use them; no core crate does, so an external app's own graph never contains them. What fails is every cargo command the scripts run in the exact2 root with `--locked --offline` (`scripts/app.mjs` `cargoReproducibilityFlags`): the web build's `exact-markdown-editor` and `exact-textflow` builds (`host/web/build.mjs`), the dev server (`host/web/dev.mjs`), deploy (`scripts/deploy.mjs`), and `cargo fetch`. Cargo resolves the whole root lockfile regardless of which package is asked for.

On 2026-09-23 DEFERRED took Messages off as the Snapback4 consumer; Interview is. Interview (outside the repo) depends on `exact2/snapback4` by path and pins the Snapback repository in its own lockfile, so the root lockfile carries a private dependency for a consumer that is no longer on the list.

Fix without a cargo feature on a core crate (CLAUDE.md): give `snapback4/` its own workspace outside the root members, as `game/` already is, so its lockfile carries the pin and the root's does not. Then Messages' Snapback client (`apps/messages/{bake.rs,native.rs,snapback-*.ts}`, `apps/messages/apple/src/snapback_tests.rs`, …) either goes (the ruling implies it) or moves into that workspace. Charlie's call: remove or move. Either is removal of apparatus from the root.

Done when `cargo metadata --locked --offline` succeeds in a fresh clone's root with no GitHub credentials, Interview still builds against `snapback4/` by path, and `Cargo.lock` at the root names no git source.

## Resolution evidence, 2026-09-30

Charlie chose “Move Messages and Snapback into a separate workspace.” The existing `snapback4/Cargo.toml` now owns its adapter plus the three Messages packages, whose explicit `package.workspace` preserves all source paths. Its own lock holds the private pins; the root excludes all four members and has no git source. The generic app resolver follows member workspace declarations for both named in-repo apps and outside apps, and nested workspaces inherit the checkout's Rust toolchain.

Root `cargo metadata --locked --offline` passed with a fresh Cargo home containing the registry cache only, no git cache or Git configuration/credentials (493 packages, zero git sources). A scratch copy of Interview, redirected to this checkout without changing its own lock, passed locked/offline metadata and still resolved the path adapter. Messages built through `bun host/web/build.mjs messages-web`, launched through the ordinary agent app selection, and opened New Message; the screenshot was inspected. Its separate workspace passed `cargo check --locked --offline --manifest-path snapback4/Cargo.toml -p messages-apple --all-targets`. App resolver tests cover named nested workspaces, inconsistent declarations, and lock refusal; root and nested formatting, caps and boot passed.
