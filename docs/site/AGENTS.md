<!-- exact:begin (exact new writes this block; bun exact.mjs update rewrites it) -->
# site: an Exact app

`app.contract` is the view, `app.ts` its data module, `app.test.contract` its tests, `app.json`
the manifest: set its `app.name` and `app.id` before the first build. The app uses the exact2
checkout at `../..` by path (`EXACT2` overrides it).

This file is already in your instructions. Start from the nearest pattern: `bun exact.mjs
pattern` lists them and `pattern <name>` writes one, tested. ../start-here-site.md (a site) or
../start-here.md is the only required reading. `bun exact.mjs contract vocab <name>` answers a
name (none: the card); `bun exact.mjs docs <heading words>` a guide's
section. Run `bun exact.mjs verify` when the features work, and `verify ios|android` at the end for a
phone app. `bun exact.mjs help` lists the verbs, and your own in app.json `"commands"`.
`bun exact.mjs update` refreshes files. A native verb (`ios`, `mac`, `linux`, `android`,
`windows`) names its platform in app.json the first time it runs and builds it in a generated
workspace; `update --without <platform>` stops it.
`bun ../../scripts/exact.mjs setup --check` names what a native build lacks.
Shared data: `exact new <path> --snapback`, or `bun exact.mjs snapback` here.
Install `snapback4@0.4.33` above; ../../snapback4/README.md has setup and server commands.

Generated, so don't edit: `exact.mjs` and this block.

## Feedback (required)

When a doc was wrong or missing, a workaround was needed, something took several tries or a
command confused you, record it: `bun exact.mjs feedback note "<what happened>"`. It attaches
exact2's commit and the last exit code, never output or code. `bun exact.mjs feedback status` says
whether notes are sent and whether to keep a diary; under `never`, skip all of this.
<!-- exact:end -->

## Search

The search dialog (Cmd+K) asks the Algolia app and index in `constants/search.ts` with its
search-only key. After a build, `bun run search:index` makes one record per section of each
page and replaces the index; it needs the app's Write API key, from `ALGOLIA_WRITE_KEY` or the macOS
Keychain item `exact-docs-algolia-write`, and the key never goes in the repository. `--dry-run` prints the record count and the first records.
