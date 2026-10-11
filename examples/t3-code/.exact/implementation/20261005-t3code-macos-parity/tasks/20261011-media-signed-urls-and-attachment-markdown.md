---
name: 20261011-media-signed-urls-and-attachment-markdown
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified-with-unverified-rows
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-media-signed-urls-and-attachment-markdown
pr_url: null
verified_commit: null
---

# Media in Files' rendered Markdown, and an attachment's Markdown media

## Outcome

The user decided on 2026-10-11 that these residual items of earlier records are built now, matching the reference T3
Code (`1e2ecbd975`):

| Id | From | Clone (feature tip `6db5cadea`) | Reference (check it first) |
| --- | --- | --- | --- |
| MD-1 | [media-actions](closed/20261005-media-actions.md) "Not built / limits": "Files' rendered Markdown preview media (the Files Markdown blocks get no signed URLs; chat Markdown does)" | Images and media in Files › "Show rendered markdown" | Load through the signed URLs the reference uses: FileMarkdownPreview is ChatMarkdown with `cwd` (the workspace) and `imageBaseDir` (the file's folder); a host path becomes a `media-file` asset of the thread (`ChatMarkdownAssetImage`), a web URL loads as written |
| MD-2 | [markdown-links-and-files-preview](closed/20261009-markdown-links-and-files-preview.md) "Found, not changed": "A sent attachment's Markdown preview still uses `r4-surfaces-render.ts`" | A sent attachment's rendered Markdown | The same renderer as the rest of the app: AttachmentFilePreview is `<ChatMarkdown text cwd={undefined} />`, with no thread |

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
image the parser would flatten, `![alt](src)` → `[alt](t3-image:src)`, outside fenced code, code spans, table lines and a
link's text, and keeps an image line that starts a block (an image line that continues a paragraph is inline, as the
parser's paragraph takes it in); the line structure is kept, so task offsets hold. The resolver wraps `link_href`: the
run's href is `t3-image:` plus the source as the URL transform leaves it (an emptied source or a fragment loads
nothing, as `markdownImageHref`), and `chat_run` makes it a `media` run, one token in `flow_tokens`. `media-views.ts
markdownInlineImages` lists the same images with the same href, so each gets a media chip and, for a host path, its
signed URL (`markdownMediaUrls` signs inline and block images); `markdown.contract FlowRuns` draws a media run with a
chip through `MarkdownInlineMedia` (`media-markdown.contract`): ChatMarkdownImage not standalone, a zero-size status
span while the URL is signed, the image in MediaActions once it is, ChatMarkdownImageFallback's chip (with the menu when
it has a source) when it cannot load. A surface whose chips carry no media (a pull request's text) keeps the alt text.
The same rule now holds in the transcript and an attachment, as the reference's ChatMarkdown draws inline images
everywhere.

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

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| MD-1 | pass (agent, 1280×840, reference over CDP): Files › `docs/media.md` rendered draws the two images inside the sentence in the line ("Beside the file:" image "and its parent:" image), as the reference; every host-path image signs as a `media-file` asset; the missing one is the 16:9 frame "Image unavailable · Gone"; the http image and the video load. The first image's media menu › Open in file viewer opens `docs/shot.png` (before: `shot.png` at the workspace root, "Unable to load workspace image."). In a draft (New thread › work) every image loads (before: loading slots only), as the reference's draft. Fragments: Bun test | [md1-files.png](https://raw.githubusercontent.com/ccheever/exact2/772421381b367736dafe755b3c6efc47f1fd3dc0/media-signed-urls-and-attachment-markdown/md1-files.png), [md1-files-end.png](https://raw.githubusercontent.com/ccheever/exact2/dbaa46e85435c48b844ff334371547dc8f23a5ed/media-signed-urls-and-attachment-markdown/md1-files-end.png), [md1-open-file.png](https://raw.githubusercontent.com/ccheever/exact2/3dd5cce7cbedafcc861ffe4c855d76ffaff9abe6/media-signed-urls-and-attachment-markdown/md1-open-file.png), [md1-draft.png](https://raw.githubusercontent.com/ccheever/exact2/d7de5b90e72e7a66d68e838bda713acf449c8d43/media-signed-urls-and-attachment-markdown/md1-draft.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/2b0d0629429aee5c63ed90e3977aa0ae231237e6/media-signed-urls-and-attachment-markdown/drive-evidence.txt) |
| MD-2 | pass (agent, reference over CDP): the sent `attached-notes.md` is the chat renderer (since #378) and its media resolve as the reference's ChatMarkdown with no `cwd` and no thread: "Image unavailable · Relative", "Image unavailable · Absolute", the http image, and in the sentence "Image unavailable · In" then the http image. Before: nothing drawn for the relative image, the absolute one signed and shown, the sentence's images as alt text | [md2-attachment.png](https://raw.githubusercontent.com/ccheever/exact2/b6d7c1a5ce2b4185f46865d466836007b5ca1d3a/media-signed-urls-and-attachment-markdown/md2-attachment.png), [drive-evidence.txt](https://raw.githubusercontent.com/ccheever/exact2/2b0d0629429aee5c63ed90e3977aa0ae231237e6/media-signed-urls-and-attachment-markdown/drive-evidence.txt) |
| MD-1 under a real pointer | open: real-input step 1 below | — |

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

- `media-signed-urls-and-attachment-markdown.test.ts` (8): the inline images and their hrefs (code spans, escapes, a
  link's text, fences and context references left out; an image line listed in both forms); a source joins the file's
  folder while its reference and Open in file viewer are the workspace's, and its fragment rides on the signed URL;
  Files signs every host-path image, inline ones too, through the thread with the workspace references; a draft signs
  through its own id; an attachment's host paths are blocked and nothing is signed; FlowRuns draws a media run through
  its chip, else the alt text; an inline image's zero-size loading span and chip; a standalone failure's 16:9 frame.
- `macos/src/markdown_media.rs` (`cargo test -p t3-code-macos --lib`, 3 new): the rewrite (inline, block, a paragraph's
  image line, code, escape, link text, fence, table cell; the line count kept), an inline image's one media run and its
  resolved href (emphasis in the alt joined, an emptied source), an attachment's image sources kept while its relative
  link is targetless; `markdown_links.rs`'s attachment test now checks `links_without_cwd`.
- `media-actions.test.ts`, `r4-timeline-chips.test.ts`: an image line's chip comes with its media-run chip; an image in
  a message is a media chip, not a file chip.

## Checks

Final code head `7ac9bdd40` (merged with `origin/feat(example)/t3-code` `6db5cadea`: nothing new to merge):

| Check | Exit | Note |
| --- | --- | --- |
| `bun test examples/t3-code --timeout 60000` | 0 | 4661 pass, 1 skip, 0 fail (312 files) |
| strict `tsc` (README command) | 0 | |
| `bun scripts/exact.mjs contract build examples/t3-code/app.contract` | 0 | 29,534,552 bytes, 112,015 nodes |
| `cargo test -p t3-code-macos --lib` | 0 | 24 passed (3 new), on the final Rust code |
| AppKit binaries | not run | no Swift changed |
| `git add -A && bun scripts/caps.mjs` | 0 | all budgets within cap |
| `cargo build --all-targets --keep-going` | 0 | |
| `cargo test --lib --bins --tests --no-fail-fast` | 0 | 3,679 passed, 0 failed |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | |
| `bun scripts/boot.mjs` | 0 | |
| `host/apple/build.mjs t3-code-macos --bundle` | 0 | once, before the live drive, from `7ac9bdd40` |

`app.contract`: 1,398 lines (unchanged). The plan grows from 28,820,298 to 29,534,552 bytes (+714,254, 2.5%; 110,195 →
112,015 nodes): `FlowRuns` is inlined 140 times (`BlockBody` 35×, four flowed block kinds each) and each now holds the
media run's `MarkdownInlineMedia` (13 nodes).

## Decision needed

- **An inline video's failure slot.** An image inside a line of text that is a video plays in the line, but if it fails
  it shows the "Video unavailable · <alt>" chip, while the reference's ChatMarkdownVideo (MediaVideoPlayer) shows the 16:9
  failure slot with Retry video and the open link, as a video alone on its line does in the clone. Giving the inline
  media the block's player (`MarkdownMediaBody`) was built and measured: the plan grows a further 752,857 bytes (to
  30,287,409, +5.1% over the base) because FlowRuns is inlined 140 times. Built: the lean chip. Decide: accept the
  difference for this rare case, or take the plan size for parity.

## Not done / not verified

- MD-1 under a real pointer (the native media menu over an inline image, Copy relative path into the pasteboard):
  real-input step 1, for the coordinator's session (real input is reserved for it).

## Seen while building, not a finding row

- Every Markdown image in the reference is a "Preview <alt>" button that opens the expanded image dialog as a gallery
  of the document's images (`expandableMarkdownImageProps`, `markdownImageGallery`); the clone's Markdown images (chat,
  Files, attachment) have no press. Not in this task's specification.
- The reference caps a Markdown image's box at `min(100%, 30rem)` with a 1px frame (`CHAT_MARKDOWN_MEDIA_FRAME_CLASS_NAME`)
  and its loading slot at 30rem; the clone's are `100%` (no frame) and 40rem. Not changed (pixel-level).

## Next action

The coordinator reviews the draft PR; real-input step 1 in the next session; the decision above.
