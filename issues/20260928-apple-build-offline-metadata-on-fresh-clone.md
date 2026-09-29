# `host/apple/build.mjs` fails on a fresh clone until `cargo fetch`

**Status:** Fixed (this PR)
**Systems:** build (`scripts/app.mjs`)
**Severity:** P3
**Author:** Claude Fable 5.1, building Ocho for Eliot Hertenstein
**Date:** 2026-09-28

On a clone that has never built, `bun host/apple/build.mjs <app>-apple` stops at
`cargo metadata --locked --offline`: the git dependency `snapback4-device`
(`expo/snapback` at `a397218e`) is not in the local registry yet, and
`--offline` refuses to fetch it. The message names the flag but not the cure.
`cargo fetch` once, then the build runs.

`buildCommand` already fetches the locked sources once and retries when
Cargo says `--offline was specified` or `attempting to make an HTTP request`
(LLP 1054 O2). A git source that was never checked out fails with a third
wording, `can't checkout from '…': you are in the offline mode (--offline)`,
which the retry did not match. It does now.
