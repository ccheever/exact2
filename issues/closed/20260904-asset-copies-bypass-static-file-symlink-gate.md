# Live and Apple asset copies bypass the static-file symlink gate

**Status:** Closed
**Resolution:** Web/live/Apple static trees share batched handle-relative capture, including optional roots; rejected links and validators preserve last-good output, with concurrent replacement and Apple-copy regressions.
**Systems:** Dev loop, Web host, Apple build, Security
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1023 D3; LLP 1030 D10

The production web bake has one explicit static-file rule: any symlink under
assets, deck, or shaders aborts the build (`host/web/build.mjs:46-64`). The
live asset path does not reuse it. On a watch event, `statSync(source)` follows
a symlink and `copyFileSync(source, target)` copies its target into the
LAN-served `dist` (`host/web/dev.mjs:142-153`). A symlink created after startup
can thus publish any readable local file under an app URL even though the next
full build would refuse it.

The same path mutates live output before validation. A `.wgsl` file is copied
at line 151 and reflected only afterward; if reflection fails, the server emits
an error but continues serving the invalid bytes. A fresh page can observe a
candidate the last-good loop said it rejected. Existence/stat/copy races can
also throw out of the timer callback and end the dev server.

Apple packaging uses unfiltered recursive `cpSync` for the same trees
(`host/apple/build.mjs:291,403-407`). Node preserves such symlinks in the
artifact rather than refusing them, so native and web bakes disagree about
what an app's static tree may contain.

Done when one shared static-tree policy rejects symlinks on every bake and
live path. A live edit is read and validated in a candidate location, then
atomically replaces the served file only on success; failure preserves the
last good bytes and keeps the process alive. Test a symlink introduced during
a session, invalid shader text, deletion/read races, and both Apple outputs.
