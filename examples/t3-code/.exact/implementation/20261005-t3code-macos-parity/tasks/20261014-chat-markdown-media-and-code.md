---
name: 20261014-chat-markdown-media-and-code
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-chat-markdown-media-and-code'
pr_url: null
verified_commit: null
---

# Chat Markdown media and code: the image's preview button and tooltip, the media key's target, the copy morph, Shiki's line time limit

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences and the kept
workarounds; the coordinator's triage gave this task five rows (`fix-wave-plan.json`, clone task
`chat-markdown-media-and-code`). The user decided on 2026-10-11 that such items are built now, matching the reference T3
Code (`1e2ecbd975`).

| Id | From | Clone (feature tip `609f392d5`) | Reference (check it first) |
| --- | --- | --- | --- |
| G4-05 | `realinput-1011c` B2 ([notes](../../../../../../../target/t3-ui-parity/realinput-1011c/b2-panels-browser/00-b2-notes.txt) G4-05 (d)); STATUS "Known differences"; [media-signed-urls-and-attachment-markdown](closed/20261011-media-signed-urls-and-attachment-markdown.md) "Seen while building" | A click on a chat Markdown image opens no expanded-image dialog; the image takes no focus (none on it after Escape); a second right-click while its menu is open opens the menu again at the new point | The image is a button "Preview <alt>" (`role="button"`, `tabIndex=0`, Enter / Space), its click opens ExpandedImagePreview as a gallery of the document's images (`markdownImageGallery`), and it keeps its MediaActions menu; a right-click while the menu is open only closes it (`menuOpen` ref) |
| MSU-1 | `realinput-1011c` B13 (MSU-1 step 2); STATUS "Known differences" | Hovering a rendered Markdown image shows no readable tooltip: an empty AX help tag, nothing drawn | MediaActions' tooltip (`TooltipPopup variant="code"`): the file's full path, else the URL, else the name |
| G4-09 | `realinput-1011c` B2 (G4-09, "not reachable"); STATUS "Known differences" | With the timeline focused, Shift+F10 or the ContextMenu key opens the menu of the media last right-clicked | No reference path opens a media menu then: MediaActions answers the key only on the focused media; a click elsewhere moves the focus |
| X18 | EXACT2-GAPS X18 ("the code-block and welcome copy buttons still cross-fade: their return runs as keyframes") | The code block's, the table's and the welcome command's copy buttons cross-fade copy and check by keyframes (`mdCheck*`, `mdCopy*`, `wc*`) | MorphIcon Copy ↔ Check on the snappy spring both ways: the code block and the table 1,200 ms, the welcome CommandBlock 1,500 ms |
| KD-code-cpp-perf | STATUS "Known differences" (the template-dense cpp line); [code-highlighting-languages](closed/20261011-code-highlighting-languages.md) "Seen while building" | A template-dense cpp line is super-linear in Hermes' regex engine and has no bound: 962 characters 675 ms, 2,960 characters 6.2 s in one slice | Shiki stops a line after 500 ms (`tokenizeTimeLimit`) |

## Steps

1. Reference first (Electron on lane base 16480, CDP 16481; source under `target/t3-ref/src-1e2ecbd975`): a thread
   whose assistant message holds a workspace image, a web image, a code fence, a table and a template-dense cpp line.
2. G4-05: the loaded Markdown image is a "Preview <alt>" button whose press opens the expanded-image dialog over the
   document's images; its MediaActions menu stays; the native menu presenter ignores the right-click that closes it.
3. MSU-1: the tooltip with the full path (or URL, or name) shows on hover.
4. G4-09: the media's menu key answers only while the media holds the focus as the person last gave it; a press
   elsewhere clears the stale target.
5. X18: a copied flag per button that a root task (`key` the copy's nonce) clears after 1,200 or 1,500 ms, drawn by
   `MorphCopyCheck` (the `d` transition) instead of the keyframes.
6. KD-code-cpp-perf: Shiki's 500 ms per-line limit in the tokenizer, counted as regex work (a data module has no clock).
7. Bun tests, contract build; one agent drive (before on the feature tip built in this worktree first, after on this
   branch); real-input steps for the rows a real pointer or key must confirm.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| G4-05 | reference comparison; Bun tests (the button, its label, the press, the menu kept); agent drive (tap the image: the dialog) | before / after / reference images |
| MSU-1 | reference comparison; Bun test (the tooltip); agent drive (hover) | before / after / reference images |
| G4-09 | reference comparison; Bun test and the native presenter's rule; agent drive (right-click a media, press elsewhere, Shift+F10) | before / after text |
| X18 | reference comparison; Bun tests (the copied flag and its reset); agent drive (a filmed copy) | before / after film |
| KD-code-cpp-perf | Bun tests (a 2,960-character template-dense cpp line stops at the limit, the rest plain); Hermes timing of the line on the app path | before / after text and image |
| G4-05, MSU-1, G4-09 under a real pointer and keys | real-input steps for the final session | — |

## Reference

- G4-05: `apps/web/src/components/ChatMarkdown.tsx` `expandableMarkdownImageProps` (role button, tabIndex 0,
  `Preview ${alt.trim() || "image"}`, Enter / Space, not inside a link), `ChatMarkdownImage` (the settled `<img>` in
  MediaActions with `cursor-zoom-in`), `MarkdownImage` (`MarkdownLinkContext`: no expand inside a link);
  `components/chat/markdownImageGallery.ts`; `components/chat/ExpandedImagePreview.tsx`;
  `components/media/MediaActions.tsx` `showMenu` (`menuOpen` ref: one menu at a time).
- MSU-1: `components/media/MediaActions.tsx` (`tooltip`, `TooltipPopup variant="code"`).
- G4-09: `components/media/MediaActions.tsx` `onKeyDown` (ContextMenu or Shift+F10 on the media itself).
- X18: `components/ChatMarkdown.tsx` MarkdownCodeBlock and MarkdownTable `handleCopy` (`setTimeout(…, 1200)`,
  `MorphIcon icon={copied ? Check : Copy}`), `components/onboarding/WelcomeWizard.tsx` CommandBlock
  (`useCopyToClipboard({ timeout: 1500 })`), `components/MorphIcon.tsx`.
- KD-code-cpp-perf: `@shikijs/primitive@4.2.0` `_tokenizeWithTheme` (`tokenizeTimeLimit = 500`, the next line from
  `result.ruleStack`), `@shikijs/vscode-textmate@10.0.2` `_tokenizeString` (the time checked before each scan; a
  capture's retokenizing has no limit).
