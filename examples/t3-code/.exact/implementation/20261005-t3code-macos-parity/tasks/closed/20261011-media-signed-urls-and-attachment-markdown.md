---
name: 20261011-media-signed-urls-and-attachment-markdown
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: partial
delivery: merged
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-media-signed-urls-and-attachment-markdown
pr_url: https://github.com/ccheever/exact2/pull/465
verified_commit: 1f813789b
---

# Media in Files' rendered Markdown, and an attachment's Markdown media

## Outcome

The user decided on 2026-10-11 that these residual items of earlier records are built now, matching the reference T3
Code (`1e2ecbd975`):

| Id | From | Clone (feature tip `6db5cadea`) | Reference (check it first) |
| --- | --- | --- | --- |
| MD-1 | [media-actions](20261005-media-actions.md) "Not built / limits": "Files' rendered Markdown preview media (the Files Markdown blocks get no signed URLs; chat Markdown does)" | Images and media in Files › "Show rendered markdown" | Load through the signed URLs the reference uses: FileMarkdownPreview is ChatMarkdown with `cwd` (the workspace) and `imageBaseDir` (the file's folder); a host path becomes a `media-file` asset of the thread (`ChatMarkdownAssetImage`), a web URL loads as written |
| MD-2 | [markdown-links-and-files-preview](20261009-markdown-links-and-files-preview.md) "Found, not changed": "A sent attachment's Markdown preview still uses `r4-surfaces-render.ts`" | A sent attachment's rendered Markdown | The same renderer as the rest of the app: AttachmentFilePreview is `<ChatMarkdown text cwd={undefined} />`, with no thread |

What the drives of the tip showed against the reference (lane `media-signed-urls-and-attachment-markdown`, base port
16480: a `docs/media.md` with two images inside a sentence, then images beside the file, in its parent, absolute, missing,
on a loopback http host and a video, each alone; a seeded thread "Media attachment" whose message carries
`attached-notes.md` with a relative, an absolute and an http image and two images inside a sentence):

- MD-1. #371 already gave Files' rendered Markdown the transcript's signed URLs: each image alone on its line loads
  (`media-file` assets) and the video plays. Still different from the reference: (a) an image inside a line of text is drawn as its alt text ("Beside the file: Shot and its parent: Logo"),
  where the reference draws the images in the line (ChatMarkdownImage, not standalone); (b) the media menu's paths are
  relative to the file's folder, not the workspace: "Open in file viewer" on `docs/shot.png`'s image opened `shot.png`
  at the workspace root ("Unable to load workspace image."), and "Copy relative path" copies `shot.png`, where the
  reference's ChatMarkdownAssetImage takes `workspaceRoot={cwd}` (`docs/shot.png`); (c) in a draft thread nothing is
  signed (`markdownMediaUrls` needs a server thread id), so every host-path image keeps its loading slot, where the
  reference signs through the draft's thread ref (the server ignores the thread for an absolute `media-file` path,
  `ws.ts`); (d) a source fragment (`clip.mp4#t=2`) is dropped from the signed URL (`markdownImageSourceFragment`); (e) an
  image alone in its block that cannot load ("Gone") is the small "Image unavailable" chip, where the reference keeps
  the 16:9 frame it held while loading with the label centred in it (ChatMarkdownImage standalone).
