# LLP 1021: Menus — the popover, its invoker, and the native pull-down

**Type:** RFC
**Status:** Draft
**Systems:** Kernel (a top layer; the popover's box anchored to its invoker), Contract (HTML popover and dialog declarations), Web host (the Popover API by identity), Apple host (menu-shaped popovers presented as UIMenu/NSMenu), Linux host (a kernel-painted top layer), Agent API (no ninth operation), Weird Castle (the account switcher, first consumer)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
**Revision:** §5.1, 2026-10-05 (Claude, Opus 5.5): the long-press context menu earned back, with its preview and commit, for the Signal clone's chat list (Charlie's approval).
**Related:** LLP 1017 §8.1 (literal HTML/CSS names, no aliases — the rule that names every row here), LLP 1001 (where a deviation from the bare element is declared), LLP 1008 (host state that never enters the plan — scroll offset — and the native text field, the one native control so far), LLP 1012 (the eight operations; `tap` is a journal entry into the runner, never OS input), LLP 1014 (the canvas capture the top layer sits outside of), LLP 1018 (the `EXACT_AGENT` presentation-swap precedent: `MemoryStore` for the keychain), weird-castle e644c82 (the hand-rolled switcher overlay this replaces). Platform record: the HTML Popover API and invoker attributes; the WAI-ARIA menu pattern; Apple HIG "Menus" and "Pull-down buttons"; `UIMenu`/`UIButton.menu`/`showsMenuAsPrimaryAction` (iOS 14+), `NSMenu` (macOS).

## 1. Summary

