---
name: 20261010-shell-context-menu
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
verified_commit: null
---

# The desktop shell's context menu wherever the page shows none

## Outcome

T3 Code's desktop shell (`apps/desktop/src/window/DesktopWindow.ts:529-604`, `installContextMenu`) answers every
`context-menu` event that the page did not take. The renderer's own menus (thread rows, drafts, file rows, …) call
`preventDefault`, so they never reach it. The shell's menu, in order:

1. On a misspelled word: up to five dictionary suggestions (replace the word), or "No suggestions" (disabled); a separator.
2. Over a safe external link (`parseSafeExternalUrl(linkURL)`): Copy Link; a separator.
3. Over an image: Copy Image; a separator.
4. Always: Cut, Copy, Paste, Select All, each enabled by Chromium's edit flags (`canCut`, `canCopy`, `canPaste`,
   `canSelectAll`).

It is installed on the main window, on windows the page opens, and on every attached `<webview>` (the Browser panel's
pages).

[realinput-1010d-followups](20261010-realinput-1010d-followups.md) RD-4 (#400) built the read-only selected
text case: a local right-click monitor (`T3TextContextMenu.swift`) replaces ExactKit's read-only text menu (Look Up,
Copy, Speech, Services) with Cut (disabled), Copy, Paste (disabled), Select All. Elsewhere the clone still differs:
a right-click on unselected text or an empty area shows nothing, a link shows ExactKit's or nothing, the composer and
other text fields show AppKit's editing menu, and the Browser panel's `WKWebView` shows WebKit's menu.

## Steps

1. Confirm each case on the live reference over CDP (`target/t3-audit/ref-app.sh`): unselected text, an empty area,
   a link in a reply, an image (a Markdown image or the Files preview), the composer with and without a selection and
   with a misspelled word, a Settings text field, and a page in the Browser panel. Record the menu items and their
   enabled state for each.
2. Extend `T3TextContextMenu` (or a sibling) so that a right-click with no app `contextmenu` handler under it shows the
   shell's menu with the same items and enabled state: Copy Link copies the URL, Copy Image copies the image, Cut and
   Paste work in editable fields, Select All selects the field's or the node's text, and spelling suggestions replace the
   word (NSSpellChecker, the field's language). Leave every app-authored menu as it is.
3. The Browser panel's pages: replace WebKit's menu with the shell's (`WKUIDelegate` / `willOpenMenu`), with Copy Link
   and Copy Image from the page's hit test.
4. Where the clone cannot reach a case from app code (a host-owned view with no hook), record it in `EXACT2-GAPS.md`
   with a one-file repro and leave the framework alone.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Each case's items and enabled state match the reference | AppKit tests of the menu builder per case; one agent drive logging the menu per case (the agent draws no menu) | text before/after per case, images where agent mode shows them |
| App-authored menus unchanged | the existing context-menu tests | test output |
| Drawn menus and their actions | exact real-input steps for the next session (they stay open until it runs) | — |

## Next action

Start now.
