# Exact Documentation

Write an app once. It runs natively on the web, macOS, iOS, and Linux.

## Set up Exact

Install the toolchain once, then make an app and start its dev loop.

```sh
bun scripts/exact.mjs setup         # once: the pinned toolchains
bun scripts/exact.mjs new ../hello  # a new app
cd ../hello
bun exact.mjs web                   # the dev loop, at http://127.0.0.1:8765/
```
