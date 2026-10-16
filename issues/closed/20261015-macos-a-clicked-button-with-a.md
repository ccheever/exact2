# macOS: a clicked button with a tabindex takes the focus

**Status:** Closed
**Resolution:** fixed: on the Mac a click focuses no button, radio or toggle whatever its tabindex (custom and native); a tabindex box still takes the click's focus
**Systems:** host/apple macOS, focus, pointer
**Author:** Daehyeon Mun
**Date:** 2026-10-15
**Severity:** P2
**Related:** LLP 1115.000 D7.1

On macOS, a click on a `button` that has an explicit `tabindex` (for example a dialog's full-window backdrop written `button tabindex=-1 press=close`) makes that button the first responder. A clicked `NSButton` never takes the focus, and the host's own comment says a click on a button never does. A dialog that closes from such a backdrop then gives the focus back to nothing: the window becomes the first responder and VoiceOver reports the window as focused.

Cause: `NodeView.takesClickFocus` (`host/apple/Sources/ExactKit/Mac/FocusMac.swift:86-88`) is `acceptsFirstResponder && !retainsFocus && (explicitTabIndex != nil || takesKeysOfItsOwn)`. `takesKeysOfItsOwn` excludes buttons, but the `explicitTabIndex != nil` branch does not. `NodeViewMac.mouseDown` (`NodeViewMac.swift:1399`) then calls `makeFirstResponder(self)`.

Reproduction: a field with the focus, plus `button tabindex=-1 press=close` covering the window behind a box; click the button and read `state focus`. The responder is the button, not the field.

Fix: leave buttons and toggles out of the explicit-tabindex branch (`explicitTabIndex != nil && !isButton && kind != "toggle"`), so a clicked button keeps the focus where it was whatever its `tabindex`, as D7.1 says for buttons. Boxes with a `tabindex` still take it.

Acceptance: in the reproduction the field keeps the focus after the click and the press still runs; a `tabindex` box that is not a button still takes the focus on a click; `retainFocus` is no longer needed on such buttons.
