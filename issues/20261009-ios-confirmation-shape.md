# An iOS confirmation is an arrowed popover that drops its Cancel, and nothing can raise one from an action

**Status:** Open
**Systems:** host/apple iOS, menus (LLP 1021), Contract actions
**Severity:** P2
**Author:** Claude (Tuft), for Charlie Cheever
**Date:** 2026-10-09
**Related:** host/apple/Sources/ExactKit/IOS/MenusIOS.swift:88, llp/1021-menus.rfc.md (The chooser), docs/contract-grammar.md:1243

Found by an Expo → Exact conversion benchmark (2026-10-08: the same 612-line Expo Router app converted by agents to SwiftUI and to Exact, then graded on the iOS Simulator). Both Exact ports lost points here, and both agents named it among what cost them the most time; the SwiftUI ports did not.

**1. The confirmation's shape.** The original is React Native's `Alert.alert("Remove book?", "\"Piranesi\" will be removed…", [Cancel, Remove(destructive)])` — a centred alert. The Exact port writes the documented confirmation: a `popover="auto" role="alertdialog"` with a text row, a destructive action and a handlerless cancel. On iPhone, `MenusIOS.Confirmation` presents a `UIAlertController(.actionSheet)` adapted to `.none`, i.e. an arrowed popover anchored at the invoker. UIKit omits a `.cancel` action from an action sheet shown as a popover, so the author's Cancel never appears (graders: "its Cancel button isn't drawn", "not in the accessibility tree"), and per LLP 1021 a confirmation keeps no title row, so `aria-label="Remove book?"` is not shown either. Tapping outside cancels. Both graders scored it as a bug.

This is by design today (LLP 1021: "a confirmation keeps no title row, as the native prompts it matches have none"), so it needs a decision rather than a patch. Options: present an author-declared alert as `UIAlertController(.alert)` (centred, title + message, Cancel kept) — e.g. a `dialog role="alertdialog"` opened by `showModal`, or the popover form with no anchor; or keep the popover and document that Cancel is dropped on iPhone so authors stop declaring one.

**2. No confirmation from an action.** The original confirms after a long-press (`onLongPress → Alert.alert`). In Exact a confirmation opens only from its invoker button (`popovertarget`). A `contextmenu` item cannot open one, and `showModal(id)` (`contract vocab`, grammar §actions) is taken only by the terminal host (LLP 1101.001 P5). Both agents found `showModal`, tried it, and fell back to a context-menu item that deletes without confirming. Two graders marked checklist item 9 PARTIAL for this.

Acceptance: an app can show a centred alert with title, message, Cancel and a destructive action on iOS, opened either from a button or from an action (a context-menu item's `press`); the iOS host honours `showModal` for that dialog; `docs/contract-for-agents.md` shows the recipe.

Benchmark artefacts (on the Tuft machine that ran it): `~/bench/convert-eval/` (REPORT.md, runs/exact-{1,2}, grading/exact-{1,2}).
