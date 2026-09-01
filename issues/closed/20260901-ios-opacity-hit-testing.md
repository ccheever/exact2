# Zero opacity disables iOS hit testing

**Status:** Closed
**Resolution:** UIKit hit testing now follows CSS opacity semantics and keeps zero-opacity nodes interactive.
**Systems:** Apple host
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1008

iOS maps the style row directly to `UIView.alpha`
(`host/apple/ios/Sources/ExactIOS/Presenter.swift`). UIKit excludes views near
alpha zero from hit testing, whereas CSS `opacity: 0` elements remain pointer
targets. The presenter already has a `PlainView` workaround acknowledging the
UIKit behavior, but ordinary node views retain the default.

An opacity transition can therefore silently disable a control before its
declared event/pointer behavior says so.

Override hit testing so opacity alone does not change participation; visibility
or pointer policy should own that decision. Add static-zero and animated-zero
tap tests against web and iOS.
