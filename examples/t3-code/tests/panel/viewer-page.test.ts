// The document viewers' page as the app frames it (viewer-frames.ts, attachment-surface.ts, files-viewer.ts,
// notebook-preview.ts; T3 Code 7c3b3b3605ef AttachmentFilePreview, FilePreviewPanel's WorkspaceLargeTextPreview,
// NotebookPreview and BinaryViewer's ArchiveFile): what each kind asks the page to show, the posts and the page's
// messages, a draft's bytes, a streamed text's Copy contents, Files' Full file and table, notebooks and archive entries.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import type { Obj } from '../../src/client/domain';
import { attachmentLocal, attachmentOp, attachmentView, forgetPreview, type AttachmentMeta, type PreviewSource } from '../../src/panel/attachment-surface';
import { newFrame, postBytes, postToViewer, viewerFrameId, viewerFrameView, viewerMessage, viewerPick } from '../../src/panel/viewer-frames';
import { memoryAppFiles, serveAssets } from '../support/asset-server';

const native = { available: true } as Native;
function client(list: AttachmentMeta[], calls: Obj[] = []) {
  const item = { type: 'user_message', attachments: list.map(file => ({ type: 'file', ...file })) };
  return {
    ready: true, generation: 1, threadId: 't1', environmentId: 'env', origin: 'http://127.0.0.1:1', local: { clientSettings: { fontFamilyCode: '' } }, presentation: {},
    projection: { visibleTurnItems: [{ item }] },
    rpc: async (_native: Native, method: string, payload: Obj) => { calls.push({ method, ...payload }); return { relativeUrl: `/api/assets/${(payload.resource as Obj).attachmentId}?sig=1` }; },
    restAccess: () => ({ call: async (request: Obj) => { calls.push({ op: request.op, text: request.text }); return { ok: true }; } }),
  } as unknown as T3Client;
}
const config = (view: { viewer: { config: string } }) => JSON.parse(view.viewer.config) as Obj;
let server: ReturnType<typeof serveAssets>;
beforeEach(() => { server = serveAssets(() => ({ body: 'a,b\n1,2\n' })); });
afterEach(() => server.restore());

describe('the frame (viewer-frames.ts)', () => {
  test('posts wait for the boot, go one at a time, and a new boot drops them with what the old page reported', () => {
    const frame = newFrame();
    postToViewer(frame, { type: 'copy' });
    const config = { view: 'pdf' as const, key: 'k', name: 'a.pdf', src: 'http://x/a', text: null, sizeBytes: 1, kind: '', delimiter: null, wrap: true, revealLine: null, revealRequestId: 0 };
    expect(viewerFrameView(frame, 'f', config).posts).toEqual([]);
    expect(viewerMessage(frame, '{"type":"boot"}')).toEqual({ type: 'boot' });
    postToViewer(frame, { type: 'copy' });
    postToViewer(frame, { type: 'pick', serial: 1, id: 'x' });
    const view = viewerFrameView(frame, 'f', config);
    expect([view.posts.length, JSON.parse(view.posts[0]!.text), JSON.parse(view.config)]).toEqual([1, { type: 'copy' }, { type: 'config', ...config }]);
    viewerMessage(frame, JSON.stringify({ type: 'posted', key: view.posts[0]!.key }));
    expect(JSON.parse(viewerFrameView(frame, 'f', config).posts[0]!.text)).toEqual({ type: 'pick', serial: 1, id: 'x' });
    viewerMessage(frame, '{"type":"entry","path":"a.txt","view":"text","text":"hi","truncated":false}');
    viewerMessage(frame, '{"type":"boot"}');
    expect([frame.posts, frame.entry]).toEqual([[], null]);
  });

  test("a Select's rows open at the trigger; a row of a superseded menu does nothing", () => {
    const frame = newFrame();
    viewerMessage(frame, '{"type":"boot"}');
    viewerMessage(frame, '{"type":"menu","serial":7,"x":10,"y":20,"width":160,"height":24,"label":"Sheet","rows":[{"id":"0","label":"Inventory","selected":true},{"id":"1","label":"Prices","selected":false}]}');
    const menu = frame.menu;
    expect([menu.x, menu.y, menu.width, menu.label, menu.rows.map(row => [row.label, row.selected])]).toEqual([10, 20, 160, 'Sheet', [['Inventory', true], ['Prices', false]]]);
    viewerPick(frame, `${menu.serial - 1}|1`);
    expect(frame.posts).toEqual([]);
    viewerPick(frame, `${menu.serial}|1`);
    expect([JSON.parse(frame.posts[0]!.text), frame.menu.rows]).toEqual([{ type: 'pick', serial: 7, id: '1' }, []]);
  });

  test('a frame id is one per preview key: keys that differ only in punctuation do not share a frame', () => {
    const ids = ['files:a.b.docx', 'files:a_b.docx', 'files:a-b.docx', 'files:a/b.docx'].map(viewerFrameId);
    expect(new Set(ids).size).toBe(4);
    expect(ids.every(id => /^[A-Za-z0-9_-]+$/.test(id))).toBe(true);
  });

  test('a draft\'s bytes go as base64 posts of a mebibyte, the last one done', () => {
    const frame = newFrame();
    postBytes(frame, 'k', new Uint8Array(1024 * 1024 + 3).fill(65));
    const posts = frame.posts.map(post => JSON.parse(post.text) as Obj);
    expect(posts.map(post => [post.key, post.done, String(post.data).length])).toEqual([['k', false, 1398104], ['k', true, 4]]);
    expect(Buffer.from(String(posts[1]!.data), 'base64').toString()).toBe('AAA');
  });
});

