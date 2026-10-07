---
name: 20261007-context-menu-gaps
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-context-menu-gaps
pr_url: https://github.com/ccheever/exact2/pull/203
verified_commit: e00144e3d
---

# The file tree, pull request links and chat file links have T3 Code's context menus

## Outcome

Right-clicking a file in the Files tree, a pull request number, or a file link in a reply opens
the same native menu T3 Code opens, with the same items, order, separators, submenus and
disabled rules. Found by the 2026-10-07 side-by-side pass against T3 Code (Nightly)
(fix-minor-ui-issues): every other reference context menu already matches.

## Scope and exclusions

| Menu | Reference | Clone today | Needed |
| --- | --- | --- | --- |
| Files tree row | `FileBrowserPanel.tsx:184`, items in `fileContextMenu.ts:79-108`: Open, the server-worded reveal item, "Open with ▸" (editors), "Copy mention", "Add to chat" | no `contextmenu=` on file-tree rows | the menu through `T3ContextMenu.swift` (it needs submenu support for "Open with ▸": `T3ContextMenu.swift:31-55` has none; `T3Sidebar.swift:131-155` `nativeTemplate` has) |
| Pull request link | `pullRequestLinkContextMenu.ts:24-29`, used by `PullRequestRow.tsx:152` and `PullRequestDetailPanel.tsx:319`: "Copy link", "Open on GitHub / GitLab / Forgejo / Bitbucket / Azure DevOps" (or "Open on host") | no right-click on a pull request number; the painted "Copy link" menus in `r4-surfaces.contract:390` and `pages-pr-detail.contract:349` are different menus | the menu on the pull request rows and the detail header, host label from the provider |
| Chat file link | `ChatMarkdown.tsx:2251-2261` | `r4-surfaces-files.ts:273-278` | "Preview media" for media links in a thread; the open item always shown when shell actions are allowed ("Open in editor" without a preferred editor or with `file-manager`, else "Open in <Editor>"); the reveal label from the server's platform (`shellRevealInFileManagerKind`), not a fixed "Reveal in Finder" |

Excluded: "Open in integrated browser" (Browser surface, X1); menu item icons (the clone's
native menus draw only the destructive trash icon, as today).

## Context and guidance

Parent specification: [spec](../../spec.md). Reference: T3 Code `1e2ecbd975`. The comparison
pass is [20261007-fix-minor-ui-issues](20261007-fix-minor-ui-issues.md). Follow the
clone's existing menus: `sidebar-menu.ts` builds items, the host shows them natively.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| none | — | — | — | — |

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Files tree menu | a thread with a workspace, Files open | right-click a file row | the five reference items; "Open with ▸" lists the detected editors | macOS | AppKit test of the template, drive record |
| Pull request link menu | the Pull Requests page with a row | right-click the row's number | "Copy link", "Open on GitHub" | macOS | unit test of the items, drive record |
| Chat file link menu | a reply with a media file link and a code file link | right-click each | "Preview media" on the media link; open and reveal labels as the reference | macOS | unit test of the items, drive record |
| Preview media | the media link's menu | pick Preview media | the expanded media dialog with the link's media (resolveMarkdownMediaPreview) | macOS | unit test, drive record |
| Right-click hookup | — | `context-menu-hookup.test.ts` | the nodes carry the handlers; their ops reach the native `contextMenu` op | — | unit test |
| Native layout | — | `contextmenu` AppKit binary | submenu parent never picked; a child's id is the pick; empty children are a plain item | macOS | `macos/tests/contextmenu/file-menus.swift` (4 tests) |

## Implementation

| Menu | Clone | Reference |
| --- | --- | --- |
| Files tree row | `r4-surfaces-files.contract` tree row `contextmenu=local("surface-files-row-menu", …)` → `context-menu-actions.ts` `filesTreeMenu` | `FileBrowserPanel.tsx` showEntryContextMenu, `fileContextMenu.ts` |
| Pull request number | the row's number (`pages-prs.contract`, `control("link-menu", row.linkMenu)` → `pageslocal:pr-link-menu`) and the detail header's number link (`pages-pr-detail.contract`, `local("pr-link-menu", …)` → `chatLocal`), both `pullRequestLinkMenu` | `PullRequestRow.tsx:152`, `PullRequestDetailPanel.tsx:319`, `pullRequestLinkContextMenu.ts` |
| Chat file link | `r4-surfaces-files.ts` `markdownFileMenu` | `ChatMarkdown.tsx:2251-2261` |

The items are built in `context-menus.ts` (ports of `buildFileContextMenuItems`,
`resolveFileContextMenuAbsolutePath` with `resolveDiffPathForWorkspace`, the reveal labels by kind and OS,
`openInEditorMenuLabel`, `openOnHostLabel`); none of the three menus has a separator, a disabled or a
destructive item, as in the reference. `T3ContextMenu.swift` now builds a submenu for an item with
children (ElectronMenu `buildTemplate`): the parent is never the pick, the child's id is. The chat link's
open item is always shown when shell actions are allowed, named for the preferred editor (the last one
used, else the first available), and the reveal label comes from `shellRevealInFileManagerKind`, else
`environment.platform.os`. Opening a link uses the module's `terminalOpenExternal` (http/https only).

