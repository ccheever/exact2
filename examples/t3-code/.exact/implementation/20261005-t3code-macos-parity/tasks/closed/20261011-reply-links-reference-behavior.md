---
name: 20261011-reply-links-reference-behavior
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-reply-links-reference-behavior
pr_url: https://github.com/ccheever/exact2/pull/474
verified_commit: 1f813789b
---

# Reply links as the reference has them: a fragment leaves the thread, a table cell's mailto opens nothing, the ContextMenu key's shell menu

## Outcome

The user decided on 2026-10-11 to match the reference on the three reply-link rows
[reply-links](20261011-reply-links.md) kept (its "Decision", "Not done" and "Seen while building"; `STATUS.md`
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
  or ⌃ held the handler leaves to Chromium, measured over CDP on 2026-10-12 (review fix, with the main process read back,
  [RL4-modified-clicks.txt](https://raw.githubusercontent.com/ccheever/exact2/d27d2a1a4a5b251d47f4dbf4387093bec565c334/reply-links-reference-behavior/RL4-modified-clicks.txt)).
  ⌘ asks for a background tab and ⇧ for a new window, and `setWindowOpenHandler` denies both, so nothing opens. ⌥ is
  Chromium's download gesture: `will-download` gets `t3code://app/#notes` as `download.html` with no save path, so
  Electron shows its Save dialog. ⌃ is the Mac's secondary click: the shell's menu opens (Cut, Copy, Paste, Select All),
  as for a right-click. The thread stays in all four cases, the same for the cell's link.
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
- **RL-6** (`modules/apple/T3TextContextMenu.swift` `shellMenuForLastClick` / `focusMenu`, `external-link-menu.ts`):
  ExactKit raises the focused link node's `contextmenu` for the ContextMenu key (`MouseEventsMac.swift`
  `contextMenuKey`), its handler asks the module for the shell's menu (`shellMenu`), and `shellMenu` answered only a fresh
  page click (reply-links review R1-4). The module's monitor now keeps the last ContextMenu key with no modifier as it
  keeps the last page click: its window and the node that had the focus. `shellMenu` with no fresh click answers that key
  once, while it is the last input (any other key or click ends it, and so does the app's own menu) and the node still
  holds the focus. It opens the shell's menu for that node: read-only Cut, Copy (enabled while page text is selected),
  Paste, Select All, no Copy Link for a URL the shell refuses, at the node's middle (`visibleRect` ∩ bounds, which on
  macOS 14 is clipped by the ancestors that clip, as ExactKit's ContextMenu key is; ExactKit also follows a CSS
  transform, which an app module cannot read and a reply's links do not have). The request names the link's `href`
  (`{ op: 'shellMenu', href }`), and a focused node whose `AXURL` is another link opens nothing (a `link` node's `AXURL`
  is its `href` since main #455, X78). A `contextmenu` that neither a fresh click nor a fresh ContextMenu key raised
  opens nothing, as before this task. The agent's keys reach its window past NSApplication (AgentMac `win.sendEvent`), so
  under the agent a `contextmenu` with no click is taken as its ContextMenu key at the focus of the window it drives;
  the agent's right-click goes through NSApplication and is a click. VoiceOver's show menu reaches no node on the host:
  `NodeView` answers no `AXShowMenu` and AppKit's `NSView` default performs nothing
  ([probe](https://raw.githubusercontent.com/ccheever/exact2/c838acad2f1032855738ec15598a80086a992cdf/reply-links-reference-behavior/X84-show-menu-probe.txt)),
  so no `contextmenu` runs: `EXACT2-GAPS.md` X84, filed on main by
  [#483](https://github.com/ccheever/exact2/pull/483) (`issues/20261012-macos-voiceover-show-menu.md`) and fixed on main by
  [#486](https://github.com/ccheever/exact2/pull/486), which this branch takes at its next adoption of main.
- **Review fix (2026-10-12).** The first version answered every `shellMenu` with no fresh click at whichever page node
  was focused, in the key, main or any ordered window. So a key or click between a right-click and the bridge's reply, a
  right-click older than 5 s, or a `contextmenu` no key raised opened a menu at a focused node that could be another
  link or in another window. The key record and the `href` check above close those cases. The AppKit tests that say
  nothing opens now hold a focused link, and the same tests built against the first version's module fail 23
  assertions ([RL6-review-guard-appkit.txt](https://raw.githubusercontent.com/ccheever/exact2/af7bca69678a1f2795eddf0d0fba02f37ac7829f/reply-links-reference-behavior/RL6-review-guard-appkit.txt)).
- **Plan size** (RL-4 and RL-5's contract changes together): 28,864,485 bytes on the tip (`fe0e39c91`'s sources),
  28,903,392 after (+38,907; the +308,280 bytes the reply-links record measured for a separate press on a cell's run is not
  spent: the merged branch adds no handler site).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| RL-4 | pass (reference, Bun, agent drive): before, a click on "notes" of "the notes" left the thread up (the later word has no press) and a cell's "the summary" was refused by the host (`exact: refused to open #summary`); after, both leave for "work / New thread" ("What should we build in work?") with the composer focused (`fragmentChanged`, `linkOpen` presses in the log), as the reference's `#/draft/…`. Modified clicks, measured on the reference in the review fix: ⌘ and ⇧ open nothing on both (the reference's window request is denied), pass; ⌥ opens nothing in the clone where the reference shows Electron's Save dialog for `download.html`, a declared difference decided not matched (EXACT2-GAPS "Reply links: ⌥-click on a fragment link"); ⌃ opens the reference's shell menu and is a press with Control held in the clone, which opens nothing (X85, filed on main by #485, fixed on main by #487, not adopted yet). The review round's drive: the ⌥- and ⌃-clicks on "notes" keep the thread | [modified clicks](https://raw.githubusercontent.com/ccheever/exact2/d27d2a1a4a5b251d47f4dbf4387093bec565c334/reply-links-reference-behavior/RL4-modified-clicks.txt), [clone ⌥ and ⌃](https://raw.githubusercontent.com/ccheever/exact2/c2ae9e0c2d869839bdae6ee92d5b6d05640bcb1a/reply-links-reference-behavior/RL4-modified-clicks-clone.png), [review drive](https://raw.githubusercontent.com/ccheever/exact2/4d180be3357a82131965d7ae80e6b529aca32b20/reply-links-reference-behavior/RL-review-drive.txt), [prose](https://raw.githubusercontent.com/ccheever/exact2/548ff623a4ab1e00aed20e783eb6dc4252cf8732/reply-links-reference-behavior/RL4-fragment-in-prose.png), [table cell](https://raw.githubusercontent.com/ccheever/exact2/7103638e1321d2156bce0611bb44a6067bda19af/reply-links-reference-behavior/RL4-fragment-in-table-cell.png), [drive](https://raw.githubusercontent.com/ccheever/exact2/e7b9ba2ad4e38e7e3ea9d7dc5e3a6b57fa1b9704/reply-links-reference-behavior/RL-drive-before-after.txt), [reference](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt) |
| RL-5 | pass (reference, Bun, agent drive, plan): before, a click on the cell's irc link went to the host (`exact: refused to open irc://irc.example.test/t3`; the mailto one would reach the mail app, not clicked on the tip); after, both are the link's press (`linkOpen`) and nothing opens (no host line, no opened URL), as the reference. Plan +38,907 bytes for RL-4 and RL-5 together (accepted ceiling +308 KB) | [drive](https://raw.githubusercontent.com/ccheever/exact2/e7b9ba2ad4e38e7e3ea9d7dc5e3a6b57fa1b9704/reply-links-reference-behavior/RL-drive-before-after.txt), [fixture](https://raw.githubusercontent.com/ccheever/exact2/67b7935f6c4023c6b26bcabb3f03ef8afefd1cbe/reply-links-reference-behavior/RL5-RL6-fixture-reply.png), [reference](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt) |
| RL-6 | partial. The ContextMenu key: pass (reference, AppKit, agent drive): before, the key on the focused "the team", "the channel", "chat" and "the notes" ran their `contextmenu` and opened no menu; after, each opens the shell's Cut, Copy (disabled), Paste (disabled), Select All with no Copy Link (`t3.textmenu:`), as the reference; the AppKit test puts it at the link's middle (6 of its assertions fail on the tip's module). Review fix: it answers only a ContextMenu key the module saw, at the node it went to, for the link the request names; the updated AppKit tests fail 23 assertions on the first version's module and pass on this one (64 run); the review round's drive opens the same menu for the four keys and for a right-click on "the team", and none for the ⌥- or ⌃-click. VoiceOver's show menu: blocked by the host on this branch (X84, filed on main by #483, fixed on main by #486, not adopted yet). A table cell's link: no keyboard focus on the host (X77's note, filed on main by #484) | [review drive](https://raw.githubusercontent.com/ccheever/exact2/4d180be3357a82131965d7ae80e6b529aca32b20/reply-links-reference-behavior/RL-review-drive.txt), [AppKit guard](https://raw.githubusercontent.com/ccheever/exact2/af7bca69678a1f2795eddf0d0fba02f37ac7829f/reply-links-reference-behavior/RL6-review-guard-appkit.txt), [drive](https://raw.githubusercontent.com/ccheever/exact2/e7b9ba2ad4e38e7e3ea9d7dc5e3a6b57fa1b9704/reply-links-reference-behavior/RL-drive-before-after.txt), [AppKit](https://raw.githubusercontent.com/ccheever/exact2/718d89fa32e199dd9b2cd7cdb243a1b0ae24707a/reply-links-reference-behavior/RL6-appkit.txt), [reference](https://raw.githubusercontent.com/ccheever/exact2/7f6f6ded49718e1f75932f3807573c454ef90054/reply-links-reference-behavior/RL-reference.txt), [X84 probe](https://raw.githubusercontent.com/ccheever/exact2/c838acad2f1032855738ec15598a80086a992cdf/reply-links-reference-behavior/X84-show-menu-probe.txt) |
| RL-4..RL-6 under real keys and pointer | open: real-input steps 1–4 below | — |

The review fix's drive (one, on this branch's bundle at `48d41d0be`'s app code, built once): [RL-review-drive.txt](https://raw.githubusercontent.com/ccheever/exact2/4d180be3357a82131965d7ae80e6b529aca32b20/reply-links-reference-behavior/RL-review-drive.txt).
Drives of the first round: one per build, the same steps ([drive.sh](https://raw.githubusercontent.com/ccheever/exact2/aaba5ca7c6c7c4b69e480080bc147b5cec51aec8/reply-links-reference-behavior/drive.sh.txt)): before on the evidence worktree at `6db5cadea`, after
on this branch's bundle (`306ba87f1`'s app code; built once). The cell's mailto link was clicked on the after build only.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, the lane home outside any git checkout) on a lane whose
fixture reply holds the links (`sqlite3 <lane>/userdata/statev2.sqlite "update orchestration_v2_projection_turn_items set
payload_json = json_set(payload_json, '$.text', cast(readfile('fixture-text.txt') as text)) where turn_item_id =
'fixture-markdown'"`, [fixture-text.txt](https://raw.githubusercontent.com/ccheever/exact2/028d7d177966fcf272a2a054011db74fea44a0dc/reply-links-reference-behavior/fixture-text.txt)); the Verification fixture thread open. Never send a message.

1. **RL-4.** Click the word "notes" of "the notes": the window shows "work / New thread" (or the lane's most recently
   active project), "What should we build in …?", the caret in the composer. Reopen the thread; click "the" of "the
   notes": the same. Reopen; ⌘-click "notes": nothing (the thread stays); ⌥-click it: nothing, and no Save dialog (the
   reference's Save dialog is not matched, decided 2026-10-12); ⌃-click it: nothing in the clone (X85: a press with
   Control held), where the reference opens its shell menu. Click the table's "the summary": the draft.
2. **RL-5.** Click the table's "the list" (mailto): no Mail window, the clone stays in front, no toast. Click "the
   room" (irc): nothing. Click "the site": it opens where Settings › "Open links in" says.
3. **RL-6, ContextMenu key** (a keyboard with a ContextMenu key, such as a PC keyboard; a Mac keyboard has none: skip and
   say so). Tab to "the team" (or click it: it opens nothing and takes the focus); press the ContextMenu key: Cut, Copy,
   Paste (disabled), Select All, no Copy Link, at the link. The same on "the notes". Right-click "the channel", then
   press any letter key and the ContextMenu key with "the team" focused: one menu per input, each at its own place. Note
   where the reference's menu opens for the same key (at the pointer, by its source).
4. **RL-6, VoiceOver** (⌘F5 on, off afterwards). Move the VoiceOver cursor to "the team"; VO-⇧-M: expected nothing in the
   clone (X84, fixed on main by #486 but not adopted yet). The same in the reference T3 Code (Alpha) lane window: record
   whether its shell menu opens.

## Tests

- `browser-links.test.ts`: a fragment link leaves for the most recently active project's draft ("work"), answering
  `sidebar:new-thread`, under either "Open links in" value, for prose's and a cell's fragment; ⌘, ⌥, ⇧, ⌃ or ⌘⇧ held
  opens nothing; the mailto/irc/ircs/xmpp row keeps the thread.
- `reply-links-reference-behavior.test.ts` (new, 2): the window sends a fragment open on `fragmentChanged` and its
  completion closes the utility page, resets the jump and draft owner and focuses the composer, as `commandCompleted`'s
  new-thread handling; `chatlocal:link-open` end to end through the client.
- `external-link-menu.test.ts`: ChatRuns gives every link run with a destination the link's press and menu in one branch
  (an emptied anchor neither); FlowRuns' later-word box covers fragment links (`pressLink`) with `tabindex=-1` for them;
  a mailto link's menu asks the module for `{ op: 'shellMenu', href }`.
- `macos/tests/contextmenu/reply-links.swift` (+6, 4 reworded): the ContextMenu key (kVK_ContextualMenu through
  `NSApp.sendEvent`) on a focused link opens the shell's menu once, at its middle (Copy enabled with page text
  selected); at the middle of the part a clipping ancestor shows, and nothing when none shows; nothing for a
  `contextmenu` no fresh click or ContextMenu key raised, with a link focused: no key, another key, ⇧ContextMenu, a key
  or click after the ContextMenu key, a key older than 5 s, the app's own menu; nothing where the focus moved after the
  key, where the focused node's `AXURL` names another link, with the window or a text input focused; a fresh
  right-click still answers at the click; under the agent the focus of its window answers a `contextmenu` with no
  click, and not a node that names another link. The older tests that say a stale click, one already answered, or a key
  or click after it opens nothing now hold a focused link.

## Checks

Review round, once, 2026-10-12, on `ffec16a91`: this branch's review fix merged with `origin/feat(example)/t3-code`
at `ca4f28e35` (#482), fetched right before the checks; no conflict.

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4712 pass, 1 skip, 0 fail (319 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | `app.contract` 1,433 lines (after the merges); plan 29,247,606 bytes on the merged head. RL-4 and RL-5's +38,907 bytes were measured on `fe0e39c91` (first round), and the review fix changes no contract |
| `cargo test -p t3-code-macos --lib` | 0 | 21 passed (no Rust changed) |
| AppKit `macos/tests/contextmenu` (README recipe) | 0 | 64 run, 0 failed; the same tests against the first version's module: 23 failed (RL6-review-guard-appkit.txt) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored (the first round's inherited `button_migration` failure is gone on this base) |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |

`host/apple/build.mjs t3-code-macos --bundle` 0, once, before the review round's live drive (`48d41d0be`: the review
fix's app code; the merge after it brought #482's contract and test changes, none in this task's files).

First round (2026-10-12, on `ec0b0a6b7`): every check 0 except `cargo test` 101 (`button_migration`, inherited from
main's `apps/color-gallery`); Bun 4664 pass; AppKit 60 run, 0 failed.

## Not done / not verified

- RL-6 under VoiceOver: blocked by the host on this branch, X84 (`NodeView` answers no `AXShowMenu`), filed on main by
  [#483](https://github.com/ccheever/exact2/pull/483) (`issues/20261012-macos-voiceover-show-menu.md`) and fixed on main
  by [#486](https://github.com/ccheever/exact2/pull/486) (2026-10-12), not adopted yet. At the adoption the host raises the
  link's `contextmenu` for VoiceOver's show menu, but `shellMenu` answers only a fresh right-click or a ContextMenu key its
  monitor saw, so the shell's menu needs a change then (read from the code, not driven); real-input step 4 records it.
- A table cell's link takes no keyboard focus (an inline run; the host's key-view loop lists views), so Tab skips it
  and the ContextMenu key cannot reach it, where the reference's anchor in a cell is a Tab stop: X77's note in
  `EXACT2-GAPS.md`, filed on main by [#484](https://github.com/ccheever/exact2/pull/484)
  (`issues/20261012-macos-inline-link-run-tab-stop.md`).
- A ⌃-click on a reply's link: the reference opens its shell menu (Chromium's secondary click on a Mac); the clone
  presses the link with Control held (a fragment, mailto, irc or xmpp link opens nothing): X85, a host gap, filed on main
  by [#485](https://github.com/ccheever/exact2/pull/485) (`issues/20261012-macos-control-click-contextmenu.md`) and fixed on
  main by [#487](https://github.com/ccheever/exact2/pull/487) (2026-10-12), not adopted yet. At the adoption a Control-click
  runs the link's `contextmenu`, but the module's monitor takes only a right mouse-down as the page click `shellMenu`
  answers, so the shell's menu needs a change then (read from the code, not driven).
- Real-input steps 1–4: ran in `realinput-1011c` ("Real-input results" below).

## Decision needed: decided

- RL-6's menu position: **decided, it stays at the focused link** (coordinator, 2026-10-12). A keyboard-invoked menu
  opens at the focused element on the Mac; the reference's pointer position is Electron's default (its shell pops the
  menu with no position, though Chromium reports the focused link's middle). The clone keeps `T3TextContextMenu.focusMenu`
  as built, where ExactKit's ContextMenu key and the module's own keyboard menus put it.
- RL-4's ⌥-click on a fragment link: **decided, not matched** (coordinator, 2026-10-12). The reference's Save dialog
  for `download.html` is Chromium's download gesture in the browser shell; the clone follows the platform for what the
  app does not say (LLP 1115) and opens nothing. Declared in `EXACT2-GAPS.md` ("Reply links: ⌥-click on a fragment
  link").

## Seen while building, not a finding row

- A table cell's link is underlined and has no favicon in the clone; the reference's has the favicon (web links) and no
  underline (already in X77's notes).
- A cell holding a code span draws its runs with `CellRuns`, whose link runs have no `href` at all (plain text, not
  link-coloured): `` | [`x` mail](mailto:…) | `` reads as prose there.
- The ContextMenu key on a focused web link: the reference opens its app link menu at the link (the renderer passes the
  keyboard event's point: `x: 414, y: 216` for "the x docs", RL-reference.txt); the clone opens the same items through
  `T3ContextMenu`, which places a request with no `anchor: "focus"` at the pointer (`external-link-menu.ts` sends none). A
  web link's row, not RL-6's; not built.

## Review fixes (2026-10-12)

An independent review of the PR found five should-fix problems:

| # | Finding | Resolution |
| --- | --- | --- |
| 1 | X84 had no issue number | Filed on main by [#483](https://github.com/ccheever/exact2/pull/483) (`issues/20261012-macos-voiceover-show-menu.md`); `EXACT2-GAPS.md` X84 and this record name it |
| 2 | A table cell's link takes no keyboard focus: recorded only here, not in `EXACT2-GAPS.md` | A note under X77 in `EXACT2-GAPS.md`, filed on main by [#484](https://github.com/ccheever/exact2/pull/484) (`issues/20261012-macos-inline-link-run-tab-stop.md`); reproduced again on main `85a116a9c` against the web target while filing X85 |
| 3 | `focusMenu` answered every `shellMenu` with no fresh click at any focused page node, in any window | The module answers only a ContextMenu key its monitor saw, at the node it went to while that node keeps the focus, and only where the node's `AXURL` is the href the request names (now sent); see "Cause and fix", RL-6. The reviewer's suggested check by `AXURL` works because a `link` node's `AXURL` is its `href` (main #455) |
| 4 | The fragment link's modified clicks were read from source, and ⌥ in particular was unconfirmed | Measured over CDP with the main process read back: ⌘ and ⇧ open nothing (match); ⌥ shows Electron's Save dialog (decided not matched, declared); ⌃ opens the shell's menu (X85, a host gap, filed on main by [#485](https://github.com/ccheever/exact2/pull/485)) |
| 5 | AppKit tests whose names said "nothing opens" passed only because nothing was focused; the menu's place differs from ExactKit's `drawnRect` | Those tests hold a focused link now and pass because of the key record (the first version's module fails 23 assertions in them). A new test shows the menu's middle follows a clipping ancestor and not a non-clipping one, as ExactKit's walk does (`visibleRect` on macOS 14); the remaining difference, a CSS transform, is noted in the code (a reply's links have none) |

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, RL-4 | RLR-4 (B13) | Pass, with round 11's (g)–(j): a ⌃-click is the secondary click (X85) | [RLR-4-ctrl-site-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/RLR-4-ctrl-site-pair.png) |
| 2, RL-5 | RLR-5 (B13) | Pass: mailto and irc open nothing, the web link opens the default browser, in both apps | [RLR-5-mailto-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/RLR-5-mailto-pair.png) |
| 3, RL-6 (the ContextMenu key) | RLR-6 (B13) | Fail (clone; step 3 passes): the ContextMenu key on a link gives AppKit's text menu (Look Up …) or nothing, not the shell's menu, and a click does not focus the link | [RLR-6-team-pair.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/pairs/RLR-6-team-pair.png) |
| 4, RL-6 (VoiceOver) | RLR-6v (B6x) | Fail (clone): VO-⇧-M opens nothing on a link or a thread row, with round 11's X84 fix in the bundle (reference: the shell's menu and the row's menu) | [RLR-6v-pair.png](https://raw.githubusercontent.com/ccheever/exact2/ed5fa6cb8cfea023e254f918544201ae53abcd5f/realinput-1011c/b6x-voiceover/pairs/RLR-6v-pair.png) |

Not passed: RLR-6 and RLR-6v.

## Delivery

Merged on 2026-10-12 as `20ce9dfdb` (#474, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`; RLR-6 and RLR-6v are in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
