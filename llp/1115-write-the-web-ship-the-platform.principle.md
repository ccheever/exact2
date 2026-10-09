# LLP 1115: Write the web, ship the platform

**Type:** Principle
**Status:** Active (Charlie Cheever, 2026-10-09: "write that up and write an LLP about it")
**Systems:** All hosts (iOS, macOS, tvOS, Linux, Windows, web), the kernel's style defaults, the Contract compiler's lowering, the agent docs and `exact new`
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-09
**Implementer:** Claude (Opus 5.5), waves 1–2 from 2026-10-09 (§6)
**Related:** `rules/RULES.md` §Scope, `AGENTS.md` ("Write the web, ship the platform"), LLP 1095 (platform colours: the first instance of this principle), LLP 1104 D4 (a reset's behaviour as a familiar path), LLP 1075.003 (header-shaped routes), LLP 1021 (menus and confirmations), LLP 1069.011.000 (native buttons), `issues/20261009-agent-port-cost-vs-swiftui.md`, `issues/20261009-ios-confirmation-shape.md`

## Summary

**What the author writes is CSS. What the author leaves unsaid is the platform's. What the
author writes wins.**

Precedence, for every value a host presents: **author > platform > CSS default.** One test
settles a dispute: *would someone who knows the platform notice?* If so it is a **tell**, and
the platform's answer wins. On iPhone the reference is a screen built by hand with UIKit or
SwiftUI following Apple's Human Interface Guidelines, on the same iOS version (James's "no
tells"); on the Mac, AppKit; each platform likewise. Not Chrome.

This replaces "the web is the standard" as the top rule. The web stays what the author writes
(the vocabulary, the layout model) and where the author works (the dev loop); it stops being
what the user sees.

## 1. Why

A user who opens an Exact app on an iPhone is comparing it, without meaning to, with every
other app on that iPhone. A grey that is `#8e8e93` instead of `secondaryLabel`, 16-point body
text beside a 17-point field, a back chevron that is missing until the author remembers to
declare one: each is small, and each says "this was not made here". The 2026-10-08 port
benchmark (`issues/20261009-agent-port-cost-vs-swiftui.md`) found agents hand-painting
headers, tab bars and segmented controls in hex, because the docs taught that and because
unsaid values came out looking like a web page. A SwiftUI port got native for free.

The web standard was right about what it protected: one vocabulary agents already know, and
one layout oracle so four hosts cannot disagree about a box (the predecessor's
four-disagreeing-default-layers bug class). It was wrong to extend that to presentation, where
the browser's defaults are themselves just one platform's.

## 2. The three layers

| Layer | Who decides | Examples |
|---|---|---|
| **Written** | The author, on that node | `color="#B5562B"` on a button; `font-size=20`; `buttonStyle="plain"`; `navigationBack="back"` |
| **Unsaid presentation** | The platform | text colour, tint, body size, heading style, control style, press feedback, separators, back navigation, sheet surfaces, keyboard behaviour, focus rings, menus |
| **Unsaid layout and names** | CSS | `display: block`, `flex-direction: row`, `flex-shrink: 1`, `box-sizing: content-box`; property names (`object-fit`, not `resizeMode`) |

Layout stays CSS because a user cannot perceive a layout default, only its result, and the
result is the author's. Names stay CSS because names are what the author writes. Everything a
user can see or feel that the author did not write is the platform's.

**Inherited is unsaid.** A `color` the author set on the page and a button only inherits is not
the button's colour: the bar item keeps UIKit's tint (d33eb49dd). The host's rule is: a value
counts as written on a node when it differs from what the node would inherit from the node
above it, or when the property does not inherit.

## 3. Mechanism

The kernel already has the pattern, from LLP 1095: a default may **name a role** that each host
resolves (`text_color` is `CanvasText`: `labelColor` on iOS, `textColor` on macOS, the browser's
own on the web). This LLP extends it rather than inventing a second mechanism:

- **Colours.** Every presentation colour default is a role, never a literal: page background
  (`Canvas` → `systemBackground`/`windowBackgroundColor`), separators (`-exact-separator`),
  links (`LinkText` → the tint on iOS), tint (`AccentColor`).
- **Type.** The root font size is the platform's body size (iOS: Dynamic Type body, 17 at the
  default size; macOS: `NSFont.systemFontSize`, 13; the web: 16). Headings resolve to the
  platform's text styles by level (§5 D3).
- **Behaviour.** A host may resolve an `auto` CSS value to the platform's behaviour where CSS's
  would be a tell (press feedback, keyboard dismissal, scroll-to-top, back navigation).
- **The web host** keeps resolving roles to the browser's own values, so the web build still
  looks like a well-made web page, which is the web platform's native.

## 4. What it costs

Parity with Chrome now holds for **layout given the same inputs**, not for every pixel: an
Apple build whose root font size is 17 lays out differently from a web build at 16, as a
native app would. Conformance tests that compared presentation defaults against Chrome move to
comparing layout at a pinned root size. Snapshots move once. The compatibility id moves when
the schema changes (LLP 1030 D3a). Agreed in advance: "it's ok if things break a little"
(Charlie, 2026-10-09).

## 5. Decisions

- **D1. The precedence and the test** above are binding (`rules/RULES.md`).
- **D2. Unset backgrounds are the platform's.** The window/viewport, launch view and sheet
  surfaces fall back to `systemBackground` (iOS) and `windowBackgroundColor` (macOS), never a
  fixed white. (Today a page with no background is white text on white in dark mode.)
- **D3. Type is the platform's.** The root font size is the platform body size (§3). Headings
  (`role="heading"`/`h1`–`h6`) get the platform's text styles by level, overridable by any
  authored `font-size`/`font-weight`. Bold Text is honoured.
