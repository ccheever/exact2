---
name: 20261010-shell-context-menu
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-shell-context-menu'
pr_url: https://github.com/ccheever/exact2/pull/407
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

## The reference, case by case

Measured on the running reference (lane `shell-context-menu`, base 16660) over CDP: the renderer's real right-click
(Playwright's right button raises Chromium's `contextmenu`), and the shell's own menu read back from the main process
through its Node inspector (`Menu.prototype.popup` recorded instead of popped, the `context-menu` params logged); a
Browser page's cases as right mouse events in the `<webview>` guest's coordinates. The clipboard was empty.

- No selection, an empty area (timeline, sidebar): Cut, Copy, Paste disabled; Select All.
- Text: Chromium on a Mac selects the word under the pointer first, so Copy is enabled ("Known", "parser" — an `<a>`
  with no `href`, so no Copy Link).
- A reply's web link (`https://…`): the app's own menu (ChatMarkdown's anchor `onContextMenu`): Open in integrated
  browser, Open in system browser, Copy Link (Link/Unlink from thread first for a pull request the thread can link).
  The shell's Copy Link appears only over a link with no menu of its own (a Browser page's link).
- An image with no menu of its own (the work group's tool icon): Copy Image, a separator, the four roles (Copy disabled).
- The composer (a contenteditable that checks spelling): empty: Cut, Copy, Paste disabled, Select All; a misspelled word:
  it is selected and up to five suggestions lead ("chek": check, chef, chew, chez, cheek); a word: Cut and Copy.
- A Settings text field: empty: all four disabled (Select All too); a word: selected, Cut and Copy.
- A Browser page: the same rules (text, Copy Link, Copy Image, a field, an empty field with Select All disabled, an empty
  area); a page that prevents `contextmenu` gets no menu.
- App-authored menus (a thread row, the chat header, a file chip) are the app's; the shell's never follows.

Full list with the raw reads: [menus-before-after-reference.txt](https://raw.githubusercontent.com/ccheever/exact2/8ffce368a55f67fee24545bb23f687bdc2e465b4/shell-context-menu/menus-before-after-reference.txt)
(tools: [ref-main.mjs](https://raw.githubusercontent.com/ccheever/exact2/6205a72de5891549ced4f61a0baa17f28d2bb912/shell-context-menu/ref-main.mjs.txt),
[ref-rc.mjs](https://raw.githubusercontent.com/ccheever/exact2/aedf309398cef376e4cbf6f707e9d1400d5f304b/shell-context-menu/ref-rc.mjs.txt),
[ref-guest.mjs](https://raw.githubusercontent.com/ccheever/exact2/7920e29bf4d18c1b13f3392bd4e0e3fe05a62b8d/shell-context-menu/ref-guest.mjs.txt),
[ref-app-inspect.sh](https://raw.githubusercontent.com/ccheever/exact2/6c7dca9356bbceae634861f637fde477217f4aa9/shell-context-menu/ref-app-inspect.sh.txt),
[the Browser fixture page](https://raw.githubusercontent.com/ccheever/exact2/556f05ebd1336d23217e609fdc61c24008975d76/shell-context-menu/fixture-page.html.txt)).

## Built

- `T3ShellMenu.swift` (new): DesktopWindow's template from Electron-like params (suggestions or "No suggestions", Copy
  Link for a `parseSafeExternalUrl` link — http(s) or a remote editor's SSH link, copied as Chromium's canonical URL —,
  Copy Image of the image's own bitmap, then Cut, Copy, Paste and Select All with ⌘X/⌘C/⌘V/⌘A); `T3ShellImageView` for
  an app-drawn image.
- `T3TextContextMenu.swift`: the right-click monitor now routes every click. A text input (a field, a textarea, the
  composer): the click ends in the monitor, the input takes the focus, the word or misspelling under the pointer is
  selected unless the click is inside the selection, and the menu follows the input's state (NSSpellChecker's guesses
  where the input checks spelling, replaced through the input's own editing path; Paste with something on the
  clipboard; Select All off in an empty text control, kept in the composer and the prompt sample, the reference's
  contenteditables; a secure field never cuts or copies). Selected page text keeps RD-4's menu (now from the shared
  template). Anywhere else the click goes on to ExactKit: a node with a `contextmenu` or context popover ends it (the
  app's menus are untouched), and an unanswered click reaches `T3ShellMenuTail`, a responder placed after the window's
  content view, which pops the page's menu (Copy while page text is selected, from ExactView's Start Speaking
  validation; Copy Link from an inline link run's accessibility URL; Copy Image from an image layer's bitmap or a
  `T3ShellImageView`). Web views answer their own clicks. Under the agent every menu goes to the log (`t3.textmenu:`).
- `T3ShellWebView.swift` (new): the Browser's pages and popups (`T3BrowserWebView` now subclasses it) and an HTML
  attachment's preview (`R6MediaPreview`) rebuild WebKit's menu in `willOpenMenu`: WebKit's first five spelling guesses
  (or "No suggestions"), its Copy Link and Copy Image items (they act on WebKit's hit element), then the four roles on
  the page; Select All is turned off in an empty field from a page read.
- `T3ToolActivityIcon.swift`: the tool icon answers Copy Image (`T3ShellImageView`).
- `T3ContextMenu.swift`: under the agent the module menu's items go to the log (`t3.contextmenu:`); nothing else changes.
- `composer.contract`, `settings-prompt-preview.contract`: the composer and the Appearance prompt sample check spelling
  (`spellcheck="false"` removed): the reference's editor sets `spellCheck={false}` only on its chips, and its composer
  underlines misspellings. (The X16 record's line "the reference sets spellCheck={false} on the composer" misread that.)
- `external-link-menu.ts` (new, port of `externalLinkContextMenu.ts`), `markdown.contract`, `app-window.contract`,
  `timeline-presentation.ts`, `pages-pr-links.ts`: a reply's web link has the reference's own menu
  (`chatlocal:link-menu`): Link or Unlink from thread for a pull request (`chatLinkThreadAction`, `changeChatLink`, with
  the chat's toast titles), Open in integrated browser where a thread can show it, Open in system browser, Copy Link.
  The plan grows by 0.94 MB (27.29 to 28.23 MB) for the handlers on the link nodes.
- `EXACT2-GAPS.md`: "Text context menu" rewritten (the workaround, what it leans on, gaps S1–S3 with one-file repros);
  Browser X1 path B gains the context menu row.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Each case's items and enabled state match the reference | 17 of 20 driven cases match (empty timeline and sidebar, a reply's link, a thread row's own menu, selected text, the composer empty / misspelled / a word, the Browser page's text, link, image, field, empty field, own menu and empty area, the Settings field empty and a word); 2 differ by S1 (unselected text and "parser": Copy disabled, the reference selects the word first). The tool icon (Copy Image) showed no image in the drive and was fixed after it (`T3ShellImageView`, AppKit row); its live check is a real-input step. AppKit: 24 new rows (page, input, template, Browser page with a real WKWebView) | [per case](https://raw.githubusercontent.com/ccheever/exact2/8ffce368a55f67fee24545bb23f687bdc2e465b4/shell-context-menu/menus-before-after-reference.txt), [composer](https://raw.githubusercontent.com/ccheever/exact2/1e24515f6482a018d012edd46e34af8048c1bfaf/shell-context-menu/composer-spellcheck.png), [drive](https://raw.githubusercontent.com/ccheever/exact2/26868a18df7279dac231f53fd297e9c36bd0af6f/shell-context-menu/drive.sh.txt) |
| App-authored menus unchanged | pass: the thread row's popover and the file chip, header and Files menus keep their own; `contextmenu` AppKit's 24 earlier rows pass unchanged (RD-4's five included), `context-menu-hookup.test.ts` passes | drive C7 in the per-case file; checks below |
| Drawn menus and their actions | open: real-input steps below | — |

The before build is the evidence worktree at `c03d7e908`; it shows nothing where the page answers no click, and pops
AppKit's or WebKit's own menu in inputs and Browser pages, which tracks in the agent's never-key window, so those cases
ran on the after build only (their before is the AppKit/WebKit menu, listed in the per-case file). The after build made
two drives: in the first the reply link answered with the shell's menu and no Copy Link (S3), which led to building the
reference's own link menu, and it stopped at a Settings step; the second, on the merged head, ran every case.

## Tests

- `macos/tests/contextmenu/shell-menu.swift` (new, 24 rows): the page (empty area, a selection elsewhere and the roles'
  targets, a node with its own `contextmenu`, an inline link and its canonical copy, an unsafe link, an image layer's
  bitmap, an app-drawn icon, RD-4 through the template, the agent's log, `destroy()` taking the tail out, a window with
  no page), inputs (an empty field, a word in a field, past the text with Paste from the clipboard, five suggestions and
  one replacing the word, "No suggestions" and spellcheck off, a click inside the selection, an empty textarea against
  the composer, a secure field, the agent), the template (safe URLs, order), and a real WKWebView (each page case,
  WebKit's guesses, `reshape`). All clicks go through `install()` and `NSApp.sendEvent`.
- `external-link-menu.test.ts` (new, 11): `externalLinkContextMenu.test.ts`'s cases ported; the link nodes carry the
  handler and the root names the op; `chatlocal:link-menu` reaches the native menu with the reference's items and Copy
  Link copies the href.

## Checks (head `73f0e32a0` with the EXACT2-GAPS wording staged; `origin/feat(example)/t3-code` `8d69a4329` merged)

`bun test examples/t3-code --timeout 60000` 0 (4384 pass, 1 skip, 0 fail); strict `tsc` 0; `contract build
examples/t3-code/app.contract` 0 (5982 slots, 28,229,553 bytes); `cargo test -p t3-code-macos --lib` 0 (17 pass); AppKit
`contextmenu` 48 run, 0 failed (24 new), `browser-capture` 34/0, `media-actions` 7/0, `r6-media` 10/0; `git add -A && bun
scripts/caps.mjs` 0; `cargo build --all-targets --keep-going` 0; `cargo test --lib --bins --tests --no-fail-fast` 0
(3679 passed, 0 failed, 34 ignored); `cargo clippy --all-targets --keep-going -- -D warnings` 0; `cargo fmt --all --
--check` 0; `bun scripts/boot.mjs` 0. The bundle was built before the live drives (exit 0, on the merged head
`77cc60949`). `app.contract`: 1,344 lines. Later commits change only this record.

## Real-input batch steps

On the T3 Code (Exact) app built from this branch, in a normal (non-agent) launch, the Verification fixture thread open,
the clipboard empty first:

1. Right-click an empty part of the timeline: a native menu with Cut, Copy, Paste disabled and Select All; pick Select
   All: the page's text is selected. Right-click again on the selected text: Copy enabled; pick it and paste into
   TextEdit: the text arrives. No second menu follows either menu.
2. Right-click the work group's tool icon (the small square left of "Ran 2 commands…"): Copy Image first; pick it,
   paste into Preview (File ▸ New from Clipboard): the icon's image.
3. Right-click "linked" in "linked $verify": Open in integrated browser, Open in system browser, Copy Link; pick Copy
   Link and paste: `https://example.test`. Pick Open in integrated browser: the Browser opens beside the thread.
4. Composer: type `Plese chek the parsr today`; the misspelled words are underlined. Right-click "chek": check, chef,
   chew, chez, cheek, a separator, Cut, Copy, Paste (disabled), Select All; pick "check": the word is replaced. Copy
   something, right-click the composer: Paste is enabled; pick it: it pastes.
5. Settings: right-click the empty search field: all four disabled. Type `theme`, right-click it: "theme" is selected,
   Cut and Copy enabled; pick Cut: the field is empty and the clipboard holds "theme".
6. Browser panel: open `https://example.com`; right-click the "More information…" link: Copy Link first (paste: the URL);
   right-click the heading text: the word is selected, Copy enabled; right-click an empty area: Cut, Copy, Paste
   disabled, Select All. No WebKit items (Look Up, Translate, Share…) appear.
7. A thread row's right-click still opens the app's thread menu, and nothing else opens after it.

## Not done / not verified

- S1, S2, S3 (EXACT2-GAPS.md "Text context menu"): framework gaps without an issue number, not filed (the brief files
  nothing upstream). S1: a right-click on unselected page text selects no word, so Copy stays disabled where the
  reference enables it. S2: a link in a plain paragraph (drawn inline in one text node) cannot carry the app's link menu
  on macOS and shows the shell's (with Copy Link). S3: a node's `href` is not readable by a module, so the clone's links
  without a menu of their own get no Copy Link. Blocker: the decision below.
- Drawn menus and their actions: the real-input steps above (the screen stays locked; open until the batch runs).

## Decision needed

Should the coordinator file S1–S3 (ExactKit: select the word under a secondary click; dispatch `contextmenu` for an
inline run; expose a node's `href` to modules)? Each has a one-file repro in EXACT2-GAPS.md. If they are filed, the
entries take their numbers; otherwise they stay declared differences. Nothing else in this task depends on it.

## Delivery

Draft PR [#407](https://github.com/ccheever/exact2/pull/407) into `feat(example)/t3-code`.

## Next action

The coordinator reviews and merges the draft PR, then runs the real-input steps in the next batch.
