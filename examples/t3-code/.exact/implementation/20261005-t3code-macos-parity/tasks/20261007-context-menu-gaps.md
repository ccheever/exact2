---
name: 20261007-context-menu-gaps
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
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

Parent specification: [spec](../spec.md). Reference: T3 Code `1e2ecbd975`. The comparison
pass is [20261007-fix-minor-ui-issues](closed/20261007-fix-minor-ui-issues.md). Follow the
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
