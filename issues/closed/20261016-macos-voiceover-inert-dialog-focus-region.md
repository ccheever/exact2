# macOS: VoiceOver reaches an inert page after a menu closes, a dialog's announcement cuts off its focused field, and region has no landmark

**Status:** Closed
**Resolution:** fixed: inert nodes are left out of the served AX children, the focus is given back and VoiceOver sent to it after a dialog or menu is said, a named region is AXLandmarkRegion (AccessibilityClientMacTests)
**Systems:** host/apple macOS, accessibility
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1115.000, LLP 1080.003

Found by the T3 Code clone's real-input session of 2026-10-16 (session F, rows 8, 10 and 13, with VoiceOver) and its README's Fails list, and confirmed here by reading what another accessibility client is served and told (`AXUIElementCopyAttributeValue` from a thread of its own, an `AXObserver`):

1. After Escape closed a painted `role="menu"` in a popover over Settings, the keyboard focus was back on the trigger but VoiceOver landed on a thread row of the page under Settings. That page is `inert`. Cause: the host hides an inert node with `setAccessibilityHidden`, which AppKit does not honour out of process: the inert page's button was in `AXChildren` (a browser leaves an inert subtree out of its tree). Also, `MenuHost.close` gave the focus back only after the accessibility pass had said `AXMenuClosed` and posted the modal's layout change with no element, while the window held the focus.
2. A dialog drawn in the page whose field takes the focus as it opens: VoiceOver said "Rename project, dialog" and never "Project title, edit text". The host posted the focus change and the layout change before the assertive announcement, which cut the field off. AppKit tells a sheet first (`AXSheetCreated`), then the field's focus (measured). In a popover (the font picker), the host sent VoiceOver to the dialog and only then focused its field, so VoiceOver stayed on "Interface font family, dialog".
3. `role="region"` with a name had no mapping; Core-AAM and Chromium serve `AXGroup`, subrole `AXLandmarkRegion`, read as "region".

Fix (host): a node the host hid from accessibility is left out of the children it serves (`NSView.exposed`); `MenuHost.close` gives the focus back before the accessibility pass; the pass says the dialog before autofocus moves the focus, and sends VoiceOver to the focus last: the changed modal's one layout change names the focus (in the modal, or given back as the modal goes), then `AXFocusedUIElementChanged`; a popover takes its autofocus before that pass; a named `region` is `AXGroup`/`AXLandmarkRegion` with the role description "region". Tests: `AccessibilityClientMacTests`.

Not verified here: what VoiceOver speaks. That needs a person with VoiceOver (session F's rows 8, 10 and 13 again).
