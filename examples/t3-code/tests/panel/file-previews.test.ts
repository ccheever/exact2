// File previews at T3 Code 7c3b3b3605ef: the classifiers (packages/shared filePreview.ts, delimitedPreview.ts),
// maskMarkdownFrontmatter, and what Files and a sent file's preview draw for Markdown with frontmatter, Mermaid files,
// images, audio, fonts, files outside the workspace and the documents whose viewers are not here yet.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { binaryPreviewKind, filePreviewDelimiter, filePreviewKind, isMarkdownPreviewFile, isMermaidPreviewFile, isWorkspaceAudioPreviewPath, isWorkspaceDocumentPath, isWorkspaceFontPreviewPath, maskMarkdownFrontmatter, officePreviewKind } from '../../src/panel/file-preview-kind';
import { attachmentLocal, attachmentView, previewBody, type AttachmentMeta } from '../../src/panel/attachment-surface';
import { filesLocal, filesState, filesView, setMarkdownTaskChecked } from '../../src/panel/files-surface';
import { filesPrefs } from '../../src/panel/files-prefs';
import { MERMAID_REFUSED } from '../../src/panel/mermaid-file';
import { base64, fontSpecimen, forgetFontPages } from '../../src/panel/font-preview';
import { imagePreviewView } from '../../src/timeline/timeline-attachments';
import { memoryAppFiles, serveAssets } from '../support/asset-server';

