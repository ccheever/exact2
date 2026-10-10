---
name: 20261010-realinput-1010g-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-realinput-1010g-followups
pr_url: https://github.com/ccheever/exact2/pull/419
verified_commit: null
---

# Findings of the real-input session realinput-1010g (the shell context menu)

## Outcome

The session `realinput-1010g` ran [shell-context-menu](closed/20261010-shell-context-menu.md)'s (#407) real-input steps
1–8 on the bundle of `6bac646cc`. Passed: steps 1, 3, 4, 6, 7 and 8. This task takes what did not pass. Each row is
compared with the reference first (`DesktopWindow.ts` `installContextMenu`, CDP plus the main-process inspector, as
#407 measured it); a row that matches it closes as such. Session notes:
[G0](https://raw.githubusercontent.com/ccheever/exact2/eb35a6a4fa4d21f56032512841efaccfadb772ac/realinput-1010g/G0-1010g-notes.txt).

Start after [realinput-1010f-followups](20261010-realinput-1010f-followups.md) merges: its RF-1 takes "Services ›" out
of the shell's menu in the same files.

## Findings

| Id | From | Clone under real input | Evidence |
| --- | --- | --- | --- |
| RG-1 | #407 step 2 | A right-click on a tool row's icon (the blue square left of "Ran 2 commands…") shows only Cut/Copy/Paste/Select All, with no Copy Image (3 tries, two layouts). The input log shows the click on a plain view with no id, and the AX tree has no image element there. #407's after drive did not cover the icon either (its fix landed after the drive). | [G2](https://raw.githubusercontent.com/ccheever/exact2/a371438dcba003b57d9cca271518c553f0266362/realinput-1010g/G2-FAIL-tool-icon-shell-menu-no-copy-image.png) |
| RG-2 | #407 step 5 | In Settings' search field, after typing `theme`, the right-click itself changed the field to "Theme". Cut and Copy were enabled; picking Cut (2 tries) closed the menu and left "Theme" in the field and the clipboard empty. The empty field's menu (all four disabled) passed. | [menu](https://raw.githubusercontent.com/ccheever/exact2/dee2bd39efc48478bb328c2f1b8363439f7314c1/realinput-1010g/G5-2-theme-menu.png), [after Cut](https://raw.githubusercontent.com/ccheever/exact2/9e259fa84efe3e90a34473d5b609c991e869c506/realinput-1010g/G5-3-FAIL-after-cut-unchanged.png) |
| RG-3 | #407 step 4 | The composer's spelling menu has "AutoFill" (and "Services ›", RF-1) after Cut/Copy/Paste/Select All. The reference's menu has neither. | [G4](https://raw.githubusercontent.com/ccheever/exact2/d39ff8da68d014c55d085790a4858eb881c0317b/realinput-1010g/G4-1-spelling-menu.png) |
| RG-4 | X78 check (#412) | A reply's paragraph links are drawn word by word as `text` nodes with an `href` and a `press` (FlowRuns). A `text` node with its own `href` is not a link in the accessibility tree on either host, so these links may not read as links at all; the reference's are `<a>` elements (AXLink with AXURL in Chrome). | [X78 record](https://github.com/ccheever/exact2/pull/412) |

## Steps

- RG-1: find what the reference's tool icon is (an `<img>`, an inline SVG or a CSS box) and what its right-click shows
  (`mediaType === "image"` gives Copy Image). If it is an image there, make the clone's icon carry what the shell menu
  needs (an image node, or the module's hit test over it), and copy the same image data. If it is not an image there,
  the clone's menu already matches: close the row.
- RG-2: find why a right-click capitalizes the word (the selection of the word under the pointer, then a text
  substitution or autocorrection replacing it; `NSSpellChecker` or the field's automatic text replacement) and why Cut
  does nothing in this field (the action's target, the field editor, or a field that is not the first responder when
  the menu acts). Compare with the reference (CDP: the same field, right-click, Cut). Fix.
- RG-4: check the clone's reply links in the accessibility tree on both hosts (`agent.mjs web|macos tree --ax`, and from another process on macOS) against the reference's (`<a>`, CDP `Accessibility.getFullAXTree`). If they are not links, make them links where Contract allows it (a `link` node, or `role="link"` with the href) without changing their look or menus; a host part is X78 (main #412).
- RG-3: AppKit adds AutoFill (and Services) to a menu it pops for a text view. Take AutoFill out where the shell's menu
  is built, with RF-1's change. Check whether "Writing Tools" or other system items appear on macOS 26 and take them out
  too.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RG-1..RG-4 | reference comparison first; an AppKit test of the menu builder and of the cause where one can be written; agent drive where agent input reaches it; exact real-input steps for the next session (they stay open until it runs) | before / after image where agent mode shows it |

## Cause and fix

- RG-1: two causes. (1) The work group header's timestamp (`timeline.contract`, TimelineRowTimestamp) is an absolute box at
  opacity 0 until the row is hovered, and it lacked the reference's `pointer-events-none` (`MessagesTimeline.tsx`:
  `pointer-events-none absolute … opacity-0 group-hover/timeline-row:pointer-events-auto group-hover/timeline-row:static`),
  so it sat at the row's start over the icon and took a right-click there (the first after drive showed it: three
  right-clicks on the icon, no Copy Image). It now takes no pointer while hidden, as the work row's own timestamp
  (`shell-tip.contract`) already did. (2) The shell looked for an app-drawn image only among the hit view's own subviews,
  and the icon is two views down (ToolActivityIcon's box › its `pointer-events="none"` hook box › T3ToolActivityIcon's
  view): `T3TextContextMenu.drawnImage` now searches below the hit, topmost first, skipping hidden views, as Chromium's hit
  test passes `pointer-events: none`. Under real input the pointer hovers the row, so the timestamp moves away and (2)
  alone dropped Copy Image (the session's "hit=NodeView (no id)" fits the outer box, which has no id). The reference's icon is an `<img>`
  (data PNG, `pointer-events: auto`), whose shell menu leads with Copy Image (#407's main-process read); Copy Image copies
  the icon's own bitmap.
- RG-2: the right-click selects the word under the pointer (as Chromium does). The session saw "theme" become "Theme" as
  the menu opened, the insertion point at 5,0 and a highlight over the word, and Cut did nothing: read as AppKit accepting
  its pending automatic correction for the word just typed as the selection moved, on a field editor with automatic
  spelling correction on ("Capitalize words automatically" is on on this Mac), which collapsed the selection so Cut had
  nothing to cut. NSTextView has no capitalization switch of its own; capitalization comes back from the spell checker
  as a `.correction` result, which a text view asks for only with automatic spelling correction on (measured: a field
  editor's checking types 8961 = orthography, replacement, correction, text completion; with `autocorrect="off"`'s switches
  8193 = orthography, text completion). The Settings search and 51 other text inputs had no `autocorrect="off"`; every
  editable text input now carries it (ExactKit #111: no correction, smart quotes, dashes or text replacement), as every
  textarea already did: a browser `<input>` never corrects or capitalizes (the reference's field keeps "theme" and its
  right-click selects it, 0-5). The capitalization was not reproduced here: in a window that is not key (headless AppKit,
  the agent's window on the locked screen) AppKit made no automatic correction at all. Real-input step 2.
- RG-3: compared by reading the item labels of the menu AppKit draws (its menu window, from a timer in the tracking loop;
  AppKit adds AutoFill and Services there, never to `menu.items`). The composer's spelling menu and the field's menu
  already lost AutoFill and Services with RF-1 (#413, after the session's 6bac646cc): the same menus popped as 6bac646cc
  popped them draw "…, AutoFill, Services". Still drawn on the tip: Services on a Browser page's word and AutoFill plus
  Services in a Browser page's field (the session's step 6 and 7 "Services: YES"), since RF-1 switched plug-ins off only
  where the shell pops its own menus and AppKit reads the switch when the pop-up starts, before `willOpenMenu` (tried: set
  there, both items stay). `T3ShellWebMenu.keepSystemItemsOut` turns the switches off on WebKit's menu as WebKit adds its
  first item (`NSMenu.didAddItemNotification`, an item with WebKit's forwarding action; every WebKit menu in the app, the
  terminal's included, as Electron's menus have none). `T3TextContextMenu.withoutSystemItems` also turns Writing Tools
  off (`automaticallyInsertsWritingToolsItems`, macOS 15.2+, where Apple Intelligence is on; not shown on this Mac).
- RG-4: the reference's link is one `<a>` with its URL (CDP: `link "li n k e d $ v e r i f y" url=https://example.test/`,
  Chrome's join of the per-character spans `breakableExternalLinkText` draws). The clone's link words were `text` nodes
  with an `href` (AXStaticText "linked ", "$verify"; on the web a generic). `FlowRuns` now draws a web link's first word as
  a Contract `link` node (href, the press and the app's link menu), its globe inside it as the reference's favicon, named
  with the whole link: `ChatRun.label`, set by `markdown_links.rs` `link_labels` from the link's runs. The link's other
  words keep their press and menu and are hidden from assistive tech (`aria-hidden`), a code span inside a link too. macOS
  in-process: `AXLink "linked $verify"`, no `$verify` text after it; the web (the same construct in a one-file app): `link
  "the docs"` (`<a>`). The look is unchanged. Out of process the link node has no AXURL (X78's host part, EXACT2-GAPS row
  updated); that read needs an unlocked screen (the locked Mac answers AXWindows with the application element, the
  reference's Electron too): real-input step 4.

The plan grows by 10.5 KB for all four rows (28,573,504 bytes at `327336447`, 28,584,027 on the final head).

Seen while building, not a finding row (left as is): a code span inside a web link (``[`x` docs](https://…)``) is drawn as
a plain code box (no press, no menu), and a link that starts with code has no link node; drawing those parts as the link
(two more `FlowRuns` branches) measured +763 KB of plan, so it was not built. A mailto, irc, xmpp or fragment link in a
reply stays plain link text (#407 round 2: no press), so it is not a link in the accessibility tree either.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RG-1 | pass (agent drive, reference, AppKit test): before (evidence worktree c03d7e908) no shell menu; the first after drive: no Copy Image (the hidden timestamp took the click); the final after drive: Copy Image, a separator, Cut, Copy and Paste disabled, Select All at three points of the icon, the reference's menu. AppKit: Copy Image under a pass-through hook, its bitmap copied (fails on the tip). Drawn menu and paste: real-input step 1 (open) | [RG1-tool-icon.txt](https://raw.githubusercontent.com/ccheever/exact2/ad5e2f9e703571dc08186ff5500e68b7957630b9/realinput-1010g-followups/RG1-tool-icon.txt) |
| RG-2 | implemented: every text input has `autocorrect="off"` (Bun: 52 inputs without it on the tip); the final drive's Settings search: `autocorrect` off, "theme" kept, menu Cut, Copy, Paste (disabled), Select All; AppKit: the menu's Cut cuts the selected word. The capitalization needs real input: step 2 (open) | [RG2-settings-search.txt](https://raw.githubusercontent.com/ccheever/exact2/ed151f471bd7a495c8cf913d90e58222477bc4e5/realinput-1010g-followups/RG2-settings-search.txt) |
| RG-3 | pass (AppKit, drawn menus): composer "chek" and field "theme" draw the template only (the session's build drew AutoFill and Services after it); a Browser page's word and field: before `…, Services` and `…, AutoFill, Services`, after the four roles only; Writing Tools off. Real menus: step 3 (open) | [RG3-drawn-menus.txt](https://raw.githubusercontent.com/ccheever/exact2/d09388babc4af2561b0621cdd69fd96fad4d0776/realinput-1010g-followups/RG3-drawn-menus.txt) |
| RG-4 | pass in-process (agent drive, reference, web repro): before AXStaticText "linked ", "$verify"; after AXLink "linked $verify" and nothing after it; the reference one `<a>` with its URL; the web `link "the docs"`. Look unchanged. Out of process (AXURL is X78): step 4 (open) | [RG4-link-ax.txt](https://raw.githubusercontent.com/ccheever/exact2/ae98bd868356071b1eeae6744129c0b59aaed3dd/realinput-1010g-followups/RG4-link-ax.txt), [RG4-reply-link-look.png](https://raw.githubusercontent.com/ccheever/exact2/d833dcdd57b197eaa2615472d799bb19c9dc91c3/realinput-1010g-followups/RG4-reply-link-look.png) |

The before build is the evidence worktree at `c03d7e908` (it predates #407, so it shows no shell menu at all); the
before of the AppKit rows is the feature tip's module (`327336447`) under the same new tests. The after drive ran twice:
the first (bundle of `09ea1c785`) found the timestamp over the icon, the retry (bundle of `15df07483`) is the result above.
Drive steps: [drive.sh.txt](https://raw.githubusercontent.com/ccheever/exact2/ee68c49b720a38061ebc4dbd68073b438a502a5b/realinput-1010g-followups/drive.sh.txt); reference tools: [ref-ax.mjs.txt](https://raw.githubusercontent.com/ccheever/exact2/a1055e2c904ebe01e30bb52313feb041b5b59b70/realinput-1010g-followups/ref-ax.mjs.txt),
[ref-rclick-at.mjs.txt](https://raw.githubusercontent.com/ccheever/exact2/bce134edafc6c1e7566712d0c88684685c91caee/realinput-1010g-followups/ref-rclick-at.mjs.txt); the AppKit checking probe: [rg2-types.swift.txt](https://raw.githubusercontent.com/ccheever/exact2/0ec5f8285506324dcedc08c87173ca68a4cbc68e/realinput-1010g-followups/rg2-types.swift.txt).

## Real-input batch steps

Launch this branch's bundle normally as a lane copy (`open -n --env …`, its own bundle id), the lane home outside any git
checkout, `R9_INPUT_LOG=$L/logs/r9-input.log`; the Verification fixture thread open; the clipboard empty first. Check that
System Settings › Keyboard › Text Input › Edit has "Capitalize words automatically" on (`defaults read -g
NSAutomaticCapitalizationEnabled` prints 1); do not change it. Never send a message.

1. **RG-1, the tool icon.** Rest the pointer on the work group's tool icon (the blue square left of "Ran 2 commands…") and
   right-click it: Copy Image, a separator, Cut, Copy and Paste disabled, Select All. Pick Copy Image; Preview › File › New
   from Clipboard: the small blue square. Move the pointer onto the icon and right-click at once (no rest): the same menu.
2. **RG-2, Settings search.** Settings; click the search field, type `theme`, wait two seconds, right-click on "theme": the
   field still reads "theme" (lowercase), the word is selected, Cut and Copy enabled. Pick Cut: the field is empty; paste
   into the composer: "theme" (then clear the composer). Type `theme ` (with a space) in the sidebar's thread search: it
   stays lowercase.
3. **RG-3, system items.** Composer: type `Plese chek`, right-click "chek": check, chef, chew, chez, cheek, a separator,
   Cut, Copy, Paste, Select All and nothing after them (no AutoFill, Services or Writing Tools). The Settings field's menu
   from step 2: nothing after Select All. Browser panel on `https://example.com`: right-click a word: nothing after Select
   All; in #407's fixture page (its step 7) right-click "Field" in the "Field text" field: nothing after Select All.
4. **RG-4, links from another process.** With the lane copy on the Verification fixture thread, from a terminal with the
   accessibility grant: `xcrun swiftc axwalk.swift -o axwalk` ([axwalk.swift.txt](https://raw.githubusercontent.com/ccheever/exact2/3bfe460231a2dfa7f672b7d5a2b50fc2eb53ebb4/realinput-1010g-followups/axwalk.swift.txt)), then
   `./axwalk <pid> 60 | grep -n -A2 'linked'`: an `AXLink "linked $verify"` (its `AXURL` answers -25205 while X78 is open),
   and no `AXStaticText "$verify"` after it.

## Tests

- `macos/tests/contextmenu/realinput-1010g.swift` (new, 4): the tool icon under a pass-through hook gets Copy Image and its
  bitmap (2 failures on the tip's module); Cut takes the word the right-click selected in a field (a field editor writing to
  the test's clipboard); the composer's and a field's drawn menus hold the template only while AppKit's own pop-up of the
  same menu draws AutoFill and Services, and Writing Tools is off (1 failure on the tip); a real WKWebView's drawn menus on a
  word and in a field (2 failures on the tip: Services, AutoFill and Services). `main.swift` runs both new suites.
- `text-entry.test.ts` (+1): every text input has `autocorrect="off"` (52 missing on the tip).
- `realinput-1010g-followups.test.ts` (new, 3): the header's hidden timestamp takes no pointer, the icon's hook box and the
  shell's search below the hit, the system-item switches (3 failures on the tip).
- `external-link-menu.test.ts`: the hookup test reads FlowRuns' link node (href, press, menu, `aria-label=run.label`, the
  globe inside), the hidden other words and code span, and `ChatRun.label`.
- `macos/src/markdown_links.rs` (+1, Rust): a link's first run is labelled with the whole link, its other runs and other
  runs with "" (`a_links_first_run_is_labelled_with_the_whole_link`).

## Checks

On the code head `642cdeb37` (`origin/feat(example)/t3-code` `327336447` merged; all exit 0): `bun test examples/t3-code
--timeout 60000` 4448 pass / 1 skip / 0 fail (304 files); strict `tsc`; `contract build` of `app.contract` (1397 lines;
28,584,027 bytes); `cargo test -p t3-code-macos --lib` 19 pass; AppKit `contextmenu` 53 run / 0 failed (4 new),
`browser-capture` 34 / 0, `media-actions` 7 / 0, `r6-media` 10 / 0 (the three use T3ShellWebView); `git add -A && bun
scripts/caps.mjs`; the five checks: `cargo build --all-targets --keep-going`, `cargo test --lib --bins --tests
--no-fail-fast` (3679 passed, 0 failed, 34 ignored), `cargo clippy --all-targets --keep-going -- -D warnings`, `cargo fmt
--all -- --check`, `bun scripts/caps.mjs`, `bun scripts/boot.mjs`. The live drive's bundle was built from `15df07483`;
`642cdeb37` only moves `autocorrect="off"` to the end of each input's attribute line (same plan size), and later commits
change records and merge `origin/feat(example)/t3-code`'s records (`79cfa3706`).

## Not done / not verified

- Real-input steps 1-4 above (the screen stays locked; open until the batch runs). RG-2's capitalization and RG-4's
  out-of-process read cannot be made in agent mode or headless here (see Cause and fix).

## Decision needed

None.

## Delivery

Draft PR [#419](https://github.com/ccheever/exact2/pull/419) into `feat(example)/t3-code`.

## Next action

Review the draft PR; run the real-input steps in the next batch.
