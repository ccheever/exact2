# Removing the Caltrain deck lab dropped LLP 1020's live iframe smoke

**Status:** Closed
**Resolution:** The cross-host smoke again drives iframe load, message, guest outline, pixels, input, and return navigation.
**Systems:** Web host, Apple host, Caltrain, Agent API
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1020 §7 M1/M2, uncommitted working tree

The working tree deletes `apps/caltrain/deck/index.html` and the `open-deck` / `deck-frame` UI, and `scripts/smoke.mjs` no longer drives load / message / guest outline / guest-blue pixels / guest tap.

Unit tests moved to `contract/corpus/iframe.contract`, which still authors `src="/deck/index.html"`. Those tests only check the batch JSON; they never load a guest. `host/web/build.mjs` still copies `app.dir/deck` when present. The live guest path (WKWebView snapshot composition, same-origin outline, script tap `isTrusted:false`) has no smoke.

QUEUE still says “the LLP 1020 deck step now does” guard `EXACT_APP_DIR`. That step is gone.

Fix: keep a host-agnostic fixture (corpus HTML next to `iframe.contract`, served by the agent carrier and copied into the Apple bundle) and restore the smoke steps, guarded so a driven app without an iframe skips them. Or declare the live iframe oracle as Weird Castle's to hold, and delete the `/deck/index.html` src from the corpus.
