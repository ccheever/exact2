# LLP 1115: Write the web, ship the platform

**Type:** Principle
**Status:** Active (Charlie Cheever, 2026-10-09: "write that up and write an LLP about it")
**Systems:** All hosts (iOS, macOS, tvOS, Android, Linux, Windows, web), the kernel's style defaults, the Contract compiler's lowering, the agent docs and `exact new`
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
tells"); on the Mac, a hand-built app on the same macOS, in AppKit or SwiftUI, chosen for each
presentation (amended 2026-10-12, Charlie, LLP 1115.002 D1: it read "on the Mac, AppKit");
each platform likewise. Not Chrome.

> **Amended (Charlie, 2026-10-11; LLP 1115.000 D12.3).** A person's accessibility settings
> come before the author where a hand-built app would apply them: Bold Text weights the
> system font even under an authored weight. The order is the person's accessibility
> settings, then the author, then the platform, then CSS's default.

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
counts as written on a node when its row carries the bit that says its author wrote it there
(LLP 1115.000 D2): an attribute of the node or a row of its class, a platform colour and a
value equal to its parent's included. An inherited value has no row on the child and so no
bit, and a row the compiler implied (a sheet's, a heading's, a tag's own) is set and not
written. Until 2026-10-11 hosts guessed this from the value: written when it differed from
what the node would inherit.

## 3. Mechanism

The kernel already has the pattern, from LLP 1095: a default may **name a role** that each host
resolves (`text_color` is `CanvasText`: `labelColor` on iOS, `textColor` on macOS, the browser's
own on the web). This LLP extends it rather than inventing a second mechanism:

- **Colours.** Every presentation colour default is a role, never a literal: page background
  (`-exact-background` → `systemBackground`/`windowBackgroundColor`; `Canvas` is the
  document's white, `textBackgroundColor` on the Mac, and is not what an unset page shows:
  LLP 1115.000 D3.6), separators (`-exact-separator`), links (`LinkText` → the tint on iOS),
  tint (`AccentColor`).
  > **Amended by LLP 1116.000 D4 (2026-10-10).** The link role reached only a run with an
  > `href` in its paragraph; a block `link` inherited, so a `tel:` or `mailto:` value was body
  > text on the web. On the web a block `link` of text whose colour is unsaid is in the accent
  > (`var(--exact-accent, AccentColor)`, `host/web/index.html`), as the plain button and the tab
  > bar are and as `LinkText` is the tint on iOS, and every link shows the pointing hand. One
  > holding a row or column, or given a `role` or a `buttonStyle`, is the author's row or
  > button and still inherits; an inline run keeps `LinkText`; the native hosts are unchanged.
  >
  > **And by LLP 1115.000 D3.5 (2026-10-11).** The compiler gives such a block `link` the
  > link role on every host (`contract/lower/src/tags.rs` `link_color`), so it is the tint on
  > iOS and `linkColor` on the Mac as well, and `LinkText` on the web is the accent
  > (`var(--exact-accent, AccentColor)`): one blue for links there. A terminal entry's block
  > link is left as it was.
- **Type.** The root font size is the platform's body size (iOS: Dynamic Type body, 17 at the
  default size; macOS: `NSFont.systemFontSize`, 13; the web: 16). Headings resolve to the
  platform's text styles by level (§5 D3).
- **Behaviour.** A host may resolve an `auto` CSS value to the platform's behaviour where CSS's
  would be a tell (press feedback, keyboard dismissal, scroll-to-top, back navigation).
