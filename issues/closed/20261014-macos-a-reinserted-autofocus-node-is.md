# macOS: a reinserted autofocus node is not focused

**Status:** Closed
**Resolution:** fixed by macOS release removing the node's record from autofocusProcessed, as iOS does; FocusFixupMacTests
**Systems:** host/apple macOS, focus
**Author:** Daehyeon Mun
**Date:** 2026-10-14
**Severity:** P2
**Related:** issues/closed/20261006-js-autofocus-only-at-boot.md; LLP 1035.000 D9

On macOS, a popup that holds an `autofocus` row focuses the row the first time it is inserted. When the popup is removed and inserted again, the new row does not get the focus. In Chrome, an `autofocus` element that is inserted takes the focus each time. Found while building an example app.

Likely cause (read in the source, not traced at runtime): the presenter records a processed autofocus node by `ObjectIdentifier(view)` in `autofocusProcessed`. iOS removes the record when a node is released (`host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:1052`). macOS `release` (`host/apple/Sources/ExactKit/Mac/PresenterMac.swift:795-822`) does not. macOS prunes the set only on a full accessibility pass (`host/apple/Sources/ExactKit/Accessibility.swift:459`), not on the incremental pass a batch runs. A new view that gets the same identifier (a reused view, or a new allocation at the address of the released one) is then skipped by the guard at `Accessibility.swift:511`.

Reproduction: a button toggles `when open` around a `column` that holds `button autofocus`. Open, close, and open again; on the second open the focus does not move to the autofocus button on macOS.

Fix: in macOS `release`, remove `ObjectIdentifier(view)` from `autofocusProcessed` before the view leaves the map, as iOS does.

Acceptance: in the reproduction, the autofocus button takes the focus on every open on macOS, as in Chrome.
