---
name: 20261011-media-signed-urls-and-attachment-markdown
plan: 20261005-t3code-macos-parity
implementation: in-progress
verification: unverified
delivery: none
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

What the first drive of the tip showed (lane `media-signed-urls-and-attachment-markdown-before`, a `docs/media.md` with
images beside the file, in its parent, absolute, missing, on a loopback http host, a video, and two images inside a
sentence; a seeded thread whose message carries a Markdown attachment with a relative, an absolute and an http image):

- MD-1. #371 already gave Files' rendered Markdown the transcript's signed URLs: each image alone on its line loads
  (`media-file` assets), the missing one is "Image unavailable · Gone", the video plays. Still different from the
  reference: (a) an image inside a line of text is drawn as its alt text ("Beside the file: Shot and its parent: Logo"),
  where the reference draws the images in the line (ChatMarkdownImage, not standalone); (b) the media menu's paths are
  relative to the file's folder, not the workspace: "Open in file viewer" on `docs/shot.png`'s image opened `shot.png`
  at the workspace root ("Unable to load workspace image."), and "Copy relative path" copies `shot.png`, where the
  reference's ChatMarkdownAssetImage takes `workspaceRoot={cwd}` (`docs/shot.png`); (c) in a draft thread nothing is
  signed (`markdownMediaUrls` needs a server thread id), so every host-path image keeps its loading slot, where the
  reference signs through the draft's thread ref (the server ignores the thread for an absolute `media-file` path,
  `ws.ts`); (d) a source fragment (`clip.mp4#t=2`) is dropped from the signed URL (`markdownImageSourceFragment`).
- MD-2. #378 (audit-wave-followups-3 FW-2) already renders a sent attachment's Markdown with the chat renderer
  (`renderFileMarkdown`, kind `attachment`; `r4-surfaces-render.ts` keeps only the CSV/TSV parser). Still different:
  with no thread and no `cwd` the reference cannot sign a host path, so a relative and an absolute image are both
  "Image unavailable · <alt>" (ChatMarkdownImageFallback); the clone draws an empty image for both (an angle-bracketed
  or relative source loses its chip match) and would sign an absolute one written without brackets.

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