"Preview media" opens the expanded media dialog (`timeline-attachments.ts` `openMarkdownMediaPreview`, the
same `ImagePreviewDialog` the sent attachments use) with the link's media alone, signed with
`assets.createUrl` as a `media-file` of the shown thread before the dialog opens (a refusal is the
reference's "Media unavailable" toast and no dialog), its media actions from the media's own source
(`imagePreviewSource`), as `resolveMarkdownMediaPreview` and `markdownImageGallery` give for a link (no
inline image matches it, so it is the only item). Retry video signs it again; leaving the thread drops it.

Declared differences: none. Excluded by scope: "Open in integrated browser" (Browser surface, X1); menu item icons.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | staged tree on `4f523ef5c`, source `c4485a7d…` (attempt 2) | `bun test examples/t3-code` 2289 tests, 0 fail, 1 skip (+34: `context-menus.test.ts` 26, `context-menu-actions.test.ts` 8); strict tsc (`--target ES2023 --lib ES2023,DOM`) clean; contract build 2543 slots, 45 resources; `cargo test -p t3-code-macos --lib` 11/0; `contextmenu` AppKit binary 18/0 (+4 `FileMenuTests`); caps within budget; macOS bundle builds. Verify runner attempt 1 passed, attempt 2 (after review fixes) passed with `source_unchanged: true` | [recipe](../../evidence/20261007-context-menu-gaps/recipe.json), [review](../../evidence/20261007-context-menu-gaps/review.md), drive 1 below | none |
| 2 (2026-10-07, coordinator follow-up) | `6281fd187` + review fixes, attempt 5 report | Preview media builds the expanded media dialog (signed first, Retry, dropped on thread change); `context-menu-hookup.test.ts` (2) and preview tests (+2); `bun test examples/t3-code` 2293 tests, 0 fail, 1 skip; strict tsc clean; contract build 2543 slots; caps within budget; `contextmenu` AppKit 18/0; bundle builds; verify runner attempt 5 passed with `source_unchanged: true` | [report](../../evidence/20261007-context-menu-gaps/attempt5-report.json), [review](../../evidence/20261007-context-menu-gaps/review.md), drive 2 below | none |

Drive (one session, 2026-10-07, under `.t3-live-drive-lock`): lane server `1e2ecbd975` runtime on
127.0.0.1:16481 with an isolated HOME/CODEX_HOME/XDG/T3CODE_HOME, a fixture repository (`README.md`,
`src/`, `docs/`, `shots/screen.png`) as project Alpha with one thread "Menu demo" (`target/lane-cmg`,
not committed). Each app was launched by path with `CFFIXED_USER_HOME` set to a lane directory (data
root isolated; the preferences domain was exported first and imported back after, the lane's Keychain
item deleted), paired through Settings › Connections › Add environment, then the thread and the Files
surface were opened and `README.md` was right-clicked with `orca computer click --mouse-button right`
(real event, app targeted by pid); the menu was captured with `screencapture` at 1352×845, dark.

```
BEFORE  t3-code-evidence-base @ 4f523ef5c (pid 88805): right-click README.md → no menu (row hover only)
AFTER   this branch (pid 14104): right-click README.md → Open · Reveal in Finder · Open with ▸ · Copy mention · Add to chat
        click "Open with" → submenu Cursor · VS Code · Zed (the lane server's detected editors); Escape closes it
```

Drive 2 (one session, 2026-10-07 13:06, under `.t3-live-drive-lock`; a 11:36 attempt was abandoned before
any input because the screen was locked). Same lane server, now with lane-only fixtures (`target/lane-cmg`,
not committed):

- a fake Codex: `bin/codex` runs the reference's own mock app-server peer
  (`apps/server/src/provider/testFixtures/codexCollabMockPeer.mjs`, copied) with a scripted reply,
  "Here is the screenshot [screen.png](…/shots/screen.png) and the entry point [index.ts](…/src/index.ts)."
  One `message.dispatch` from the lane RPC script produced it; no provider account was used.
- a fake GitHub CLI: `bin/gh` answers `auth token`, `api user` and the GraphQL search and detail reads for
  `t3-fixture/menu-demo` #7 from canned JSON and logs every call; the fixture repository's `origin` is that
  identity behind a dead proxy and a `/dev/null` push URL. No network, no real account; this is not the
  held `20261005-fake-github-fixture` task.
- lane-only preferences: each app is a copy of its bundle with its own bundle id
  (`com.exact.t3code.lanecmg.before` / `.after`, ad-hoc signed), so its preferences domain and data root are
  the lane's. The user's `com.exact.t3code.macos` domain hashes the same before and after the drive
  (`acc150dd…`); the lane's Keychain item was deleted.

```
BEFORE  base copy (pid 43864): media link → Open in Cursor · Reveal in Finder · Copy relative path · Copy full path
        code link → the same four; PR row #7 → no menu; detail header #7 → no menu
AFTER   branch copy (pid 79828): media link → Preview media · Open in Cursor · Reveal in Finder · Copy relative path · Copy full path
        Preview media → the expanded media dialog with screen.png, its name and close
        code link → Open in Cursor · Reveal in Finder · Copy relative path · Copy full path
        PR row #7 → Copy link · Open on GitHub; detail header #7 → Copy link · Open on GitHub
```

Drive 2 ran on `6281fd187`. The second review's fixes came after it (sign before opening, Retry, drop on
thread change); their success path shows the same dialog and the changed paths are unit-tested
(`context-menu-actions.test.ts`), not re-driven (the drive budget was spent).

Evidence (before/after, one image per menu):

| Menu | Image |
| --- | --- |
| Files tree row | ![files](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/01-files-tree-row-menu-before-after.png) |
| Chat file link, media | ![media](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/02-chat-media-link-menu-before-after.png) |
| Chat file link, code | ![code](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/03-chat-code-link-menu-before-after.png) (unchanged in this fixture: an editor is available and the server is macOS) |
| Preview media | ![preview](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/04-preview-media-dialog-before-after.png) |
| Pull Requests row number | ![row](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/05-pr-row-number-menu-before-after.png) |
| Pull request detail header number | ![detail](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/06-pr-detail-number-menu-before-after.png) |