describe('a sent file in the viewer page (AttachmentFilePreview)', () => {
  test('each kind asks for its viewer: PDF.js, the Office and document viewers, the archive and database viewer', async () => {
    const files: AttachmentMeta[] = [
      { id: 'p-pdf', name: 'guide.pdf', mimeType: 'application/pdf', sizeBytes: 24_077 },
      { id: 'w', name: 'report.docx', mimeType: '', sizeBytes: 37_009 },
      { id: 'o', name: 'stock.ods', mimeType: '', sizeBytes: 1641 },
      { id: 'n', name: 'analysis.ipynb', mimeType: '', sizeBytes: 2410 },
      { id: 'z', name: 'bundle.zip', mimeType: 'application/zip', sizeBytes: 1329 },
      { id: 's', name: 'app.sqlite', mimeType: '', sizeBytes: 102_400 },
    ];
    const c = client(files);
    const asked = [];
    for (const file of files) {
      const view = await attachmentView(c, native, file, 1000);
      const { view: kind, kind: sub, src, sizeBytes } = config(view);
      asked.push([view.preview, kind, sub, src, sizeBytes, view.canSave, view.canCopy]);
    }
    expect(asked).toEqual([
      ['viewer', 'pdf', '', 'http://127.0.0.1:1/api/assets/p-pdf?sig=1', 24_077, true, false],
      ['viewer', 'office', 'docx', 'http://127.0.0.1:1/api/assets/w?sig=1', 37_009, true, false],
      ['viewer', 'office', 'ods', 'http://127.0.0.1:1/api/assets/o?sig=1', 1641, true, false],
      ['viewer', 'office', 'ipynb', 'http://127.0.0.1:1/api/assets/n?sig=1', 2410, true, false],
      ['viewer', 'binary', 'archive', 'http://127.0.0.1:1/api/assets/z?sig=1', 1329, true, false],
      ['viewer', 'binary', 'sqlite', 'http://127.0.0.1:1/api/assets/s?sig=1', 102_400, true, false],
    ]);
    // Nothing is read by the app: the page fetches each file itself.
    expect(server.calls).toEqual([]);
  });

  test('text past 1 MB streams in the large-text viewer with word wrap and Copy contents, which the page answers', async () => {
    const calls: Obj[] = [];
    const log: AttachmentMeta = { id: 'l', name: 'server.log', mimeType: 'text/plain', sizeBytes: 40 * 1024 * 1024 };
    const c = client([log], calls);
    const view = await attachmentView(c, native, log, 1000);
    expect([view.preview, view.showWrap, view.canCopy, view.copyLabel, view.truncatedNote]).toEqual(['viewer', true, true, 'Copy contents', '']);
    expect(config(view)).toMatchObject({ view: 'text', src: 'http://127.0.0.1:1/api/assets/l?sig=1', sizeBytes: 40 * 1024 * 1024, delimiter: null, wrap: true });
    expect(server.calls).toEqual([]);
    // Text the page sends unasked never reaches the clipboard.
    await attachmentLocal(c, native, 'viewer:{"type":"boot"}', log.id, 0);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', text: 'unasked' })}`, log.id, 0);
    expect(calls.filter(call => call.op === 'copyText')).toEqual([]);
    // Copy contents pressed before a Try again's page booted goes to that page once it does; its answer is copied once.
    await attachmentLocal(c, native, 'retry', log.id, 0);
    await attachmentLocal(c, native, 'copy', log.id, 0);
    await attachmentLocal(c, native, 'copy', log.id, 0);
    expect((await attachmentView(c, native, log, 1000)).viewer.posts).toEqual([]);
    await attachmentLocal(c, native, 'viewer:{"type":"boot"}', log.id, 0);
    const asked = await attachmentView(c, native, log, 1000);
    expect([asked.viewer.posts.length, JSON.parse(asked.viewer.posts[0]!.text)]).toEqual([1, { type: 'copy' }]);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', text: 'whole file' })}`, log.id, 0);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', text: 'again' })}`, log.id, 0);
    expect(calls.filter(call => call.op === 'copyText')).toEqual([{ op: 'copyText', text: 'whole file' }]);
    expect((await attachmentView(c, native, log, 1500)).copyLabel).toBe('Copied');
  });

  test('a CSV past 1 MB streams as a table, and its toggle shows the source; Try again mounts the viewer afresh', async () => {
    const csv: AttachmentMeta = { id: 'c', name: 'orders.csv', mimeType: 'text/csv', sizeBytes: 2 * 1024 * 1024 };
    const c = client([csv]);
    const table = await attachmentView(c, native, csv, 1000);
    expect([table.preview, table.renderLabel, table.showWrap, config(table).delimiter]).toEqual(['viewer', 'Show source', false, ',']);
    await attachmentLocal(c, native, 'render', csv.id, 0);
    const source = await attachmentView(c, native, csv, 1000);
    expect([source.renderLabel, source.showWrap, config(source).delimiter, config(source).key === config(table).key]).toEqual(['Show table', true, null, false]);
    await attachmentLocal(c, native, 'viewer:{"type":"retry"}', csv.id, 0);
    expect(config(await attachmentView(c, native, csv, 1000)).key).not.toBe(config(source).key);
  });

  test("a draft's staged file reaches the page by message; one past 16 MB says why it waits for the send", async () => {
    const name = '11111111-2222-3333-4444-555555555555';
    const files = memoryAppFiles(new Map([[`app:/data/composer-files/${name}`, new TextEncoder().encode('%PDF-1.4 draft')]]));
    const meta: AttachmentMeta = { id: 'd1', name: 'draft.pdf', mimeType: 'application/pdf', sizeBytes: 14 };
    const c = client([]);
    const source: PreviewSource = { id: 'd1', key: 'draft:d1', origin: 'Draft', dialog: true, local: { url: `t3-staged:${name}`, text: async () => ({ ok: false }), save: () => {} } };
    const before = await attachmentView(c, native, meta, 1000, source);
    expect([before.preview, config(before).src, before.viewer.posts]).toEqual(['viewer', '', []]);
    await attachmentOp(c, native, meta, source, 'viewer:{"type":"boot"}', 0);
    const booted = await attachmentView(c, native, meta, 1000, source);
    const post = JSON.parse(booted.viewer.posts[0]!.text) as Obj;
    expect([post.type, post.key, post.done, Buffer.from(String(post.data), 'base64').toString()]).toEqual(['bytes', config(booted).key, true, '%PDF-1.4 draft']);
    forgetPreview(c, 'draft:d1');
    // A staged copy past readFile's 16 MiB cannot be posted until a bundled page can read an app file itself.
    const big = { ...source, key: 'draft:d2' };
    (files as unknown as Map<string, Uint8Array>).clear();
    const fs = (await import('../../src/media/save-file')).appFiles().fs as unknown as { stat?: (path: string) => Promise<{ size: number }> };
    fs.stat = async () => ({ size: 17 * 1024 * 1024 });
    await attachmentOp(c, native, meta, big, 'viewer:{"type":"boot"}', 0);
    await attachmentView(c, native, meta, 1000, big);
    const failed = await attachmentView(c, native, meta, 1000, big);
    expect([failed.preview, failed.error]).toEqual(['error', 'Preview of a draft file is limited to 16 MB until it is sent.']);
  });
});

describe('what the page hands back (NotebookPreview, ArchiveFile)', () => {
  const notebook = { type: 'notebook', language: 'python', sizeBytes: 2410, cells: [
    { kind: 'markdown', source: '# Sales\nSee [docs](https://example.com) and ![chart](x.png) with `code` <b>bold</b>.', outputs: [] },
    { kind: 'code', source: 'print(1)\nprint(2)', outputs: [{ kind: 'text', text: '1\n2\n' }, { kind: 'html', text: '<table><tr><td>1</td></tr></table>' }, { kind: 'image', text: 'data:image/png;base64,iVBORw0KGgo=' }] },
    { kind: 'raw', source: 'raw text', outputs: [] },
  ] };
  test('a notebook the page parsed is drawn here: its status, cells, outputs, and Show more past twenty cells', async () => {
    const meta: AttachmentMeta = { id: 'nb', name: 'analysis.ipynb', mimeType: '', sizeBytes: 2410 };
    const c = client([meta]);
    await attachmentView(c, native, meta, 1000);
    await attachmentLocal(c, native, `viewer:${JSON.stringify(notebook)}`, meta.id, 0);
    const view = await attachmentView(c, native, meta, 1000);
    expect([view.preview, view.notebook.status, view.notebook.more, view.notebook.empty]).toEqual(['notebook', '3 cells · python · 3 KB', '', false]);
    const [markdown, code, raw] = view.notebook.cells;
    // A Markdown cell goes to the renderer whole, as a `notebook` document, which it makes inert (data/src/markdown.rs
    // notebook_inert_tests): parsing it here as text could disagree with the renderer about where an image starts.
    expect([markdown!.label, markdown!.document !== '', view.markdownSource.map(source => [source.kind, source.body])])
      .toEqual(['markdown · 1', true, [['notebook', notebook.cells[0]!.source]]]);
    expect([code!.label, code!.lines.map(line => line.number), code!.sourceHeight, code!.outputs.map(output => [output.kind, output.text, output.url])])
      .toEqual(['python · 2', ['1', '2'], 56, [['text', '1\n2\n', ''], ['html', '', ''], ['image', '', 'data:image/png;base64,iVBORw0KGgo=']]]);
    expect([raw!.label, raw!.lines.length]).toEqual(['raw · 3', 1]);
    // An HTML output is a frame of the page, as tall as it reports.
    const output = code!.outputs[1]!;
    expect([JSON.parse(output.frame.config).view, JSON.parse(output.frame.config).text, output.height]).toEqual(['output', '<table><tr><td>1</td></tr></table>', 80]);
    await attachmentLocal(c, native, `viewer-output:${output.id}|{"type":"height","value":42}`, meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).notebook.cells[1]!.outputs[1]!.height).toBe(42);
    const many = { ...notebook, cells: Array.from({ length: 45 }, (_, index) => ({ kind: 'code', source: `x = ${index}`, outputs: [] })) };
    await attachmentLocal(c, native, `viewer:${JSON.stringify(many)}`, meta.id, 0);
    const first = await attachmentView(c, native, meta, 1000);
    expect([first.notebook.cells.length, first.notebook.more]).toEqual([20, 'Show more cells (25 remaining)']);
    await attachmentLocal(c, native, 'viewer-more', meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).notebook.more).toBe('Show more cells (5 remaining)');
    // An image output that is not a picture's data URL shows nothing: it would load with no press.
    const remote = { ...notebook, cells: [{ kind: 'code', source: 'x', outputs: [{ kind: 'image', text: 'https://attacker.example/p.png' }, { kind: 'image', text: 'data:text/html;base64,PGI+' }] }] };
    await attachmentLocal(c, native, `viewer:${JSON.stringify(remote)}`, meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).notebook.cells[0]!.outputs.map(output => output.url)).toEqual(['', '']);
  });
  test("an archive's opened entry: its source, its picture written for the image view, or why not; Back closes it", async () => {
    const files = memoryAppFiles();
    const meta: AttachmentMeta = { id: 'zip', name: 'bundle.zip', mimeType: 'application/zip', sizeBytes: 1329 };
    const c = client([meta]);
    await attachmentView(c, native, meta, 1000);
    await attachmentLocal(c, native, 'viewer:{"type":"boot"}', meta.id, 0);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'entry', path: 'src/main.ts', view: 'text', text: 'export const a = 1;\n', truncated: true })}`, meta.id, 0);
    const text = await attachmentView(c, native, meta, 1000);
    expect([text.preview, text.entry.open, text.entry.preview, text.entry.lines.length, text.entry.truncatedNote]).toEqual(['viewer', true, 'code', 2, 'Preview limited to the first 1 MB.']);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'entry', path: 'assets/logo.png', view: 'image', data: Buffer.from('png').toString('base64') })}`, meta.id, 0);
    const image = await attachmentView(c, native, meta, 1000);
    expect([image.entry.preview, image.entry.url.startsWith('app:/cache/media-preview/entry-'), new TextDecoder().decode(files.get(image.entry.url))]).toEqual(['image', true, 'png']);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'entry', path: 'bin/tool.bin', view: 'failure', message: 'Only UTF-8 text and images can be opened inside an archive.' })}`, meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).entry).toMatchObject({ preview: 'error', message: 'Only UTF-8 text and images can be opened inside an archive.' });
    await attachmentLocal(c, native, 'viewer:{"type":"entry-closed"}', meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).entry.open).toBe(false);
  });
});
