# Manifest: tsconfig `paths` replace `typescript.aliases`, outside-app resolution is `--locked`, and the dark launch colour is `background_color_dark`

**Status:** Open
**Systems:** `scripts/app.mjs`, `scripts/app.schema.json`, `js/bake`, `host/apple/build.mjs`, apps' `app.json`
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1036.001 and LLP 1030 (the 2026-09-27 ruling), LLP 1069.008 (permissions)

**Do:**
- Read TypeScript import aliases from the app's `tsconfig.json` `compilerOptions.paths` (and `baseUrl`), and delete `typescript.aliases` from the schema and from every app that uses it (weird-castle and grnl are outside this repo: note what they need to change).
- Run `resolveApp`'s Cargo resolution with `--locked` (`scripts/app.mjs:174`, `:348`). A lockfile that is out of date is an error naming the update command, never a silent rewrite.
- Replace `launch.background`/`launch.backgroundDark` with the manifest's `background_color` and a new `background_color_dark`, in the web manifest's snake_case; update the schema, `host/apple/build.mjs` and the apps.
- Permissions are LLP 1069.008's (delete `host.*.permissions`); don't duplicate that work here.

From Charlie's rulings of 2026-09-27 on the review of Seth's PR #47.
