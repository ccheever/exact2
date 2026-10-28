// File previews at T3 Code 7c3b3b3605ef: the classifiers (packages/shared filePreview.ts, delimitedPreview.ts),
// maskMarkdownFrontmatter, and what Files and a sent file's preview draw for Markdown with frontmatter, Mermaid files,
// images, audio, fonts, files outside the workspace and the documents whose viewers are not here yet.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import { ClientError, type Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { binaryPreviewKind, filePreviewDelimiter, filePreviewKind, isMarkdownPreviewFile, isMermaidPreviewFile, isWorkspaceAudioPreviewPath, isWorkspaceDocumentPath, isWorkspaceFontPreviewPath, maskMarkdownFrontmatter, officePreviewKind } from '../../src/panel/file-preview-kind';
import { attachmentLocal, attachmentView, previewBody, type AttachmentMeta } from '../../src/panel/attachment-surface';
import { linkedViews } from '../../src/panel/pr-attachment-surfaces';
import { filesHidden, filesLocal, filesState, filesView, setMarkdownTaskChecked } from '../../src/panel/files-surface';
import { panelState } from '../../src/panel/panel-surfaces';
import { filesPrefs } from '../../src/panel/files-prefs';
import { MERMAID_REFUSED } from '../../src/panel/mermaid-file';
import { base64, fontSpecimen, forgetFontPages } from '../../src/panel/font-preview';
import { imagePreviewView } from '../../src/timeline/timeline-attachments';
import { lastSave, memoryAppFiles, serveAssets } from '../support/asset-server';
import { adoptedFiles } from '../../src/media/save-file';
import { contractComponent, type Env } from '../support/contract-action-fixture';
import { appText } from '../support/app-files';

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
    expect(maskMarkdownFrontmatter('\uFEFF---\na: 1\n---\n# Body')).toBe('    \n    \n   \n# Body');
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