- **D4. Inherited colours never tint controls.** Alerts, spinners, tab selection, swipe
  actions, bar items: the platform tint unless the control's own `color`/`accent-color` is set.
- **D5. Back is always there.** A pushed route without an authored back control still gets the
  system back button, edge swipe and sheet swipe-down; the host pops and reports it as
  `history.back()` would. An authored control is still what the button presses.
- **D6. Confirmations are the platform's alert** (`issues/20261009-ios-confirmation-shape.md`):
  an `alertdialog` becomes `UIAlertController(.alert)` with its title and Cancel kept; `showModal`
  is honoured on iOS; the compiler stops requiring `closedby="any"` for it.
- **D7. Docs say less.** The agent guide, `exact new` and the recipe apps omit colours, fonts and
  control metrics; where colour is needed they name roles. A short starter guide is the only
  required reading.
- **D8. Each platform's own conventions** (the Mac's Help and Edit menus, frame autosave,
  non-selectable UI labels, `NSSearchField`; Windows's IME and accessibility) are in scope on the
  same test.

## 6. Waves

The 2026-10-09 audit (four read-only passes: iOS host, macOS/Windows hosts, kernel and
compiler defaults, docs and apps; ~80 findings) is summarised in Appendix A.

**Wave 1 (2026-10-09, Claude).** The clear wins: D2; D4; iOS keyboard Return (resign on
done/go/search/send, advance on next), `scrollsToTop` on the active scroller only, sheet
scroll-to-expand, swipe-action default grey; macOS Help menu, full Edit menu, frame autosave,
document apps that outlive their last window, tooltips on icon-only buttons, a larger default
window, `plain` buttons in label colour; links coloured `LinkText` (the tint on iOS) and the
pointing-hand cursor on Mac; `hr` in the separator role; the root font size (D3, first half);
the docs and template (D7).

**Wave 2 (from 2026-10-10, Claude).** D3's heading styles (a font-role table in the schema
beside `colors`), D5, D6, platform press feedback for custom pressables, Mac label selection
and focus rings, `NSSearchField`/`UISearchTextField`, a canonical native recipe app with a lint
for literal colours and font sizes.

**Later, needs an owner.** A plain `button`'s default style on iOS (bordered → plain; changes
every app), checkbox on iOS (switch or checkmark), plain-list separators and highlight, Mac
sheets and `NSPopover`, Mac native lists and sidebars, drag out, window restoration, and the
Windows host's IME, accessibility and native controls.

## Appendix A. Audit, condensed (2026-10-09)

Most visible first, per area. `→` is the platform's answer.

**iOS host.** Unset background white in dark mode → `systemBackground` · no back chevron/edge
swipe/sheet swipe-down without an authored back control → always · confirmation is an arrowed
popover without Cancel or title → `.alert` · custom pressables give no press feedback → dim or
highlight · bare `button` is a grey bordered capsule → plain tinted text (deferred) · 16 pt
body → 17 · alert/spinner/tab-selection/swipe-action colours from inherited text colour or
fixed blue → system · sheets on `secondarySystemGroupedBackground` → `systemBackground` · Return
never dismisses the keyboard or advances → does · status-bar tap ambiguous between scrollers →
active one · scrolling in a medium sheet doesn't expand it → does · re-tapping a tab does
nothing → pops to root · Safari-style checkbox → switch/checkmark · plain list rows without
separators/highlight · scrolling never dismisses the keyboard → interactive · nested scrollers
chain instead of bouncing · large title collapses only with `navigationScroll` · in-content
search field without magnifier/clear · painted focus ring → system halo · Bold Text ignored.

**macOS host.** Unset background white in dark mode → `windowBackgroundColor` · 16 pt body →
13 · no Help menu · Edit menu lacks Find, Spelling, Substitutions, Paste and Match Style ·
frame not autosaved unless declared · app quits with its last window even for documents ·
right-click on selected text shows nothing → Look Up/Copy/Services · icon-only buttons without
help tags · search field is a plain field → `NSSearchField` · phone-shaped default window ·
every label drag-selectable · dialogs painted in-window → sheets/`NSAlert` · route headers not
projected to title bar/toolbar · Tab reaches buttons regardless of Keyboard Navigation ·
scrollers forced to overlay · no native lists/sidebars · painted popovers → `NSPopover` ·
`plain` buttons in accent → label colour · no state restoration · no drag out. **Windows:** no
native controls, no IME, no UI Automation, no Alt menus, no Mica, no saved position.

**Kernel and compiler.** Inline links in body colour → `LinkText`/tint · arrow cursor on Mac
links → pointing hand · `hr` fixed grey, two-toned 2 px, Chrome margins → separator hairline ·
compiler requires `closedby="any"` on an alertdialog · Chrome's checkbox/radio/range margins on
every platform · 16 px initial font size, no heading styles · `press-scale: 1` ruling ("UIKit
shows none") · Mac `user-select: auto` selects UI labels · SVG `fill` black → `currentcolor` ·
`scroll-behavior: auto` jumps · `keyboardDismissMode` none · no focus ring on Mac bare buttons.
Fine as is: `system-ui` family, `Highlight` selection.

**Docs, template, apps.** `exact new` paints hex backgrounds, `font-size=28`, hand safe-area
padding · its AGENTS.md makes ~100K tokens required reading · the guides never name the colour
roles · the tabs recipe hard-codes chrome · no recipes for header projections, segmented title,
header search, confirmation, swipe actions, pull-to-refresh · Messages (called canonical) is a
pixel imitation in hex · native-fixture is a test fixture, not a style guide · the human guide
teaches hex and px and says font sizes "remain author decisions" · symbols sized by box.
