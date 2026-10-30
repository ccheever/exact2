# A drive of a bundle that `EXACT_MAC_BIN` names serves no bundled page

**Status:** Open
**Systems:** Agent driver (`scripts/agent-stdio.mjs`), Apple host (WebArm)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the T3 Code clone's files preview lane
**Date:** 2026-10-30

**Problem.** A drive names a bundle with `EXACT_MAC_BIN` from a shell that does not export the `CARGO_TARGET_DIR` the bundle was built with. Then the app serves no bundled page (LLP 1020.001): each such frame is an empty document, with no paint and no `postMessage`, and the WebView node shows `loading=false` and `guest: []`. With the variable exported, the same bundle and ops serve every page. No op reply and no `logs` line says why.

**Cause.** The driver sets `EXACT_ASSETS` to the build capture under its own target directory (`scripts/agent-stdio.mjs:52`, `capture` from `host/apple/build.mjs:188`). It does this even when `EXACT_MAC_BIN` names a binary from somewhere else (`agent-stdio.mjs:26`). WebArm reads a scheme-less `src` as a file under `EXACT_ASSETS`, which takes the place of the bundle's resources (`host/apple/webarm/WebArm.swift:995`). The capture of another target directory has no such file, so `localBytes` returns nil, and `local` is nil (`WebArm.swift:783`). The page is then neither local nor remote, so on macOS it is loaded directly (`WebArm.swift:805`) as a 200 response with an empty body (`WebArm.swift:831`). This path writes nothing to stderr, so the host lines that `logs` returns (`agent-stdio.mjs:75`, `scripts/agent.mjs:814`) have nothing to show.

**Reproduction.** Build an app with a framed page under a different `CARGO_TARGET_DIR`, then run `EXACT_MAC_BIN=<bundle>/Contents/MacOS/<name> bun scripts/agent.mjs macos tree` without that variable: the page's frame is empty.

**Fix.** When `EXACT_MAC_BIN` names a binary inside an `.app`, set `EXACT_ASSETS` to that bundle's `Contents/Resources`. When the capture the driver would use is missing, refuse, as the driver already refuses a stale build. When a scheme-less `src` names no file under the asset root, WebArm writes a line to stderr, as its asset scheme's refusal does (`WebArm.swift:179`), instead of serving an empty document.

**Acceptance.** The reproduction serves the page. A drive of a page whose file is missing shows a line in `logs` that names the file.
