---
name: 20261014-chat-markdown-media-and-code
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: stale
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: 'feat(example)/t3-code-chat-markdown-media-and-code'
pr_url: https://github.com/ccheever/exact2/pull/541
verified_commit: 401edb148
---

# Chat Markdown media and code: the image's preview button and tooltip, the media key's target, the copy morph, Shiki's line time limit

## Outcome

The user asked on 2026-10-14 to fix everything fixable from `realinput-1011c`'s known differences and the kept
workarounds; the coordinator's triage gave this task five rows (`fix-wave-plan.json`, clone task
`chat-markdown-media-and-code`). The user decided on 2026-10-11 that such items are built now, matching the reference T3
Code (`1e2ecbd975`).

| Id | From | Clone (feature tip `609f392d5`) | Reference (check it first) |
| --- | --- | --- | --- |
| G4-05 | `realinput-1011c` B2 (`b2-panels-browser/00-b2-notes.txt` G4-05 (a), (d)); STATUS "Known differences"; [media-signed-urls-and-attachment-markdown](closed/20261011-media-signed-urls-and-attachment-markdown.md) "Seen while building" | A click on a chat Markdown image opens no expanded-image dialog; the image takes no focus (none on it after Escape); a second right-click while its menu is open opens the menu again at the new point | The image is a button "Preview <alt>" (`role="button"`, `tabIndex=0`, Enter / Space), its click opens ExpandedImagePreview as a gallery of the document's images (`markdownImageGallery`), and it keeps its MediaActions menu; a right-click while the menu is open only closes it (`menuOpen` ref) |
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

