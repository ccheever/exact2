// media-signed-urls-and-attachment-markdown (T3 Code 1e2ecbd975, MIT; see LICENSE-T3): ChatMarkdown's `img` renderer
// in Files' rendered Markdown (FileMarkdownPreview: `cwd` the workspace, `imageBaseDir` the file's folder, the thread or
// draft ref) and in a sent attachment's (AttachmentFilePreview: no `cwd`, no thread).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { surfaceLocal, panelView } from './r4-surfaces-panel';
import { draftThreadId } from './r7-handoff-thread';
import { decodeClientPrefs } from './settings-core';
import { decodeMediaSource } from './media-actions';
import { INLINE_IMAGE, markdownImages, markdownInlineImages, markdownMedia, markdownMediaChips } from './media-views';

type Call = { method: string; payload: Record<string, unknown> };
const native = { available: true } as unknown as Native;
const FILE = '# Media\n\nBeside the file: ![Shot](shot.png) and its parent: ![Logo](../assets/logo.png)\n\n![Shot](shot.png)\n\n![Abs](</repo/assets/logo.png>)\n\n![Web](http://127.0.0.1:16483/badge.png)\n\n![Clip](clip.mp4#t=2)\n';
const ATTACHED = '# Attached notes\n\n![Relative](docs/shot.png)\n\n![Absolute](/repo/assets/logo.png)\n\n![Web](http://127.0.0.1:16483/badge.png)\n\nInline ![In](/repo/docs/shot.png) too.\n';

function fakeClient(over: Record<string, unknown> = {}) {
  const calls: Call[] = [];
  const replies: Record<string, (payload: Record<string, unknown>) => unknown> = {
    'projects.listEntries': payload => ({ entries: payload.directoryPath === '' ? [{ path: 'docs', kind: 'directory' }] : [{ path: 'docs/media.md', kind: 'file' }], truncated: false }),
    'projects.readFile': () => ({ relativePath: 'docs/media.md', contents: FILE, byteLength: FILE.length, truncated: false }),
    'assets.createUrl': payload => ({ relativeUrl: `/api/assets/${encodeURIComponent(String((payload.resource as { path?: string; attachmentId?: string }).path ?? (payload.resource as { attachmentId?: string }).attachmentId))}?sig=1`, expiresAt: 0 }),
    attachmentText: () => ({ ok: true, text: ATTACHED, truncated: false }),
  };
  const request = async (method: string, payload: Record<string, unknown>) => { calls.push({ method, payload }); const reply = replies[method]; if (!reply) throw new Error(`no reply for ${method}`); return reply(payload); };
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, connection: 'connected', origin: 'http://127.0.0.1:16482', revision: 0, generation: 1,
    diffOpen: false, diffText: '', diffError: '', diffLoading: false,
    get draftKey() { return `env:${this.threadId || `new:${this.projectId}`}`; },
    config: { environment: { capabilities: {} } },
    local: { clientSettings: { ...decodeClientPrefs({}), wordWrap: true }, composerControls: {} },
    projection: { visibleTurnItems: [{ item: { type: 'user_message', text: 'Read it.', attachments: [{ type: 'file', id: 'att-md', name: 'attached-notes.md', mimeType: 'text/markdown', sizeBytes: 200 }] } }], messages: [], thread: {} },
    shell: { projects: [{ id: 'p1', title: 'work', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1', pullRequests: [] as unknown[] }] },
    rpc: async (_native: unknown, method: string, payload: Record<string, unknown>) => request(method, payload),
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>) => request(method, payload),
      call: async (call: Record<string, unknown>) => { calls.push({ method: String(call.op), payload: call }); const reply = replies[String(call.op)]; return reply ? reply(call) : {}; },
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `draft-thread-${index}`),
    }),
    ...over,
  } as unknown as T3Client;
  return { client, calls, replies };
}
const signed = (calls: Call[]) => calls.filter(call => call.method === 'assets.createUrl').map(call => call.payload.resource as Record<string, string>);
async function renderedFile(client: T3Client) {
  await surfaceLocal(client, native, 'file', 'docs/media.md', '');
  let files = (await panelView(client, native, 1000)).files;
  if (files.preview !== 'markdown') { await surfaceLocal(client, native, 'files-render', 'docs/media.md', ''); files = (await panelView(client, native, 1000)).files; }
  return files;
}

