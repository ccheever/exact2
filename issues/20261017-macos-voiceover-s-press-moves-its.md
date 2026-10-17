# macOS: VoiceOver's press moves its cursor to an element that appears during the press

**Status:** Open
**Systems:** host/apple macOS, accessibility, VoiceOver
**Severity:** P3
**Author:** Daehyeon Mun
**Date:** 2026-10-17
**Related:** LLP 1021 (press timing), issues/closed/20261013-macos-axpress-on-a-button-whose.md, issues/closed/20261016-macos-voiceover-landing-before-focus.md

When VoiceOver presses an element (VO-Space) and the press's commit makes a new element appear, VoiceOver moves its cursor to the new element, and the keyboard focus follows it. A hand-built AppKit app leaves VoiceOver on the pressed element, or on the focus the press moved. A pointer press, or an AXPress from another client, moves nothing, and the same press made while the element is already showing keeps the cursor where it was.

Seen in the T3 Code clone: VO-Space on a composer citation chip, from the end of the thread, scrolls the timeline to the cited text; the 'Scroll to end' button appears as the timeline leaves the end, and VoiceOver goes to it. The app posts nothing that names it.

Likely cause: the host tells VoiceOver nothing after an assistive press. The press runs on a later main-queue turn, after AppKit's AXPress request has returned (`NodeView.accessibilityPerformPress`, host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1384), so its commit lands after VoiceOver has its answer. The accessibility pass then posts only for live regions, menus and a changed modal (`syncAccessibility`, host/apple/Sources/ExactKit/Accessibility.swift:540); the landing that names where VoiceOver belongs runs only when the modal changed (`tellLanding`, Accessibility.swift:703, its layout change at :735). Nothing is posted for a node that appears. VoiceOver, still settling its cursor after the press and finding the element tree changed with no element named, likely re-picks its cursor and takes the new element. Not confirmed: whether the pressed element going away matters (the clone's chip elements are rebuilt when the editor refreshes), and which notification VoiceOver acts on.

Reproduction: a `button` whose press shows a second `button` elsewhere; VO-Space on the first, with VoiceOver's "keyboard focus follows VoiceOver cursor" on (the default).

Fix: record an assistive press and, once its commit's commands have run, land VoiceOver as `tellLanding` does: post the layout change naming the pressed element if it is still in the tree, else the focus, so no element that appeared during the press is left as VoiceOver's only choice. Measure a hand-built AppKit window (an `NSButton` whose action unhides another) first, with an `AXObserver` in another process, to post only what AppKit posts.

Acceptance: in the reproduction, after VO-Space VoiceOver stays on the pressed button and the focus does not move to the button that appeared; a pointer press and another client's AXPress behave as today; `AccessibilityClientMacTests` covers the posts.

## Reopened, 2026-10-17

It was closed (#694) as the clone's, and that was wrong. A Contract repro with real VoiceOver does not reproduce it: a pressed button that survives its press keeps VoiceOver, as in a hand-built AppKit window, where AppKit posts only AXValueChanged on the pressed button. The clone was then changed to keep its chip's accessibility elements across refreshes. An accessibility probe shows the pressed link is the same valid element after the press. VoiceOver still moves to 'Scroll to end' (1 of 1), and posting AXLayoutChanged naming the link after the press does not stop it. The cause is not found. What the repro lacks is the clone's shape: the pressed element is an `NSAccessibilityElement` child of a text view, not a NodeView, and the press scrolls a list, which shows the pill. Next: a repro with a custom accessibility element in a text view whose press scrolls a scroller and shows a button.

