# Content-region layout still uses the quadratic line breaker

**Status:** Open
**Systems:** Apple host, Text
**Severity:** P2
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1044

`RegionWorkerLayout` says CoreText's word-break iterator rescans the whole prefix on every line of a giant paragraph, and `suggestBreak` exists to avoid that. That path runs only when `compact` is set. Otherwise each line calls `CTTypesetterSuggestLineBreak`.

`RegionShapeRequest.compact` defaults to false. The live region controller submits shapes without setting it (`RegionController.swift`). The markdown reader's separate path does set `compact: true` (`Mac/RegionReaderMac.swift`). `apps/markdown-stress` turns on `contentRegion` for its 1 MiB and 4 MiB paragraphs, so those go through the controller and the quadratic breaker.

Use the indexed break for the controller's shapes, the same way the reader already does. Done when a content-region paragraph of the stress app's size breaks lines without rescanning the prefix per line, and the reader's pixels stay the same.
