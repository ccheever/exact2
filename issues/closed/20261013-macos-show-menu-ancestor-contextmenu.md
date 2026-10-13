# [Bug] macOS: VoiceOver's show menu ignores a contextmenu on an ancestor

**Status:** Closed
**Resolution:** fixed by #526
**Systems:** host/apple macOS, accessibility
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-13
**Related:** `issues/closed/20261012-macos-voiceover-show-menu.md` (X84, #486); `issues/closed/20261010-macos-inline-run-contextmenu.md` (X77); T3 Code clone realinput-1011c RLR-6v and G4-06 (https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/STATUS.md)

## Summary

VoiceOver's show menu (VO-⇧-M, the `AXShowMenu` action) runs a node's `contextmenu` only when the element VoiceOver
is on has the handler itself. #486 (X84) made `NodeView.accessibilityPerformShowMenu` and an inline run's element
(`InlineAccessibility`) run their own `contextmenu` or `contextPopover`, and offer `AXShowMenu` only then
(`Accessibility.swift:326-339`, `TextInteraction.swift:225-227`). Where the handler is on an ancestor, the element
offers `AXPress` alone and a show-menu request is refused:

- a button inside a row whose `column` has the menu (a sidebar's thread row);
- an image inside a focusable box that has the menu (a chat's media);
- a link run inside a paragraph that has the menu, or inside a span run that has it.

The ContextMenu key at the same focus already walks up: `contextMenuKey` (`MouseEventsMac.swift:24`) goes from the
focused node through `menus.parent` to the first node whose `contextmenu` or `contextPopover` answers. It misses one
case too: from a focused link run it tries the run's own handler and then the paragraph, never the runs between, so a
span run's handler around the link is not reached.

In Chrome both requests take one path. Blink's `AXObject::OnNativeShowContextMenuAction` and the ContextMenu key call
`EventHandler::ShowNonLocatedContextMenu`, which fires `contextmenu` at the element (its bounds' middle), and the event
bubbles to the first handler. Chrome lists `AXShowMenu` on every web content node (`SupportsShowMenuAction`).

## Why this arose

The T3 Code clone puts a thread row's menu on the row's `column` (`contextmenu=menu(t.id) contextPopover="thread-menu"`)
around the row's button, and a chat image's menu on a focusable box around the image. Under VoiceOver
(realinput-1011c) VO-⇧-M opened nothing on a thread row (RLR-6v), and the chat media offered no `AXShowMenu` (G4-06),
after the clone adopted #486. RLR-6v also reported nothing on a reply link, whose `link` node has its own handler.

## Reproduction

Main `beaee003d82233cc11d31d516598f3b33fc7a803`, macOS 26.6.2 (25G83), Apple Silicon, Google Chrome 155.0.8059.40. A
one-file app made with `bun scripts/exact.mjs new <dir>` (`app.ts` exports an empty `sources`): a `column` with
`contextmenu` around a button (`row-open`); a `link` with its own `contextmenu`, an `aria-label`, an icon and a text
child (`reply-link`, the clone's FlowRuns link); a `box tabindex=0` with `contextmenu` around an image (`media-image`);
a paragraph with `contextmenu` and a link run without (`paragraph-link`); a span run with `contextmenu` around a link
run without (`span-link`); a plain button; and a text with five counts.

VoiceOver was not turned on and no real input was used. The session was locked; while it is, an `AXUIElement` read
from another process stops at the application element (X84). Scenario C loads a probe dylib into the built binary
(`DYLD_INSERT_LIBRARIES`) and calls the functions AppKit's accessibility server calls for an assistive app's requests:
`NSAccessibilityEntryPointActionNames` and `NSAccessibilityEntryPointPerformAction(element, "AXShowMenu")`, the
handler of `AXUIElementPerformAction`, which refuses an action the element does not offer.

| Scenario | Platform | Actual | Expected | Evidence |
|---|---|---|---|---|
| A: agent, the ContextMenu key at `row-open`, `media`, `paragraph-link`, `span-link`, `plain` | macOS | row, media and paragraph menus run; the span's does not (`span 0`); nothing for `plain` | span 1 | record A |
| B: the same steps on the web | Chrome (web agent) | row, paragraph, span and media menus run; nothing for `plain` | (the oracle) | record B |
| C: `AXShowMenu` through AppKit's server on each element | macOS, in-process probe | `row-open`, `media-image`, `paragraph-link`, `span-link`: actions `[AXPress]`, show menu refused, counts unchanged; `reply-link` and `span`: `[AXPress, AXShowMenu]`, performed (`link 1`, `span 1`); `plain` `[AXPress]`, refused | each of the four offers `AXShowMenu` and runs its ancestor's handler; `plain` still offers none | record C |

Record (the contract, the commands and their output, the code read):
https://raw.githubusercontent.com/ccheever/exact2/a7ed12773ca1d5d55982a5beae310451be1fb7b7/fw/macos-show-menu-ancestor/macos-show-menu-ancestor-before.txt.
The probe's source:
https://raw.githubusercontent.com/ccheever/exact2/04bb615be2f912bc3f8ec933f3396b3c1830834e/fw/macos-show-menu-ancestor/showmenu-probe.m.txt.

The reply link with its own handler works through the server (C): one `AXLink` with no children, and its show menu
runs `linkMenu`. What VoiceOver did differently on the clone's link was not reproduced here.

Not run: VoiceOver itself and `AXShowMenu` from another process (the lock); Chrome's show menu (read from source; its
ContextMenu key, the same Blink function, is B); iOS, Linux, Windows.

## Constraints

- Show menu keeps the ContextMenu key's path (X84): from the element VoiceOver is on, up through `menus.parent` to the
  first node whose `contextmenu` or `contextPopover` answers, at the element's drawn middle, as `contextMenuKey` walks.
  For an inline run: the run, then its ancestor runs (as a secondary click's `inlineContextTarget` walks, X77), then
  the paragraph and up. The ContextMenu key on a focused run takes the same walk, which reaches a span run's handler.
- `AXShowMenu` is offered where that walk finds a handler that runs (not disabled or inert), and nowhere else, so an
  element with no menu above it still offers `AXPress` alone. A field keeps its own menu.
- Right-click, Control-click, `preventDefault` and a node's own show menu are unchanged.

## Acceptance criteria

- In the repro, `AXShowMenu` on `row-open`, `media-image`, `paragraph-link` and `span-link` is offered and runs the
  row's, the media box's, the paragraph's and the span's handler once each; `plain` offers none.
- The ContextMenu key on `span-link` runs the span's handler, as on the web.
- An AppKit test (`ContextMenuMacTests`) performs show menu on a button inside a wrapper with the handler and on a link
  run inside a span run and inside a paragraph with one, and sees each ancestor's handler run at the element's middle.
