---
name: 20261007-context-menu-gaps
plan: 20261005-t3code-macos-parity
implementation: verified
verification: passed
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-context-menu-gaps
pr_url: null
verified_commit: null
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
| Chat file link menu | a reply with a media file link and a code file link | right-click each | "Preview media" on the media link; open and reveal labels as the reference | macOS | unit test of the items |
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

Declared differences:

- "Preview media" on a reply's media file link opens the file in the Files surface's media preview,
  which is where a click on that link already goes in the clone; the reference opens its expanded media
  dialog. The clone has no expanded dialog for reply file links (in-app follow-up, not a framework limit).
- Excluded by scope: "Open in integrated browser" (Browser surface, X1); menu item icons.

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | staged tree on `4f523ef5c`, source `c4485a7d…` (attempt 2) | `bun test examples/t3-code` 2289 tests, 0 fail, 1 skip (+34: `context-menus.test.ts` 26, `context-menu-actions.test.ts` 8); strict tsc (`--target ES2023 --lib ES2023,DOM`) clean; contract build 2543 slots, 45 resources; `cargo test -p t3-code-macos --lib` 11/0; `contextmenu` AppKit binary 18/0 (+4 `FileMenuTests`); caps within budget; macOS bundle builds. Verify runner attempt 1 passed, attempt 2 (after review fixes) passed with `source_unchanged: true` | [recipe](../../evidence/20261007-context-menu-gaps/recipe.json), [report](../../evidence/20261007-context-menu-gaps/attempt2-report.json), [review](../../evidence/20261007-context-menu-gaps/review.md), drive record below | none |

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

Not driven live: the pull request number menu (a Pull Requests row needs a signed-in GitHub or the
fake GitHub fixture, task `20261005-fake-github-fixture`, not built) and the chat file-link menu (a reply
needs an authenticated provider turn). Both are covered by `context-menu-actions.test.ts` (the items sent
to the native op and what each pick does) and `file-menus.swift` (the native layout).

Evidence (before/after):

| Menu | Before | After |
| --- | --- | --- |
| Files tree row | ![files](https://raw.githubusercontent.com/ccheever/exact2/t3-code-evidence/context-menu-gaps/01-files-tree-row-menu-before-after.png) | (same image) |
| Pull request number (text) | no menu (the platform's default) | Copy link · Open on GitHub (GitLab, Forgejo, Bitbucket, Azure DevOps, else "Open on host") |
| Chat file link, media (text) | Open in Cursor (only with an editor) · Reveal in Finder (fixed label) · Copy relative path · Copy full path | Preview media · Open in Cursor / Open in editor · Reveal in Finder / File Explorer / Files (server) · Copy relative path · Copy full path |