A node opens a floating layer through the web's own machinery: a container
carries the **`popover`** attribute, a `button` names it with
**`popovertarget`** (by **`id`**, HTML's reference), and the host owns the
top layer — light dismiss, one auto popover at a time, Escape closes,
anchored to the invoker. Open state is **host state**, never plan state,
the way scroll offset is (LLP 1008): opening a menu causes no relayout and
no commit, and an app declares no `menuOpen` slot.

When a popover's content is **menu-shaped** — `button` rows and `hr`
separators, nothing else — an Apple host may present it as the platform's
own pull-down menu: `UIMenu` off the invoker on iOS, `NSMenu` on macOS,
with `aria-checked` as the system checkmark and `disabled` as the dimmed
row. Selection dispatches the row's `press` into the runner by view id —
the same journal entry a painted tap makes — so the runner cannot tell
which presentation was up, and neither can a test. The web arm is the
oracle for semantics; native presentation is a declared divergence of
pixels only, exactly the standing the native text field already has.

First consumer: Weird Castle's account switcher (e644c82), today a
hand-rolled absolute overlay with no light dismiss, painted inside the
night surface's texture. It becomes a real pull-down.

## 2. Motivation

Three pressures, one primitive:

- **The switcher is a reimplementation, and an incomplete one.** e644c82
  builds the menu from `position="absolute"` rows, a `menuOpen` slot, and
  a `toggleMenu` action. It has no light dismiss (tapping the sky leaves
  it open), no Escape, no top layer (it is sampled into the night
  surface's texture like any other child), and its open state round-trips
  through the runner for what is presentation. Every app that needs a
  menu next will hand-roll the same overlay slightly differently — the
  four-disagreeing-layers class (`CLAUDE.md` §The web is the standard),
  arriving one app at a time.
- **The platform pattern is specific and it is not what we drew.** The
  HIG's affordance for "a button that reveals related actions or a mode
  switch" is the *pull-down button* — the App Store profile button,
  Safari's profile switcher — with the current choice checkmarked and
  destructive actions styled by the system. A long-press *context menu*
  is for actions on a piece of content and must never be the only path to
  a feature; the switcher's tap-on-the-mark gesture is already the
  pull-down's, so the native mapping is exact.
- **The web grew the standard vocabulary.** The Popover API (`popover`,
  `popovertarget`, `popovertargetaction`) is shipped in every engine and
  is precisely this: a declarative floating layer with light dismiss in
  the top layer, opened by a button, no script. LLP 1017 §8.1 obliges
  these names; the dev-loop browser implements them; the web host gets
  the whole behavior **by identity, zero new bytes**.

## 3. What the platforms teach

**The HIG's split.** Pull-down menus attach to a visible button and open
on tap — discoverable, primary. Context menus (`UIContextMenuInteraction`)
open on long-press over content, and the HIG is explicit that they are
supplementary: essential functionality must be reachable another way. An
account switcher is essential navigation → pull-down, and this RFC builds
only that; the long-press trigger is refused until content earns it (§5).

**The UIMenu mapping is data, not pixels.** A `UIAction` is a title, an
optional image, `state` (`.on` renders the checkmark), and `attributes`
(`.disabled`, `.destructive`); separators fall out of inline sections.
`UIButton.menu` + `showsMenuAsPrimaryAction` makes the invoker's tap open
it; the system owns placement, glass, Dynamic Type, VoiceOver, and
dismissal. Nothing in that list is a box the kernel laid out — which is
why D3 extracts *data* from the menu grammar rather than trying to teach
UIKit our pixels, and why non-menu popovers stay kernel-painted.

**The web's grammar for the same thing.** ARIA's menu pattern is
`role="menu"` containing `role="menuitem" | "menuitemradio"` with
`aria-checked` for the current choice — names the tag table already half
has (`role`, `aria-label` — `contract/lower/src/tags.rs:165`). The
grammar D3 recognizes is exactly this pattern spelled with rows we
already lower, plus `hr`, HTML's separator.

## 4. Design

**D1 — the vocabulary is the Popover API, by its names.** Four attributes
and one tag join the table (LLP 1017 §8.1; every name is the HTML one):

- `popover` on a container — v1 admits only the `auto` value (light
  dismiss, one auto popover open at a time, Escape closes). `manual` is
  refused (§5).
- `id` on any node — HTML's reference, introduced for `popovertarget` and
  useful to nothing else yet.
- `popovertarget` on a `button` — the invoker; `popovertargetaction`
  (`toggle`/`show`/`hide`, default `toggle`). A `button` may carry both
  `press` and `popovertarget`; both fire, the spec's behavior — the
  switcher uses exactly this to refresh `accounts` as the menu opens (D6).
- `hr` — a void row, the separator; outside a popover it is the element's
  bare self (a rule). *Built 2026-10-04:* a `view` whose `semanticTag` is
  `hr`, carrying the UA stylesheet's rows (`margin: 0.5em auto`, a 1px
  `inset` border, `color: gray`, `overflow: hidden`), the author's own rows
  winning; children are refused (`lower-void`). `inset` joined the border
  styles for it (LLP 1001 §1, "Border semantics": Chrome's two shades on
  the native hosts). The sheet names no `border-color`, as Chrome's does
  not; a `currentcolor` inset side paints from `#eeeeee`, so a bare `hr` is
  `#9a9a9a` over `#eeeeee` whatever its `color` (measured in Chrome 154). Like the browser's, an `hr` in a flex column has auto
  side margins and so no width until the author zeroes them.
- `aria-checked` on a `button` — the ARIA state, lowered like its three
  siblings at `tags.rs:165`.

**D2 — the top layer is the host's; open state never enters the plan.**
The kernel lays the popover subtree out on every commit regardless of
visibility — a hidden layer, anchored: top-left at the invoker's
bottom-left, clamped to the viewport, sized by content. The host shows
and hides it; opening is **no relayout and no dispatch**, the instant-menu
property, and the same discipline as scroll offset (LLP 1008: host state,
never plan state). Light dismiss, the one-auto-popover rule, and Escape
are the host's, per the spec.

Declared deviation (LLP 1001's ledger): a bare `[popover]` on the web is
a *centered* fixed box (`inset:0; margin:auto`). v1's popovers are
**anchored to their invoker** instead — v1 serves menus, and the newest
spec gives an invoker-opened popover exactly this implicit anchor. CSS
`position-area` is the vocabulary for placement other than this rule, and
it is admitted for a subset ("Placement", below; §5): the rule is `none`, which a
popover without the row keeps. As built, the web passes a popover with no
`position-area` to the browser's default centred placement (the one glue
rule was never written); one with the row is placed by the browser's own
anchor positioning against the same implicit anchor.

The top layer sits **outside every canvas capture** — the web's top layer
cannot be sampled by anything on the page, and that is the parity: a menu
over Weird Castle's night floats above the sky, not inside its texture
(today's switcher is sampled into it, LLP 1014). Menus read as chrome on
every platform; this makes it so here.

**Messages attachment reference and probe (Codex, 2026-09-10):** public
XCTest opens Add from an existing native conversation. The rich, scrolling
popover is 320 × 456.6 points, at (10, 373) with the keyboard closed and
(10, 274) over an open keyboard; the draft editor stays focused. The current
Messages example still has an authored emoji tray. Its Add and emoji controls
now declare `retainFocus`, verified physically and in the browser, but that
repairs editing intent only. Rich content still needs D2's presentation owner.

A temporary public-UIKit probe tests `UIPopoverPresentationController`, no
arrow, source overlap allowed, and the reference's content size. The ordinary
popover preserves focus but places its keyboard-open box at y=73. Extending
its layout margins using the measured keyboard overlap moves the box to y=274,
but pixels show the keyboard covering its lower rows. A third, separate-window
probe with ordinary margins again places it at y=73. All three open/dismiss
scripts finish; none proves the required keyboard-overlaid presentation. This
three-round prototype is stopped without host integration. A future approach
must prove painting and touch order as well as its rectangle, and respect
LLP 1031's containing-app/session ownership boundary. Source, screenshots and
XCTest records: `/tmp/messages-attachment-menu/` and its `probe/` directory.

**Modal confirmation (Codex, 2026-09-11, implemented for Messages):** a
`dialog` is invoked by a button's HTML `commandfor="id" command="show-modal"`;
its action and explicit Cancel use `command="close"` for that same id. The
browser supplies the real modal top layer, focus, Escape, outside dismissal
when `closedby="any"`, and input exclusion. A closed dialog is hidden; its tag
lowers the HTML absolute-position default so it consumes no ordinary flow.
No application open-state slot or host script reproduces the browser's dialog.
Root navigation Escape and global authored shortcuts respect its input owner.

UIKit admits the existing confirmation grammar: direct explanatory text, one
handled action, and one handlerless closing Cancel, with `closedby="any"`.
Other dialog content and close policies are unsupported. The same session-owned
`UIAlertController` used below extracts the live action data after the invoker's
press, retains the presenting editor, and binds completion to the original
source, route and action. The browser's explicit Cancel remains in the declared
content; UIKit omits that row in its popover presentation. AppKit and Linux have
no dialog projection yet and keep the subtree hidden; this is not general
cross-platform dialog support. Existing contact popovers retain their separate
nonmodal browser declaration and native confirmation projection.

Messages removes `confirmDelete` and its authored cancellation overlay. Four
physical cases on the rebuilt iPhone app cover one/two selections in light/dark:
the action rectangle is (26,766,208,48), matching native Messages. Outside
cancellation preserves selection and draft; confirmation deletes the selected
local messages and exits selection with the keyboard closed. The browser passes
the same cases, including an outside click over another selectable message;
first Escape closes confirmation, second Escape exits selection. A two-session
native fixture preserves the live editor through cancellation, dispatches confirm
once, refuses a stale action after reload, and cannot revive the prompt after
unmount/remount or destruction. These lifetime checks use agent activation;
the Messages opening/outside-cancel/confirm cases use public XCTest taps.

The earlier auto-popover prototype allowed outside dismissal to select the
message underneath; the [HTML dialog model](https://html.spec.whatwg.org/multipage/interactive-elements.html#the-dialog-element)
supplies the missing modality. Prototype failures remain in
`/tmp/messages-selection-owner/`. Integrated candidates and evidence are in
`/tmp/messages-modal-confirmation/`: R1 passed interaction checks but a bare
closed dialog occupied flow; R2's default used the wrong internal style-row name
and failed both bakes; R3 corrects it and passes both bakes and actual-app drives.
Native Close/Forward foreground dimming, transition timing and browser visual
polish remain owed. LLP 1035.004 D6's three-variant UIKit probe rules out ordinary
tint adjustment and disabled-control appearance as exact dimming replacements. Geometry and interaction checks do not establish full pixel
or motion parity.

**Contact confirmation reference and probe (Codex, 2026-09-10):** native
Messages' Block Contact opens a 240×190.333-point confirmation at (10,481),
with one destructive action and outside-tap cancellation. New Contact's Close
opens a 240×168.333 confirmation at (10,72), even before editing the prefilled
contact. The first-name field and keyboard remain active beneath that prompt.
Outside cancellation keeps the form; Discard Changes closes it. This is a
presentation over its source, not a replacement page-sheet route.

A temporary public-UIKit probe uses `UIAlertController(.actionSheet)` with a
destructive action and a cancel action, presented without an arrow through
`UIPopoverPresentationController`, allowing source overlap and refusing adaptive
replacement. Anchoring Block to the full row centers it at x=81; using its text
label's bounds gives native x=10, with every measured message/action rectangle
matching. Close uses its own 44-point control bounds and matches the second
confirmation. The Block crop is pixel-identical (411,120 pixels), with the saved
native source screenshot as the prototype's backdrop. The Discard crop has a
6.33/255 mean channel difference despite matching geometry; the bitmap backdrop
does not establish native source rendering or exact material parity. A third
probe uses an actual text field: public XCTest typing, opening, outside cancel,
reopening and local discard retain editing until discard, then remove the field
and keyboard. No native Messages send or contact save is performed. Native
cleanup explicitly chooses Discard Changes and verifies the original conversation
draft, `Short again`, with no remaining contact form or confirmation.

The three standalone prototype candidates stopped here. At that point these
APIs and results were not yet an Exact presentation: declared trigger/cancel/confirm actions, source anchoring,
session lifetime, browser behavior and application integration remain work.
The contact form therefore stays a named consumer alongside Block Contact;
building its Close as unconditional dismissal would contradict the reference.
Artifacts: `/tmp/messages-contact-actions/`, including failed native reference
runs, successful cleanup, the standalone probe, XCTest records and pixel counts.

**Subsequent integration (Codex, 2026-09-10):** Messages now uses a declared
`popover="auto" role="alertdialog"` for local Block Contact. The iOS host
recognizes direct explanatory text, one press button and one handlerless cancel;
both buttons target that popover with `popovertargetaction="hide"`. The invoker's
own press runs first, synchronously through the session, before extracting the
updated content. Its direct text bounds anchor a labelled row; an icon invoker
uses its whole box. The host adds no application open-state slot.

This shape uses the same `UIAlertController` under normal and agent launches,
an exception to D4's older menu swap. A session owns the presentation until
native dismissal completes and its selection callback has returned. Confirm dispatches once only if the original source,
route and action still exist; outside cancellation dispatches nothing. Reload,
unmount and destruction dismiss that session's confirmation. The presenting
editor remains mounted and focused during cancellation. The two-session Exact
fixture verifies retained text/focus, confirm removing the editor, an old action
refused after reload, unmount/remount without revival, and destruction leaving
the other session editable. Destruction while the observed phase is still
`transition` also leaves the other session editable. Public XCTest software keys
then prove editing through open/outside cancel/reopen/Discard on the Exact
fixture, with `autocorrect="off"` for literal retention (spellcheck alone does not
disable correction). `state.navigation.popover` reports the owner/phase;
agent action activation is identified as activation. `layout` reports unavailable
native action geometry, since UIAlertAction exposes no public action view.
`screenshot(path, true)` captures the containing app window and its presentation;
it does not promise to capture separate system windows such as the keyboard.

The browser keeps its native Popover API. `[popover]:not(:popover-open)` must
remain `display: none` despite the authored container's flex display; the first
browser drive exposed and repaired that override. Both hosts pass local
block/unblock, cancel, draft restoration and suppression of pending fixture
replies. Real UIKit touches pass open/outside cancel/confirm/unblock/return.

**Limits at initial integration:** the iOS default action painted black despite
the authored red tint; the explicit role repair below closes that gap.
Browser placement remains its default centred popover. Window `drawHierarchy` includes
the prompt but differs from the Simulator capture in its glass backdrop (the
measured Block crop differs by 1.40/255 mean channel error); physical captures
remain the pixel evidence. Rich
attachment popovers, complete New Contact fidelity, adaptive placement,
and the other native hosts remain separate work. Evidence and failed candidates:
`/tmp/messages-confirmation-integration/verification.json`.

**Browser dismissal ownership (Codex, 2026-09-11):** the New Contact sheet's
`closedby="none"` handler also prevented the discard popover's default Escape
dismissal. The modal handler now defers to the document's open auto/hint popover
and respects an already-prevented key. Browser keystrokes close the confirmation
while retaining the form, editor focus and draft; the next Escape leaves the
protected sheet open. Dismissible Compose, a containing-page popover, manual
popover behavior and consumed keys also pass. The selected-message toolbar's
authored `key` handler remains a separate migration issue above. Evidence and
the corrected populated-Compose fixture: `/tmp/messages-popover-escape/`.

**New Contact consumer (Codex, 2026-09-10):** the Messages form now uses the
same declared confirmation for Close. iOS/browser activation verifies unchanged
prefill, edited cancellation, fresh reopening after Discard, local Save, renamed
inbox lookup and restored conversation draft/focus. Public XCTest software keys
continue typing into the same editor after outside cancellation; a downward sheet
drag retains the form, Discard removes it, and local Save returns to details.
The native action rectangle is (26,176.333,208,48), matching the reference.
This did not clear the destructive-style/material limits above. The form itself
starts ten points above native; the container probe in LLP 1035.001 D4 locates
that difference in modal ancestry. Multiple addresses, photo/pronoun/tone controls,
other native contact fields and editing remain unfinished. Source, versioned
binaries, physical captures and the local data regression are recorded under
`/tmp/messages-new-contact/`.

**Recovery consumer (Codex, 2026-09-11):** Messages' filter menu now switches
between Messages and Recently Deleted within the same inbox route. Recovery and
permanent deletion use the existing alertdialog grammar. The invoker snapshots
the selected conversation ids and message count before presentation; the app's
local data source owns the archived records, ordering and expiration. No host
property or application popup-open flag was added. Three application candidates
corrected boolean ARIA values and made the popovers absolute so their logical
boxes do not consume the inbox's layout space.

Browser and iOS activation verify selected-message recovery/cancellation/purge
with the conversation draft retained. Ordinary-launch public XCTest verifies
swipe deletion, the native filter menu, outside cancellation and removal of the
archived row after Recover. Returning to Messages then fails with a loading menu;
the complete native recovery/purge flow is unverified. The three-candidate app
loop stops here. `MenuHost.sync` replaces the deferred menu on every batch and
this app requests time-dependent recovery data; their causal role in the observed
failure has not been isolated. Apple's iOS 26 guide images supply the visual reference, not a live
MobileSMS recovery capture. The native default Recover action remains black on
gray, whereas the guide shows blue; filter-menu icons/coverage, browser selection
outlines, agent-mode menu presentation and exact motion remain open. Source,
versioned builds and the failed physical drive: `/tmp/messages-recovery/`.

**Selection completion order (Codex, 2026-09-10):** the rebuilt New Contact
physical flow passes all 16 functional captures but incurs 13 XCTest animation-idle
timeouts after Discard (799 seconds). A small public-UIKit comparison does not
reproduce those timeouts with any of its three dismissal strategies. It does
observe native action handlers after the alert leaves its window, an explicit
`dismiss` completing synchronously inside that handler, and the dismissal delegate
arriving afterward. Starting parent teardown there precedes that final callback.

MenuHost now retains its finishing owner through one main-queue turn after the
completion, then rechecks the original identities/route before dispatching once.
The subsequent native dismissal delegate cannot turn a selected action into
cancellation. Reset still invalidates the pending owner. Block/Unblock, retained
cancel, reload/unmount/destruction and destruction during presentation pass. A
further two-session drive reloads and destroys while a selected action is actually
pending: the old action never dispatches afterward and the other editor survives.
Two fresh physical New Contact runs pass 16 captures each without idle timeouts
(19.3 and 19.5 seconds). This supports the ordering repair but does not establish
the cause of the earlier intermittent XCTest symptom. Source, binaries, callback
trace and runs: `/tmp/messages-confirmation-completion/verification.json`.

**Explicit action role (Codex, 2026-09-10):** Block and Discard now declare
`destructive=true`. Schema row 68 is generalized from `swipeDestructive` to
`destructive`, without another row or an alias. UIKit maps it to
`UIAlertAction.Style.destructive`, `UIAction.Attributes.destructive` and the
existing swipe-action role. The web emits `data-destructive`; authored CSS and
dispatch stay unchanged. AppKit/Linux presentation of the role remains owed.
`state.navigation.popover.actionStyle` reports the actual confirmation style.
The native menu invoker also retains its authored accessible name, identifier
and disabled state; the first physical menu fixture found an anonymous button.

An ordinary-launch public-XCTest fixture presents an ordinary action authored
red and a destructive action authored blue: only the latter paints native red,
for both menus and confirmations. Physical selection updates the intended value
and increments the confirmation count once. iOS activation and browser DOM
checks also cover a changing flag, cancellation and dispatch. Two 22-capture
Messages drives retain editing after cancellation, discard/save locally, restore
the draft and delete the intended swipe row, without XCTest idle timeouts.
Block and Discard have the reference's red text bounds and dominant RGB
(255,56,60). Their action-crop mean channel differences are 1.06 and 6.73/255;
That run leaves Block two points above native; Discard's rectangle matches.
This establishes the action style, not complete material or geometry
parity. Versioned binaries, failed fixtures and current evidence:
`/tmp/messages-destructive-actions/verification.json`.

**Contact source geometry (Codex, 2026-09-10):** correcting the authored
name/address heights and two 52-point contact rows removes Block's two-point
anchor error. The separator sits inside the first row, and each button owns
the full 370-point width. The native sheet now matches the reference at
(10,481,240,190.3), including its action at (26,607.3,208,48). Browser geometry
and physical left-edge activation pass; all 22 contact-flow captures pass
without idle timeouts. The raw full-sheet crop differs by 0.47/255 mean channel
value, so material parity remains open. This uses existing CSS declarations;
no presentation offset or host change. `/tmp/messages-contact-geometry/verification.json`.

**The chooser (Claude, 2026-10-04):** an "Open in…" button offering the
installed map providers is the first consumer that chooses among N rows built
from data. On iOS a `role="alertdialog"` popover is now a sheet with one action
per press row and at most one handlerless cancel. It had required exactly one
action and one cancel and opened nothing otherwise. Each action dispatches its
own press, once, by the same owner rules as above. The owner is invalid if any
of its actions stops being live, inside its popover or closing it, or now
shows a different title or enablement, so a reused row given another
provider never dispatches under its old title. A disabled action is
presented dimmed (`isEnabled = false`), and the chosen one is checked again
before dispatch.
A shape the sheet cannot present (no action, every action disabled, two
cancels, a row that is not text, an action or the cancel, an action that does
not hide the popover) logs
`confirmation <id> refused: <why>` and opens nothing; it is no longer silent.
The popover's `aria-label` titles the `UIMenu`, and titles the sheet only
when it is a chooser: more than one action and no explanatory text. A
confirmation keeps no title row, as the native prompts it matches have none. A menu row's
item image is its symbol, else its `img` once that has loaded, fitted to 24
points. The menu reads the bitmap the hidden row already holds, so opening it
never fetches. `UIAlertAction` has no public image, so sheet rows show no icon.
`state.navigation.popover.actions` counts the sheet's actions. The author
writes `role="menu"` on a column: there is no `menu` tag. An `hr` row (D1)
is a section break in the `UIMenu`, and `NSMenu.separator()` in macOS's
`NSMenu`.
The fixture's `open-in` (a menu, its providers, an `hr`, then Cancel) and
`open-in-sheet` (a sheet), with rows from its `providers()` source, are the
evidence.

*Refused at compile time (2026-10-04).* What the sheet refuses at the tap,
Contract refuses where a literal shows it, as `lower-alertdialog` naming the
row: a popover with `role="alertdialog"` (or a `dialog` with that role, which
also needs `closedby="any"`) whose rows, read through `each`, `when` and
`match` to the elements they produce, are anything but `text`, actions (a
`button` with `press` that hides it: `popovertarget` and
`popovertargetaction="hide"`, or `commandfor` and `command="close"`; a `link`
is refused as any other row, since the hosts present buttons) and at
most one cancel (a `button` without `press` that hides it; one on each arm of
one `when` is one, the same component used on both arms included, and one
inside `each` may repeat and is refused), or that has no action. A value known only at run time (a bound `popovertargetaction`, an
`id` that is not a literal) is left to the host, which still refuses and logs. Placement
beyond D2's rule is `position-area` ("Placement", below); the keyboard
contract stays where §5 puts it.

**The chooser on macOS (Claude, 2026-10-04):** the same shape, by the same
rules and with the same refusal lines, is an `NSMenu` popped up against its
invoker (`popUp(positioning:at:in:)`), as a button menu already was; it had
been painted in the top layer. The menu's top-left is where the popover's
box would sit by its `position-area` ("Placement", below), the menu's own
size taken as the box: below the invoker by default. One item per action: its title, its image (as
iOS's menu rows), `.on` for `aria-checked`, dimmed when disabled, red when
`destructive`. A chooser is headed by its `aria-label` as a section header
(`NSMenuItem.sectionHeader`): a pop-up `NSMenu` never shows its own title,
and the HIG's way to label a menu's items is a header above them. A
confirmation's text rows are disabled lines at the top, wrapped at 260
points, then a separator, and no heading. The cancel has no item: Escape
and a click outside end a menu and dispatch nothing, as UIKit drops the
cancel from a sheet shown as a popover. A chosen item is recorded, and its
row is pressed by view id once, on the next main-queue turn after the menu
ends, as iOS presses after the sheet is dismissed. AppKit sends an item's
action inside `popUp`, while it still tracks the menu in the invoker, and a
press whose batch unmounts that invoker (Messages' Discard Changes
navigates back) must not run under it. The owner checks above run then,
and again as each batch lands, while the menu is open and after it has
ended with a choice awaiting its turn: a batch that changes a row's title
or enablement, hides it (its own or an ancestor's `display: none` or
hiding, though not the popover's own, hidden in place while its menu shows
it) or makes it inert, that unmounts, hides, disables or makes inert the
invoker or points it elsewhere, a reset or an unmount ends the menu, and
the choice is cancelled for good: a later batch that undoes the change
does not revive it. A choice belongs to its presentation: presenting the
popover again cancels one still awaiting its turn, whose press would
otherwise run under the new presentation (its hide closing it). A button
menu's item is pressed the same way: on the next turn, once per menu, and
only if its row is still the node the menu showed (live, in its popover,
enabled, shown, the same title; an id reused by another node is not it),
its invoker still opens the popover as above, and the popover has not been
presented again; the same per-batch checks cancel it for good. A hidden or
inert row is an item that cannot be chosen, as a chooser's. A reset before
the turn presses nothing. A refused shape
is logged and keeps its painted presentation, which macOS, unlike iOS, has.
A menu-shaped popover is headed by its `aria-label` the same way, and a
row of an `img` and text is menu-shaped, its item showing the bitmap fitted
to 16 points. Under the agent (D4) every popover stays painted, so the
agent's taps need no host activation here. `ChooserMacTests` is the
evidence; `<dialog>` stays the session's modal top layer on macOS.

**Placement, `position-area` (Claude, 2026-10-04):** the chooser's next
consumer is an "Open in…" button at the foot of a card, whose chooser must
sit above it or centred on it: the first popover that cannot live at the
invoker's bottom-left, which §5 named as the condition. CSS `position-area`
is admitted on a `popover` only, its anchor the invoker that opens it (the
implicit anchor; no `anchor-name`, `position-anchor` or `position-try`),
with these values, spelled as CSS spells them:

| `position-area` | CSS meaning (implicit anchor = the invoker) | placed |
|---|---|---|
| `none` (no row) | — | D2's rule: top-left at the invoker's bottom-left |
| `bottom span-right` | the bottom row, centre and right columns; aligned to the anchor's left edge | the same as D2's rule |
| `bottom`, `bottom span-all` | the bottom row, all three columns; `anchor-center` | below, centred on the invoker |
| `top span-right` | the top row, centre and right columns | bottom-left at the invoker's top-left |
| `top`, `top span-all` | the top row, all three columns; `anchor-center` | above, centred on the invoker |
| `center` | `center center`, the anchor's own cell; `anchor-center` in both axes | centred over the invoker |

A single keyword names its row and spans every column, so `top` *is* `top
span-all` (centred), not `top span-right`; `bottom` alike. Any other
value (a corner, `left`/`right`, `span-left`, logical keywords, `span-all
top`) fails the build with `lower-css-position-area`, as does the row on a
node that is not a popover. Every host clamps the box to the viewport (the
layer): CSS does the same, as an absolutely positioned box that overflows
its area but fits its containing block is shifted back into it. Nothing
flips: a flip is `position-try`, still refused.

The row is a schema enum (`PositionArea`, bit 176). The web writes it as
the CSS declaration on both targets and the browser places the popover
(Chrome, the oracle, implements it with the invoker as the implicit
anchor; an engine without anchor positioning keeps its default centred
popover, and nothing in the host reimplements it). The UA sheet's
`[popover] { inset: 0; margin: auto }` needs no reset: with a
`position-area` Chrome 154 uses zero margins (measured: a 242×122 popover
under `top span-all` over an invoker at x 190, width 172, y 801 is at
155,679, centred and flush; without the row, margin auto centres it in the
viewport), and the fixture's `open-above-sheet` lays out the same way.
The painted top layers — iOS under the agent (`lift`)
and macOS's — place by the table, from one function (`PositionArea.swift`),
and macOS pops a menu up at the point it gives (the menu's size the box).
As CSS does, it aligns the popover's margin box, not its border box:
`position-area="top"` with `margin-bottom=12` leaves 12 points between the
popover and the invoker, and clamping keeps the margins inside the layer.
Margins are the kernel's resolved points (`auto` and a percentage are 0):
the Apple style encoder, which leaves margins out of every other box's
dictionary (the kernel placed the box), carries the four on a popover
with a `position-area` other than `none`, as it carries a dialog's insets.
The iOS sheet ignores them: UIKit places it.
The iOS sheet (`role="alertdialog"`, a `UIAlertController` popover) takes a
side from it: a `top` area permits only a down arrow, which UIKit places
above the source, `bottom` only an up arrow; a centred area anchors at the
whole invoker rather than its label, which UIKit centres on where it fits.
`center` anchors at the whole invoker too, with no arrow permitted and
`canOverlapSourceViewRect`, `none`'s arrowless presentation: UIKit centres
the sheet across the invoker (the probe above: anchored to the full row,
Block centres at x=81) and picks its vertical position itself, as for
`none`, shifted to stay on screen. It is not centred over the invoker in
both axes as CSS's `center` is; no public API does that short of a custom
`popoverBackgroundViewClass`.
A permitted direction makes UIKit draw its arrow toward the invoker; no
public API places a popover on a side without one short of a custom
`popoverBackgroundViewClass`, which this does not take on.
`none` keeps the arrowless placement measured above. `UIMenu` exposes no
public placement control: a menu-shaped popover presented as the system
menu is placed by UIKit whatever its `position-area`. Linux has no popover
presentation yet (`POPOVER_UNSUPPORTED`), so nothing there reads the row.
The native fixture's `open-above` (a menu) and `open-above-sheet` (a
sheet), in a row below the Detail screen's scroll, are the evidence: under
the agent, on iOS, macOS and in Chrome (both web targets), each opens above
the invoker, centred on it (the menu clamped to the left edge): the sheet
with its bottom at the invoker's top, the menu, whose `margin-bottom=12`
crosses the Apple encoder, 12 points above it (measured on iOS: bottom
665, invoker top 677; in Chrome: 789 and 801; macOS was driven before the
margin was added).

**D3 — menu-shaped popovers may present natively.** A popover whose
children are exclusively `button` rows (each with optional
`role="menuitem"`/`"menuitemradio"`, `aria-checked`, `disabled`, and text
content) and `hr` rows is *menu-shaped*. An Apple host presenting one
natively builds the platform menu from the **extracted data** — title:
the row's concatenated text; `aria-checked` → `UIAction.state = .on`;
`disabled` → `.disabled`; `destructive` → `.destructive` on iOS;
`hr` → an inline-section boundary — and renders
none of the subtree's own pixels; the invoker becomes the pull-down
(`UIButton.menu`, `showsMenuAsPrimaryAction`; `NSMenu` on macOS).
Selecting an item dispatches that row's `press` **by view id into the
runner** — the journal entry LLP 1012 defines, identical to a painted
tap — so state, tests, and the journal cannot tell the presentations
apart. A popover that is not menu-shaped always paints as kernel nodes in
D2's top layer, on every host. Presentation-only divergence is declared
here once, the way the native text field's keyboard already is (LLP 1008).

**D4 — the agent sees one presentation and needs no ninth op.**
Under `EXACT_AGENT=1` the Apple hosts present D2's kernel-painted layer,
never the native menu — the `MemoryStore` precedent (LLP 1018): agent
runs deterministic and capturable, a finger gets the platform. Nothing
else changes: `tap <item>` is a journal entry whichever presentation is
up; `tree` always holds the popover subtree (it is in the plan and laid
out), with the node reporting `{open}` the way layout reports scroll;
`screenshot` composes the open layer because in agent mode it is ours.
The eight operations stay eight (`rules/DEFERRED.md` §Agent API).

*As built on iOS (2026-10-03).* Under the agent a popover is hidden while
closed (it had been painted in place and taken taps meant for what lay
under it). Its opener's tap is delivered, then the popover opens in a top
layer above the page's root, anchored below the opener, and its
`autofocus` field is focused; a tap outside dismisses it before it is
delivered (and still presses what it lands on); its hide-only button
closes it after its own tap. `tree` reports `{open}` and `state.navigation.popover`
names it, as on macOS; `screenshot` composes the layer. `smoke.mjs ios
--app native-fixture` checks each step.

**D5 — every host, from one table.** Web: the attributes land on the real
elements and the browser does the rest (one glue rule for D2's anchoring).
Apple: D2's layer painted by the existing presenters, D3's native arm on
top for menu shapes. Linux: D2's layer and light dismiss over the pointer
it already has (VNC/evdev); no native arm, nothing declared absent — a
menu paints everywhere. Caltrain gains a menu only if it wants one; the
smoke's popover steps ride the fixture app of M1.

**D6 — what the switcher becomes (the acceptance).** Weird Castle drops
`state menuOpen` and `toggleMenu`; the mark button becomes
`button press=refreshAccounts popovertarget="account-menu"` (both fire,
D1); the menu column becomes `column id="account-menu" popover` holding
`button role="menuitemradio" aria-checked=a.active press=switchTo(a.username)
popovertarget="account-menu" popovertargetaction="hide"` rows — the
spec's own way to close on selection, which the native arm ignores
because `UIMenu` closes itself — then `hr`, Add account, Log out. On an
iPhone the mark drops a real pull-down with the current account
checkmarked; on the web it is a real popover with light dismiss; the
agent flow and the seeded-book CDP recipe keep working unchanged.

## 5. What v1 refuses, and what earns each back

- **`popover="manual"`** — the first surface that must stay open through
  outside interaction (a persistent panel). Menus are `auto`.
- **`command`/`commandfor`** (the general invoker vocabulary) — when a
  popover must be driven by something other than toggle/show/hide, take
  the newer names; `popovertarget` is the shipped subset.
- **CSS anchor positioning rows** (`anchor-name`, `position-anchor`,
  `position-try`) — a popover placed against something other than its
  invoker, or one that must flip when it does not fit. **`position-area`
  — earned back 2026-10-04** by an "Open in…" chooser at the foot of a
  card that must open above its invoker: a subset, on popovers only, with
  the invoker as the implicit anchor ("Placement", D2).
- **A destructive row — earned back 2026-09-10:** the original return
  condition was a web-standard name or a consumer measuring the miss.
  Messages' black Block/Discard actions supply the measurement. The existing
  swipe row is generalized as `destructive`; D2 records the native and web
  behavior and LLP 1001 §1 declares the non-web presentation hint.
- **Submenus** — `UIMenu` nests and ARIA allows it; nothing here needs
  it. First consumer brings the nesting rules.
- **The menu keyboard contract** (arrow traversal, typeahead, `role=menu`
  focus management) — Escape works (D2, the spec's dismissal); the rest
  arrives with the events lane (QUEUE §2), which owns keys generally.
- **The long-press context menu** (`UIContextMenuInteraction`) — content
  actions, not navigation; per the HIG never the sole path. **Earned back
  2026-10-05** (§5.1) by the Signal clone's chat list, whose long press
  Signal-iOS presents as the system menu with the conversation as its
  preview, and whose tap opens the same conversation (the other path).
- **A `select`-shaped value picker** — the customizable `<select>` is the
  web's other native-menu door; it is a form control with a value, a
  different contract. Its own line when a form needs one.

## 5.1 The context menu, its preview and its commit (2026-10-05)

**The consumer.** The Signal clone draws its chat-list long press: a
hand-built card of the conversation's latest messages over a blur, with
`contextTarget` morphing a copy out of the row (its DIARY, 2026-10-04:
"No native UIContextMenu preview for list rows (LLP 1021 §5 defers it)").
Signal-iOS does not draw it. `CLVTableDataSource.tableView(_:contextMenuConfigurationForRowAt:point:)`
returns a `UIContextMenuConfiguration` whose `previewProvider` is a
`ConversationViewController` in preview mode (`createPreviewController`) and
whose `actionProvider` is the row's actions as a `UIMenu`;
`willPerformPreviewActionForMenuWith` calls
`presentThread(animated: false)` inside `animator.addAnimations`, at the
default `.pop` commit style; `previewForDismissingContextMenuWithConfiguration`
targets the row's cell. The system lifts the row, places the menu, and
grows the preview into the conversation when it is tapped. None of that is
in the app's pixels, so none of it should be in the clone's.

Before this section exact2 had no context menu: `contextmenu=` on iOS was
a long-press recognizer that fired an action (`NodeViewIOS.openContext`),
and D3's native arm served only a button's pull-down.

**The web's names, and where they stop.** HTML's own declaration, the global
`contextmenu` attribute naming a `<menu type="context">`, was removed from
the standard (Firefox alone shipped it, and later removed it), and `contextmenu` here is
already the event (D1, LLP 1005). The Popover API supplies the rest: a
`popover` whose rows are `button`s and `hr` (D3's grammar) is the menu,
and a popover shown with a `source` element is anchored to it (the
implicit anchor `position-area` already uses). HTML's interest invokers
(`interestfor`) also open a popover on a long press. They do not fit,
because on a pointer they open on hover, which is not a context menu, and
macOS's menu comes from the secondary click. Two non-web rows join the
schema, beside `contextTarget` and `contextMagnify`:

- **`contextPopover="<id>"`** on any node (props 238): the popover its
  context menu shows.
- **`contextPreview=true`** on one row of that popover (prop 239): that row
  is the menu's preview, not an item. Its `press` is the **commit**, what
  tapping the preview does.

```
button press=open(t.id) contextmenu=menuFor(t.id) contextPopover="chat-menu" …
column id="chat-menu" popover="auto" role="menu" position="absolute" width=360
  button contextPreview=true press=open(menuChat) popovertarget="chat-menu" popovertargetaction="hide" height=420 …
    …the conversation's latest messages…
  button press=op("pin") popovertarget="chat-menu" popovertargetaction="hide"
    image "symbol:sf/pin"
    text "Pin"
  hr
  button press=op("delete") destructive=true popovertarget="chat-menu" popovertargetaction="hide"
    text "Delete"
```

The node's own `contextmenu` action fires first, then the popover is read:
D1's "both fire", in D6's order (an invoker's press refreshes its menu).
That ordering lets one popover serve every row of a list: `menuFor(t.id)`
records which row was pressed, and the popover's rows and preview are of
that row. The rows' `popovertargetaction="hide"`, the preview's included,
is the spec's way to close on a choice; the native arms ignore it, as D6's
switcher does.

**iOS.** A node carrying `contextPopover` gets a `UIContextMenuInteraction`
(`ContextMenusIOS.swift`), and its plain long press (the recognizer that
fired `contextmenu`) is disabled while it has one. UIKit owns the gesture
and its arbitration with scrolling, the lift, the haptic, placement, the
menu's glass and the dismissal. In the delegate:

- `configurationForMenuAtLocation` answers a configuration when the node is
  eligible (live, enabled, not inert, in the active route: the
  confirmation's test) and some popover has that id. It reads nothing else,
  because UIKit asks before the press has become a long press, and a tap
  must not fire `contextmenu`.
- The `actionProvider` and `previewProvider` run only when the menu is
  going to show. Whichever runs first fires the node's `contextmenu`, once,
  synchronously through the session (`Session.press`'s path), then reads
  the popover as that batch left it.
- The **menu** is D3's extraction, `MenuHost.items(of:)`, skipping the
  preview row: titles, item images, `aria-checked`, `disabled`,
  `destructive`, and `hr` as an inline section. The popover's `aria-label`
  titles it, as D3's pull-down. A chosen item presses its row by view id.
- The **preview** is the `contextPreview` row itself. It moves out of the
  hidden popover into a plain `UIViewController`'s view, at the origin,
  with `preferredContentSize` set to its laid-out box, the way the agent's
  painted popover moves into its top layer (D4). It is the kernel's box, and
  it is never re-rendered or snapshotted: an answer that lands while it
  shows updates it in place. Any frame a batch gives it (one at the origin
  too) is taken as its new home, and it is put back at the controller's
  origin. A children op keeps it out of the tree while it shows
  (`MenuHost.lifted`): one that keeps it records its index, one that drops
  it leaves it out, and one that moves it to another parent becomes its
  home. When the menu has ended (`willEndFor` for its own configuration,
  after its animator completes) it goes back there: into its popover's
  container as it is then, which a material change may have replaced, and
  hidden with the popover. A source that is unmounted or stops naming the
  popover while the menu shows ends it (`dismissMenu`), and the row goes
  back at once, without waiting for `willEndFor`. During a commit the row
  and the interaction stay until the commit is done. A press that did not
  navigate returns the row as it returns; a navigating one returns it in
  its animator's completion. A hidden row, or no row,
  means no preview, and UIKit's own lifted node is the preview. Two rows
  are refused, logged as `context menu <id> refused a preview: …`.
- The **targeted preview** (highlight and dismissal) is the node itself
  (`UITargetedPreview(view:)`), with a `visiblePath` for its
  `border-radius`, so the lift and the morph are Apple's. After a commit
  that navigated, the dismissal has no target: the node is under the new
  screen, as Signal's is under the conversation. After a press that
  stayed, the dismissal returns to the node.
- **The commit** (`willPerformPreviewActionForMenuWith`): if the preview row
  is still the popover's, enabled and has a `press`, it is pressed by view
  id, synchronously, with the navigation stack's own push animation off for
  that one batch (`NavigationHost.unanimated`, held until the stack applies
  the push when a transition already in progress defers it). The row must
  still be a row of the popover its source names, and the source still
  eligible. Neither may be disabled or inert in the authored tree: the
  lifted row is not in that tree, so its popover's ancestry is checked. If
  the batch changed the
  selected route (`activeKey`), the commit style is `.pop`, with an animation
  added (the pushed screen's layout): UIKit grows the preview into the screen
  the press pushed, as Signal's `presentThread(animated: false)` inside
  `addAnimations` gives it. At `.pop` with no animation added, UIKit shrinks
  the preview away as `.dismiss` would; the first build did that, and a
  recording on iOS 27 showed it. Otherwise the style is `.dismiss`.
  The style is read off the outcome. An author writes no
  `pop`/`dismiss` row, and a press that sometimes navigates gets the right
  one each time.
- **The focus while the menu shows** (`MenuFocusIOS.swift`). UIKit's menus
  take typing for type-to-select: as one shows, its key input
  (`_UITypeSelectKeyInput`) takes the place of a first responder that is not
  text. With no hardware keyboard attached, that raises the software
  keyboard over the menu's lower rows. Exact always has such a responder:
  `ExactView` holds the focus for key commands, and a pressed node takes
  it. A bare UIKit app on the same iOS 27 simulator shows its context menu
  with no keyboard, and shows the keyboard as soon as a plain `UIView` is
  first responder. Setting that responder aside as the menu shows removes
  the keyboard (measured both ways). So a context menu
  (`willDisplayMenuFor`) and an invoker's pull-down (`MenuButton`) set the
  session's focus aside as they show, with no `blur`, and return it with no
  `focus` once they have ended, unless something else has taken it. A text
  field's focus stays where it is. A bar item's pull-down
  (`NavigationBarIOS`) gives no hook early enough: UIKit decides from the
  focus it saw as the menu began, and setting the focus aside in the
  menu's rows provider is too late (measured). So a touch on a navigation
  bar whose items hold a menu sets the focus aside as it begins
  (`BarTouch`, a recognizer that recognizes nothing). The menu says nothing
  when it ends, and a focus returned while it shows lets its input take it
  again, raising the keyboard (measured: returned 5 s after the touch, the
  keyboard rose). So the view's own focus comes back at the next touch on
  the page (a recognizer on the viewport, which a presentation carries
  along), and a focused node is blurred, `blur` dispatched, as a touch on a
  page's chrome blurs an element on the web, the view then taking the focus
  at the next touch on the page. A node that resigns hands the focus to
  `ExactView`, its nearest ancestor that takes it (UIKit's fallback,
  measured), so every set-aside releases the view too. Which bar item was touched is
  not known (UIKit exposes no bar item's view), so any touch on such a bar
  does this. The bar's recognizer is installed as each stack is prepared,
  rebuilt stacks included. A bare UIKit app's
  bar item behaves the same way: no keyboard without a first responder,
  the keyboard with one.
- `state.navigation.popover` is `{kind: "contextmenu", source, popover,
  preview, phase}` once the menu is showing (a provider has run), with
  `phase` `commit` after a commit. A configuration UIKit asked for that
  ended as a tap is never reported.
- **A row with a context menu parks in the node pool** (`NodePoolIOS`, a
  virtualized `list`'s rows; 2026-10-05). The pool had refused any view with
  interactions or gesture recognizers, the `contextmenu` long press
  included. It now admits a context menu's own while they are at rest: the
  long press in `.possible`, the interaction, and the eight recognizers and
  two interactions UIKit adds with it, which the host records as it adds it.
  The interaction stays on the view while it is parked, no node's: the
  delegate reads the node from the view, and a parked view is hidden. At
  the next sync it becomes the new node's, if that node names a popover,
  and otherwise comes off with the long press back on. Removing and adding
  it would cost much of what pooling saves. A tree that leaves the pool
  takes its interactions with it. UIKit leaves a click-presentation
  feedback generator behind when only the menu's interaction is removed
  (measured), so the host removes the ones it recorded too. A focus a menu
  set aside goes back only to the node it was taken from, never to a node
  the view was lent to since. A row whose menu is up, or whose long press
  is judging a touch, is not parked: it is destroyed as before, and its
  menu ends as any unmounted source's does. Measured on the native fixture's 200-row Rows list, every
  row naming a popover, reuse on: eight flings, three runs each, a
  measurement-only Save Trace on SIGUSR1. Before, 0–2 takes. After, 132–187
  takes, and batch apply over the 10-second window fell from 358–441 ms to
  261–298 ms, with p90 per batch from 2.1–2.6 ms to 1.9–2.2 ms. Neither had
  a late frame on the simulator. The Signal clone's chat list is not a
  virtualized `list`, and its rows are swipe rows that the pool refuses for
  their live scroll view, so this does not reach it.

**macOS.** A secondary click on the node fires its `contextmenu` (as it
did), then, on the next turn, pops up the popover's menu rows as an
`NSMenu` at the click (`MenuHost.context`, `menu(of:)`), the preview row
skipped. A Mac's context menu has no preview, and the HIG offers none. A
picked item presses on the next turn, once, by the button menu's rules (no
invoker to recheck). `isMenuShaped` ignores the preview row.

**Web (both targets).** The node's `contextpopover` attribute is wired
(`rt.js` `cp`, `glue.js`). Its `contextmenu` prevents the browser's menu,
and after the node's own action the popover opens with
`showPopover({source: node})` (not for a disabled or inert node, and no
ancestor hears the event, as macOS consumes the click): anchored to the node and placed by its
`position-area` (D2's table), with light dismiss and Escape. The preview
row is an ordinary row (`data-context-preview`), drawn as a card above the
items, and tapping it presses it. That is the fallback the clone drew by
hand. Chrome on Android sends `contextmenu` for a long press, so the same
path serves a phone there; Safari on iOS has not been tried. A field's own
edit menu stays the browser's. The popover is resolved in a timeout after
the node's action. A bound `contextPopover` that the action itself changes
therefore opens the popover it named before when that page defers its
commit: a view transition on the JS target (`shared.commit`), or the
presence module still loading on the wasm host. QUEUE has this.

**Linux** has no popover presentation (`POPOVER_UNSUPPORTED`, §4), so
`contextPopover` shows nothing there and the node's `contextmenu` still
fires.

**The agent (D4).** Under `EXACT_AGENT=1` there is no interaction. `tap
<node> contextmenu` fires the node's action and opens the popover painted
in the top layer, anchored to the node (`MenuHost.agentContext` on iOS,
`show` on macOS, the browser's own on the web). It works on a node with
no `contextmenu` action too. `tree` reports it `{open}`, with the preview
row and the items as nodes the agent reads, `screenshot` composes it, and
`tap peek-preview` commits: it presses the preview's row, which navigates,
and its `hide` closes the popover. The eight operations stay eight.

**Evidence.** The native fixture's `peek` row (Home): a context menu whose
`contextmenu` counts openings, a 300×180 preview whose press pushes Detail,
then Bump, an `hr`, and a destructive Clear. `ContextMenuIOSTests` (UIKit,
on a simulator, calling the delegate as UIKit would): the interaction
replaces the recognizer; nothing fires at configuration; `contextmenu`
fires once, from whichever provider runs first; the preview row is the
controller's view at its laid-out size; the menu is Bump | Clear with the
destructive attribute; the highlight targets the node; the commit
presses, pushes, and answers `.pop` with one animation and no dismissal
target; a press that stays answers `.dismiss` and returns to the node; a
dismissal returns the row to its hidden popover.
`ContextMenuLifetimeIOSTests`: a frame op while lifted (origin unchanged,
height changed) resizes the preview and is where the row returns; a source
that stops naming the popover ends the menu and returns the row at once; a
configuration that never shows reports no menu; a row dropped from its
popover and given to another parent returns there; an inert popover
refuses the commit. `ContextMenuMacTests`: the `NSMenu` omits the preview
row and presses a picked item on the next turn. On the simulator (iOS 27,
real touches through `axe`): the row lifts, morphs into the preview with
Bump and Clear below, and tapping the preview grows it into Detail. After
the focus fix, no keyboard rises over the context menu or the "Open in
Maps" pull-down, and `ExactView` is first responder again after each
(read with lldb). The
agent opens, reads and commits the painted popover on iOS and in Chrome
(`tap peek contextmenu`, `tap peek-preview`: `peeks 1`, then Detail's
route). `MenuHost.context` and the right-mouse path on macOS were not
driven: `popUp` tracks the menu modally, and no test reaches it.

**Still deferred.** The preview's own interactivity: UIKit takes every touch
on a preview as the commit, as QUEUE's reaction-picker note found, so
the Signal message menu's reaction bar is not a preview, and stays
authored (a reaction bar beside the menu is its own work). Submenus (as §5). A preview for a node that does not name a
popover. macOS's preview (none exists). Linux's popover presentation. The
keyboard route to a context menu (Shift+F10, the context-menu key; QUEUE's
"Keyboard access to a message's context menu"). Compile-time checks: a
literal `contextPopover` naming no popover, or two `contextPreview` rows,
is not refused by the compiler. iOS logs both at the press, macOS logs a
missing popover, and the web logs neither.

## 6. Delivery

- **M1 — web + kernel.** The five rows and the `hr` tag in
  `kernel/tables/schema.json` + `tags.rs`; the hidden layer and D2's
  anchoring in the kernel; the web host passing the attributes through
  and the one anchoring rule; smoke: open by `tap`, light-dismiss by
  tapping outside, `tree` shows `{open}`, screenshot fixture. The oracle
  exists the day M1 lands.
- **M2 — Apple + Linux.** D2's painted layer on the three native
  presenters (agent mode always this); D3's UIMenu/NSMenu arm behind the
  menu-shape test, selection dispatching by view id; Linux light dismiss.
  Boot metrics unchanged (`metrics.mjs` — nothing loads for a plan with
  no `popover`).
- **M3 — the switcher adopts (weird-castle).** D6 verbatim, driven on
  web/macOS/iOS by `exact.mjs agent` and the seeded-book recipe; a
  finger on the iPhone gets the pull-down. An RFC for a control is proven
  by a control.

Verification is the standing recipe: the five checks, `smoke.mjs` per
host, the CDP recipe for the live web page.

## 7. Open questions

- **Q1 — where `{open}` reports.** `tree` (a prop) or `layout` (beside
  scroll's `sx`/`sy`, the host-state precedent)? D4 says tree; the
  implementer may find layout truer. Decide in M1.
- **Q2 — `press` + `popovertarget` ordering.** The spec fires both; the
  order (action before toggle?) matters to D6's refresh-as-it-opens.
  Verify against the dev-loop browser in M1 and write the order down.
- **Q3 — the one-auto rule across surfaces.** One auto popover at a time
  is per-document on the web; with a future second window (macOS) it is
  per-what? Per-presenter, presumably. Record when a second window
  exists.
- **Q4 — native presentation opt-out.** Does an app ever *refuse* the
  native arm for a menu-shaped popover (brand styling over platform
  chrome)? If a consumer asks, the web-true dial is
  `appearance: base-select`-adjacent territory — name it then, not now.
