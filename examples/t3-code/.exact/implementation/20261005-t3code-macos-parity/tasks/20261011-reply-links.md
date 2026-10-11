---
name: 20261011-reply-links
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-reply-links
pr_url: https://github.com/ccheever/exact2/pull/434
verified_commit: null
---

# Reply links that are not links: other schemes, fragments, and a code span inside a link

## Outcome

STATUS "Known differences" rows 4 and 5 (the user asked on 2026-10-11 to fix what can be fixed now). Both were seen while
building [realinput-1010g-followups](closed/20261010-realinput-1010g-followups.md) (#419, "Seen while building").

| Id | Clone | Reference (check it first) |
| --- | --- | --- |
| RL-1 | A reply's `mailto:`, `irc:` or `xmpp:` link is plain text: no press, no menu, not a link to accessibility (#407 round 2 left it without a press) | `ChatMarkdown`'s `<a>`: find what a click does in the desktop app (`shell.openExternal` for these schemes, or nothing) and what its context menu shows |
| RL-2 | A reply's `#fragment` link is plain text too | Find what the reference does with an in-page fragment in a reply (scroll to a heading, open nothing, or the link as text) |
| RL-3 | A code span inside a web link (``[`x` docs](https://…)``) is a plain code box (no press, no menu), and a link that starts with code has no link node, so its words read as text, not as a link. Drawing them as the link (two more `FlowRuns` branches) measured +763 KB of plan in #419 | The whole `<a>` is one link, its code part included |

## Steps

1. Reference first (CDP on the reference lane, and its source under `target/t3-ref/src-1e2ecbd975`): RL-1 and RL-2's
   click, menu and accessibility; RL-3's link with a code span inside and one that starts with a code span.
2. Match each row in the clone (`FlowRuns`, `macos/src/markdown_links.rs`, the reply renderer). A scheme the reference
   opens goes out through the same system handler (`NSWorkspace`) the web links use; keep #419's link node and labels.
3. RL-3: find a way that does not cost hundreds of KB of plan (for example one shared branch, or labelling the code run as
   part of the link node instead of a new branch). Report the plan size before and after; if no way stays under about
   100 KB, build it anyway only if the cost is under 300 KB, otherwise record the measurement and leave the row open.
4. Tests (Bun; the Rust test for `markdown_links.rs` if it changes); one agent drive with a fixture reply that holds all
   four kinds of link; `tree --ax` read for the link roles; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RL-1..RL-3 | reference comparison first; tests; agent drive; `tree --ax` | before / after / reference images |
| A click on a mailto link opening the mail app, under a real pointer | a real-input step for the final session | — |

## Reference

Measured on the reference (lane reply-links: the pinned 1e2ecbd975 in Electron, CDP on 17121 and its main process's
inspector on 17123 (`ref-app.sh` with `--inspect`), the lanes' fixture reply holding the four kinds of link and two web
links with code spans, [fixture-text.txt](https://raw.githubusercontent.com/ccheever/exact2/21efc1da7750a87ba56a417d8a86c3eb7d4622e1/reply-links/fixture-text.txt)).
The main process's `shell.openExternal` was replaced by a recorder, the session's `openExternal` permission requests
recorded and denied, `will-navigate`, `did-navigate-in-page` and `did-create-window` recorded, and `Menu.prototype.popup`
recorded instead of shown ([main-eval](https://raw.githubusercontent.com/ccheever/exact2/c27238ac595cc38c4ec0e64cc1425351b4b91d55/reply-links/ref-main-eval.mjs.txt),
[hook](https://raw.githubusercontent.com/ccheever/exact2/95af91ce8a9f4cb6ac791c9e8a7364ad93a15378/reply-links/ref-hook.js.txt),
[hook-menu](https://raw.githubusercontent.com/ccheever/exact2/f87f1bd5bac090b1ff1998f549c5b9fe9c61345f/reply-links/ref-hook-menu.js.txt),
[ref-ax](https://raw.githubusercontent.com/ccheever/exact2/c8ac212b38f05c4eed66e8e25cee08b37960c3b5/reply-links/ref-ax.mjs.txt)).

- RL-1: a mailto, irc or xmpp link is an `<a href target=_blank>` (react-markdown and the sanitize schema keep the
  schemes): Chrome's AX `link "the team" url=mailto:…`. A click is not prevented (`resolveLinkTarget` answers "system" for a
  non-web URL), its `_blank` reaches `setWindowOpenHandler`, which opens only `parseSafeExternalUrl`'s URLs (http(s) and
  remote editor links): nothing opens (no `openExternal`, no permission request, no navigation). Its right-click: the anchor's
  `onContextMenu` returns before `preventDefault` (no favicon host), so the shell's menu opens: Cut (disabled), Copy, Paste
  (disabled), Select All, no Copy Link.
- RL-2: a fragment link is an `<a href="#notes">` (AX `link "the notes"`), link-coloured, the same menu. Its click runs
  `handleMarkdownFragmentClick`, which scrolls to an element with that id inside the reply (only GFM footnotes and raw-HTML
  ids have one; the clone draws neither). With no target the click goes on, sets the page's hash, and Electron's hash router
  (`createHashHistory`) reads `#notes` as a route: the app leaves the thread for the new-thread draft
  (`t3code://app/#/draft/…`). That is "Decision needed" below.
- RL-3: ``[the `x` docs](…)`` and ``[`y` docs](…)`` are one `<a>` each, the favicon first
  (`MarkdownExternalLinkContent` puts it before the first child, a code span too): AX `link "the x docs"`, `link "y docs"`.
  A click on a code span opens the link (`shell.openExternal https://example.com/c`, `…/d`); its right-click is the link
  menu (Open in integrated browser, Open in system browser, Copy Link).

## Cause and fix

- RL-1, RL-2 (`macos/src/markdown_links.rs`, `markdown.rs`, `media-views.ts`, `markdown.contract` FlowRuns,
  `browser-links.ts`, `external-link-menu.ts`, `T3TextContextMenu.swift`, `T3Module+Window.swift`): a fragment kept no
  href (`link_href` emptied it), so it was prose; it now keeps it (an image's fragment source still loads nothing). FlowRuns
  drew a mailto, irc or xmpp link as `text` nodes with an `href` and no handler: not a link to accessibility, and a click
  went to the host's own link following (the drive's `exact: refused to open irc://…`; a mailto URL goes to NSWorkspace, the
  mail app). Every reply link's first run is now the one link node (`linkRun`: any link but a pull request link), named
  with the whole link; only a web link's holds the favicon (`when webLink(run.href)`). Its press is `linkOpen`, which opens
  nothing for a non-web URL (`openLinkFromUi`), and its `contextmenu` is the link menu, which for a link with no web host asks
  the module for the shell's menu of the click (`shellMenu`: `T3TextContextMenu` keeps the last page click ExactKit took and
  answers it once, within five seconds). The link's later words are `text` with no href (a click follows nothing), hidden
  from assistive tech as a web link's are.
- RL-3 (`markdown.contract` FlowRuns, `markdown_links.rs` `link_labels`): a code span in a web link was the plain code box
  (no press, no menu), and a link that started with one had no link node. A link's first code span is now drawn inside its
  link node (after the favicon), and a web link's later word or code span is one pressable box with the link's menu (the
  former word `text` inside it, or the code span). `link_labels` labels a link that starts with a code span too.
- Plan size ([RL-plan-size.txt](https://raw.githubusercontent.com/ccheever/exact2/af67716db69ffc5334a3f27eed15127939cb848e/reply-links/RL-plan-size.txt)): 28,585,446 bytes on the
  tip, 28,708,013 after (+122,567): RL-1 and RL-2 +1,021, RL-3 +121,546 (over "about 100 KB", under the 300 KB ceiling, so
  built). The first design (a separate mailto link branch, a separate pressable code box, a press on a table cell's run) was
  +882,679: a new handler site in FlowRuns costs about 1 KB per inlined use (140), so the final design adds none and reuses
  the link node's and the later word's handlers.

## Review round 1 (independent review of #434, 2026-10-11)

Four should-fix findings, all fixed (`29120f2ad`, `69b6c0dd8`); text before/after from one agent drive per build, the same
steps ([RL-review-before-after](https://raw.githubusercontent.com/ccheever/exact2/110d7d6945664560ecdb6201647f1479d9a09c71/reply-links/RL-review-before-after.txt), [drive2.sh](https://raw.githubusercontent.com/ccheever/exact2/81f3c81881b2793ab4cb20199690845e27704083/reply-links/RL-review-drive2.sh.txt)): before on this worktree's bundle of the PR head
(`a56f127ff`'s code; its sources restored for the drive with their mtimes set back to before that build, content
identical), after on the bundle of `69b6c0dd8`.

- **R1-1, the pointer over a link's later words.** A mailto, irc, xmpp or fragment link's later words had an `href` on the
  base, which ExactKit gives the pointing hand (`resetCursorRects`); the PR took the href away (a click no longer reaches
  the host's link following) and with it the pointer, while the reference's `<a>` shows `cursor: pointer` over all of it.
  FlowRuns gives those words, and such a link's later code span, `cursor="pointer"` (a run with a link `label`; an
  emptied anchor has none, as an `<a>` with no href shows no pointer). Drive: `layout team`, `notes`, `channel` were
  `cursor auto (initial)`, now `pointer`. Plan +3,276 bytes (28,711,289).
- **R1-2, table cells.** Keeping the fragment href also reached Markdown table cells. (a) Copy as Markdown wrote
  `[the notes](#notes)`: `cell_markdown` now keeps a link only for an http(s) URL, as `serializeAnchor` does
  (`if (!/^https?:\/\//i.test(href)) return content;`); a file link still copies its source (the chip's
  `data-markdown-copy`) and a context or citation reference its link. A fragment, mailto, irc or xmpp link copies its text;
  this also changes a mailto link, which the base copied as `[mail](mailto:…)`. (b) A cell's fragment link is now drawn as
  a link (ChatRuns' inline run: link colour, an AX link), as the reference's `<a href="#notes">` in a cell, where the base
  drew prose; a click on it goes to the host's own link following, which refuses a URL with no scheme (`exact: refused to
  open #notes`, ExactMac `open`): nothing opens and the thread stays, as RL-2's fragment in prose. Rust test
  `a_table_cell_copies_only_a_web_link_as_a_link`.
- **R1-3, the image fragment test.** `markdown.rs` `an_image_with_a_fragment_source_loads_nothing`: `![a](#shot)` is an
  image block with an empty href (and an emptied `javascript:` source; a web source keeps it).
- **R1-4, a stale click for `shellMenu`.** `lastPageClick` stayed set after the tail or a node's own menu answered a click,
  so a `contextmenu` no click raised (the ContextMenu key on a focused link) within 5 s popped the shell's menu at that old
  click. Now any key or click the monitor sees (it watches keys and the other buttons too), the tail's menu
  (`pageClick`), and the app's own menu (T3Module's `contextMenu` op, `pageClickAnswered`) end the click. The agent's keys
  go to the window and bypass NSApplication's monitor (AgentMac), as VoiceOver's show menu sends no event, hence the
  third. Drive: the ContextMenu key on the focused "the team" link after the tail's menu on "team", and on "chat" after the
  web link's own menu on `x`, each popped the shell's menu for the old click; after, neither opens a menu. AppKit test
  `testAMenuThatAnsweredItOrALaterKeyOrClickEndsTheClick`; its first version (tail, key, click) failed 4 assertions on the
  PR head's module ([old vs new](https://raw.githubusercontent.com/ccheever/exact2/d38b276053d5892ce56f50c5d4b423412eb29904/reply-links/RL-review-appkit.txt)).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RL-1 | pass (agent drive, reference, Bun, AppKit): before, "the team", "the channel", "chat" were `AXStaticText` words, a click on irc or xmpp went to the host (`exact: refused to open irc://…`, `…xmpp:…`; a mailto one to the mail app, not clicked); after, `AXLink "the team"`, `"the channel"`, `"chat"` with their other words hidden, a click is the link node's press and opens nothing (no host log, no recorded URL), the right-click is the shell's menu with no Copy Link through `shellMenu`, as the reference. Under real input: step 1 (open) | [menus and clicks](https://raw.githubusercontent.com/ccheever/exact2/279b68b49356f7e32955991bdad09a07de30fc16/reply-links/RL-menus-clicks.txt), [AX](https://raw.githubusercontent.com/ccheever/exact2/c629a0a0b6ffdbb6493531b05dc3902282831ba8/reply-links/RL-ax.txt), [look](https://raw.githubusercontent.com/ccheever/exact2/9379cc255f964ea3e5ff399738477c5f85538d11/reply-links/RL-reply-links-look.png) |
| RL-2 | partial: before, "the notes" was prose (no link colour, no link); after, `AXLink "the notes"`, link-coloured, the shell's menu, and a click opens nothing and keeps the thread. The reference's click leaves the thread for the new-thread draft (its hash router): not copied, Decision needed | [fragment click](https://raw.githubusercontent.com/ccheever/exact2/d11d26b75cb10eb10a874a59395dbffdc6c4f636/reply-links/RL2-fragment-click.png), [menus and clicks](https://raw.githubusercontent.com/ccheever/exact2/279b68b49356f7e32955991bdad09a07de30fc16/reply-links/RL-menus-clicks.txt) |
| RL-3 | pass (agent drive, reference): before, `x` and `y` were plain code boxes (the shell's menu, a click did nothing), `y docs` had no globe and read as text; after, a right-click on either is the link menu, a click opens `https://example.com/c` and `…/d` (recorded), `AXLink "y docs"` holds the globe and the code span, as the reference. Plan +121.5 KB | [menus and clicks](https://raw.githubusercontent.com/ccheever/exact2/279b68b49356f7e32955991bdad09a07de30fc16/reply-links/RL-menus-clicks.txt), [AX](https://raw.githubusercontent.com/ccheever/exact2/c629a0a0b6ffdbb6493531b05dc3902282831ba8/reply-links/RL-ax.txt), [look](https://raw.githubusercontent.com/ccheever/exact2/9379cc255f964ea3e5ff399738477c5f85538d11/reply-links/RL-reply-links-look.png), [plan size](https://raw.githubusercontent.com/ccheever/exact2/af67716db69ffc5334a3f27eed15127939cb848e/reply-links/RL-plan-size.txt) |
| A click on a mailto link under a real pointer | open: real-input step 1. The reference opens nothing (not the mail app), so the step checks that nothing opens | — |

Drives: one per build, the same steps ([drive.sh](https://raw.githubusercontent.com/ccheever/exact2/459d5fd616ce6e1aeca346e0be540106153f9e4e/reply-links/drive.sh.txt);
before on the evidence worktree at `eb9752918`, after on the bundle of `a56f127ff`; logs
[before](https://raw.githubusercontent.com/ccheever/exact2/c93c0201da2af37a1986f9f922d7f0da3badb009/reply-links/RL-drive-before-log.txt),
[after](https://raw.githubusercontent.com/ccheever/exact2/cc525aa0972709799ff1d7a4d634d30fa52ee5f0/reply-links/RL-drive-after-log.txt)). The lane's clone
server runs on 16938: the clone's development build refuses a local server port outside 16000–16999
(`T3LocalBackend.laneRange`), so the given base 17120 served the reference only. No mailto link was left-clicked in a clone
drive.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) on a lane whose
fixture reply holds the links (`sqlite3 <lane>/userdata/statev2.sqlite "update orchestration_v2_projection_turn_items set
payload_json = json_set(payload_json, '$.text', cast(readfile('fixture-text.txt') as text)) where turn_item_id =
'fixture-markdown'"`, [fixture-text.txt](https://raw.githubusercontent.com/ccheever/exact2/21efc1da7750a87ba56a417d8a86c3eb7d4622e1/reply-links/fixture-text.txt)); the
Verification fixture thread open. Never send a message.

1. **RL-1.** Hover "the team" (mailto), "the channel" (irc), "chat" (xmpp) and "the notes", each on its first and its last
   word: the pointing hand on each (review R1-1). Click each on its first and its last word: nothing opens (no Mail, no
   browser, no toast). Right-click "the team": Cut (disabled), Copy (disabled, or enabled with page text
   selected), Paste (disabled), Select All, and no Copy Link.
2. **RL-2.** Click "the notes": the thread stays (until the decision below says otherwise). Right-click it: the menu of step 1.
3. **RL-3.** Right-click the code span `x` in "the `x` docs": Open in integrated browser, Open in system browser, Copy Link;
   pick Copy Link and paste into the composer: `https://example.com/c` (clear the composer). Click `y` in "`y` docs": the
   link opens where "Open links in" says.
4. **Assistive tech from another process.** `./axwalk <pid> 60 | grep -n 'AXLink'` (realinput-1010g's axwalk): `AXLink
   "the team"`, `"the channel"`, `"chat"`, `"the notes"`, `"the x docs"`, `"y docs"`, with no `AXStaticText "team"`,
   `"channel"`, `"notes"` or `"docs"` after them.

## Tests

Review round 1: `markdown_links.rs` `a_table_cell_copies_only_a_web_link_as_a_link` (a cell's copy text keeps only the web
link; its fragment run is drawn as a link with `#notes`); `markdown.rs` `an_image_with_a_fragment_source_loads_nothing`;
`reply-links.swift` `testAMenuThatAnsweredItOrALaterKeyOrClickEndsTheClick` (the tail's menu, a later key, a later click
and the app's own menu each end the click); `external-link-menu.test.ts` pins the `cursor` of a link's later words and
code span.


- `browser-links.test.ts` (+1): a mailto, irc, ircs, xmpp or fragment link through `chatlocal:link-open` sends nothing to
  the module or the server and toasts nothing, with "Open links in" on either value and ⌘ held; a web link still opens.
- `external-link-menu.test.ts`: the hookup test reads the new FlowRuns (the link node for every non-pull-request link, the
  favicon under `when webLink`, the code span inside it, the web link's later box with the link's handlers, the plain code
  box, prose with no href); a mailto link's `chatlocal:link-menu` asks for `shellMenu` and shows no menu of its own.
- `r4-timeline-chips.test.ts`: `markdownLinkHref('#section')` keeps the fragment; `markdownImageHref('#shot')` loads nothing.
- `macos/src/markdown_links.rs` (Rust): `link_href` keeps a fragment; `link_labels` labels a link that starts with a code
  span, and mailto, irc, xmpp and fragment links with their hrefs.
- `macos/tests/contextmenu/reply-links.swift` (new, 4): a link node's own `contextmenu` runs, then `shellMenuForLastClick`
  pops the shell's menu for that click once (no Copy Link for a mailto URL; Copy with page text selected; nothing for no
  click or a stale one; the template's Copy Link for a safe URL). `main.swift` runs the suite. On the tip's module the
  suite does not compile (no `shellMenuForLastClick`).

## Checks

Review round 1, on the code head `69b6c0dd8` (`origin/feat(example)/t3-code` `0292f4364`: already merged, no new commits;
all exit 0): `bun test examples/t3-code --timeout 60000` 4454 pass / 1 skip / 0 fail (305 files); strict `tsc`; `contract
build` of `app.contract` (1397 lines; 110,029 nodes, 28,711,289 bytes: +3,276 for R1-1); `cargo test -p t3-code-macos --lib`
21 pass (2 new; the Rust has not changed since); AppKit `contextmenu` 58 run / 0 failed (1 new); `git add -A && bun
scripts/caps.mjs`; the five checks: `cargo build --all-targets --keep-going`, `cargo test --lib --bins --tests
--no-fail-fast` (3679 passed, 0 failed, 34 ignored), `cargo clippy --all-targets --keep-going -- -D warnings`, `cargo fmt
--all -- --check`, `bun scripts/caps.mjs`, `bun scripts/boot.mjs`. The bundle of `69b6c0dd8` was built once for the drive.

Before, on the code head `1cc7f3391`: the same checks, all exit 0 (4454 pass; 28,708,013 bytes; 19 Rust; AppKit 57 / 0).

## Not done / not verified

- Real-input steps 1–4 above: the coordinator's session (step 1 now also hovers for the pointer, review R1-1).
- RL-2's click: Decision needed below.
- The ContextMenu key on a focused mailto, irc, xmpp or fragment link opens no menu (review R1-4 ends a click the key did
  not raise, and `shellMenu` has nothing else to answer); the reference's shell opens its menu at the focused anchor. A Mac
  keyboard has no ContextMenu key and Chrome on a Mac gives Shift+F10 no default (MouseEventsMac), so it is reachable by
  a PC keyboard or VoiceOver's show menu only; not built.

## Seen while building, not a finding row

- A Markdown table cell's mailto link (`ChatRuns` in `TableCell`, inline runs) is a link to accessibility, and a click
  still goes to the host's own link following, which hands a mailto URL to the mail app (the reference opens nothing). A
  press on that run stops it, measured +308,280 bytes of plan (ChatRuns has many more inlined uses); not built.
- A table cell's fragment link is a link there too since RL-2 kept the href (review R1-2): drawn and read as a link, as the
  reference's; a click is refused by the host (`exact: refused to open #notes`), so nothing opens and the thread stays.

## Decision needed

RL-2: in the reference a click on a reply's fragment link with no target in the reply (`[the notes](#notes)`; every
fragment in the clone, which draws no footnotes or HTML ids) sets the page's hash, which Electron's hash router reads as a
route, so the app leaves the thread for the new-thread draft. The clone keeps the thread. Copy the reference's navigation
(the link opens the new-thread draft), or keep the thread?

## Next action

Review the draft PR (review round 1 fixed); answer the decision; real-input steps 1–4 in the coordinator's session.