describe('MD-1: media in Files\' rendered Markdown', () => {
  test('an image inside a line of text is listed with the href markdown_media.rs gives its run', () => {
    const text = 'See ![Logo **x**](<../a b.png> "t") and ![Web](https://a.com/w.png) or ![Bad](javascript:x.png).\n![Alone](a.png)\n`![code](c.png)` \\![esc](e.png) [![linked](l.png)](https://a.com) ![ctx](t3-context://v1/image/i)\n```\n![fenced](f.png)\n```';
    expect(markdownInlineImages(text)).toEqual([
      { alt: 'Logo **x**', src: '../a b.png', href: `${INLINE_IMAGE}t3-file:../a b.png` },
      { alt: 'Web', src: 'https://a.com/w.png', href: `${INLINE_IMAGE}https://a.com/w.png` },
      { alt: 'Bad', src: 'javascript:x.png', href: INLINE_IMAGE },
      { alt: 'Alone', src: 'a.png', href: `${INLINE_IMAGE}t3-file:a.png` },
    ]);
    // An image line may be a block or continue a paragraph (markdown_media.rs decides): both hrefs have its chip.
    expect(markdownMediaChips('![Alone](a.png)', '/repo').map(chip => [chip.href, chip.target])).toEqual([['t3-file:a.png', 'media:/repo/a.png'], [`${INLINE_IMAGE}t3-file:a.png`, 'media:/repo/a.png']]);
  });
  test('a badge row is two inline images, not one image line; table lines and an alt text with a `[` are not media runs', () => {
    // The review's case: `![CI](ci.png) ![npm](npm.png)` was one image whose source was `ci.png) ![npm](npm.png`.
    const badges = '![CI](ci.png) ![npm](npm.png)\n\n![Alone](a.png "t")\n\n![a](x.png) and (b)';
    expect(markdownImages(badges).map(image => image.src)).toEqual(['a.png']);
    expect(markdownMediaChips(badges, '/repo').filter(chip => chip.icon === 'environment').map(chip => [chip.href, chip.target])).toEqual([
      ['t3-file:a.png', 'media:/repo/a.png'], [`${INLINE_IMAGE}t3-file:ci.png`, 'media:/repo/ci.png'], [`${INLINE_IMAGE}t3-file:npm.png`, 'media:/repo/npm.png'],
      [`${INLINE_IMAGE}t3-file:a.png`, 'media:/repo/a.png'], [`${INLINE_IMAGE}t3-file:x.png`, 'media:/repo/x.png'],
    ]);
    // A table's cells draw text (markdown_media.rs leaves table lines), so nothing there is signed; nor is an alt text with a `[`.
    expect(markdownInlineImages('| a | b |\n| - | - |\n| ![cell](c.png) | x |\n\nSee ![a[b](x.png) and ![ok](o.png)')).toEqual([]);
    expect(markdownInlineImages('See ![a[b](x.png) here\n\n| a |\n| - |\n| ![cell](c.png) |\n\nThen ![ok](o.png)')).toEqual([{ alt: 'ok', src: 'o.png', href: `${INLINE_IMAGE}t3-file:o.png` }]);
  });
  test('a source joins the file\'s folder, its references are the workspace\'s, its fragment rides on the signed URL', () => {
    const shot = markdownMedia('Shot', 'shot.png', '', '/repo/docs', { cwd: '/repo' });
    expect(shot).toMatchObject({ access: 'environment', key: 'media:/repo/docs/shot.png', fragment: '',
      source: { reference: { kind: 'file', path: '/repo/docs/shot.png', relativePath: 'docs/shot.png' }, openFile: 'docs/shot.png', asset: { resource: { _tag: 'media-file', path: '/repo/docs/shot.png' } } } });
    expect(markdownMedia('Clip', 'clip.mp4#t=2', '', '/repo/docs', { cwd: '/repo' })).toMatchObject({ kind: 'video', key: 'media:/repo/docs/clip.mp4#t=2', fragment: '#t=2',
      source: { asset: { resource: { path: '/repo/docs/clip.mp4' } }, openFile: 'docs/clip.mp4' } });
    // Before: the folder was the reference root too (Open in file viewer opened `shot.png` at the workspace root).
    expect(markdownMedia('Shot', 'shot.png', '', '/repo/docs').source).toMatchObject({ openFile: 'shot.png' });
  });
  test('Files signs every host-path image through the thread, inline ones too, with the workspace references', async () => {
    const { client, calls } = fakeClient();
    const files = await renderedFile(client);
    expect(files.markdownSource).toEqual([{ id: 'file:docs/media.md', kind: 'file', title: '', body: FILE }]);
    const media = files.md.chips.filter(chip => chip.kind === 'media');
    expect(media.filter(chip => chip.href.startsWith(INLINE_IMAGE)).map(chip => [chip.href, chip.icon, chip.target])).toEqual([
      [`${INLINE_IMAGE}t3-file:shot.png`, 'environment', 'media:/repo/docs/shot.png'], [`${INLINE_IMAGE}t3-file:../assets/logo.png`, 'environment', 'media:/repo/docs/../assets/logo.png'],
      [`${INLINE_IMAGE}t3-file:/repo/assets/logo.png`, 'environment', 'media:/repo/assets/logo.png'], [`${INLINE_IMAGE}http://127.0.0.1:16483/badge.png`, 'direct', 'http://127.0.0.1:16483/badge.png'],
      [`${INLINE_IMAGE}t3-file:clip.mp4#t=2`, 'environment', 'media:/repo/docs/clip.mp4#t=2'],
    ]);
    const shot = media.find(chip => chip.href === 't3-file:shot.png')!;
    expect(decodeMediaSource(client, shot.detail)).toMatchObject({ reference: { relativePath: 'docs/shot.png' }, openFile: 'docs/shot.png' });
    expect(signed(calls).sort((a, b) => a.path!.localeCompare(b.path!))).toEqual(['/repo/assets/logo.png', '/repo/docs/../assets/logo.png', '/repo/docs/clip.mp4', '/repo/docs/shot.png'].map(path => ({ _tag: 'media-file', path, threadId: 't1' })));
    expect(files.mdUrls.map(entry => [entry.id, entry.url])).toContainEqual(['media:/repo/docs/clip.mp4#t=2', 'http://127.0.0.1:16482/api/assets/%2Frepo%2Fdocs%2Fclip.mp4?sig=1#t=2']);
  });
  test('in a draft the Files preview signs through the draft\'s own thread id, as the reference\'s draft thread ref', async () => {
    const { client, calls } = fakeClient({ threadId: '' });
    const files = await renderedFile(client);
    expect(draftThreadId(client)).toBe('draft-thread-0');
    expect(signed(calls).map(resource => resource.threadId)).toEqual(['draft-thread-0', 'draft-thread-0', 'draft-thread-0', 'draft-thread-0']);
    expect(files.mdUrls.map(entry => entry.id)).toContain('media:/repo/docs/shot.png');
  });
});