describe('filePreviewKind (filePreview.test.ts)', () => {
  test.each([
    ['zip', 'archive', 'application/zip'], ['jar', 'archive', 'application/java-archive'], ['tar', 'archive', 'application/x-tar'], ['gz', 'archive', 'application/gzip'],
    ['tgz', 'archive', 'application/gzip'], ['sqlite', 'sqlite', 'application/vnd.sqlite3'], ['sqlite3', 'sqlite', 'application/vnd.sqlite3'], ['db', 'sqlite', 'application/vnd.sqlite3'],
  ])('recognizes binary preview %s', (extension, kind, mimeType) => {
    expect(filePreviewKind({ name: `files#draft/sample.${extension.toUpperCase()}`, mimeType: 'application/octet-stream' })).toBe(kind as never);
    expect(filePreviewKind({ name: 'download', mimeType })).toBe(kind as never);
  });
  test('compound archives, MIME aliases and a definite type over the name', () => {
    expect(filePreviewKind({ name: 'archive.tar.gz' })).toBe('archive');
    expect(filePreviewKind({ name: 'download', mimeType: 'application/x-gzip' })).toBe('archive');
    expect(filePreviewKind({ name: 'download', mimeType: 'application/x-sqlite3' })).toBe('sqlite');
    expect(filePreviewKind({ name: 'misleading.zip', mimeType: 'application/pdf' })).toBe('pdf');
    expect(binaryPreviewKind({ name: 'x.db', mimeType: 'text/csv' })).toBeNull();
  });
  test.each(['ttf', 'otf', 'woff', 'woff2'])('previews %s fonts', extension => {
    expect(filePreviewKind({ name: `sample.${extension}`, mimeType: 'application/octet-stream' })).toBe('font');
    expect(filePreviewKind({ name: 'download', mimeType: `font/${extension}` })).toBe('font');
    expect(isWorkspaceFontPreviewPath(`/tmp/sample.${extension.toUpperCase()}`)).toBe(true);
    expect(isWorkspaceFontPreviewPath(`sample.${extension}?download=1`)).toBe(false);
    expect(filePreviewKind({ name: `sample.${extension}`, mimeType: 'application/zip' })).toBe('archive');
  });
  test.each([
    ['quarterly-report.DOCX', 'docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'],
    ['finance#q4/operating-plan.xlsx', 'xlsx', 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'],
    ['slides?review/quarterly-review.pptx', 'pptx', 'application/vnd.openxmlformats-officedocument.presentationml.presentation'],
    ['notes.odt', 'odt', 'application/vnd.oasis.opendocument.text'], ['budget.ods', 'ods', 'application/vnd.oasis.opendocument.spreadsheet'],
    ['deck.odp', 'odp', 'application/vnd.oasis.opendocument.presentation'], ['notes.rtf', 'rtf', 'application/rtf'],
    ['guide.epub', 'epub', 'application/epub+zip'], ['analysis.ipynb', 'ipynb', 'application/x-ipynb+json'],
  ])('recognizes Office assets at literal path %s', (name, kind, mimeType) => {
    expect(officePreviewKind({ name })).toBe(kind as never);
    expect(filePreviewKind({ name, mimeType: 'application/octet-stream' })).toBe(kind as never);
    expect(filePreviewKind({ name: 'download', mimeType })).toBe(kind as never);
    expect(filePreviewKind({ name, mimeType: 'application/zip' })).toBe('archive');
    expect(isWorkspaceDocumentPath(name)).toBe(true);
  });
  test.each([
    ['example.json', 'application/octet-stream', 'text'], ['report.pdf', 'application/pdf', 'pdf'], ['page.HTML', '', 'html'], ['diagram.mmd', '', 'mermaid'],
    ['diagram.MERMAID', 'text/plain', 'mermaid'], ['download', 'text/vnd.mermaid', 'mermaid'], ['source-code.otf', 'application/vnd.oasis.opendocument.formula-template', 'font'],
    ['notes.rtf', 'application/rtf', 'rtf'], ['notes.rtf', '', 'rtf'], ['download', 'text/rtf', 'rtf'], ['analysis.IPYNB', 'application/json', 'ipynb'],
    ['clip.weba', '', 'audio'], ['image.bmp', 'application/octet-stream', 'image'], ['photo.heic', 'application/octet-stream', 'image'], ['scan.tiff', 'application/octet-stream', 'image'],
    ['recording.mp3', '', 'audio'], ['recording.webm-audio', '', 'audio'], ['recording.webm', 'audio/webm', 'audio'], ['payload', 'application/problem+json', 'text'],
    ['archive.zip', 'application/zip', 'archive'], ['misleading.json', 'application/pdf', 'pdf'], ['misleading.pdf', 'application/zip', 'archive'],
    ['notes.markdown', '', 'markdown'], ['Makefile', '', 'text'], ['blob.bin', 'application/octet-stream', 'unsupported'],
  ])('classifies %s (%s) as %s', (name, mimeType, expected) => {
    expect(filePreviewKind({ name, mimeType })).toBe(expected as never);
  });
  test.each(['doc', 'xls', 'ppt', '7z', 'rar'])('keeps no preview for %s', extension => {
    expect(filePreviewKind({ name: `sample.${extension}` })).toBe('unsupported');
  });
  test('Files paths: Markdown, Mermaid, audio; delimiters', () => {
    expect(['a.md', 'a.mdx', 'guide.markdown', 'a.MDOWN', 'a.mkd', 'a.txt'].map(isMarkdownPreviewFile)).toEqual([true, true, true, true, true, false]);
    expect(['flow.mmd', 'flow.Mermaid', 'flow.md'].map(isMermaidPreviewFile)).toEqual([true, true, false]);
    expect(['/tmp/tone.MP3', 'tone.m4a', 'tone.webm-audio', 'clip.mp4', 'notes.txt'].map(isWorkspaceAudioPreviewPath)).toEqual([true, true, true, false, false]);
    expect([filePreviewDelimiter({ name: 'x.tsv' }), filePreviewDelimiter({ name: 'x.csv', mimeType: 'application/json' }), filePreviewDelimiter({ name: 'x', mimeType: 'text/csv' })]).toEqual(['\t', null, ',']);
  });
  test('the kinds whose viewer is not here yet: a notebook is its source, the others have no preview', () => {
    expect(['ipynb', 'docx', 'archive', 'sqlite', 'epub', 'rtf', 'font', 'mermaid'].map(kind => previewBody(kind as never)))
      .toEqual(['text', 'unsupported', 'unsupported', 'unsupported', 'unsupported', 'unsupported', 'font', 'mermaid']);
  });
});

describe('maskMarkdownFrontmatter (markdownFrontmatter.test.ts)', () => {
  test('blanks leading frontmatter without moving later offsets', () => {
    const markdown = '---\r\nname: skill\r\n---\r\n# Skill\n- [ ] item\n';
    const masked = maskMarkdownFrontmatter(markdown);
    expect(masked).toBe('   \r\n           \r\n   \r\n# Skill\n- [ ] item\n');
    expect(masked.indexOf('[ ]')).toBe(markdown.indexOf('[ ]'));
    // The offset the data module finds in the masked text writes the file's own line.
    expect(setMarkdownTaskChecked(markdown, masked.indexOf('[ ]'), true)).toBe('---\r\nname: skill\r\n---\r\n# Skill\n- [x] item\n');
  });
  test('a byte order mark, CR line endings, a ... closing fence, and UTF-16 units kept', () => {
    expect(maskMarkdownFrontmatter('﻿---\na: 1\n---\n# Body')).toBe('    \n    \n   \n# Body');
    expect(maskMarkdownFrontmatter('---\ra: 1\r...\r# Body')).toBe('   \r    \r   \r# Body');
    expect(maskMarkdownFrontmatter('---\n---\n# Body')).toBe('   \n   \n# Body');
    const emoji = '---\ntitle: 🎉 é\n---\n- [ ] x';
    expect(maskMarkdownFrontmatter(emoji).length).toBe(emoji.length);
    expect(maskMarkdownFrontmatter(emoji).indexOf('[ ]')).toBe(emoji.indexOf('[ ]'));
  });
  test('leaves unterminated or non-leading fences alone', () => {
    expect(maskMarkdownFrontmatter('---\na: 1\n')).toBe('---\na: 1\n');
    expect(maskMarkdownFrontmatter('# A\n---\nb\n---\n')).toBe('# A\n---\nb\n---\n');
    expect(maskMarkdownFrontmatter('---\na: 1\n----\n')).toBe('---\na: 1\n----\n');
  });
});

describe('the font specimen (FontViewer)', () => {
  test('base64 pads only the last chunk', () => {
    expect([base64(new Uint8Array([])), base64(new TextEncoder().encode('f')), base64(new TextEncoder().encode('fo')), base64(new TextEncoder().encode('foo'))]).toEqual(['', 'Zg==', 'Zm8=', 'Zm9v']);
    const long = new Uint8Array(12_288 * 2 + 1).map((_, at) => at % 251);
    expect(base64(long)).toBe(Buffer.from(long).toString('base64'));
  });
  test('the document: its policy admits only the data font and its own script, the samples, the hidden body', () => {
    const page = fontSpecimen('data:application/octet-stream;base64,AAAA');
    expect(page).toContain(`content="default-src 'none'; font-src data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"`);
    expect(page).toContain("@font-face{font-family:preview;src:url('data:application/octet-stream;base64,AAAA')}");
    expect(page).toContain('<h1>Font preview</h1>');
    expect(page).toContain('ABCDEFGHIJKLMNOPQRSTUVWXYZ<br>abcdefghijklmnopqrstuvwxyz<br>0123456789 !?&amp;@#%(){}[]');
    expect([12, 16, 24, 40, 64].every(size => page.includes(`<p style="font-size:${size}px"><small>${size} px</small>The quick brown fox jumps over the lazy dog.</p>`))).toBe(true);
    expect(page).toContain('visibility:hidden');
    expect(page).toContain('parent.postMessage(ok?"font-loaded":"font-failed","*")');
  });
});

// A sent file's preview: the server's signed asset routes and the app's files in memory.
const native = { available: true } as Native;
function attachmentClient(list: AttachmentMeta[], calls: Obj[] = []) {
  const item = { type: 'user_message', attachments: list.map(file => ({ type: 'file', ...file })) };
  return {
    ready: true, generation: 1, threadId: 't1', environmentId: 'env', origin: 'http://127.0.0.1:1', local: { clientSettings: { fontFamilyCode: '' } }, presentation: {},
    projection: { visibleTurnItems: [{ item }] },
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, ...payload }); return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}?sig=1` }; },
    restAccess: () => ({ call: async () => ({ ok: true }) }),
  } as unknown as T3Client;
}

describe('a sent file\'s preview (AttachmentFilePreview at 7c3b3b3605ef)', () => {
  let server: ReturnType<typeof serveAssets>;
  let body: string | Uint8Array = '';
  beforeEach(() => { body = ''; server = serveAssets(() => ({ body })); forgetFontPages(); });
  afterEach(() => server.restore());

  test('rendered Markdown skips its frontmatter; the source keeps it', async () => {
    const meta = { id: 'md1', name: 'SKILL.md', mimeType: 'text/markdown', sizeBytes: 40 };
    body = '---\nname: skill\n---\n# Skill\n- [ ] item\n';
    const c = attachmentClient([meta]);
    const view = await attachmentView(c, native, meta, 1000);
    expect([view.preview, view.markdownSource[0]?.body]).toEqual(['markdown', '   \n           \n   \n# Skill\n- [ ] item\n']);
    await attachmentLocal(c, native, 'render', meta.id, 1000);
    const source = await attachmentView(c, native, meta, 1000);
    expect([source.preview, source.lines[0]?.runs.map(run => run.text).join('')]).toEqual(['code', '---']);
  });

  test('a Mermaid file draws its diagram, toggles to its numbered source, and refuses configuration', async () => {
    const meta = { id: 'm1', name: 'flow.mmd', mimeType: '', sizeBytes: 30 };
    body = 'graph TD\n  A --> B\n';
    const c = attachmentClient([meta]);
    const view = await attachmentView(c, native, meta, 1000);
    expect([view.preview, view.canRender, view.renderLabel, view.renderIcon, view.showWrap, view.diagrams.length, view.diagrams[0]?.code, view.mermaidNote])
      .toEqual(['mermaid', true, 'Show Mermaid source', 'code', false, 1, 'graph TD\n  A --> B', '']);
    expect(view.diagrams[0]?.srcLight).toContain('view=file');
    await attachmentLocal(c, native, 'render', meta.id, 1000);
    const source = await attachmentView(c, native, meta, 1000);
    expect([source.preview, source.renderLabel, source.renderIcon, source.showWrap, source.lines.length]).toEqual(['code', 'Show rendered diagram', 'eye', true, 3]);
    const unsafe = { id: 'm2', name: 'init.mermaid', mimeType: '', sizeBytes: 30 };
    body = '%%{init: {"theme": "dark"}}%%\ngraph TD\n  A --> B\n';
    const refused = await attachmentView(attachmentClient([unsafe]), native, unsafe, 1000);
    expect([refused.preview, refused.diagrams, refused.mermaidNote]).toEqual(['mermaid', [], MERMAID_REFUSED]);
  });

  test('a font shows its specimen page; a face that does not load, or a font over 10 MB, has no preview and says why', async () => {
    const files = memoryAppFiles();
    const meta = { id: 'f1', name: 'Inter.ttf', mimeType: 'font/ttf', sizeBytes: 4 };
    body = new Uint8Array([0, 1, 0, 0]);
    const c = attachmentClient([meta]);
    const view = await attachmentView(c, native, meta, 1000);
    expect([view.preview, view.canSave, view.canCopy, view.canRender]).toEqual(['font', true, false, false]);
    expect(view.url).toMatch(/^app:\/cache\/media-preview\/font-\d+\.html$/);
    expect(new TextDecoder().decode(files.get(view.url))).toContain("src:url('data:application/octet-stream;base64,AAEAAA==')");
    expect(server.calls.map(call => call.url)).toEqual(['http://127.0.0.1:1/api/assets/f1?sig=1']);
    await attachmentLocal(c, native, 'font-failed', meta.id, 1000);
    const failed = await attachmentView(c, native, meta, 1000);
    expect([failed.preview, failed.noPreview, failed.noPreviewAlert, failed.canSave]).toEqual(['none', 'Invalid font data in ArrayBuffer. Save it to open in another app.', true, true]);
    // Try again reads the font afresh.
    await attachmentLocal(c, native, 'retry', meta.id, 1000);
    expect((await attachmentView(c, native, meta, 1000)).preview).toBe('font');
    expect(server.calls).toHaveLength(2);
    const large = { id: 'f2', name: 'Huge.otf', mimeType: '', sizeBytes: 11 * 1024 * 1024 };
    const big = await attachmentView(attachmentClient([large]), native, large, 1000);
    expect([big.preview, big.noPreview]).toEqual(['none', 'Font previews are limited to 10 MB. Save it to open in another app.']);
  });

  test('an image opens the expanded image dialog on its signed URL; a document without its viewer has no preview; a notebook is its source', async () => {
    const image = { id: 'i1', name: 'shot.png', mimeType: 'image/png', sizeBytes: 9 };
    const docx = { id: 'd1', name: 'report.docx', mimeType: '', sizeBytes: 9 };
    const notebook = { id: 'n1', name: 'analysis.ipynb', mimeType: 'application/x-ipynb+json', sizeBytes: 9 };
    const c = attachmentClient([image, docx, notebook]);
    const shown = await attachmentView(c, native, image, 1000);
    expect([shown.preview, shown.url]).toEqual(['image', 'http://127.0.0.1:1/api/assets/i1?sig=1']);
    await attachmentLocal(c, native, 'image-open', image.id, 1000);
    expect([imagePreviewView(c).imagePreviewName, imagePreviewView(c).imagePreviewReturn]).toEqual(['shot.png', 'attachment-image-zoom']);
    await attachmentLocal(c, native, 'media-error', image.id, 1000);
    expect([(await attachmentView(c, native, image, 1000)).preview, (await attachmentView(c, native, image, 1000)).error]).toEqual(['error', 'Unable to load image.']);
    const office = await attachmentView(c, native, docx, 1000);
    expect([office.preview, office.noPreview, office.noPreviewAlert]).toEqual(['none', 'Save it to open in an app that supports docx files.', false]);
    body = '{"cells": []}';
    expect((await attachmentView(c, native, notebook, 1000)).preview).toBe('code');
  });
});

// Files: the workspace /repo with notes, a guide, a diagram and a tone; /tmp/out outside it.
type Call = { method: string; payload: Record<string, unknown> };
function filesClient() {
  const calls: Call[] = [];
  const contents: Record<string, string> = {
    'guide.markdown': '---\ntitle: Guide\n---\n# Guide\n- [ ] read\n', 'flow.mmd': 'graph TD\n  A --> B\n', 'notes.md': 'first',
    'src/a.ts': 'export {}', '/tmp/out/page.html': '<h1>Out</h1>',
  };
  let listings: Record<string, { path: string; kind: string }[]> = {
    '': [{ path: 'guide.markdown', kind: 'file' }, { path: 'flow.mmd', kind: 'file' }, { path: 'tone.mp3', kind: 'file' }, { path: 'notes.md', kind: 'file' }, { path: 'src', kind: 'directory' }],
    src: [{ path: 'src/a.ts', kind: 'file' }],
  };
  let failListing = false;
  const projection = { visibleTurnItems: [] as Obj[], checkpoints: [] as Obj[] };
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, origin: 'http://127.0.0.1:9', presentation: {}, available: true, connection: 'connected',
    config: {}, local: { clientSettings: { wordWrap: true, fontFamilyCode: '' } }, projection,
    shell: { projects: [{ id: 'p1', title: 'demo', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1' }] },
    rpc: async (_native: Native, method: string, payload: Record<string, unknown>) => { calls.push({ method, payload }); return { relativeUrl: `/api/assets/signed/${calls.length}?sig=1` }; },
    restAccess: () => ({
      request: async (method: string, payload: Record<string, unknown>) => {
        calls.push({ method, payload });
        if (method === 'projects.listEntries') { if (failListing) throw new Error('Listing failed.'); return { entries: listings[String(payload.directoryPath)] ?? [] }; }
        if (method === 'projects.readFile') { const path = String(payload.relativePath), text = contents[path] ?? ''; return { contents: text, byteLength: text.length, truncated: false }; }
        return {};
      },
    }),
  } as unknown as T3Client;
  return { client, calls, contents, projection, setListing: (path: string, entries: { path: string; kind: string }[]) => { listings = { ...listings, [path]: entries }; }, failListings: (on: boolean) => { failListing = on; } };
}
const surface = (path: string) => ({ id: `file:${path}`, kind: 'file', path, reveal: 0 }) as never;
const mints = (calls: Call[]) => calls.filter(call => call.method === 'assets.createUrl').map(call => call.payload.resource);

describe('Files (FilePreviewPanel at 7c3b3b3605ef)', () => {
  test('a .markdown file renders, frontmatter blanked, and offers "Show rendered markdown" while Markdown shows as source', async () => {
    const { client } = filesClient();
    const source = await filesView(client, native, surface('guide.markdown'), 1000);
    expect([source.preview, source.canRender, source.renderLabel]).toEqual(['code', true, 'Show rendered markdown']);
    await filesLocal(client, native, 'render', 'guide.markdown', '');
    const rendered = await filesView(client, native, surface('guide.markdown'), 1000);
    expect([rendered.preview, rendered.markdownSource[0]?.kind, rendered.markdownSource[0]?.body]).toEqual(['markdown', 'file', `   \n${' '.repeat('title: Guide'.length)}\n   \n# Guide\n- [ ] read\n`]);
  });

  test('a Mermaid file draws its diagram (renderMermaid, on by default) and toggles to its numbered source', async () => {
    const { client } = filesClient();
    const view = await filesView(client, native, surface('flow.mmd'), 1000);
    expect([view.preview, view.canRender, view.rendered, view.renderLabel, view.rawText, view.diagrams.length, view.mermaidNote]).toEqual(['mermaid', true, true, 'Show Mermaid source', false, 1, '']);
    await filesLocal(client, native, 'render', 'flow.mmd', '');
    expect(filesPrefs(client).renderMermaid).toBe(false);
    const source = await filesView(client, native, surface('flow.mmd'), 1000);
    expect([source.preview, source.renderLabel, source.rawText, source.lines.length]).toEqual(['code', 'Show rendered diagram', true, 3]);
  });

  test('an audio file plays from its media-file URL; the player\'s error is "Unable to load audio." until Try again signs it afresh', async () => {
    const { client, calls } = filesClient();
    const view = await filesView(client, native, surface('tone.mp3'), 1000);
    expect([view.preview, view.url.startsWith('http://127.0.0.1:9/api/assets/signed/'), view.editable, view.rawText]).toEqual(['audio', true, false, false]);
    expect(mints(calls)).toEqual([{ _tag: 'media-file', threadId: 't1', path: '/repo/tone.mp3' }]);
    await filesLocal(client, native, 'audio-error', view.url, '');
    expect((await filesView(client, native, surface('tone.mp3'), 1000)).preview).toBe('audio-error');
    await filesLocal(client, native, 'audio-retry', '/repo/tone.mp3', '');
    const again = await filesView(client, native, surface('tone.mp3'), 1000);
    expect([again.preview, mints(calls).length]).toEqual(['audio', 2]);
  });

  test('a PDF or page outside the workspace is signed as a media-file and opens as its document', async () => {
    const { client, calls } = filesClient();
    const files = memoryAppFiles();
    const server = serveAssets(() => ({ body: '%PDF-1.4' }));
    try {
      const pdf = await filesView(client, native, surface('/tmp/out/report.pdf'), 1000);
      expect([pdf.preview, pdf.url.startsWith('app:/cache/media-preview/')]).toEqual(['pdf', true]);
      expect(files.has(pdf.url)).toBe(true);
      const page = await filesView(client, native, surface('/tmp/out/page.html'), 1000);
      expect([page.preview, page.canRender, page.renderLabel]).toEqual(['html', true, 'Show HTML source']);
      expect(mints(calls)).toEqual([{ _tag: 'media-file', threadId: 't1', path: '/tmp/out/report.pdf' }, { _tag: 'media-file', threadId: 't1', path: '/tmp/out/page.html' }]);
    } finally { server.restore(); }
  });

  test('a workspace change reads the open file and the listed folders again; a failed refresh keeps the folder and offers a retry', async () => {
    const { client, contents, projection, setListing, failListings } = filesClient();
    expect((await filesView(client, native, surface('notes.md'), 1000)).text).toBe('first');
    await filesLocal(client, native, 'crumb', '.', 'open');
    contents['notes.md'] = 'second';
    setListing('', [{ path: 'notes.md', kind: 'file' }, { path: 'new.md', kind: 'file' }]);
    // No change yet: the read is kept.
    expect((await filesView(client, native, surface('notes.md'), 1000)).text).toBe('first');
    projection.visibleTurnItems.push({ item: { id: 'cmd-1', type: 'command_execution', status: 'completed' } });
    const after = await filesView(client, native, surface('notes.md'), 1000);
    expect([after.text, after.rows.map(row => row.path)]).toEqual(['second', ['new.md', 'notes.md']]);
    expect(after.crumbMenu.entries.map(entry => entry.path)).toEqual(['new.md', 'notes.md']);
    failListings(true);
    projection.visibleTurnItems.push({ item: { id: 'cmd-2', type: 'file_change', status: 'completed' } });
    const failed = await filesView(client, native, surface('notes.md'), 1000);
    expect([failed.crumbMenu.status, failed.crumbMenu.retry, failed.crumbMenu.entries.length]).toEqual(['', 'Refresh failed. Retry', 2]);
    failListings(false);
    await filesLocal(client, native, 'crumb-refresh', '', '');
    expect((await filesView(client, native, surface('notes.md'), 1000)).crumbMenu.retry).toBe('');
    expect(filesState(client).errors.size).toBe(0);
  });
});
