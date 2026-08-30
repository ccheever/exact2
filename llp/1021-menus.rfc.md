# LLP 1021: Menus — the popover, its invoker, and the native pull-down

**Type:** RFC
**Status:** Draft
**Systems:** Kernel (a top layer; the popover's box anchored to its invoker), Contract (four attributes, one tag — all by their HTML names), Web host (the Popover API by identity), Apple host (menu-shaped popovers presented as UIMenu/NSMenu), Linux host (a kernel-painted top layer), Agent API (no ninth operation), Weird Castle (the account switcher, first consumer)
**Author:** Claude (Fable 5) for Charlie Cheever
**Date:** 2026-08-30
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
  bare self (a rule).
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
anchor positioning (`position-area`) is the vocabulary if an app later
needs placement control; not in v1 (§5). The web host applies the same
one anchoring rule from the invoker's box, so the oracle and the kernel
agree by construction until implicit anchors are universal.

The top layer sits **outside every canvas capture** — the web's top layer
cannot be sampled by anything on the page, and that is the parity: a menu
over Weird Castle's night floats above the sky, not inside its texture
(today's switcher is sampled into it, LLP 1014). Menus read as chrome on
every platform; this makes it so here.

**D3 — menu-shaped popovers may present natively.** A popover whose
children are exclusively `button` rows (each with optional
`role="menuitem"`/`"menuitemradio"`, `aria-checked`, `disabled`, and text
content) and `hr` rows is *menu-shaped*. An Apple host presenting one
natively builds the platform menu from the **extracted data** — title:
the row's concatenated text; `aria-checked` → `UIAction.state = .on`;
`disabled` → `.disabled`; `hr` → an inline-section boundary — and renders
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
The eight operations stay eight (`rules/NOT-DOING.md` §Agent API).

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
- **CSS anchor positioning rows** (`position-area`, `anchor-name`) — the
  first popover that cannot live at the invoker's bottom-left. D2's one
  rule until then.
- **A destructive row** — the HIG renders Log out red; the web has no
  name for it (`aria` has no destructive state, HTML no attribute). The
  rule that every row is a web name outranks the red; Log out sits last,
  behind an `hr`, undecorated natively. Earn-back: a web-standard name
  appearing, or a consumer measuring the miss.
- **Submenus** — `UIMenu` nests and ARIA allows it; nothing here needs
  it. First consumer brings the nesting rules.
- **The menu keyboard contract** (arrow traversal, typeahead, `role=menu`
  focus management) — Escape works (D2, the spec's dismissal); the rest
  arrives with the events lane (QUEUE §2), which owns keys generally.
- **The long-press context menu** (`UIContextMenuInteraction`) — content
  actions, not navigation; per the HIG never the sole path. The first
  content surface (the deck list?) earns the trigger, as a second way to
  open the same declared menu.
- **A `select`-shaped value picker** — the customizable `<select>` is the
  web's other native-menu door; it is a form control with a value, a
  different contract. Its own line when a form needs one.

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