describe('MD-2: a sent attachment\'s Markdown media', () => {
  test('with no folder and no thread a host path is unavailable and nothing is signed; a web URL loads as written', async () => {
    const { client, calls } = fakeClient();
    await surfaceLocal(client, native, 'r5-attachment', 'att-md', '');
    const view = await panelView(client, native, 1000);
    expect(view.attachment.markdownSource).toEqual([{ id: 'attachment:att-md', kind: 'attachment', title: '', body: ATTACHED }]);
    const chips = view.attachment.md.chips.filter(chip => chip.kind === 'media');
    // ChatMarkdownImageFallback for a relative and an absolute host path (no actions), the web image as written; an image line's chip
    // comes again as a media run's (markdownInlineImages), with the same resolution.
    expect(chips.filter(chip => !chip.href.startsWith(INLINE_IMAGE)).map(chip => [chip.href, chip.icon, chip.detail === '' ? '' : decodeMediaSource(client, chip.detail)?.src])).toEqual([
      ['t3-file:docs/shot.png', 'blocked', ''], ['t3-file:/repo/assets/logo.png', 'blocked', ''], ['http://127.0.0.1:16483/badge.png', 'direct', 'http://127.0.0.1:16483/badge.png'],
    ]);
    expect(chips.find(chip => chip.href === `${INLINE_IMAGE}t3-file:/repo/docs/shot.png`)).toMatchObject({ icon: 'blocked', detail: '', label: 'In' });
    expect(view.attachment.mdUrls).toEqual([]);
    expect(signed(calls).map(resource => resource._tag ?? '')).toEqual(signed(calls).map(() => 'attachment'));
  });
});

