# exact new: a folder named exact collides with the SDK's crates, and a failed scaffold removes the author's directory

**Status:** Closed
**Resolution:** fixed by bbd846b41: SDK crate names are refused before writing, and a failed scaffold keeps the author's directory
**Systems:** scripts/exact.mjs new (game/new.mjs)
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

`bun scripts/exact.mjs new exact` scaffolds, then the first build fails: the app's crates are `exact-apple`, `exact-linux` and `exact-web`, the names of the SDK crates they depend on (`host/apple/Cargo.toml:8`, `host/linux/Cargo.toml:11`, `host/web/Cargo.toml:7`); `exact-js` collides with `js/web`'s `exact-js-web`. And a scaffold that fails after `mkdirSync` runs `rmSync(dir, { recursive: true })` (`game/new.mjs:167, 176`), which removes an empty directory the author had made.

## Fix

Refuse a name whose host crates would be an SDK crate's before writing anything, naming the crate; on failure remove only what the scaffold wrote, keeping a directory that existed before.

## Fixed (2026-10-19)

`createApp` refuses a folder whose host crates would be one of the SDK's (`exact`, `exact-js`) before writing anything, naming the crate, and a failed scaffold removes only what it wrote: a directory that existed before is emptied, not removed. `game/new-app.test.mjs` covers both.
