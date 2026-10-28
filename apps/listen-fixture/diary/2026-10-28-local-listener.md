surfaces: macos, linux, web
agent: Claude Code, Claude Opus 5.5
task: build a small app that listens on a local socket and answers a command-line tool

## Rough

- Nothing in exact2 could accept a connection, so the listener was built
  for this app (LLP 1133.005): `exactListen`, the `net.listen` grant, the
  listener module (`host/listen`) and its executor glue.
- `prelude.js` and `rt.js`, and `host/apple/build.mjs`, sat at the
  1,500-line cap: the socket handles moved into `js/src/binary.js`, and the
  build's changes had to fit the lines already there.
- A fixture copied from another TypeScript app needs `deploy.store` levels
  in `app.json`, or its Apple crate fails to compile against an update
  crate it does not depend on (`exact_apple_update` in `linked.rs`); the
  error names the generated file, not the manifest.
- Journal lines are stamped with the time, so a test matches their ends.

## Lean in

- The two-way socket's rules (LLP 1016.000 D7) carried over to connections
  whole: the mapper answers its own line, a mutation a kept handle.

## Needed

- local socket server: by hand (built here as `exactListen`)
- window activation: provided (`focus()`)

## Checkpoints
