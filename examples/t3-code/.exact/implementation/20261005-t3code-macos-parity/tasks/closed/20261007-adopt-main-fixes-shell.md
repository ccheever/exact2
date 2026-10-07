---
name: 20261007-adopt-main-fixes-shell
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-adopt-main-fixes-shell
pr_url: https://github.com/ccheever/exact2/pull/181
verified_commit: null
---

# The clone uses main's shell, image and hover fixes instead of its workarounds

## Outcome

The feature branch merged `origin/main` on 2026-10-07 (`dbae6c2e0`). For each issue main fixed in
the window, image and hover areas, the clone's workaround is removed and main's capability is used,
so the clone behaves like T3 Code where it did not before. What main's fix does not cover stays
recorded with what is missing.

## Scope and exclusions

| Issue (plan id) | Main PR | Clone workaround or gap | Adoption |
| --- | --- | --- | --- |
| [#106](https://github.com/ccheever/exact2/issues/106) (X7) | #173 `app.json` `appTransportSecurity` | none: rendered HTML in the bundle could not load `http://` from a named host | set `host.macos.appTransportSecurity.allowsArbitraryLoadsInWebContent` |
| [#113](https://github.com/ccheever/exact2/issues/113) (X27) | #164 frame restored after the final style | `R8PointerWindowFrame.swift`, the app's own frame record | delete it; keep `T3WindowChrome` (title row) and `T3FullScreen` (full-screen fact), which #164 does not cover |
| [#121](https://github.com/ccheever/exact2/issues/121) (X44) | #177 `image` `load` / `error` | none: a broken image kept its box | the reference's `onError` fallbacks on chat Markdown, expanded, Files and attachment images and the theme result icon |
| [#139](https://github.com/ccheever/exact2/issues/139) (X24) | #174 hover follows layout and scrolling | `t3-rehover` hook on the thread list and the legacy project list, R10Connect's passes | remove them |
| [#122](https://github.com/ccheever/exact2/issues/122) (X45) | #170 | check only | not adopted: #170 only moves `reload()`'s log to stderr; no process relaunch |
| [#137](https://github.com/ccheever/exact2/issues/137) (XS3) | #159 | check only | the clone still compiles |

Excluded: framework changes; issues still open (#135 stays recorded); `img onError` sites the
reference has and the clone never cited as #121 workarounds (see "Not adopted").

## Context and guidance

Parent specification: [spec](../../spec.md). Brief: wave 4, ADDENDUM 10 "Adoption tasks". Reference:
T3 Code `1e2ecbd975` (`ChatMarkdown.tsx` ChatMarkdownImage, `ExpandedImageDialog.tsx`,
`FilePreviewPanel.tsx` WorkspaceImagePreview, `AttachmentFilePreview.tsx`,
`settings/ThemeSearchSection.tsx` ThemeExtensionIcon). Port range 16380–16399.

## Dependencies

| Kind | Readable task/issue/decision | Remote reference | Required condition | Resolution evidence |
| --- | --- | --- | --- | --- |
| main merge | feature branch | `dbae6c2e0` | origin/main (with #159, #164, #170, #173, #174, #177) in the feature branch | `git merge-base --is-ancestor` true for each merge commit |
| merged task PR | terminal-drawer | #175 | rebased on it before the PR | rebased onto `1a50d0df3` |

## Implementation notes

One commit per issue; the two `T3Module.swift` registration lines are their own commit.

## Acceptance and reproduction

| Criterion | Setup/reset and fixture | Action or command | Expected result | Required platform | Proof |
| --- | --- | --- | --- | --- | --- |
| Named-host http in the bundle | lane server 16380, HTML fixture 16381, `media/page.html` linking `http://localtest.me:16381/style.css` and `/badge.png` | drive the assembled `.app` (`EXACT_MAC_BIN`), open the page in Files | both requests reach the fixture; styled page | macOS | drive records, before/after shot |
| Broken image | `media/broken.png` (not an image) | open it in Files | "Unable to load workspace image." | macOS | drive records, shot |
| Hover follows a scroll | 30 seeded threads | hover a row, wheel the list | the row under the resting pointer is hovered | macOS | `state` hover id |
| Frame kept | — | AppKit `r8-pointer` | the title row keeps the restored frame | macOS | test log |
| Regression gates | — | clone checks, AppKit binaries, five checks | green | macOS | numbers below |

## Progress

Implemented 2026-10-07 on the feature branch `1a50d0df3` (after #175). Verification: unverified.

| Issue | Result | Detail |
| --- | --- | --- |
| #106 (X7) | adopted | `app.json` `host.macos.appTransportSecurity: { allowsArbitraryLoadsInWebContent: true }`; the bundle's Info.plist carries `NSAppTransportSecurity {NSAllowsArbitraryLoadsInWebContent: true}` (the BEFORE bundle has no key). Web views only; `URLSession` in the module stays under ATS. #135 stays open and is recorded; no clone page is served from `assets/`. |
| #113 (X27) | partly adopted | `R8PointerWindowFrame.swift` deleted (host autosave only). Still missing on main: a title-row height / traffic-light setting and a full-screen fact, so `T3WindowChrome.swift` (empty unified toolbar, now `T3WindowChrome.titleRow`) and `T3FullScreen.swift` stay. |
| #121 (X44) | adopted | chat Markdown image → "Image unavailable · <alt>" chip; expanded image → "Image unavailable. The file may have been moved or deleted."; Files → "Unable to load workspace image."; attachment preview → "Unable to load image."; theme result → palette glyph. SVG on Apple is still an `error` (main draws none). |
| #139 (X24) | adopted | `t3-rehover` removed from `app.json`, `sidebar.contract`, `legacy-sidebar.contract` (with their `data-frame` trigger), R10Connect's passes and their AppKit test. `T3TimelineTooltip`'s scroll latch stays (main's re-hover is a hover, not mouse motion). |
| #122 (X45) | not adopted | main has no process relaunch (#170 only writes `reload()`'s line to stderr). local-primary-environment and this-machine-network-access relaunch rows (decision U4) stay blocked. |
| #137 (XS3) | confirmed | `contract build` passes. |

Not adopted (not cited as #121 workarounds; same `error=` pattern would apply): PR avatar
(`pullRequestPresentation.tsx:444`, `pages-prs.contract` PrAvatar), project favicon
(`ProjectFavicon.tsx`), Markdown link favicons (`ChatMarkdown.tsx` MarkdownLinkFavicon), the ACP
registry icon (provider-settings-upkeep, on hold). The Files failure text sits at the top of the
panel where the reference centres it (existing layout).

Not run: the relaunch-twice check of the frame in a plain launch (a plain launch uses the
clone's shared user defaults and Keychain; unverified (attended)); 840×620 and dark; oracle and
trace-diff rows (not run).

## Attempts and evidence

| Attempt | Revision/fingerprint | Checks and outcomes | Evidence | Remaining blocker |
| --- | --- | --- | --- | --- |
| 1 (2026-10-07) | `41587c17b` on `1a50d0df3` | `bun test examples/t3-code` 2188 pass / 0 fail / 1 skip (base 2187); strict tsc: 8 errors, all in #175's terminal files, the same 8 on the base, none in files this task touched; `contract build` 2535 slots, 45 resources; `cargo test -p t3-code-macos --lib` 11/0; AppKit r8-pointer 2/0, r10-connect 4/0, media-actions 7/0, r6-media 5/0, r5-panels 8/0, sidebar 5/0, contextmenu 14/0; five checks pass (cargo build, cargo test 3310 pass / 0 fail / 32 ignored, clippy, fmt, caps, boot); macOS bundle builds | drive records below; PR screenshots | none for the adopted rows; X27 title row/full screen and X45 upstream |

Drives: one BEFORE (`t3-code-evidence-base` at `1a50d0df3`) and one AFTER, each the assembled
`.app` through `EXACT_MAC_BIN` (the agent's bare executable has no ATS), 1280×840, lane server
16380 with 30 seeded threads and the fixture repo, HTML fixture 16381. Fixtures (seed script,
fixture server, repo) live in `target/lane-shell`, not committed. Records (AFTER; BEFORE equal
except where noted):

```
hover Fixture thread 27 at [127.5,390] → sidebarHoverId bc5518a2…~active
wheel thread-list 0 360 at [127.5,450] → sidebarHoverId b7ea9057…~active (Fixture thread 22, under the pointer)   (BEFORE: same)
open media/broken.png → [file-image-failed, file-image-error] "Unable to load workspace image."
                         (BEFORE: [file-image, file-image-actions, file-image-picture], an empty box)
open media/page.html  → fixture requests: GET /style.css Host=localtest.me:16381, GET /badge.png Host=localtest.me:16381
                         (BEFORE: none; unstyled page, broken badge)
```

## Next action

Review the PR. The coordinator decides whether to reopen or comment on #122 (no process relaunch)
and #113 (title row and full-screen fact).