- MD-2. #378 (audit-wave-followups-3 FW-2) already renders a sent attachment's Markdown with the chat renderer
  (`renderFileMarkdown`, kind `attachment`; `r4-surfaces-render.ts` keeps only the CSV/TSV parser). Still different:
  with no thread and no `cwd` the reference cannot sign a host path, so a relative and an absolute image are both
  "Image unavailable · <alt>" (ChatMarkdownImageFallback); the clone drew nothing for the relative one (its source lost
  its chip match: the attachment's link resolver emptied it) and signed and showed the absolute one, and drew the two
  images inside a sentence as their alt text.

## Steps

1. Reference first (Electron on lane base 16480, CDP 16481): Files › `docs/media.md` rendered; the attachment's preview.
2. MD-1: separate the resolving folder (`imageBaseDir`) from the reference root (`cwd`) in `media-views.ts`; an image
   inside a line becomes a media run (`macos/src/markdown_media.rs`) drawn by `markdown.contract FlowRuns` through the
   same media chip and signed URL as a block image; sign with the draft's thread id; keep the fragment.
3. MD-2: the attachment's Markdown resolves media with no thread and no folder: host paths are unavailable, web URLs
   load; its image hrefs match the chips again.
4. Tests (Bun, Rust); one agent drive; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| MD-1 | reference comparison; Bun and Rust tests; agent drive (Files › `docs/media.md` rendered: images in the line and alone, missing, http, video; the media menu's "Open in file viewer" through `T3_AGENT_MEDIA_PICKS`; a draft thread's Files preview) | before / after / reference images |
| MD-2 | reference comparison; tests; agent drive (the attachment's preview) | before / after / reference image |

## Reference

- `apps/web/src/components/files/FileMarkdownPreview.tsx` (ChatMarkdown with `cwd`, `imageBaseDir`, `threadRef`),
  `files/FilePreviewPanel.tsx` (`RenderedMarkdownSurface`; `threadRef` is the active thread's or the draft's,
  `ChatView.tsx` `activeThreadRef`), `files/AttachmentFilePreview.tsx` (`<ChatMarkdown text cwd={undefined} />`).
- `apps/web/src/components/ChatMarkdown.tsx`: the `img` renderer (`MarkdownImage`: `classifyMarkdownImageSource(src,
  imageBaseDir ?? cwd)`, a `WorkspaceFile` with a `threadRef` → `ChatMarkdownAssetImage` with a `media-file` resource,
  `srcFragment`, `workspaceRoot={cwd}`; without a `threadRef` → `ChatMarkdownImageFallback`), `ChatMarkdownImage`
  (`standalone`: `markStandaloneImages`), `ChatMarkdownAssetImage` (`mediaFileReference(path, workspaceRoot)`,
  `onOpenFile` with the relative path).
- `packages/client-runtime/src/markdownImages.ts`, `mediaSource.ts`, `mediaReference.ts`.
- `apps/server/src/ws.ts` `assets.createUrl` (an absolute `media-file` path is signed without a thread lookup),
  `assets/AssetAccess.ts`.

## Cause and fix

**MD-1 (a), images inside a line of text.** The repository's Markdown parser (`apps/markdown/parse`, shared with the
Markdown app, so not changed here) keeps an image only when it starts a block on a line of its own (`image_of`) and
flattens any other one into its alt text. `macos/src/markdown_media.rs` (new, included by `markdown.rs`) rewrites each
image the parser would flatten, `![alt](src)` → `[alt](t3-image:src)` with IMAGE_START (U+E001, a private-use mark)
before the alt text, outside fenced code, code spans, table lines and a link's text (and not one whose alt text holds a `[`), and keeps an
image line that starts a block: one image spanning the line (an image line that continues a paragraph is inline, as
the parser's paragraph takes it in); the line structure is kept, so task offsets hold. The resolver wraps `link_href`:
the run's href is `t3-image:` plus the source as the URL transform leaves it (an emptied source or a fragment loads
nothing, as `markdownImageHref`), and `chat_run` makes it a `media` run; `flow_tokens` makes each image one token
(IMAGE_START begins it, the alt text's emphasis runs join it). `media-views.ts markdownInlineImages` lists the same
images with the same href, so each gets a media chip and, for a host path, its signed URL (`markdownMediaUrls` signs
inline and block images); `markdown.contract FlowRuns` draws a media run with a chip through `MarkdownMedia`
(`media-markdown.contract`) with `standalone` false: a zero-size status span while the URL is signed, the image in
MediaActions once it is, ChatMarkdownImageFallback's chip (with the menu when it has a source) when it cannot load; a
video is the block's player (ChatMarkdownVideo). A surface whose chips carry no media (a pull request's text) keeps the
alt text. The same rule now holds in the transcript and an attachment, as the reference's ChatMarkdown draws inline
images everywhere.

**MD-1 (b), references.** `markdownMedia` took one root for both the resolving folder and the reference. It now takes a
`MediaScope`: `root` is `imageBaseDir ?? cwd`, `cwd` the workspace that `mediaFileReference` and "Open in file viewer"
are relative to (ChatMarkdownAssetImage's `workspaceRoot={cwd}`). Files' `renderedMarkdown` passes the file's folder and
the workspace; the transcript is unchanged (both are the workspace).

**MD-1 (c), drafts.** `markdownMediaUrls` returned nothing without a server thread id. FilePreviewPanel's `threadRef` is
the draft's own (`ChatView.tsx activeThreadRef`) and the server signs an absolute `media-file` path without looking the
thread up, so Files' preview now signs through `client.threadId`, else the draft's id (`ensureDraftThreadId`, the id the
draft launches as, allocated only when the file has a host-path image).

**MD-1 (d), fragments.** The media key and the signed URL carry `markdownImageSourceFragment(src)`, as
ChatMarkdownAssetImage appends `srcFragment`; the resource's path does not.

**MD-1 (e), a standalone image that cannot load.** `MarkdownMedia` (an image block) draws a refused signature or a
picture's `error` as `MediaFailedFrame`: the 16:9 frame (`max-width` 30rem, the muted fill and border) with the label in
it, as ChatMarkdownImage keeps its slot; an inline image and one that can never load keep the chip.

**MD-2.** #378 already made the attachment's preview the chat renderer. Its Markdown parsed with
`link_href_without_cwd`, which emptied a relative image source as it does a relative link, so the image block's href no
longer matched its chip and an empty image was drawn; and `renderedAttachment` signed host paths through the thread.
Now the attachment parses with `link_href` like every document and `links_without_cwd` empties only relative file
links (the deleted `link_href_without_cwd`'s rule, applied to runs); `renderedAttachment` resolves media with
`{ thread: false }` and no folder, so a relative or absolute host path is `blocked` ("Image unavailable · <alt>", no
actions, as MarkdownImage's ChatMarkdownImageFallback with no `threadRef`), a web URL loads as written, and nothing is
signed (`mdUrls` is empty).

**Review round 1 (2026-10-12).** An independent review of the draft PR found three should-fix items; each is fixed
here (the branch first merged `origin/feat(example)/t3-code` `e91bdc8af`, no conflicts):

- **R1-1, an inline video's failure slot** (filed as "Decision needed"; the reference settles it: MarkdownImage draws a
  video through ChatMarkdownVideo whether or not it is standalone, and the user's relayed request says to build items
  that wait on a decision). `MarkdownInlineMedia` is gone: `MarkdownMedia` takes `standalone` and is drawn by both the
  image block (`standalone=true`) and FlowRuns' media run. An inline image keeps its own states (a zero-size status span
  while it is signed, ChatMarkdownImageFallback's chip in MediaActions when it fails); a video, inline or not, is
  `MarkdownMediaBody`'s player: the 16:9 slot while it loads, the player, MediaVideoPlayer's 16:9 failure slot with Retry
  video and the open link. The plan cost is in Checks.
- **R1-2, a line of several images.** `sole_image_line` copied `image_of`'s loose test (starts with `![`, holds `](`,
  ends with `)`), so `![CI](ci.png) ![npm](npm.png)` stayed one image block whose source was `ci.png) ![npm](npm.png`
  (and `IMAGE_LINE` agreed): a nonexistent path was signed, refused, and drawn as one "Image unavailable · CI" frame.
  A line is now an image block only when `image_span` from its start ends at the line's end (an image the span cannot
  read is left to the parser); any other line is rewritten image by image, so a badge row is two inline images, as the
  reference's (a block of several images is not standalone). `markdownImages` checks `imageSpan` the same way.
- **R1-3, smaller gaps.** (1) `push_media_token` joined every adjacent run with one href, so `![a](x.png)![a](x.png)`
  drew one image with the alt text "aa": the rewrite now puts IMAGE_START before each alt text and a marked run begins
  a new token (`unmark_runs` takes the mark out of every other run), so the two stay two; an image with no alt text,
  which left no run at all (it vanished), now draws, as the reference's "Preview image". (2) An image that is the only
  content of a list item, a quote or a paragraph (`- ![x](a.png)`, `> ![x](a.png)`, `**![x](a.png)**`) is standalone in
  the reference (`markStandaloneImages`: p, li, blockquote, through a link or emphasis): `standalone_media` sets its
  media run's `icon` to "standalone" (not a task item, whose checkbox shares the item, nor a heading's image, nor one
  beside text, a line break or another image) and FlowRuns passes it as `standalone`: the 16:9 slot while it loads,
  the 16:9 failure frame. (3) An image in a table cell and one inside a link's text still draw their alt text: listed
  under "Seen while building" with what each needs. (4) `markdownInlineImages` skips table lines as markdown_media.rs
  does (`tableLines`, a port of r4_timeline_tables.rs `table_lines`), and an alt text holding a `[`, so nothing there
  is signed.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| MD-1 | pass (agent, 1280×840, reference over CDP): Files › `docs/media.md` rendered draws the two images inside the sentence in the line ("Beside the file:" image "and its parent:" image), as the reference; every host-path image signs as a `media-file` asset; the missing one is the 16:9 frame "Image unavailable · Gone"; the http image and the video load. The first image's media menu › Open in file viewer opens `docs/shot.png` (before: `shot.png` at the workspace root, "Unable to load workspace image."). In a draft (New thread › work) every image loads (before: loading slots only), as the reference's draft. Fragments: Bun test | [md1-files.png](https://raw.githubusercontent.com/ccheever/exact2/772421381b367736dafe755b3c6efc47f1fd3dc0/media-signed-urls-and-attachment-markdown/md1-files.png), [md1-files-end.png](https://raw.githubusercontent.com/ccheever/exact2/dbaa46e85435c48b844ff334371547dc8f23a5ed/media-signed-urls-and-attachment-markdown/md1-files-end.png), [md1-open-file.png](https://raw.githubusercontent.com/ccheever/exact2/3dd5cce7cbedafcc861ffe4c855d76ffaff9abe6/media-signed-urls-and-attachment-markdown/md1-open-file.png), [md1-draft.png](https://raw.githubusercontent.com/ccheever/exact2/d7de5b90e72e7a66d68e838bda713acf449c8d43/media-signed-urls-and-attachment-markdown/md1-draft.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/2b0d0629429aee5c63ed90e3977aa0ae231237e6/media-signed-urls-and-attachment-markdown/drive-evidence.txt) |
| MD-2 | pass (agent, reference over CDP): the sent `attached-notes.md` is the chat renderer (since #378) and its media resolve as the reference's ChatMarkdown with no `cwd` and no thread: "Image unavailable · Relative", "Image unavailable · Absolute", the http image, and in the sentence "Image unavailable · In" then the http image. Before: nothing drawn for the relative image, the absolute one signed and shown, the sentence's images as alt text | [md2-attachment.png](https://raw.githubusercontent.com/ccheever/exact2/b6d7c1a5ce2b4185f46865d466836007b5ca1d3a/media-signed-urls-and-attachment-markdown/md2-attachment.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/2b0d0629429aee5c63ed90e3977aa0ae231237e6/media-signed-urls-and-attachment-markdown/drive-evidence.txt) |
| Review R1-1 (inline video failure) | pass (agent, reference over CDP): Files › `docs/review.md`, "A clip in a line of text: ![Broken clip](broken.mp4)" (a file that is not a video) draws MediaVideoPlayer's 16:9 failure slot "Video unavailable · Broken clip" with Retry video and Open in browser inside the line, as the reference (before: the alt text; the PR's first head: the small "Video unavailable" chip). Bun test | [r1-review.png](https://raw.githubusercontent.com/ccheever/exact2/00f7c1e532a78d100e17ffde894ad75546378540/media-signed-urls-and-attachment-markdown/r1-review.png), [r1-drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/e4f7a4873f488f28355325d421cffaf7a5a3afec/media-signed-urls-and-attachment-markdown/r1-drive-evidence.txt) |
| Review R1-2 (badge row) | pass (agent, reference over CDP): `![CI](ci.png) ![npm](npm.png)` draws two inline images, as the reference (before: one "Image unavailable · CI", its source `ci.png) ![npm](npm.png`). Rust and Bun tests | [r1-review.png](https://raw.githubusercontent.com/ccheever/exact2/00f7c1e532a78d100e17ffde894ad75546378540/media-signed-urls-and-attachment-markdown/r1-review.png), [r1-drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/e4f7a4873f488f28355325d421cffaf7a5a3afec/media-signed-urls-and-attachment-markdown/r1-drive-evidence.txt) |
| Review R1-3 (adjacent, no alt, list item, quote, table lines) | pass (agent, reference over CDP): `![CI](ci.png)![CI](ci.png)` draws two images and `![](npm.png)` its image (before: "CICI", "npm.png"); `- ![Gone item](missing.png)` and `> ![Gone quote](missing.png)` draw the 16:9 frame "Image unavailable · …" in the item and the quote, as the reference (before: the alt text). Table lines and an alt text with a `[`: Rust and Bun tests (no media run, nothing signed). Table cells and linked images: "Seen while building" | [r1-review.png](https://raw.githubusercontent.com/ccheever/exact2/00f7c1e532a78d100e17ffde894ad75546378540/media-signed-urls-and-attachment-markdown/r1-review.png), [r1-drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/e4f7a4873f488f28355325d421cffaf7a5a3afec/media-signed-urls-and-attachment-markdown/r1-drive-evidence.txt) |
| MD-1 under a real pointer | open: real-input step 1 below | — |

Review round 1's drive (one session each side, the same steps, fresh storage `msu-r1-before` / `msu-r1-after`): a new
`docs/review.md` in both lanes' project (two 96×20 badges `ci.png`, `npm.png`, and `broken.mp4`, a file that is not a
video); before on the evidence worktree, after on this branch's bundle built from the round's code, the reference on
the after lane's project in the thread "Media attachment".

Drives (agent mode, 1280×840, `--epoch 2026-10-12T06:00:00Z`, fresh storage each): before on the evidence worktree at
`6db5cadea` (lane `media-signed-urls-and-attachment-markdown-before`), after on this branch's bundle (lane
`media-signed-urls-and-attachment-markdown`), the same steps; a loopback fixture server on 16483 served the http image;
`T3_AGENT_MEDIA_PICKS=open-file` answered the media menu. The reference is T3 Code `1e2ecbd975` (Electron, `ref-app.sh`,
CDP 16481) on the after lane's project and its own T3 home with the same seeded thread. No message was sent.

## Real-input batch steps

Launch this branch's bundle as a lane copy (its own bundle id, a lane T3 home outside any git checkout, the lane's
`projects/work` with `docs/media.md`, `docs/shot.png`, `assets/logo.png` from the lane seed); a thread in project work.
Never send a message.

1. **MD-1, an inline image under a real pointer.** Files › docs › media.md › Show rendered markdown. Hover the first
   image in "Beside the file: [image] and its parent: [image]": the tooltip shows its full path
   (`…/projects/work/docs/shot.png`). Right-click it: the media menu opens at the pointer with Copy full path, Copy
   relative path, Open in file viewer, Save image, Copy image. Choose Copy relative path, then paste into the composer
   (do not send): `docs/shot.png`. Clear the composer. Right-click it again › Open in file viewer: a `shot.png` tab opens
   with the breadcrumbs "work › docs › shot.png" and the image.

## Tests

- `media-signed-urls-and-attachment-markdown.test.ts` (10): the inline images and their hrefs (code spans, escapes, a
  link's text, fences and context references left out; an image line listed in both forms); a badge row is two inline
  images and no image line, and table lines and an alt text with a `[` give no media run (review R1-2, R1-3 (4)); a
  source joins the file's folder while its reference and Open in file viewer are the workspace's, and its fragment
  rides on the signed URL; Files signs every host-path image, inline ones too, through the thread with the workspace
  references; a draft signs through its own id; an attachment's host paths are blocked and nothing is signed; FlowRuns
  draws a media run through its chip with `standalone` from the run, else the alt text, and the image block passes
  `standalone=true`; an inline image's zero-size loading span and chip, a standalone image's 16:9 frame; a video,
  inline or not, is MarkdownMediaBody's player with Retry video and the open link (R1-1).
- `macos/src/markdown_media.rs` (`cargo test -p t3-code-macos --lib`, 4 new): the rewrite (inline, block, a paragraph's
  image line, code, escape, link text, an alt text with a `[`, fence, table cell, a badge row and an image with text
  after it; the line count kept), an inline image's one media run and its resolved href (emphasis in the alt joined, an
  emptied source; two adjacent images with one source stay two, an image with no alt text has its run; a badge row's
  two runs; no run keeps IMAGE_START), standalone images (a list item's, a quote's, an emphasised paragraph's; not a
  task item's, a heading's, one beside text, two in an item, nor a sent message's image line between lines of text),
  an attachment's image sources kept while its relative link is targetless; `markdown_links.rs`'s attachment test
  checks `links_without_cwd`.
- `media-actions.test.ts`, `r4-timeline-chips.test.ts`: an image line's chip comes with its media-run chip; an image in
  a message is a media chip, not a file chip.

## Checks

Review round 1, final code head `cc86dcd5b` (merged with `origin/feat(example)/t3-code` `e91bdc8af` first):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4677 pass, 1 skip, 0 fail (314 files). The first run of the round exited 1: Bun's path filter also matched two temporary copies of `examples/t3-code` this round had extracted under `target/` to measure plan sizes (its 14 failures were all in those copies); the copies were removed and the suite run again |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 30,266,551 bytes, 115,901 nodes |
| `cargo test -p t3-code-macos --lib` | 0 | 25 passed (4 in `markdown_media.rs`) |
| AppKit binaries | not run | no Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,756 passed, 0 failed, 36 ignored |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once, before the round's live drive, from the code of `cc86dcd5b` |

`app.contract`: 1,398 lines (unchanged). Plan size: the feature tip `e91bdc8af` is 28,865,666 bytes (110,196 nodes); this
PR's first head merged with it, 29,580,060 (112,016); this round, 30,266,551 (115,901): +1,400,885 bytes over the tip
(+4.9%), +686,491 for the round. Nearly all of it is R1-1: `FlowRuns` is inlined 140 times (`BlockBody` 35×, four
flowed block kinds each) and its media run now holds `MarkdownMedia` with `MarkdownMediaBody` (the video player and its
failure slot) where it held the 13-node `MarkdownInlineMedia`. `caps.mjs` sets no plan budget.

Earlier, the first head `7ac9bdd40` (on `6db5cadea`): every check above exited 0 (4661 Bun tests, 24 Rust tests in
`t3-code-macos`, 3,679 workspace tests), 29,534,552 bytes.

## Decision needed: decided

The inline video's failure slot (R1-1) is built as the reference draws it, at +4.9% plan size (30,266,551 bytes, +1,400,885
over the feature tip `e91bdc8af`). Decided by the coordinator on 2026-10-12: the size is accepted, as the user accepted
size costs for parity before (U15, RL-5).

## Not done / not verified

- MD-1 under a real pointer: ran in `realinput-1011c` ("Real-input results" below); the image's tooltip was not seen.

## Seen while building, not a finding row

- Every Markdown image in the reference is a "Preview <alt>" button that opens the expanded image dialog as a gallery
  of the document's images (`expandableMarkdownImageProps`, `markdownImageGallery`); the clone's Markdown images (chat,
  Files, attachment) have no press. Not in this task's specification.
- The reference caps a Markdown image's box at `min(100%, 30rem)` with a 1px frame (`CHAT_MARKDOWN_MEDIA_FRAME_CLASS_NAME`)
  and its loading slot at 30rem; the clone's are `100%` (no frame) and 40rem. Not changed (pixel-level).
- Review R1-3 (3), not a regression from the base: an image in a table cell (`| ![x](a.png) |`) and an image inside a
  link's text (`[![CI](ci.svg)](https://ci)`, a README's linked badge) still draw their alt text (the cell's text; the
  link's words), where the reference draws the image (a cell is a standalone block; a linked image is drawn in its
  `<a>`, standalone when alone). The clone's table sizes each column from its widest cell's text
  (r4_timeline_tables.rs `table_block`, an invisible sizer row), which has no measure for an image, and a run is a link
  or a media run, not both (one `href`): each needs its own structure (an image cell sizer; a media run that carries
  its link's target and press). markdown_media.rs and `markdownInlineImages` leave both as text, so nothing there is
  signed.
- A Markdown video whose signature is refused (a missing file) shows the 16:9 "Video unavailable · <alt>" slot without
  Retry video, block and inline alike, where MediaVideoPlayer offers Retry video for any failure (`onRetry` re-signs
  the URL); a video that fails to play has Retry video and the open link in both. Pre-existing in the block (media-actions).
- The reference draws a list item's bullet at the foot of a standalone image's 16:9 frame (the marker sits on the
  last line box's baseline); the clone's sits at the top of the item. Pixel-level.

## Real-input results

`realinput-1011c` (2026-10-12 21:43 to 10-13 20:17 KST, five sittings; the attended session for the verify-1011c groups g1–g5 and the rows the 2026-10-11/12 merges brought) ran on the bundle of `1f813789b`, the T3 tip after #497, which holds this task: one lane copy per batch beside the reference T3 Code `1e2ecbd975`, real keys and pointer, light and dark from each lane app's own Appearance setting, never System Settings. Each row got one pass; failures were recorded, not retried. Session notes: [00-session-notes.txt](https://raw.githubusercontent.com/ccheever/exact2/a94ddd5cdbf485f1f57dc92c965c46c4bb17c8be/realinput-1011c/00-session-notes.txt).

| Step | Session row | Result | Evidence |
| --- | --- | --- | --- |
| 1, MD-1 | MSU-1 (B13) | Partial: the media menu at the pointer, Copy relative path (`docs/shot.png`) and Open in file viewer pass ((3), (4)); (2) not observed: hovering the image shows no readable tooltip (an empty AX help tag, nothing drawn), so its full path could not be read | [MSU-1-tip.png](https://raw.githubusercontent.com/ccheever/exact2/e82734c90b35e3677e8f3119050ddec5516f241e/realinput-1011c/b13-fixture-lane/MSU-1-tip.png) |

The g4 rows over the media ran on the same bundle: G4-07's save sheet passes in the clone; G4-05 (a click on a chat Markdown image opens no expanded-image dialog) and G4-06 (the media take no focus, and offer no `AXShowMenu`) pass in part. All are in `STATUS.md` "Known differences".

## Delivery

Merged on 2026-10-12 as `bce7abdee` (#465, squash) into `feat(example)/t3-code`.

## Next action

None. Closed after `realinput-1011c`; MSU-1's tooltip is in `STATUS.md` "Known differences" (the user's decision of 2026-10-11: no new tasks from real-input failures).