What the reference does on this lane (T3 Code `1e2ecbd975`, Electron, CDP 16481, the same seed): the message's images are the
buttons "Preview Shot", "Preview Logo" and "Preview Badge"; a click on Shot opens "Expanded image preview" with "Shot (1/3)",
the focus on Close image preview, → to "Logo (2/3)", Escape back to "Preview Shot" focused; hovering the inline Logo shows the
code-style tooltip with its full path above it, and in Files' rendered `docs/media.md` the inline Logo's tooltip
(`…/projects/work/docs/../assets/logo.png`, the reference's text too) reaches over the column's edge; Copy code morphs to the
check at once and back at about 1.2 s (frames over CDP); the 2,960-character template-dense cpp line is coloured to its end.

## Cause and fix

**G4-05, the image's press.** The clone's Markdown image sat in MediaActions (a box with the menu, keys and an AppKit title)
and had no press; the expanded image dialog served sent attachments and a file link's media only. Now a settled image is
`MediaPreviewActions` (`media-actions.contract`): a `button` "Preview <alt>" (`alt.trim() || "image"`), `tabindex=0`,
the zoom-in cursor, MediaActions' menu and keys kept. Its press is `image-md-open` with its media key, document owner,
node id and ordinal in the rendered document (`timeline-attachments.ts`). Every renderer notes its document by the owner its chips carry
(`noteMarkdownDocument`, called by `messageChips`: a message row's id, `files:<path>` for Files, `attachment:<id>` for a sent
attachment), and `markdownGallery` lists the document's images in order as `markdownImageGallery` does (an image that can
load; not a video, not one in a table cell or a link's text, which draw their alt text). The renderer derives gallery
occurrence ids from the same parsed blocks and runs that draw each image. The press sends
that ordinal; the data action checks its media key and selects that entry, so Shot, Logo, Shot opens the later Shot at 3/3.
Standalone images use their own block's alt text, even when a shared chip points to the same URL. The existing dialog shows the
gallery: "Shot (1/3)", previous and next round both ways, the media's own menu; its URL list gains the gallery's web images
as written and its host paths signed through the thread (a draft's own in Files). A secondary click focuses the image, as the
reference's mousedown does, and closing the dialog gives the focus back to the image that opened it (Base UI Dialog's final
focus). The video, the loading slot and a failure keep MediaActions.

**G4-05, a right-click while the menu is open.** AppKit hands the click that dismissed the native menu to the window after
`popUp` returns, where the media's `contextmenu` opened the menu again at the new point. `T3MenuDismissal`
(`T3MediaActions.swift`): for half a second after a media menu closes with no pick, a secondary click (the right button, or
Control and the primary) made before it closed, by its event timestamp, goes no further; one made after it is a new click.

**MSU-1, the tooltip.** The clone's media tooltip was the node's `title` (AppKit's help tag, a declared difference), which
showed an empty tag over an image inside a line of text. The reference draws MediaActions' TooltipPopup (variant code,
portaled). Now the settled Markdown image's tooltip is the window's hover layer (`hover-layer.contract`, a new kind "code":
monospace 11px, up to 30rem, Base UI's 600 ms delay, flipped near an edge), at the button's frame by its node id
(`media-<owner>-<block>[-<run>]`, one per occurrence: `docs/media.md` shows the same image twice), so the Files column does not
cut it; every other media node (a failure, a video, a Files image file, the dialog's media) draws the same card in place
(`MediaTip`). Opening the menu or the dialog closes it. A thread change now ends a hover tip whose trigger went with no
leave (`hoverSurface`'s key gains the thread), which the agent's tap left on screen.

**G4-09, the media's stale focus.** A media node takes the focus from a click (tabindex 0), and AppKit keeps it there when the
next press lands on something that takes none (a tab, a toolbar button): the ContextMenu key and Shift+F10 then reached that
media. Chromium's mousedown moves the focus off an element it does not land on. `T3MediaFocus` (`T3MediaActions.swift`, hook
`t3-media` with `data-media-kind="actions"` on MediaActions and MediaPreviewActions; the hook's other two handlers act on PDF,
HTML and device kinds only): after a press outside the focused media has gone through, a media still holding the focus lets it
go (`makeFirstResponder(nil)`), so a key reaches no stale media. A press on the media, or one whose target takes the focus
itself, keeps what it has.

**X18, the copy morph.** The code block's, the table's and the welcome command's copy buttons cross-faded copy and check with
keyframes (`mdCheck*`, `mdCopy*`, `wc*`), since a keyframed `d` is refused. Now each draws `MorphCopyCheck`
(`shell-morph.contract`, the `d` transition on morphicons' spring) on while its copy is the last one and not yet released: the
snapshot carries the thread's code copy nonce (`codeCopyNonce`), and the root's tasks keyed on it and on the welcome copy
counter release it after 1,200 ms and 1,500 ms (`codeCopyDone`, `welcomeCopyDone`, provided to the buttons), as the
reference's timers do. The keyframes are deleted.

**KD-code-cpp-perf, the template-dense cpp line.** Two causes. (1) The super-linear cost was not Hermes' regex speed alone: the
tokenizer kept one search memo and cleared it whenever a capture's retokenizing searched another string, so cpp's
function-definition regexes searched the rest of the line again at every template. Each line now keeps its own memo (the
same tokens: 205,144 lines, 7.86 million characters of the record's corpus, the base tokenizer and this one, 0 lines differ).
(2) Shiki's 500 ms per-line limit: a data module has no clock, so the tokenizer counts regex work (a search costs the
characters it scanned times its pattern's length; 500 ms is 7.5e8 units, measured in the pinned Hermes over whole lines).
A line that reaches it keeps its tokens, paints the rest plain and gives the next line the stack where it stopped, as
vscode-textmate's `stoppedEarly` (Shiki runs its last token to the line's end; here the rest is plain). A turn's regex work is
capped before each search with a long rest (charged its whole rest times its pattern), and the line goes on next turn from
the scan it was in (`tokenizeLineWithin` takes the line's progress), so no turn passes the runner's 100 ms answer budget on
these lines.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| G4-05 | pass (agent, reference over CDP): before, a mouse click on the image focused it and opened nothing; after, it opens the expanded image dialog "Shot (1/3)" with the focus on Close, as the reference; Escape gives the focus back to "Preview Shot" (reference: the same); a right-click focuses "Preview Shot" and its menu still answers (the agent's pick: "Path copied"). Bun tests (the gallery and its steps, the button). The second right-click while the menu is open: AppKit test (`T3MenuDismissal`); under a real pointer, open: step 2 below | [image](https://raw.githubusercontent.com/ccheever/exact2/33ca4b6ab129fdf6cfbb99c6ea9bb9c8a56ec6f4/chat-markdown-media-and-code/g405-click.png), [drive record](https://raw.githubusercontent.com/ccheever/exact2/49d0578ec3e14cb4a5b5a3452f44bf988bc4a7d7/chat-markdown-media-and-code/drive-evidence.txt) |
| MSU-1 | pass (agent, reference over CDP): before, no drawn tooltip (the AppKit help tag, empty over an inline image in `realinput-1011c`); after, the inline Logo's full path in the code-style tooltip in chat and in Files' rendered `docs/media.md`, not cut by the Files column, the same text as the reference. Bun tests. Under a real pointer: step 3 | [chat](https://raw.githubusercontent.com/ccheever/exact2/572be81c924b0cd8c3d26854422ab8dabdfd637f/chat-markdown-media-and-code/msu1-chat.png), [Files](https://raw.githubusercontent.com/ccheever/exact2/c196b4aeb92724b3e7cddf321ad07de719f0beb1/chat-markdown-media-and-code/msu1-files.png) |
| G4-09 | pass (agent): the image focused, a mouse click on Toggle thread details (a button that takes no focus), then Shift+F10 and the ContextMenu key: before, the image kept the focus and each key opened its menu ("Path copied" twice); after, the focus is off the image and neither key opens a menu. AppKit tests (`T3MediaFocus`, events through `NSApplication.sendEvent`). Under real keys: step 4 | [image](https://raw.githubusercontent.com/ccheever/exact2/b666bdcea9ccc622a50884bbc132f6854fe9bbdc/chat-markdown-media-and-code/g409-shift-f10.png), [drive record](https://raw.githubusercontent.com/ccheever/exact2/49d0578ec3e14cb4a5b5a3452f44bf988bc4a7d7/chat-markdown-media-and-code/drive-evidence.txt) |
| X18 | pass (agent film on the agent's clock, `screenshot … window` frames; reference frames over CDP): before, the check at 120 ms and a cut back to copy at about 1.2 s; after, copy morphs into the check (60 and 120 ms mid-morph) and back from about 1.23 s, as the reference. Bun tests | [film](https://raw.githubusercontent.com/ccheever/exact2/a3171429efda0bd6426718b75236c9b70e04ef40/chat-markdown-media-and-code/x18-copy-film.png) |
| KD-code-cpp-perf | pass (Hermes and agent): before, the 2,960-character line took 6.4 s in one turn in Hermes (8.5 s alone) and stopped the agent drive ("`highlightSlice` took 147.7 ms, over the 100 ms budget"); after, 226 ms over 49 turns of at most 48 ms, every character coloured as the reference (no stop needed); Files' ten 962-character lines 512 ms in turns of at most 27 ms (before 6.9 s in turns of 1.4 s); a 5,920-character line stops at the limit after 0.41 s, the rest plain. The corpus' tokens unchanged. Bun tests | [image](https://raw.githubusercontent.com/ccheever/exact2/e5ba93c023931b58ed5a9c5e5881e734831cd79c/chat-markdown-media-and-code/kd-template.png), [Hermes and corpus record](https://raw.githubusercontent.com/ccheever/exact2/49d0578ec3e14cb4a5b5a3452f44bf988bc4a7d7/chat-markdown-media-and-code/drive-evidence.txt) |
| G4-05, MSU-1, G4-09 under a real pointer and keys | open: real-input steps 1-4 below | — |

Drives (agent mode, 1280×840, one session each, fresh storage): before on the feature tip `609f392d5` built in this worktree
before the change (the copy film on the same tip, rebuilt from its files for that session), after on this branch's bundle
(`2c8d30b12`'s app code); lane `chat-markdown-media-and-code`, base port 16480, the fixture http server on 16483. The film op
(`screenshot … over`) captures blank frames while the screen is locked, so the films are `screenshot … window` frames on the
agent's clock. Each drive's Copy code wrote `export const ok = true;` to the Mac's general pasteboard (`copyText`); the media
picks went to the agent's private pasteboard.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, a lane T3 home outside any git checkout) on the lane
`chat-markdown-media-and-code` seed (`seed.py`: the threads "Markdown media" and "Template line" in project work, `docs/shot.png`,
`assets/logo.png`, `docs/media.md`; the fixture http server for `http://127.0.0.1:16483/badge.png`). The reference T3 Code (Alpha)
beside it on the same seed. Never send a message.

1. **G4-05, a click and the focus.** Open "Markdown media". Click the large "Shot" image: the expanded image dialog opens with
   "Shot (1/3)" under it and the focus on Close image preview; → and ← (or the arrows) step to "Logo (2/3)" and "Badge (3/3)" and
   round. Escape closes it; the focus is on the button "Preview Shot" (read AX: AXButton "Preview Shot" focused), as the reference.
2. **G4-05, a second right-click.** Right-click "Shot": the media menu opens at the pointer (Copy full path, Copy relative path,
   Open in file viewer, Save image, Copy image) and the image takes the focus. With the menu open, right-click the "Logo" image:
   the menu closes and no new menu opens (reference: the same). Escape: the focus stays on "Preview Shot".
3. **MSU-1, the tooltip.** Hover the inline "Logo" image 1 s: a monospace tooltip with its full path
   (`…/projects/work/assets/logo.png`) above it. New thread › work, right panel › Files › docs › media.md › Show rendered
   markdown; hover the first "Shot" in "Beside the file: [Shot] and its parent: [Logo]": the tooltip shows
   `…/projects/work/docs/shot.png` whole, not cut by the Files column (reference: the same, over the column's edge).
   Then open an image's expanded dialog, press Escape while the pointer remains over its opener, and move the pointer
   into empty space. Confirm that the tooltip closes and stays closed while clicking an unrelated control. This
   dismiss-on-pointer-away case is unverified; the earlier agent clicks did not move the pointer.
4. **G4-09, Shift+F10 after a media right-click.** Back in "Markdown media": right-click the "Badge" image, press Escape
   (the focus on "Preview Badge"), then click the right panel's toggle or the Toggle thread details button (a button that takes
   no focus). Press Shift+F10: no menu opens. Press the ContextMenu key if the keyboard has one: no media menu opens.
5. **X18 (optional, visual).** Click the ts block's Copy code: the copy icon morphs into a check, and about 1.2 s later morphs
   back into the copy icon (not a cut).

## Tests

- `chat-markdown-media-and-code.test.ts` (11): KD: the 2,960-character line finishes under 0.6 of the limit with every
  character in its scopes (before the memo fix 1.2e10 units); sliced, it spreads over turns and paints as unsliced; a
  5,920-character line stops at the limit, the rest plain, the next line from where it stopped, plain in `highlight()`.
  G4-05: the press opens the document's gallery in order (a video and a table cell's image left out), steps round both ways,
  the tooltip and the return focus in the view, the URL list holds the web image as written and the signed host path once,
  a thread change drops it, an unknown image throws; Files and attachments name their documents; the settled image is the
  button (label, press, menu, keys, tabindex, zoom-in cursor, the hook, the right-click focus, the hover layer's code tip),
  the video, loading slot and failure keep MediaActions. MSU-1: MediaActions draws MediaTip with no AppKit title and closes
  it on its menu; the dialog's media carries the tooltip; the hover layer's "code" kind (600 ms, HoverCode). X18: no keyframes;
  the three buttons draw `MorphCopyCheck` on a live copy; the root's tasks (1,200 and 1,500 ms) and provide; the snapshot's
  nonce.
- `macos/tests/media-actions/focus.swift` (AppKit, 6, in the `media-actions` binary: 13 run): a press on something that takes
  no focus takes it off the media, one on the media keeps it, one that takes the focus keeps what it took, an unregistered view
  keeps its focus; the secondary click made before the menu closed goes no further (right button and Control-click), a later
  one and a plain click go on; the watch ends. With both guards disabled, 5 assertions in 2 tests fail.
- Updated for the new text: `code-highlighting-languages.test.ts` (the retry loop goes on from the line's progress),
  `media-signed-urls-and-attachment-markdown.test.ts`, `timeline-attachments.test.ts`, `r6-media.test.ts`,
  `hover-layer.test.ts`.

## Seen while building, not a finding row

- The message's Copy button (`timeline-work.contract` CopyButton: MessageCopyButton's copy, check or x for 2 s) still
  cross-fades by keyframes; recorded under X18 in `EXACT2-GAPS.md`.
- After the dialog closes under a still pointer, the hover layer shows the image's tooltip (the host re-reads hover under a
  resting pointer); in the historical agent drive it then stayed through clicks elsewhere, since the agent's clicks move no
  pointer. The repair drive explicitly moved agent hover to the toolbar and the tooltip disappeared. Real-pointer movement
  has not been verified; step 3 explicitly checks this case.
- A template-dense cpp line over about 5,000 characters: its first function-definition search takes up to 105 ms in Hermes by
  itself, past the runner's 100 ms answer budget (declared in `EXACT2-GAPS.md` "Shiki's line time limit"). Minified lines of
  20,000 characters or more in a chat fence take many short turns (katex.min.css 922 turns of at most 12 ms), since each long
  search is charged its whole rest.
- Plan size before this repair: the feature tip's `app.contract` is 33,309,129 bytes (120,810 nodes); the reviewed head's is
  37,185,217 bytes (120,443 nodes), +3,876,088 bytes (+11.6%). The decoded-plan review below identifies duplicated window
  captures as the main avoidable cost and removes the growth. `caps.mjs` sets no plan budget.

## Checks

Historical checks before this repair, on `e867bef04` (after it only `scheduled-tasks-followups.test.ts`'s two hover-layer text
expectations and this record changed, and the Bun suite ran again on them):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4866 pass, 1 skip, 0 fail (329 files), on the final tests. The run on `e867bef04` exited 1: two `scheduled-tasks-followups.test.ts` tests read the hover layer's old delay and width text; updated |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 37,185,217 bytes, 120,443 nodes; `app.contract` 1,466 lines |
| AppKit `macos/tests/media-actions` (README recipe) | 0 | 13 run, 0 failed (6 new in `focus.swift`); with both guards disabled, 5 assertions in 2 tests fail |
| `cargo test -p t3-code-macos --lib`, other AppKit binaries | not run | no Rust changed; the other binaries' module sources are compiled by the `media-actions` binary too (every file in `modules/apple/`) |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 4,353 passed, 0 failed, 46 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | before the live drive, from `2c8d30b12`'s app code |

## Review repair, 2026-10-14

- Reference source checked read-only at `apps/web/src/components/chat/markdownImageGallery.ts`: the gallery selects the
  clicked element by identity. The shared URL is not the occurrence identity.
- The new behavioral regression opens the last image in Shot, Logo, Shot at 3/3, preserves its own alt text and return
  focus id, steps both ways, and rejects mismatched or out-of-range ordinals. It failed at 1/3 before the repair.
- `EXACT2-GAPS.md` now distinguishes the hover layer used for settled Markdown images from in-place `MediaTip`.
- The existing MSU-1 Files image contains after and reference only. No before capture for that scenario was retained;
  it cannot prove a before/after Files comparison. The chat before capture and historical report remain separate evidence.
- Existing MSU-1 and X18 Bun tests check source wiring, not runtime behavior. Their historical agent drives and film
  are the runtime evidence; this repair does not claim that those source assertions exercise timers or hover.
- Guidance: Exact skill library `2026-10-05-platforms-v3`, foundations, state/data, accessibility and testing/debugging.
  The current checkout's parser, Contract provider/injection code and native driver are the implementation authority.

Occurrence-repair development checks, with Bun 1.4.2, on `c8b10717e`:

| Check | Result |
| --- | --- |
| `bun test examples/t3-code --timeout 60000` | 4,867 passed, one existing skip, zero failures |
| strict `tsc`, README command | Exit 0 |
| `target/debug/contract build examples/t3-code/app.contract -o /tmp/t3-markdown-repair.plan --json` | Exit 0; 37,196,589 bytes; existing dialog/colour warnings |
| Compiled `markdownGalleryIds` fixture, executed with `exact-terminal <fixture.plan> --size 100x10 print` | Output `b0,b1-r1,b1-r3`, `clicked-index=2`; video, blocked image, table and code entries excluded |
| `host/apple/build.mjs t3-code-macos --bundle` with `EXACT_APP_DIR=examples/t3-code` | Exit 0; refreshed app bake 341.3 s, Swift/package passed |
| Isolated macOS agent drive, 1280 × 840, local server port 16482 | Exit 0; later rendered Shot opened 3/3, previous Logo 2/3, next Shot 3/3, Escape closed, explicit hover-away dismissed the reopened tooltip |
| `cargo build --all-targets --keep-going`, `cargo fmt --all -- --check`, caps and boot | Exit 0 |
| Root test/clippy/workspace | Coordinator owns these checks. The worker's root test run was interrupted on that direction; clippy/workspace were not started |

The compiled helper fixture and native drive records are local-only under `target/review-repair/`. The helper imported
`markdownGalleryIds` from the application, using a standalone Shot block and one inline block with a video, Logo, a blocked
image and a repeated Shot, followed by table and code blocks. It exercised the compiled expression in the runner; it did
not render the macOS controls.

The native drive used its own copied home and workspace, with this fixture in `md-media`'s assistant message:

```markdown
![Shot](docs/shot.png)

Inline ![Logo](assets/logo.png), then ![Shot](docs/shot.png).
```

It selected the third rendered `markdown-image-actions` node, `media-["md-media","md-asst-1"]-1-4`, through the platform
agent carrier. After Escape, hovering that image for 750 ms showed `media-tooltip`; hovering `toggle-thread-details` and
advancing 1,000 ms removed it. This tests agent hover delivery, not global mouse input. `size-before-drive.log` and
`size-before-drive-host-log.json` preserve these results. Use `T3_LOCAL_HOME=target/review-repair/t3-home`, port 16482 and the
coordinator's staged `t3-0.0.46-nightly.20261005.2667-darwin-arm64` runtime. The initial port 17482 was refused by the
development policy (only 16000–16999); `drive-refused-port.log` and its host log retain that resolved setup failure.

## Plan size repair, 2026-10-14

Commit `95ae39500` moves the unchanged hover state and `hoverTipAt` action from `T3Window` to `T3Code`, beside its existing
hover clock. The window receives and provides `hoverTip`, `hoverShown` and `hoverTipAt`; the trigger actions, layer,
placement, opening and closing delays, surface-change cancellation, menus and copy morphs keep their behavior.

The decoded plan and source map showed 175 `MediaPreviewActions` instances (140 in `FlowRuns`, 35 standalone), each with
four handlers. The window action carried 134 hidden captures of `T3Window` properties into those handlers. At the app root
it takes only its three authored parameters. No framework change or functionality reduction was needed.

| Measurement | Before, `c8b10717e` | After, `95ae39500` |
| --- | ---: | ---: |
| Compiled bytes | 37,196,589 | 33,177,857 |
| `hoverTipAt` parameters | 137 | 3 |
| Parameter rows | 310,085 | 160,376 |
| Argument rows | 736,016 | 583,745 |
| UI nodes | 120,443 | 120,443 |
| Bindings | 990,430 | 990,430 |

The reduction is 4,018,732 bytes (10.8%). The isolated PR is now 131,272 bytes below the feature baseline of 33,309,129
bytes (0.39% smaller). This removes the isolated size tradeoff; the coordinator measures the combined integration.
The earlier attribution to copy-morph path data was overstated: its four long path strings are interned once, totaling
2,444 bytes. Removing captures reduced the string pool by 1,392,048 bytes, parameter rows by 1,197,672 bytes and argument
rows by 1,218,168 bytes.

Both measurements used the same existing compiler, `target/debug/contract build examples/t3-code/app.contract -o <plan>
--json`. The before artifact is `/tmp/t3-markdown-repair.plan`, SHA-256
`aea91278bbfd62bdf714780fa5e734db34d356e7056171d299f109e5c56c701a`; after is
`/tmp/t3-markdown-size-repair.plan`, SHA-256 `4fb6b1b3cb20223ca81a313ce7dac9622bb0da8cfde224c95a58e4cc63e04131`.

Size-repair development checks: 35 focused hover/media/scheduled tests passed; full Bun suite 4,867 passed, one existing
skip, zero failures; strict TypeScript, Contract compilation, diff check and caps passed. `app.contract` is 1,490 lines.
The root test/clippy/workspace checks remain the coordinator's integration checks.

The fresh native build passed (Cargo 256.6 s, Swift and packaging passed), and the expanded macOS agent drive passed on
the same isolated fixture. The tooltip stayed hidden through 500 ms and appeared by 750 ms. The later Shot opened at 3/3;
previous and next selected Logo 2/3 then Shot 3/3; Escape closed the dialog and returned focus to the clicked occurrence.
Explicit hover-away dismissed the reopened tooltip, and opening Settings while it was visible dismissed it through the
unchanged surface-change clock. The global pointer was not used.

`target/review-repair/size-native-build.log`, `size-drive.log`, `size-drive-host-log.json` and the existing `drive.mjs`
preserve this proof. One fixture attempt held a numeric node id across an asynchronous rerender and could not find it;
the driver now resolves the current node from its stable authored occurrence id. `size-drive-stale-node.log` records that
resolved fixture failure. The application source stayed at `95ae39500` throughout both attempts; no second source round
was needed.

## Not done / not verified

- G4-05 (a click, the focus, a second right-click while the menu is open), MSU-1 (the tooltip under a real pointer) and
  G4-09 (Shift+F10 after a media right-click) under real input: open, "Real-input batch steps" 1-4 for the coordinator's
  final session (agent drives and AppKit tests only).
- MSU-1 after closing the image dialog: moving the pointer away must dismiss the reopened tooltip. This is still
  unverified under real input, with explicit reproduction in step 3; the explicit agent hover-away check passed.
- The MSU-1 Files proof lacks a before panel; its existing image demonstrates after/reference only.

## Delivery

Draft PR [#541](https://github.com/ccheever/exact2/pull/541) into `feat(example)/t3-code`; the coordinator reviews and merges.

## Next action

The repair checks passed; implementation is ready for fresh verification. The coordinator performs independent
review, updates the draft PR from the local repair, measures the combined integration, and runs the real-input steps.

## Coordinator publication, 2026-10-14

Draft PR [#541](https://github.com/ccheever/exact2/pull/541) was read back at `8b61e102c317da4be58a01cdc087860031f961b8`, based on `feat(example)/t3-code`. Coordinator review passed the hover move at `95ae39500`. The isolated plan is now below its original baseline, so no size/functionality decision remains. The current eleven-branch integration also compiles at 27,894,502 bytes. Combined repository gates, the final app drive and real pointer remain open. This bookkeeping does not close the task or replace the earlier verification revision.

### Remote delivery, 2026-10-14

GitHub reports this PR merged at 03:04 UTC into `feat(example)/t3-code`, published as
first-wave checkpoint `1e31522094c2587ce6e7a30bdd8ca0e87bc8310a`. This records delivery
only; the task's remaining acceptance and real-input qualifications above still apply.
