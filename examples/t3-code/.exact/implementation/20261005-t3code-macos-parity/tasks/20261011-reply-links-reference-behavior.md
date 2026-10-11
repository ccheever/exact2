---
name: 20261011-reply-links-reference-behavior
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-reply-links-reference-behavior
pr_url: https://github.com/ccheever/exact2/pull/474
verified_commit: b23693933
---

# Reply links as the reference has them: a fragment leaves the thread, a table cell's mailto opens nothing, the ContextMenu key's shell menu

## Outcome

The user decided on 2026-10-11 to match the reference on the three reply-link rows
[reply-links](closed/20261011-reply-links.md) kept (its "Decision", "Not done" and "Seen while building"; `STATUS.md`
"Known differences" rows "A reply's `#fragment` link", "A `mailto:` link in a reply's Markdown table cell" and "The
ContextMenu key (or VoiceOver's show menu) on a focused …").

| Id | Clone (feature tip `6db5cadea`) | Reference (check it first) |
| --- | --- | --- |
| RL-4 | A click on a reply's `#fragment` link opens nothing and keeps the thread (reply-links RL-2, "kept by choice") | `handleMarkdownFragmentClick` finds no element with that id, the click sets the page's hash, and Electron's hash router (`createHashHistory`) reads it as a route: the app leaves the thread for the new-thread draft |
| RL-5 | A `mailto:` link in a reply's Markdown table cell (`TableCell` → `ChatRuns`, an inline run) has no press, so a click goes to the host's own link following, which hands the URL to the mail app; a press there was measured at +308,280 bytes of plan (accepted by the user: measure it) | Opens nothing (the anchor's `_blank` reaches `setWindowOpenHandler`, which opens only http(s) and remote editor links) |
| RL-6 | The ContextMenu key (or VoiceOver's show menu) on a focused `mailto:`, `irc:`, `xmpp:` or `#fragment` link opens no menu: the link node's `contextmenu` asks for the shell's menu, which answers only a fresh page click (reply-links review R1-4) | Chrome raises `context-menu` at the focused anchor and the shell's menu opens; build it if the host allows it, else record the blocker |

## Steps

1. Reference first (the reference app on this lane, CDP and its main process's inspector; source under
   `target/t3-ref/src-1e2ecbd975`): RL-4's navigation and focus, from prose and from a table cell; RL-5's click on a
   cell's mailto link; RL-6's ContextMenu key on each kind of focused link, the menu's items and where it pops up.
2. RL-4: `browser-links.ts` `openLinkFromUi` opens the most recently active project's draft for a fragment (the index
   route's `IndexDraftLanding`), with the window's new-thread handling (the composer takes the focus); every word of
   the link presses it (`markdown.contract` FlowRuns), and a table cell's fragment link too.
3. RL-5: the table cell's link run gets the link's press without a new handler site (merge `ChatRuns`' web and other
   link branches); measure the plan.
4. RL-6: `T3TextContextMenu` `shellMenu` answers a `contextmenu` no click raised with the shell's menu for the focused
   link; find whether VoiceOver's show menu reaches `contextmenu` at all on the host.
5. Tests (Bun; AppKit `contextmenu`); one agent drive per build with the lane's fixture reply; before / after /
   reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| RL-4 | reference comparison first; tests; agent drive (`tap "the notes"`, `tap "the summary"`) | before / after / reference images |
| RL-5 | reference; tests; plan size; agent drive (the cell's irc link before and after, its mailto link after only) | text before / after (logs, tree) and images |
| RL-6 | reference; AppKit test; agent drive (`type "the team" key ContextMenu`) | text before / after (the menu's log line) |
| RL-4..RL-6 under real keys and pointer | real-input steps for the coordinator's session | — |

## Reference

Measured 2026-10-12 on the reference (lane reply-links-reference-behavior: the pinned 1e2ecbd975 in Electron, backend
16620, CDP 16621, its main process's inspector 16623, 1280×840; the lane's fixture reply, [fixture-text.txt](https://raw.githubusercontent.com/ccheever/exact2/028d7d177966fcf272a2a054011db74fea44a0dc/reply-links-reference-behavior/fixture-text.txt):
reply-links' prose links plus a table whose cells hold a mailto, an irc, a fragment and a web link). The main process's
`shell.openExternal`, `openExternal` permission requests, `will-navigate`, `did-navigate-in-page`, `did-create-window`,
the page's `context-menu` params and `Menu.prototype.popup` (its items, options and the pointer) were recorded:
[RL-reference.txt](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt).

- RL-4: a click on "notes" of `[the notes](#notes)` (`apps/web/src/components/ChatMarkdown.tsx`
  `handleMarkdownFragmentClick`: no element has that id, so the click goes on) sets `t3code://app/#notes`; the hash
  router (`apps/web/src/main.tsx` `createHashHistory`) reads it as the index route, whose `IndexDraftLanding`
  (`routes/_chat.index.tsx`: `sortScopedProjectsForSidebar(projects, threads, "updated_at")[0]`, `handleNewThread(…,
  { replace: true })`) replaces it with `#/draft/<id>` of the most recently active project ("work"), the composer focused
  (`document.activeElement` is "Message"). A table cell's `[the summary](#summary)` does the same. A click with ⌘, ⌥, ⇧
  or ⌃ the handler leaves alone, and the shell denies the window it asks for (`setWindowOpenHandler`; read from source).
- RL-5: clicks on a cell's `[the list](mailto:…)` and `[the room](irc://…)`: nothing (no `openExternal`, no permission
  request, no navigation, no window); a cell's web link: `shell.openExternal https://example.com/e`.
- RL-6: CDP's ContextMenu key on each focused mailto, irc, xmpp and fragment link: Chromium raises `context-menu`
  (`menuSourceType: "keyboard"`) at the anchor's middle and `installContextMenu`
  (`apps/desktop/src/window/DesktopWindow.ts`) pops Cut, Copy, Paste (disabled with nothing selected), Select All, no
  Copy Link (`parseSafeExternalUrl` refuses the URL). `popupTemplate` (`apps/desktop/src/electron/ElectronMenu.ts`)
  passes no position, so Electron places that menu at the mouse pointer, not at the link (see Decision needed).

## Cause and fix

- **RL-4** (`browser-links.ts` `openFragmentLink`, `app.contract` `fragmentChanged` / `fragmentOpened`,
  `markdown.contract` FlowRuns): `openLinkFromUi` returned nothing for a fragment. A Markdown link's fragment now reads
  the click's modifiers (`composerSendIntent`, as a web link does): with one held it opens nothing; with none it opens
  the draft of the most recently active project (`client.openDraft(mostRecentProjectId(shell))`, the clone's index
  landing, as the sidebar's last-thread delete does) and answers `sidebar:new-thread`. The window sends a fragment's
  `chatlocal:link-open` on its own mutation, whose completion does what `commandCompleted` does for a new thread: the
  utility page closes, the timeline jump and draft owner reset, the composer takes the focus (a call to `command` or a
  send of `changed` from `chatLocal` was refused by the compiler's `analyze-send-twice` and `analyze-call-stale-read`:
  `pageKey` and `save` reach both). In FlowRuns a fragment link's later words and code spans were prose with no press
  (only the link node's first word opened anything); they are now the web link's pressable box (`pressLink`: a web or
  fragment link), with `tabindex=-1` for a fragment's so the `<a>` stays one Tab stop (a web link's boxes keep theirs, as
  before). A mailto, irc or xmpp link's later words stay prose: their click opens nothing either way.
- **RL-5** (`markdown.contract` ChatRuns): a table cell's runs are inline runs of one text node (`TableCell` →
  `ChatRuns`); a web link's run had the link's press and menu, any other link's only its `href`, so a click went to the
  host's own link following (`exact: refused to open irc://…`; a mailto URL to the mail app). The two branches are now one
  for every link run with a destination (press `linkOpen`, menu `linkMenu`), and an emptied anchor keeps a branch with
  neither, so no handler site is added. A cell's mailto, irc or xmpp link opens nothing, its fragment link leaves for the
  draft (RL-4), its web link is unchanged. Its right-click is still the tail's shell menu (an inline run's `contextmenu`
  never runs on macOS, X77), which has no Copy Link for a URL the shell refuses, as the reference's.
- **RL-6** (`modules/apple/T3TextContextMenu.swift` `shellMenuForLastClick` / `focusMenu`): ExactKit raises the focused
  link node's `contextmenu` for the ContextMenu key (`MouseEventsMac.swift` `contextMenuKey`), its handler asks the
  module for the shell's menu (`shellMenu`), and `shellMenu` answered only a fresh page click (reply-links review R1-4).
  With no such click it now opens the shell's menu for the focused page node of a visible window (not a text input, whose
  menu is its right-click's): read-only Cut, Copy (enabled while page text is selected), Paste, Select All, no Copy Link
  for a URL the shell refuses, at the node's middle. VoiceOver's show menu reaches no node on the host: `NodeView`
  answers no `AXShowMenu` and AppKit's `NSView` default performs nothing ([probe](https://raw.githubusercontent.com/ccheever/exact2/c838acad2f1032855738ec15598a80086a992cdf/reply-links-reference-behavior/X84-show-menu-probe.txt)), so no `contextmenu`
  runs; recorded as `EXACT2-GAPS.md` X84 (new, not filed: for the coordinator).
- **Plan size** (RL-4 and RL-5's contract changes together): 28,864,485 bytes on the tip (`fe0e39c91`'s sources),
  28,903,392 after (+38,907; the +308,280 bytes the reply-links record measured for a separate press on a cell's run is not
  spent: the merged branch adds no handler site).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RL-4 | pass (reference, Bun, agent drive): before, a click on "notes" of "the notes" left the thread up (the later word has no press) and a cell's "the summary" was refused by the host (`exact: refused to open #summary`); after, both leave for "work / New thread" ("What should we build in work?") with the composer focused (`fragmentChanged`, `linkOpen` presses in the log), as the reference's `#/draft/…`; a modified click opens nothing (Bun) | [prose](https://raw.githubusercontent.com/ccheever/exact2/548ff623a4ab1e00aed20e783eb6dc4252cf8732/reply-links-reference-behavior/RL4-fragment-in-prose.png), [table cell](https://raw.githubusercontent.com/ccheever/exact2/7103638e1321d2156bce0611bb44a6067bda19af/reply-links-reference-behavior/RL4-fragment-in-table-cell.png), [drive](https://raw.githubusercontent.com/ccheever/exact2/e7b9ba2ad4e38e7e3ea9d7dc5e3a6b57fa1b9704/reply-links-reference-behavior/RL-drive-before-after.txt), [reference](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt) |
| RL-5 | pass (reference, Bun, agent drive, plan): before, a click on the cell's irc link went to the host (`exact: refused to open irc://irc.example.test/t3`; the mailto one would reach the mail app, not clicked on the tip); after, both are the link's press (`linkOpen`) and nothing opens (no host line, no opened URL), as the reference. Plan +38,907 bytes for RL-4 and RL-5 together (accepted ceiling +308 KB) | [drive](https://raw.githubusercontent.com/ccheever/exact2/e7b9ba2ad4e38e7e3ea9d7dc5e3a6b57fa1b9704/reply-links-reference-behavior/RL-drive-before-after.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/67b7935f6c4023c6b26bcabb3f03ef8afefd1cbe/reply-links-reference-behavior/RL5-RL6-fixture-reply.png), [reference](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt) |
| RL-6 | partial. The ContextMenu key: pass (reference, AppKit, agent drive): before, the key on the focused "the team", "the channel", "chat" and "the notes" ran their `contextmenu` and opened no menu; after, each opens the shell's Cut, Copy (disabled), Paste (disabled), Select All with no Copy Link (`t3.textmenu:`), as the reference; the AppKit test puts it at the link's middle (6 of its assertions fail on the tip's module). VoiceOver's show menu: blocked by the host (X84, new) | [drive](https://raw.githubusercontent.com/ccheever/exact2/e7b9ba2ad4e38e7e3ea9d7dc5e3a6b57fa1b9704/reply-links-reference-behavior/RL-drive-before-after.txt), [AppKit](https://raw.githubusercontent.com/ccheever/exact2/718d89fa32e199dd9b2cd7cdb243a1b0ae24707a/reply-links-reference-behavior/RL6-appkit.txt), [reference](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt), [X84 probe](https://raw.githubusercontent.com/ccheever/exact2/c838acad2f1032855738ec15598a80086a992cdf/reply-links-reference-behavior/X84-show-menu-probe.txt) |
| RL-4..RL-6 under real keys and pointer | open: real-input steps 1–4 below | — |

Drives: one per build, the same steps ([drive.sh](https://raw.githubusercontent.com/ccheever/exact2/aaba5ca7c6c7c4b69e480080bc147b5cec51aec8/reply-links-reference-behavior/drive.sh.txt)): before on the evidence worktree at `6db5cadea`, after
on this branch's bundle (`306ba87f1`'s app code; built once). The cell's mailto link was clicked on the after build only.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) on a lane whose
fixture reply holds the links (`sqlite3 <lane>/userdata/statev2.sqlite "update orchestration_v2_projection_turn_items set
payload_json = json_set(payload_json, '$.text', cast(readfile('fixture-text.txt') as text)) where turn_item_id =
'fixture-markdown'"`, [fixture-text.txt](https://raw.githubusercontent.com/ccheever/exact2/028d7d177966fcf272a2a054011db74fea44a0dc/reply-links-reference-behavior/fixture-text.txt)); the Verification fixture thread open. Never send a message.

1. **RL-4.** Click the word "notes" of "the notes": the window shows "work / New thread" (or the lane's most recently
   active project), "What should we build in …?", the caret in the composer. Reopen the thread; click "the" of "the
   notes": the same. Reopen; ⌘-click "notes": nothing (the thread stays). Click the table's "the summary": the draft.
2. **RL-5.** Click the table's "the list" (mailto): no Mail window, the clone stays in front, no toast. Click "the
   room" (irc): nothing. Click "the site": it opens where Settings › "Open links in" says.
3. **RL-6, ContextMenu key** (a keyboard with a ContextMenu key, such as a PC keyboard; a Mac keyboard has none: skip and
   say so). Tab to "the team" (or click it: it opens nothing and takes the focus); press the ContextMenu key: Cut, Copy,
   Paste (disabled), Select All, no Copy Link, at the link. The same on "the notes". Note where the reference's menu opens
   for the same key (at the pointer, by its source).
4. **RL-6, VoiceOver** (⌘F5 on, off afterwards). Move the VoiceOver cursor to "the team"; VO-⇧-M: expected nothing in the
   clone (X84). The same in the reference T3 Code (Alpha) lane window: record whether its shell menu opens.

## Tests

- `browser-links.test.ts`: a fragment link leaves for the most recently active project's draft ("work"), answering
  `sidebar:new-thread`, under either "Open links in" value, for prose's and a cell's fragment; ⌘, ⌥, ⇧, ⌃ or ⌘⇧ held
  opens nothing; the mailto/irc/ircs/xmpp row keeps the thread.
- `reply-links-reference-behavior.test.ts` (new, 2): the window sends a fragment open on `fragmentChanged` and its
  completion closes the utility page, resets the jump and draft owner and focuses the composer, as `commandCompleted`'s
  new-thread handling; `chatlocal:link-open` end to end through the client.
- `external-link-menu.test.ts`: ChatRuns gives every link run with a destination the link's press and menu in one branch
  (an emptied anchor neither); FlowRuns' later-word box covers fragment links (`pressLink`) with `tabindex=-1` for them.
- `macos/tests/contextmenu/reply-links.swift` (+2): the ContextMenu key on a focused link opens the shell's menu at its
  middle (Copy enabled with page text selected); nothing with no focused page node, the page itself or a text input
  focused; a fresh right-click still answers at the click.

## Checks

Once, 2026-10-12, on `ec0b0a6b7` (`origin/feat(example)/t3-code` was `fe0e39c91`, this branch's base, with no newer
commits when fetched before the checks), except the Bun suite's passing run, on `b23693933`, which changes one source
test and this record only:

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4664 pass, 1 skip, 0 fail (312 files). The first run, 1: `audit-wave-followups.test.ts` FU-3 pins `chatLocal`'s send chain, which gained the fragment's arm; the test now names it (`b23693933`) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,413 lines; plan 28,903,392 bytes (tip 28,864,485: +38,907) |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed (no Rust changed) |
| AppKit `macos/tests/contextmenu` (README recipe) | 0 | 60 run, 0 failed (2 new); the same tests against the tip's module: 6 failed (RL6-appkit.txt) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 101 | 3,744 passed, 1 failed, 36 ignored: `button_migration::every_app_corpus_and_example_compiles_and_the_button_migration_is_complete` on main's `apps/color-gallery/app.contract:269`, inherited (adopt-main-fixes-r9 AD-1, progress-native-bars); nothing this branch touches |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

`host/apple/build.mjs t3-code-macos --bundle` 0, once, before the live drive (`306ba87f1`'s app code; the code has not
changed since).

## Not done / not verified

- RL-6 under VoiceOver: blocked by the host, X84 (`NodeView` answers no `AXShowMenu`; new, not filed: the coordinator
  files it). The clone's change answers a show-menu `contextmenu` as soon as the host raises one.
- A table cell's link takes no keyboard focus (an inline run; the host's key-view loop lists nodes), so the ContextMenu
  key cannot reach it; the reference's anchor in a cell is Tab-reachable where Chromium Tabs to links. Not compared
  further; recorded here.
- Real-input steps 1–4: the coordinator's session.

## Decision needed

- RL-6's menu position. The reference's shell pops its menu with no position, so Electron places it at the mouse
  pointer, wherever that is, though Chromium reports the focused link's middle. The clone opens it at the focused link
  (the specification's "at the link", and where ExactKit's ContextMenu key and the module's own keyboard menus put it).
  Keep it at the link, or follow the pointer as Electron does (one line in `T3TextContextMenu.focusMenu`)?

## Seen while building, not a finding row

- A table cell's link is underlined and has no favicon in the clone; the reference's has the favicon (web links) and no
  underline (already in X77's notes).
- A cell holding a code span draws its runs with `CellRuns`, whose link runs have no `href` at all (plain text, not
  link-coloured): `` | [`x` mail](mailto:…) | `` reads as prose there.
- The ContextMenu key on a focused web link: the reference opens its app link menu at the link (the renderer passes the
  keyboard event's point: `x: 414, y: 216` for "the x docs", RL-reference.txt); the clone opens the same items through
  `T3ContextMenu`, which places a request with no `anchor: "focus"` at the pointer (`external-link-menu.ts` sends none). A
  web link's row, not RL-6's; not built.

## Delivery

Draft PR [#474](https://github.com/ccheever/exact2/pull/474) into `feat(example)/t3-code`.

## Next action

The coordinator: review the draft PR; answer "Decision needed"; file X84; run real-input steps 1–4 in the next session.