// The right panel showing a sent file's tab, then another surface, as the window's views do (pr-attachment-surfaces.ts).
const showTab = (client: T3Client, meta: AttachmentMeta, now = 1000) => linkedViews(client, native, { id: `attachment:${meta.id}`, kind: 'attachment', path: meta.name, line: 0, reveal: 0, attachment: meta }, true, now).then(views => views.attachment);
const showOther = (client: T3Client) => linkedViews(client, native, null, true, 1000);

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
    // The tab shown again after another surface checks the font again, with the page it kept.
    await showOther(c);
    expect((await showTab(c, meta)).preview).toBe('font');
    expect(server.calls).toHaveLength(1);
    const large = { id: 'f2', name: 'Huge.otf', mimeType: '', sizeBytes: 11 * 1024 * 1024 };
    const big = await attachmentView(attachmentClient([large]), native, large, 1000);
    expect([big.preview, big.noPreview]).toEqual(['none', 'Font previews are limited to 10 MB. Save it to open in another app.']);
  });

  test('a font whose specimen left the four kept pages is written again, from a URL signed afresh after five minutes', async () => {
    const files = memoryAppFiles();
    body = new Uint8Array([0, 1, 0, 0]);
    const metas = [1, 2, 3, 4, 5].map(n => ({ id: `f${n}`, name: `F${n}.ttf`, mimeType: 'font/ttf', sizeBytes: 4 }));
    const calls: Obj[] = [];
    const c = attachmentClient(metas, calls);
    const pages: string[] = [];
    for (const meta of metas) pages.push((await attachmentView(c, native, meta, 1000)).url);
    await Bun.sleep(1); // the page that left is deleted
    expect([files.has(pages[0]!), pages.slice(1).every(page => files.has(page))]).toEqual([false, true]);
    // Shown within five minutes, the page is written again from the URL signed first.
    const first = await attachmentView(c, native, metas[0]!, 1000 + 60_000);
    expect([first.preview, first.url !== pages[0], files.has(first.url), calls.length]).toEqual(['font', true, true, 5]);
    // A kept page is shown as it is, however old its URL.
    await attachmentView(c, native, metas[0]!, 1000 + 10 * 60_000);
    expect([calls.length, server.calls.length]).toEqual([5, 6]);
    // Its page gone again, ten minutes after the URL was signed, the font is read from a URL signed afresh.
    for (const meta of metas.slice(1)) await attachmentView(c, native, meta, 1000 + 10 * 60_000);
    const again = await attachmentView(c, native, metas[0]!, 1000 + 10 * 60_000);
    expect([again.preview, files.has(again.url), calls.filter(call => (call.resource as Obj).attachmentId === 'f1').length]).toEqual(['font', true, 2]);
  });

  test('the specimen pages an earlier run left are deleted before the first page is written; other cached files stay', async () => {
    const files = memoryAppFiles(new Map([['app:/cache/media-preview/font-9999.html', new Uint8Array([1])], ['app:/cache/media-preview/3.pdf', new Uint8Array([1])]]));
    (adoptedFiles()!.fs as { readdir?: (dir: string) => Promise<string[]> }).readdir = async dir => [...files.keys()].filter(path => path.startsWith(`${dir}/`)).map(path => path.slice(dir.length + 1));
    body = new Uint8Array([0, 1, 0, 0]);
    const meta = { id: 'f8', name: 'Kept.ttf', mimeType: 'font/ttf', sizeBytes: 4 };
    const view = await attachmentView(attachmentClient([meta]), native, meta, 1000);
    expect([view.preview, files.has(view.url), files.has('app:/cache/media-preview/font-9999.html'), files.has('app:/cache/media-preview/3.pdf')]).toEqual(['font', true, false, true]);
  });

  test('a font that could not be read keeps its reason while its tab is shown, and reads again when it is shown again', async () => {
    memoryAppFiles();
    server.restore();
    server = serveAssets(() => ({ status: 404 }));
    const meta = { id: 'f9', name: 'Gone.ttf', mimeType: 'font/ttf', sizeBytes: 4 };
    const c = attachmentClient([meta]);
    expect((await showTab(c, meta)).noPreview).toBe('The font could not be loaded. Save it to open in another app.');
    await showTab(c, meta);
    expect(server.calls).toHaveLength(1);
    await showOther(c);
    expect((await showTab(c, meta)).noPreview).toBe('The font could not be loaded. Save it to open in another app.');
    expect(server.calls).toHaveLength(2);
  });

  test('a sent PDF is signed again when its tab is shown again; one stored as -bin is shown from a copy', async () => {
    const files = memoryAppFiles();
    body = '%PDF-1.4';
    const pdf = { id: 'p1-pdf', name: 'report.pdf', mimeType: 'application/pdf', sizeBytes: 8 };
    const stored = { id: 'p2-bin', name: 'report', mimeType: 'application/pdf', sizeBytes: 8 };
    const calls: Obj[] = [];
    const c = attachmentClient([pdf, stored], calls);
    const first = await showTab(c, pdf);
    expect([first.preview, first.url]).toEqual(['pdf', 'http://127.0.0.1:1/api/assets/p1-pdf?sig=1']);
    // An hour on, the signature has lapsed: shown again, the tab signs it afresh, as the reference's mount does.
    await showOther(c);
    await showTab(c, pdf, 1000 + 61 * 60_000);
    expect(calls.filter(call => (call.resource as Obj).attachmentId === 'p1-pdf')).toHaveLength(2);
    // Served as a download (no usable extension), it is read as bytes and shown from a copy.
    const bin = await showTab(c, stored);
    expect([bin.preview, bin.url.startsWith('app:/cache/media-preview/'), files.has(bin.url)]).toEqual(['pdf', true, true]);
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
  // The pinned server's `fileRevision` for a media-file (AssetAccess.ts), by path; none for an older server.
  const fileRevisions: Record<string, string> = {};
  // Paths that are folders, and a save the test holds open until it lets it go.
  const folders = new Set<string>(), binaries = new Set<string>();
  let holding = false, release: (() => void) | null = null;
  const projection = { visibleTurnItems: [] as Obj[], checkpoints: [] as Obj[] };
  const client = {
    environmentId: 'env', threadId: 't1', projectId: 'p1', ready: true, origin: 'http://127.0.0.1:9', presentation: {}, available: true, connection: 'connected',
    config: {}, local: { clientSettings: { wordWrap: true, fontFamilyCode: '' }, composerControls: {} }, projection, draftKey: 'draft:1',
    shell: { projects: [{ id: 'p1', title: 'demo', workspaceRoot: '/repo' }], threads: [{ id: 't1', projectId: 'p1' }] },
    rpc: async (_native: Native, method: string, payload: Record<string, unknown>) => {
      calls.push({ method, payload });
      const revision = fileRevisions[String((payload.resource as Obj | undefined)?.path)];
      return { relativeUrl: `/api/assets/signed/${calls.length}?sig=1`, ...(revision ? { fileRevision: revision } : {}) };
    },
    restAccess: () => ({
      ids: async (count: number) => Array.from({ length: count }, (_, index) => `draft-thread-${index + 1}`),
      request: async (method: string, payload: Record<string, unknown>) => {
        calls.push({ method, payload });
        if (method === 'projects.listEntries') { if (failListing) throw new Error('Listing failed.'); return { entries: listings[String(payload.directoryPath)] ?? [] }; }
        if (method === 'projects.readFile') {
          const path = String(payload.relativePath), text = contents[path] ?? '';
          if (folders.has(path)) throw new Error(`${path} is a directory`);
          if (binaries.has(path)) throw new ClientError('The file is binary.', 'client', false, { context: { failure: 'binary_file', resolvedPath: `/repo/${path}`, cwd: '/repo' } });
          return { contents: text, byteLength: text.length, truncated: false };
        }
        if (method === 'projects.writeFile') {
          contents[String(payload.relativePath)] = String(payload.contents);
          if (holding) await new Promise<void>(resolve => { release = resolve; });
          return {};
        }
        return {};
      },
    }),
  } as unknown as T3Client;
  const hold = (on: boolean) => { holding = on; if (!on) { release?.(); release = null; } };
  return { client, calls, contents, projection, fileRevisions, folders, binaries, hold, setListing: (path: string, entries: { path: string; kind: string }[]) => { listings = { ...listings, [path]: entries }; }, failListings: (on: boolean) => { failListing = on; } };
}
const surface = (path: string) => ({ id: `file:${path}`, kind: 'file', path, reveal: 0 }) as never;
const mints = (calls: Call[]) => calls.filter(call => call.method === 'assets.createUrl').map(call => call.payload.resource);

describe('Files (FilePreviewPanel at 7c3b3b3605ef)', () => {
  // A font's specimen is kept per preview across views, so each test starts with none.
  beforeEach(() => forgetFontPages());
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
      // Framed at its signed URL: nothing is downloaded, and nothing waits on it.
      const pdf = await filesView(client, native, surface('/tmp/out/report.pdf'), 1000);
      expect([pdf.preview, pdf.url.startsWith('http://127.0.0.1:9/api/assets/signed/'), files.size, server.calls.length]).toEqual(['pdf', true, 0, 0]);
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

  test('a folder whose first listing failed offers "Retry loading folder", whose press lists it again', async () => {
    const { client, failListings } = filesClient();
    failListings(true);
    await filesView(client, native, surface('notes.md'), 1000);
    await filesLocal(client, native, 'crumb', '.', 'open');
    const failed = await filesView(client, native, surface('notes.md'), 1000);
    expect([failed.crumbMenu.status, failed.crumbMenu.statusRetry, failed.crumbMenu.entries.length]).toEqual(['Retry loading folder', true, 0]);
    failListings(false);
    // The row presses its folder as a folder entry does (files-surface.contract `pick(menu.dir, true)`).
    await filesLocal(client, native, 'crumb-dir', failed.crumbMenu.dir, '');
    const listed = await filesView(client, native, surface('notes.md'), 1000);
    expect([listed.crumbMenu.status, listed.crumbMenu.statusRetry, listed.crumbMenu.entries.length]).toEqual(['', false, 5]);
  });

  test('a file in another tab reads again when it is shown after a workspace change (revisionedQuery)', async () => {
    const { client, calls, contents, projection } = filesClient();
    contents['a.md'] = 'a-first'; contents['b.md'] = 'b-first';
    expect((await filesView(client, native, surface('a.md'), 1000)).text).toBe('a-first');
    expect((await filesView(client, native, surface('b.md'), 1000)).text).toBe('b-first');
    contents['a.md'] = 'a-second'; contents['b.md'] = 'b-second';
    projection.visibleTurnItems.push({ item: { id: 'cmd-1', type: 'command_execution', status: 'completed' } });
    expect((await filesView(client, native, surface('b.md'), 1000)).text).toBe('b-second');
    expect((await filesView(client, native, surface('a.md'), 1000)).text).toBe('a-second');
    // Shown again with no change since: neither reads again.
    const reads = () => calls.filter(call => call.method === 'projects.readFile').length, before = reads();
    await filesView(client, native, surface('b.md'), 1000);
    await filesView(client, native, surface('a.md'), 1000);
    expect(reads()).toBe(before);
  });

  test('a notebook or an RTF file opens in the attachment body under "File", signed as a media-file; a change mounts it afresh', async () => {
    const { client, calls, projection } = filesClient();
    const files = memoryAppFiles();
    const server = serveAssets(() => ({ body: '{"cells": []}' }));
    try {
      const notebook = await filesView(client, native, surface('analysis.ipynb'), 1000);
      expect([notebook.preview, notebook.attachment.origin, notebook.attachment.name, notebook.attachment.preview, notebook.attachment.lines[0]?.runs.map(run => run.text).join('')])
        .toEqual(['attachment', 'File', 'analysis.ipynb', 'code', '{"cells": []}']);
      expect(mints(calls).at(-1)).toEqual({ _tag: 'media-file', threadId: 't1', path: '/repo/analysis.ipynb' });
      expect([notebook.lines, notebook.editable, notebook.rawText]).toEqual([[], false, false]);
      projection.visibleTurnItems.push({ item: { id: 'cmd-n', type: 'command_execution', status: 'completed' } });
      await filesView(client, native, surface('analysis.ipynb'), 1000);
      expect(server.calls).toHaveLength(2);
      const rtf = await filesView(client, native, surface('notes.rtf'), 1000);
      expect([rtf.preview, rtf.attachment.preview, rtf.attachment.noPreview]).toEqual(['attachment', 'none', 'Save it to open in an app that supports rtf files.']);
      expect(files.size).toBe(0);
    } finally { server.restore(); }
  });

  test('a file Files cannot read as text opens in the attachment body: "File › tool.exe", Save file, and why there is no preview', async () => {
    const { client, calls, binaries } = filesClient();
    binaries.add('tool.exe');
    const files = memoryAppFiles();
    const server = serveAssets(() => ({ body: new Uint8Array([77, 90]) }));
    try {
      const view = await filesView(client, native, surface('tool.exe'), 1000);
      expect([view.preview, view.previewError, view.errorPath, view.attachment.origin, view.attachment.name, view.attachment.preview, view.attachment.noPreview, view.attachment.canSave])
        .toEqual(['attachment', '', '', 'File', 'tool.exe', 'none', 'Save it to open in an app that supports exe files.', true]);
      // The server signs a file of no preview type only as a download (AssetPreviewTypeValidationError otherwise).
      expect(mints(calls).at(-1)).toEqual({ _tag: 'media-file', threadId: 't1', path: '/repo/tool.exe', disposition: 'attachment' });
      await filesLocal(client, native, 'att-save', view.attachment.id, '');
      expect(mints(calls).at(-1)).toEqual({ _tag: 'media-file', threadId: 't1', path: '/repo/tool.exe', disposition: 'attachment' });
      expect(lastSave(client, files)?.name).toBe('tool.exe');
    } finally { server.restore(); }
  });

  test('a draft signs a binary file under its own thread id; a document\'s Save file asks for it as an attachment', async () => {
    const { client, calls, binaries } = filesClient();
    binaries.add('tool.exe');
    const files = memoryAppFiles();
    const server = serveAssets(() => ({ body: new Uint8Array([1, 2]) }));
    try {
      const report = await filesView(client, native, surface('report.docx'), 1000);
      await filesLocal(client, native, 'att-save', report.attachment.id, '');
      expect(mints(calls).at(-1)).toEqual({ _tag: 'media-file', threadId: 't1', path: '/repo/report.docx', disposition: 'attachment' });
      expect(lastSave(client, files)?.name).toBe('report.docx');
      (client as unknown as { threadId: string }).threadId = '';
      const exe = await filesView(client, native, surface('tool.exe'), 1000);
      expect([exe.attachment.preview, exe.attachment.canSave, exe.attachment.error]).toEqual(['none', true, '']);
      expect(mints(calls).at(-1)).toEqual({ _tag: 'media-file', threadId: 'draft-thread-1', path: '/repo/tool.exe', disposition: 'attachment' });
    } finally { server.restore(); }
  });

  test('a document is not read as text after a change or a Refresh, and Refresh mounts its body afresh', async () => {
    const { client, calls, projection } = filesClient();
    memoryAppFiles();
    const server = serveAssets(() => ({ body: new Uint8Array([0, 1, 0, 0]) }));
    try {
      const reads = () => calls.filter(call => call.method === 'projects.readFile' && call.payload.relativePath === 'Inter.ttf').length;
      await filesView(client, native, surface('Inter.ttf'), 1000);
      const before = reads();
      projection.visibleTurnItems.push({ item: { id: 'cmd-d', type: 'command_execution', status: 'completed' } });
      await filesView(client, native, surface('Inter.ttf'), 1000);
      await filesLocal(client, native, 'att-font-failed', '', '');
      expect((await filesView(client, native, surface('Inter.ttf'), 1000)).attachment.preview).toBe('none');
      const tabs = panelState(client), font = surface('Inter.ttf') as unknown as { id: string };
      tabs.surfaces = [font as never]; tabs.active = font.id; tabs.visible = true;
      await filesLocal(client, native, 'refresh', '', '');
      expect(reads()).toBe(before);
      expect((await filesView(client, native, surface('Inter.ttf'), 1000)).attachment.preview).toBe('font');
    } finally { server.restore(); }
  });

  test('Files hidden unmounts the body; shown again, it tries again', async () => {
    const { client } = filesClient();
    memoryAppFiles();
    const server = serveAssets(() => ({ body: new Uint8Array([0, 1, 0, 0]) }));
    try {
      await filesView(client, native, surface('Inter.ttf'), 1000);
      await filesLocal(client, native, 'att-font-failed', '', '');
      expect((await filesView(client, native, surface('Inter.ttf'), 1000)).attachment.preview).toBe('none');
      filesHidden(client);
      expect((await filesView(client, native, surface('Inter.ttf'), 1000)).attachment.preview).toBe('font');
    } finally { server.restore(); }
  });

  test('a file the agent turns binary after a good read is the attachment body\'s, as its read error now says', async () => {
    const { client, contents, projection, binaries } = filesClient();
    contents['out.json'] = '{"ok": true}';
    expect((await filesView(client, native, surface('out.json'), 1000)).preview).toBe('code');
    binaries.add('out.json');
    projection.visibleTurnItems.push({ item: { id: 'cmd-gz', type: 'command_execution', status: 'completed' } });
    const view = await filesView(client, native, surface('out.json'), 1000);
    expect([view.preview, view.editable, view.attachment.name]).toEqual(['attachment', false, 'out.json']);
  });

  test('a font in Files shows its specimen in the attachment body; a draft\'s document is its draft-workspace-file', async () => {
    const { client, calls } = filesClient();
    const files = memoryAppFiles();
    const server = serveAssets(() => ({ body: new Uint8Array([0, 1, 0, 0]) }));
    try {
      const font = await filesView(client, native, surface('Inter.ttf'), 1000);
      expect([font.preview, font.attachment.preview, font.attachment.origin, files.has(font.attachment.url)]).toEqual(['attachment', 'font', 'File', true]);
      expect(mints(calls).at(-1)).toEqual({ _tag: 'media-file', threadId: 't1', path: '/repo/Inter.ttf' });
      await filesLocal(client, native, 'att-font-failed', font.attachment.id, '');
      expect((await filesView(client, native, surface('Inter.ttf'), 1000)).attachment.noPreview).toBe('Invalid font data in ArrayBuffer. Save it to open in another app.');
      // Another file, then the font again: its body mounted afresh checks the font again.
      await filesView(client, native, surface('notes.md'), 1000);
      expect((await filesView(client, native, surface('Inter.ttf'), 1000)).attachment.preview).toBe('font');
      (client as unknown as { threadId: string }).threadId = '';
      await filesView(client, native, surface('report.docx'), 1000);
      expect(mints(calls).at(-1)).toEqual({ _tag: 'draft-workspace-file', cwd: '/repo', path: '/repo/report.docx' });
    } finally { server.restore(); }
  });

  test('an audio file keeps its URL across a change that leaves it alone, by the fileRevision the server reports', async () => {
    const { client, calls, projection, fileRevisions } = filesClient();
    fileRevisions['/repo/tone.mp3'] = '1:2:100:5:5';
    const first = await filesView(client, native, surface('tone.mp3'), 1000);
    expect(new URL(first.url).searchParams.get('file-revision')).toBe('1:2:100:5:5');
    // `ls`: signed again, the same file, the same URL, so the player keeps its place.
    projection.visibleTurnItems.push({ item: { id: 'cmd-ls', type: 'command_execution', status: 'completed' } });
    const unchanged = await filesView(client, native, surface('tone.mp3'), 1000);
    expect([unchanged.url, mints(calls).length]).toEqual([first.url, 2]);
    // The agent rewrites it: a new revision, a new URL.
    fileRevisions['/repo/tone.mp3'] = '1:2:200:6:6';
    projection.visibleTurnItems.push({ item: { id: 'cmd-write', type: 'file_change', status: 'completed' } });
    const changed = await filesView(client, native, surface('tone.mp3'), 1000);
    expect(new URL(changed.url).searchParams.get('file-revision')).toBe('1:2:200:6:6');
  });

  test('with a server that reports no fileRevision, an audio file takes the workspace revision (WorkspaceAudioPreview)', async () => {
    const { client, calls, projection } = filesClient();
    const first = await filesView(client, native, surface('tone.mp3'), 1000);
    // Six minutes on, the signature is renewed, and the player keeps the URL it plays.
    const later = await filesView(client, native, surface('tone.mp3'), 1000 + 6 * 60_000);
    expect([later.url, mints(calls).length]).toEqual([first.url, 2]);
    projection.visibleTurnItems.push({ item: { id: 'cmd-a', type: 'command_execution', status: 'completed' } });
    const changed = await filesView(client, native, surface('tone.mp3'), 1000 + 6 * 60_000);
    expect(mints(calls).length).toBe(3);
    expect(changed.url).not.toBe(first.url);
    expect(new URL(changed.url).searchParams.get('workspace-revision')).toBe('["cmd-a",null]');
  });

  // WorkspaceVideoPreview at 7c3b3b3605ef: each workspace change signs the video again, and the file's revision says
  // whether it changed: the same revision keeps the player's source, a new one (the file rewritten) is a new URL.
  test('a video file the agent rewrites gets a new URL naming its new revision; one left alone keeps its revision', async () => {
    const { client, projection, fileRevisions } = filesClient();
    fileRevisions['/repo/clip.mp4'] = 'v1';
    const first = (await filesView(client, native, surface('clip.mp4'), 1000)).media;
    expect([new URL(first.url).searchParams.get('file-revision'), first.revision]).toEqual(['v1', 'v1']);
    projection.visibleTurnItems.push({ item: { id: 'cmd-ls', type: 'command_execution', status: 'completed' } });
    expect((await filesView(client, native, surface('clip.mp4'), 1000)).media.revision).toBe('v1');
    fileRevisions['/repo/clip.mp4'] = 'v2';
    projection.visibleTurnItems.push({ item: { id: 'cmd-write', type: 'file_change', status: 'completed' } });
    const changed = (await filesView(client, native, surface('clip.mp4'), 1000)).media;
    expect([changed.url === first.url, new URL(changed.url).searchParams.get('file-revision'), changed.revision]).toEqual([false, 'v2', 'v2']);
  });

  test('an image, a video and a PDF carry the workspace revision in their URL, so a change loads them again (a playing video holds)', async () => {
    const { client, projection } = filesClient();
    const files = memoryAppFiles();
    const server = serveAssets(() => ({ body: '%PDF-1.4' }));
    try {
      const image = await filesView(client, native, surface('photo.png'), 1000);
      expect([image.preview, image.media.url.includes('workspace-revision')]).toEqual(['media', false]);
      const video = await filesView(client, native, surface('clip.mp4'), 1000);
      expect([video.media.kind, video.media.url.startsWith('http://127.0.0.1:9/api/assets/signed/')]).toEqual(['video', true]);
      const pdf = await filesView(client, native, surface('report.pdf'), 1000);
      expect(pdf.preview).toBe('pdf');
      projection.visibleTurnItems.push({ item: { id: 'cmd-m', type: 'file_change', status: 'completed' } });
      const imageAfter = await filesView(client, native, surface('photo.png'), 1000);
      expect(new URL(imageAfter.media.url).searchParams.get('workspace-revision')).toBe('["cmd-m",null]');
      // A server that reports no file revision: the video is signed again and names the workspace's, which its player
      // takes once playback pauses (media-actions.contract FilesVideo).
      const videoAfter = (await filesView(client, native, surface('clip.mp4'), 1000)).media;
      expect([videoAfter.url === video.media.url, new URL(videoAfter.url).searchParams.get('workspace-revision'), videoAfter.revision]).toEqual([false, '["cmd-m",null]', '["cmd-m",null]']);
      // The frame loads the new revision itself (BrowserDocumentFrame's documentKey); no copy is written.
      const pdfAfter = await filesView(client, native, surface('report.pdf'), 1000);
      expect([pdfAfter.preview, new URL(pdfAfter.url).searchParams.get('workspace-revision'), files.size]).toEqual(['pdf', '["cmd-m",null]', 0]);
    } finally { server.restore(); }
  });

  test('a file open in the editor takes a change from outside when no save is pending; with one pending it keeps its text', async () => {
    const { client, contents, projection, hold } = filesClient();
    contents['notes.md'] = 'first';
    await filesView(client, native, surface('notes.md'), 1000);
    await filesLocal(client, native, 'begin-edit', 'notes.md', '1');
    const editing = await filesView(client, native, surface('notes.md'), 1000);
    expect([editing.editing, editing.editorText, editing.editorRevision]).toEqual([true, 'first', 0]);
    contents['notes.md'] = 'the agent\'s';
    projection.visibleTurnItems.push({ item: { id: 'cmd-e1', type: 'file_change', status: 'completed' } });
    const replaced = await filesView(client, native, surface('notes.md'), 1000);
    expect([replaced.editing, replaced.editorText, replaced.editorRevision]).toEqual([true, 'the agent\'s', 1]);
    // A save pending (selectedFilePending): the file is not read again, and the editor keeps what was typed.
    hold(true);
    const saving = filesLocal(client, native, 'edit', 'notes.md', 'typed');
    contents['notes.md'] = 'the agent\'s again';
    projection.visibleTurnItems.push({ item: { id: 'cmd-e2', type: 'file_change', status: 'completed' } });
    const kept = await filesView(client, native, surface('notes.md'), 1000);
    expect([kept.text, kept.editorRevision, kept.pending]).toEqual(['typed', 1, true]);
    hold(false);
    await saving;
  });

  test('images, audio and video are not read again after a workspace change; a folder is, whatever its name', async () => {
    const { client, calls, projection, folders } = filesClient();
    folders.add('album.png');
    const paths = ['photo.png', 'tone.mp3', 'clip.mp4', 'album.png'];
    const reads = () => paths.map(path => calls.filter(call => call.method === 'projects.readFile' && call.payload.relativePath === path).length);
    for (const path of paths) await filesView(client, native, surface(path), 1000);
    const before = reads();
    projection.visibleTurnItems.push({ item: { id: 'cmd-r', type: 'command_execution', status: 'completed' } });
    for (const path of paths) await filesView(client, native, surface(path), 1000);
    expect(reads()).toEqual([before[0], before[1], before[2], before[3]! + 1]);
  });

  test('a press on the picture opens the expanded image dialog on it (PreviewImageContent)', async () => {
    const { client } = filesClient();
    await filesLocal(client, native, 'image-open', 'http://127.0.0.1:9/api/assets/signed/1?sig=1', 'photo.png');
    expect([imagePreviewView(client).imagePreviewName, imagePreviewView(client).imagePreviewReturn]).toEqual(['photo.png', 'file-image-zoom']);
  });
});

// PreviewImageContent's bar, run from preview-image.contract with Contract's action semantics.
describe('the image size and zoom bar (PreviewImageContent)', () => {
  const bar = contractComponent('preview-image.contract', 'PreviewImage');
  const helpers: Env = { min: Math.min, max: Math.max, round: Math.round, formatNumber: (n: number) => n.toLocaleString('en-US'), expand: () => {}, failed: () => {} };
  const derived = (state: Env) => bar.derives(state, helpers);
  const run = (state: Env, action: string, ...args: unknown[]) => bar.call(action, state, { ...helpers, ...derived(state) }, args);

  test('the natural size, the fitted scale, ×1.25 steps between 5 % and 800 %, Fit image and Actual image size', () => {
    const state = bar.state();
    expect([derived(state).sizeLabel, derived(state).canZoomIn, derived(state).canZoomOut]).toEqual(['Loading image…', false, false]);
    run(state, 'viewport', 540, 700);
    run(state, 'pictured', 4000, 3000);
    run(state, 'pictured', 100, 100); // only the first, unsized layout is the natural size
    expect([derived(state).sizeLabel, derived(state).percent, state.zoom]).toEqual(['4,000 × 3,000', '13%', -1]); // fit: (540 − 32) / 4000
    run(state, 'changeZoom', 1.25);
    expect(derived(state).percent).toBe('16%');
    for (let step = 0; step < 40; step++) run(state, 'changeZoom', 1.25);
    expect([state.zoom, derived(state).percent, derived(state).canZoomIn]).toEqual([8, '800%', false]);
    for (let step = 0; step < 60; step++) run(state, 'changeZoom', 0.8);
    expect([state.zoom, derived(state).percent, derived(state).canZoomOut]).toEqual([0.05, '5%', false]);
    run(state, 'actualSize');
    expect([state.zoom, derived(state).percent]).toEqual([1, '100%']);
    run(state, 'fitImage');
    expect([state.zoom, derived(state).percent]).toEqual([-1, '13%']);
  });
});

// FilePreview's attLocal: the attachment body's controls go to Files' own ops; the rest go on to the window.
describe('the attachment body in Files (files-surface.contract)', () => {
  test('its controls become surface-files-att-* ops; the rest pass through', () => {
    const preview = contractComponent('files-surface.contract', 'FilePreview');
    const sent: string[][] = [];
    const run = (op: string) => preview.call('attLocal', preview.state(), { local: (...args: string[]) => sent.push(args) }, [op, 'id', 'v']);
    run('surface-linked-att-save');
    run('chatlocal:copy-code');
    expect(sent).toEqual([['surface-files-att-save', 'id', 'v'], ['chatlocal:copy-code', 'id', 'v']]);
  });
});

// StandardAudioPreview's loadedSource, run from files-surface.contract's FilesAudio.
describe('the audio player in Files (AudioPreview)', () => {
  test('a URL renewed while it plays waits until playback pauses', () => {
    const audio = contractComponent('files-surface.contract', 'FilesAudio');
    const state = audio.state();
    let url = 'http://h/a?sig=1';
    const src = () => audio.derives(state, { url }).src;
    const run = (action: string) => audio.call(action, state, { url, failed: () => {}, ...audio.derives(state, { url }) }, []);
    expect(src()).toBe('http://h/a?sig=1');
    run('started');
    url = 'http://h/a?sig=2&workspace-revision=r2';
    expect(src()).toBe('http://h/a?sig=1');
    run('started'); // playing again after a stall keeps the URL it plays
    expect(src()).toBe('http://h/a?sig=1');
    run('stopped');
    expect(src()).toBe('http://h/a?sig=2&workspace-revision=r2');
  });

  test('a failure of the URL it held drops it for the newest; only a failure of the newest is the file\'s', () => {
    const audio = contractComponent('files-surface.contract', 'FilesAudio');
    const state = audio.state(), failures: string[] = [];
    let url = 'http://h/a?file-revision=1';
    const run = (action: string, ...args: unknown[]) => audio.call(action, state, { url, failed: (message: string) => failures.push(message), ...audio.derives(state, { url }) }, args);
    run('started');
    url = 'http://h/a?file-revision=2';
    run('broke', 'decode'); // the URL it held, replaced while it played
    expect([failures, audio.derives(state, { url }).src]).toEqual([[], 'http://h/a?file-revision=2']);
    run('broke', 'decode'); // the newest
    expect(failures).toEqual(['decode']);
  });

  test('the view wires the player\'s events, and reports a failure for the URL it gave', () => {
    const view = appText('files-surface.contract');
    expect(view).toContain('video src controls=true preload="metadata" aria-label=label playing=started pause=stopped ended=stopped error=broke');
    expect(view).toContain('FilesAudio(url=files.url, label=files.path, failed=local("surface-files-audio-error", files.url))');
  });
});