describe('the views', () => {
  const markdown = readFileSync(new URL('./markdown.contract', import.meta.url), 'utf8');
  const media = readFileSync(new URL('./media-markdown.contract', import.meta.url), 'utf8');
  const component = (name: string) => media.slice(media.indexOf(`component ${name}\n`), media.indexOf('\ncomponent ', media.indexOf(`component ${name}\n`) + 1) >>> 0);
  test('FlowRuns draws a media run through its media chip, standalone when its run says so, else its alt text', () => {
    expect(markdown).toContain('when run.kind == "media" and length(filter(chips, (chip) => chip.kind == "media" and chip.href == run.href)) > 0');
    expect(markdown).toContain('MarkdownMedia(alt=run.text, kind=chip.size, access=chip.icon, target=chip.target, tip=chip.tip, source=chip.detail, urls=urls, local=local, standalone=(run.icon == "standalone"), owner=chip.owner)');
    expect(markdown).toContain('MarkdownMedia(alt=chip.label, kind=chip.size, access=chip.icon, target=chip.target, tip=chip.tip, source=chip.detail, urls=urls, local=local, standalone=true, owner=chip.owner)');
    expect(markdown).toContain('and not (run.kind == "media" and length(filter(chips, (chip) => chip.kind == "media" and chip.href == run.href)) > 0)\n          when ms < 0\n            text run.text');
    expect(media).not.toContain('MarkdownInlineMedia');
  });
  test('an inline image has a zero-size status span while it loads and the fallback chip with its menu when it fails; a figure keeps its 16:9 frame', () => {
    const block = component('MarkdownMedia');
    expect(block).toContain('box width=(standalone or kind == "video" ? "100%" : "0px") max-width="40rem" aspect-ratio="16 / 9"');
    expect(block).toContain('when kind != "video" and standalone\n          MediaFailedFrame(alt=alt)\n        when kind != "video" and not standalone\n          MediaUnavailableLabel(alt=alt, video=false)');
    const body = component('MarkdownMediaBody');
    // chat-markdown-media-and-code G4-05: a failed image keeps MediaActions; the settled one is MediaPreviewActions' button.
    expect(body).toContain('when standalone\n          MediaFailedFrame(alt=alt)\n        else\n          MediaUnavailableLabel(alt=alt, video=false)');
    expect(body).toContain('image url aria-label=alt error=imageBroke');
    const frame = component('MediaFailedFrame');
    expect(frame).toContain('max-width="30rem" aspect-ratio="16 / 9"');
    expect(frame).toContain('role="alert"');
  });
  test('a video in a line of text is the block\'s player: its failure slot has Retry video and the open link (ChatMarkdownVideo)', () => {
    const block = component('MarkdownMedia');
    expect(block).toContain('MarkdownMediaBody(url=url, alt=alt, kind=kind, tip=tip, source=source, original=(access == "direct" and (startsWith(target, "http:") or startsWith(target, "https:"))), local=local, standalone=standalone, open=local("image-md-open", target, owner))');
    expect(block).not.toMatch(/when [^\n]*kind == "video"[^\n]*\n\s+video url/);
    const body = component('MarkdownMediaBody');
    expect(body).toContain('aria-label="Retry video"');
    expect(body).toContain('testId="markdown-video-open"');
  });
});