- **The web host** keeps resolving roles to the browser's own values, so the web build still
  looks like a well-made web page, which is the web platform's native.
  > **Amended by LLP 1116 D1 (2026-10-10).** The web host resolves roles to a designed default
  > look that a web developer would not notice as unstyled (LLP 1116 §4): the browser's bare
  > button, checkbox switch, plain header and bordered dialog are tells on the web, so the
  > platform's answer there is a carefully built web app's.

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
  > **Amended by LLP 1116.021 D7 (2026-10-17).** On iOS a sheet's surface is its route's own
  > background where the route has one: a sheet whose scroll is a grouped list is on the
  > grouped background (LLP 1116.007 D5), as a hand-built grouped sheet is, so no white band
  > shows between it and a keyboard's bar. A route with none keeps `systemBackground`.
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
  is honoured on iOS; the compiler stops requiring `closedby="any"` for it. *Landed (wave 2,
  2026-10-09):* by the HIG, text + one cancel + at most three buttons is the alert (title the
  `aria-label`, else the first line; message the rest); a chooser with a cancel is an action sheet
  unanchored on a compact screen (its Cancel drawn; iOS 27 centres it); a shape with no cancel stays the popover, which a tap outside
  dismisses (an alert has no outside). `showModal(id)`/`close(id)` from an action present and
  dismiss it on iOS with no invoker, show the `dialog` on the web and macOS; LLP 1021's "no title
  row" is superseded. macOS keeps its `NSMenu` chooser for invoker-opened alertdialog popovers
  (no `NSAlert` yet).
  > **Amended by LLP 1115.000 D13.4 (2026-10-11).** "An alert has no outside" holds on every
  > host now, not only where UIKit draws it: a `dialog` of the alert's shape implies
  > `closedby="closerequest"`, where every `alertdialog` implied `any`, so the web's `dialog`
  > and Android's are not dismissed by a tap outside either (the Mac's is `NSAlert` since
  > LLP 1115.000 D7.6). An author's `closedby="any"` is the platform's dismissible form: an
  > action sheet or popover on iOS, a light-dismiss dialog on the web, the painted dialog on
  > the Mac, a Material dialog cancelled by a touch outside on Android. The shape rule is one
  > function in the kernel (`kernel/src/confirmation.rs`), which lowering and each host read.
  >
  > **Extended (2026-10-15, the euxy port's rename prompt).** A prompt is the alert with a text
  > field: one `input` row of an alert-shaped `alertdialog` counts with its text and is
  > `UIAlertController`'s text field on iOS, `NSAlert`'s accessory field on the Mac, and the
  > `dialog`'s field on a line of its own on the web, standing in for the authored `input`.
  > Return, on every host: the field's key handlers first; a field with a `submit` then commits
  > and runs it, and that is all; any other commits and presses the first action that is not
  > destructive, whatever its style, nothing while that action is disabled. Lowering refuses a field in any
  > other shape; Linux paints it as authored.
  >
  > **Amended (2026-10-16).** On iOS Return is the field delegate's, and the alert has no
  > `preferredAction` unless its author gave one: a hand-built `UIAlertController` and React
  > Native's `Alert.prompt` draw no action filled, so an unsaid prompt drew "Rename" as a tell. An
  > alert's preferred action is the Mac's default button (LLP 1115.000 D7.3): its one
  > `buttonStyle="filled"` action that is not destructive; on a prompt only when that is the action
  > Return presses, so the bold action and Return's are always one.
- **D7. Docs say less.** The agent guide, `exact new` and the recipe apps omit colours, fonts and
  control metrics; where colour is needed they name roles. A short starter guide is the only
  required reading.
- **D8. Each platform's own conventions** (the Mac's Help and Edit menus, frame autosave,
  non-selectable UI labels, `NSSearchField`; Windows's IME and accessibility) are in scope on the
  same test.
- **D9. The iOS dev menu is the exception** (Charlie, 2026-10-16). A release iOS build keeps it
  by default, as a shipped page keeps its browser's developer tools: the build's info and
  release notes are worth more to the people testing it than the gesture costs anyone else.
  `EXACT_DEV_MENU=0` removes it, set when building (`ExactDevMenu` in `Info.plist`, which an
  installed app needs: it is launched with no environment) or when launching. A release Mac
  build ships its Develop menu hidden (LLP 1115.000 D7.8), since there it is a menu-bar item
  every person sees and its chords take ⌘R and ⌘D: `defaults write <bundle id> ExactDevMenu
  -bool YES` shows it, as Safari's settings show Safari's, unless the build said `0`.

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

*D3's heading styles, as built (2026-10-10).* The schema's `textStyles` beside `colors`:
eleven styles (`large-title` … `caption2`), each with its UIKit/AppKit name, WebKit alias,
weight and Apple's ramp, the size at each body size (13 the Mac's; 14–53 iOS's Dynamic Type
categories). A style is a value of `font-size` (`-exact-title1`, `-apple-system-title1`)
that the **kernel** resolves, like `rem`, against the root font size (wave 1's platform body
size), so the hosts need no new fact and layout agrees on every host given the same root;
Dynamic Type's nonlinear scaling comes from the ramp, not a ratio (AX5's title1 is 58, not
87). The web writes the ramp at 16 px in `rem`. Lowering gives `text role="heading"` an
unsaid `font-size` and `font-weight` by `aria-level` (1 `title1`, 2 and unset `title2`, 3
`title3`, 4+ `headline`; HIG weights: titles regular, `headline` semibold), before the
author's rows. `font="-exact-footnote"` (CSS's `font` shorthand, a text style only) sets a
style's size and weight. Not done: Bold Text, the Mac's bold `headline` (one weight per
style, iOS's semibold), and per-style leading (line height stays `normal`).

*Status (2026-10-10).* Wave 1 landed whole (eaa9f1b41 docs, 69c5455aa iOS, 550aa871a macOS,
8dd49f60c type/links/`hr`), plus a pitfall for a fixed light background in dark mode
(`color-scheme="light"`, 67a1892b6). Wave 2 landed but for the recipe app and its lint: Mac
label selection, `NSSearchField` and the selected-text menu (3d4713c02; the system focus ring
was already drawn); D5 (29bd3dcd1 by the root's `navigate`, 6e78068b4 by the runner's
`host_back` where there is none; `exact new`'s root names `navigationBack`, a92deb3eb);
press feedback and `UISearchTextField` (edd4690de); D6 (b968df0d8, `showModal`/`close` on every
host); D3's heading styles (2d26c223f); the recipe app, `apps/shelf`, and its lint,
`scripts/no-tells.mjs` (held by the app's `app.test.ts`), with what it found fixed on
iOS: a `role="heading"` without `aria-level` is a level-2 heading to the bar and
VoiceOver (it was neither, so a sheet's header stayed painted), a segmented control in a
grouped-list row is judged where it shows (it was disabled), and a route's bar follows a
grouped list drawn in its scroller's place (a large title over one never showed); symbol
roles `star`, `star-fill`, `minus`, `chart`, `sparkles`. Still open: Bold Text; `text
aria-level` without `role="heading"`; the grouped list's fixed sizes as text styles; and
from the recipe, a determinate `progress` (`UIProgressView`), a stepper (`UIStepper`), an
app-wide tint (`accent-color` on the root reaches the tab bar and controls, but a bar
button takes only its own `color`; LLP 1095 D4's `theme`), swipe actions and context menus on
grouped-list rows, borderless fields in list cells, and a footer's colour.

*Status (2026-10-11), after a second audit (LLP 1115.000).* Corrections to the record above,
which read as "on every host" where it was iOS and the web:

- **D5** (Back without an authored control) landed on iOS and the web. macOS, Linux and
  Android do not call `host_back`; a pushed route on the Mac has no way back without one.
- **D6**'s `showModal`/`close` landed on iOS, macOS and the web. The Linux host, and so
  Android, refuses both and presents no dialog or popover.
- **Press feedback for custom pressables** landed on iOS only.
- **D3's root font size** landed on iOS, macOS and the web. Linux, Android and Windows show
  16, and tvOS scales 16.
- **Heading weights:** a title is bold since LLP 1116 D7, not "regular" as written above.
- Of "Still open", four have landed: a determinate `progress` (LLP 1116 D8), an app-wide
  tint (#440), swipe actions on grouped-list rows (LLP 1116.001 D1) and a footer's colour
  (LLP 1116.003 D7).
- Ten items of Appendix A are in neither list above. LLP 1115.000 §1 names them and gives
  each a decision.

What is left, on every host, is LLP 1115.000 (and LLP 1115.001 for Android). Since this
note, LLP 1115.000's wave 1 has landed D5 and D6's alert on macOS, and LLP 1115.001's first
slice has begun Android's; each document's "as built" says what holds.

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
