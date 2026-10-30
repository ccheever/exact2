# A drive of a bundle that `EXACT_MAC_BIN` names serves no bundled page

**Status:** Open
**Systems:** Agent driver (`scripts/agent-stdio.mjs`), Apple host (asset root, WebArm)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's files preview lane
**Date:** 2026-10-30

**Problem.** A drive names a bundle with `EXACT_MAC_BIN` from a shell that does not export the `CARGO_TARGET_DIR` the bundle was built with. Then every bundled page (LLP 1020.001) is empty: no paint, no `postMessage`, and the WebView node shows `loading=false` and `guest: []`. With the variable exported, the same bundle and ops show every page. No op reply and no `logs` line says why.

**Cause.** The driver sets `EXACT_ASSETS` to the build capture under its own target directory (`scripts/agent-stdio.mjs:52`, `artifacts.capture` from `host/apple/build.mjs:188`). It does this even when `EXACT_MAC_BIN` names a binary from somewhere else (`agent-stdio.mjs:26`). The session takes `EXACT_ASSETS` over the bundle's `Contents/Resources` (`host/apple/Sources/ExactKit/Session.swift:134`), so the asset scheme finds no page and answers 404 (`host/apple/webarm/WebArm.swift:184`). Its line "… is not an app asset under …" (`WebArm.swift:179`) goes only to the app's stderr, which the driver keeps as host lines (`agent-stdio.mjs:75`).

**Reproduction.** Build an app with a framed page under a different `CARGO_TARGET_DIR`, then run `EXACT_MAC_BIN=<bundle>/Contents/MacOS/<name> bun scripts/agent.mjs macos tree` without that variable: the page's frame is empty.

**Fix.** When `EXACT_MAC_BIN` names a binary inside an `.app`, set `EXACT_ASSETS` to that bundle's `Contents/Resources`. When the capture the driver would use is missing, refuse, as the driver already refuses a stale build. Report a refused bundled page on the WebView node in `tree`, or as a `logs` line.

**Acceptance.** The reproduction shows the page. A page that answers 404 under a drive appears in `logs`.
