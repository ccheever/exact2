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
import { linkedViews } from '../../src/panel/pr-attachment-surfaces';
import { imagePreviewView } from '../../src/timeline/timeline-attachments';
import { toasts } from '../../src/shell/toast';

/** Waits until `ready` holds (in-process state), failing after a second. */
async function until(ready: () => unknown, what: string): Promise<void> {
  const start = Date.now();
  while (!ready()) {
    if (Date.now() - start > 1000) throw new Error(`waited for ${what}`);
    await new Promise(resolve => setTimeout(resolve, 5));
  }
}

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
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', key: config(view).key, text: 'unasked' })}`, log.id, 0);
    expect(calls.filter(call => call.op === 'copyText')).toEqual([]);
    // The page's Try again keeps its frame, which takes the new config and mounts its viewer afresh: a Copy pressed
    // then goes to it at once, for the new key, and only its answer for that key is copied, once.
    await attachmentLocal(c, native, 'viewer:{"type":"retry"}', log.id, 0);
    const retried = await attachmentView(c, native, log, 1000);
    expect([retried.viewer.id, config(retried).key === config(view).key]).toEqual([view.viewer.id, false]);
    await attachmentLocal(c, native, 'copy', log.id, 0);
    await attachmentLocal(c, native, 'copy', log.id, 0);
    const asked = await attachmentView(c, native, log, 1000);
    expect(asked.viewer.posts.map(post => JSON.parse(post.text))).toEqual([{ type: 'copy', key: config(retried).key }]);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', key: config(view).key, text: 'the old file' })}`, log.id, 0);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', key: config(retried).key, text: 'whole file' })}`, log.id, 0);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'copy', key: config(retried).key, text: 'again' })}`, log.id, 0);
    expect(calls.filter(call => call.op === 'copyText')).toEqual([{ op: 'copyText', text: 'whole file' }]);
    expect((await attachmentView(c, native, log, 1500)).copyLabel).toBe('Copied');
    // A stream whose URL was refused keeps Copy contents, which says it has nothing to copy yet and asks nothing later.
    const refused = client([log]);
    (refused as unknown as { rpc: () => Promise<never> }).rpc = async () => { throw new Error('Reconnect to the environment and try again.'); };
    const failed = await attachmentView(refused, native, log, 1000);
    expect([failed.preview, failed.canCopy]).toEqual(['error', true]);
    await attachmentLocal(refused, native, 'copy', log.id, 0);
    expect(toasts(refused).map(toast => [toast.title, toast.description])).toEqual([['Could not copy file', 'Wait for the file to finish loading before copying.']]);
    (refused as unknown as { rpc: unknown }).rpc = c.rpc;
    await attachmentLocal(refused, native, 'retry', log.id, 0);
    await attachmentView(refused, native, log, 1000);
    await attachmentLocal(refused, native, 'viewer:{"type":"boot"}', log.id, 0);
    expect((await attachmentView(refused, native, log, 1000)).viewer.posts).toEqual([]);
    // The frame is one per file and origin the page may reach: a page narrows its connections once.
    const moved = client([log]);
    (moved as unknown as { origin: string }).origin = 'http://127.0.0.1:2';
    expect((await attachmentView(moved, native, log, 1000)).viewer.id === view.viewer.id).toBe(false);
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

  test("a draft's staged file reaches the page by message; one past 16 MB is a file in app:/cache the page is given", async () => {
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
    // A staged copy past readFile's 16 MiB is copied into app:/cache and named in the frame's src, where the host gives
    // the page a URL of it (LLP 1020.001 D7); no bytes are posted.
    const big = { ...source, key: 'draft:d2' };
    const fs = (await import('../../src/media/save-file')).appFiles().fs as unknown as { stat?: (path: string) => Promise<{ size: number }> };
    fs.stat = async () => ({ size: 17 * 1024 * 1024 });
    const shown = await attachmentView(c, native, meta, 1000, big);
    const file = decodeURIComponent(new URL(shown.viewer.src, 'exact-app://app/').searchParams.get('file') ?? '');
    expect([shown.preview, shown.viewer.src.startsWith('assets/file-preview/index.html?file=app%3A%2Fcache%2Fmedia-preview%2F'), file.endsWith('.pdf'), config(shown).given, config(shown).src])
      .toEqual(['viewer', true, true, true, '']);
    expect(new TextDecoder().decode(files.get(file))).toBe('%PDF-1.4 draft');
    await attachmentOp(c, native, meta, big, 'viewer:{"type":"boot"}', 0);
    expect((await attachmentView(c, native, meta, 1000, big)).viewer.posts).toEqual([]);
    delete fs.stat;
    forgetPreview(c, 'draft:d2');
  });

  test("Try again after a draft's copy could not be read reads it once, for the page that boots in the new frame", async () => {
    const name = '22222222-3333-4444-5555-666666666666', path = `app:/data/composer-files/${name}`;
    const files = memoryAppFiles(new Map());
    const fs = (await import('../../src/media/save-file')).appFiles().fs as unknown as { readFile: (path: string) => Promise<ArrayBuffer> };
    const read = fs.readFile.bind(fs);
    let reads = 0;
    fs.readFile = async (at: string) => { if (at === path) reads += 1; return read(at); };
    const meta: AttachmentMeta = { id: 'd3', name: 'draft.pdf', mimeType: 'application/pdf', sizeBytes: 14 };
    const c = client([]);
    const source: PreviewSource = { id: 'd3', key: 'draft:d3', origin: 'Draft', dialog: true, local: { url: `t3-staged:${name}`, text: async () => ({ ok: false }), save: () => {} } };
    await attachmentView(c, native, meta, 1000, source);
    await attachmentOp(c, native, meta, source, 'viewer:{"type":"boot"}', 0);
    // No staged copy: the next view's failure takes the frame's place.
    await attachmentView(c, native, meta, 1000, source);
    expect([(await attachmentView(c, native, meta, 1000, source)).preview, reads]).toEqual(['error', 1]);
    files.set(path, new TextEncoder().encode('%PDF-1.4 draft'));
    await attachmentOp(c, native, meta, source, 'retry', 0);
    const remounted = await attachmentView(c, native, meta, 1000, source);
    expect([remounted.preview, remounted.viewer.posts, reads]).toEqual(['viewer', [], 1]);
    await attachmentOp(c, native, meta, source, 'viewer:{"type":"boot"}', 0);
    const booted = await attachmentView(c, native, meta, 1000, source);
    expect([booted.viewer.posts.map(post => (JSON.parse(post.text) as Obj).type), reads]).toEqual([['bytes'], 2]);
    expect((await attachmentView(c, native, meta, 1000, source)).viewer.posts).toHaveLength(1);
    expect(reads).toBe(2);
    forgetPreview(c, 'draft:d3');
  });
});

describe("a draft's copy in app:/cache", () => {
  test('is made again on Try again when the cache let it go, and a copy that fails says why', async () => {
    const name = '33333333-4444-5555-6666-777777777777', staged = `app:/data/composer-files/${name}`;
    const files = memoryAppFiles(new Map([[staged, new TextEncoder().encode('%PDF-1.4 large draft')]]));
    const fs = (await import('../../src/media/save-file')).appFiles().fs as unknown as {
      stat?: (path: string) => Promise<{ size: number }>; copyFile: (from: string, to: string) => Promise<void>; readFile: (path: string) => Promise<ArrayBuffer>;
    };
    const copy = fs.copyFile, read = fs.readFile.bind(fs);
    let reads = 0;
    fs.stat = async () => ({ size: 17 * 1024 * 1024 });
    fs.readFile = async (path: string) => { if (path === staged) reads += 1; return read(path); };
    const meta: AttachmentMeta = { id: 'd4', name: 'large.pdf', mimeType: 'application/pdf', sizeBytes: 17 * 1024 * 1024 };
    const c = client([]);
    const source: PreviewSource = { id: 'd4', key: 'draft:d4', origin: 'Draft', dialog: true, local: { url: `t3-staged:${name}`, text: async () => ({ ok: false }), save: () => {} } };
    const given = async () => decodeURIComponent(new URL((await attachmentView(c, native, meta, 1000, source)).viewer.src, 'exact-app://app/').searchParams.get('file') ?? '');
    const first = await given();
    files.delete(first);
    await attachmentOp(c, native, meta, source, 'retry', 0);
    const second = await given();
    expect([second === first, files.has(first), new TextDecoder().decode(files.get(second))]).toEqual([false, false, '%PDF-1.4 large draft']);
    // A copy that cannot be made: its own message, and no read of the draft past what a read takes.
    fs.copyFile = async () => { throw new Error('No space left on device'); };
    await attachmentOp(c, native, meta, source, 'retry', 0);
    await attachmentView(c, native, meta, 1000, source);
    const failed = await attachmentView(c, native, meta, 1000, source);
    expect([failed.preview, failed.error, reads]).toEqual(['error', 'No space left on device', 0]);
    fs.copyFile = copy; fs.readFile = read; delete fs.stat;
    forgetPreview(c, 'draft:d4');
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
    // Try again, and the tab shown again after another surface, mount the preview afresh: the page parses the notebook
    // again, and twenty cells show.
    await attachmentLocal(c, native, 'retry', meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).preview).toBe('viewer');
    await attachmentLocal(c, native, `viewer:${JSON.stringify(many)}`, meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).notebook.more).toBe('Show more cells (25 remaining)');
    const tab = { id: `attachment:${meta.id}`, kind: 'attachment' as const, path: meta.name, line: 0, reveal: 0, attachment: meta };
    await linkedViews(c, native, tab, true, 1000);
    await attachmentLocal(c, native, `viewer:${JSON.stringify(many)}`, meta.id, 0);
    await attachmentLocal(c, native, 'viewer-more', meta.id, 0);
    expect((await linkedViews(c, native, tab, true, 1000)).attachment.notebook.more).toBe('Show more cells (5 remaining)');
    await linkedViews(c, native, null, true, 1000);
    expect((await linkedViews(c, native, tab, true, 1000)).attachment.preview).toBe('viewer');
    await attachmentLocal(c, native, `viewer:${JSON.stringify(many)}`, meta.id, 0);
    expect((await linkedViews(c, native, tab, true, 1000)).attachment.notebook.more).toBe('Show more cells (25 remaining)');
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
    // The picture's press opens the expanded image dialog on that file; its Save and Copy image have no URL to fetch.
    expect(imagePreviewView(c).imagePreviewId).toBe('');
    await attachmentLocal(c, native, 'entry-image-open', meta.id, 0);
    const dialog = imagePreviewView(c);
    expect([dialog.imagePreviewId, dialog.imagePreviewName, dialog.imagePreviewReturn, JSON.parse(dialog.imagePreviewSource).src])
      .toEqual([image.entry.url, 'assets/logo.png', 'attachment-entry-image-zoom', null]);
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'entry', path: 'bin/tool.bin', view: 'failure', message: 'Only UTF-8 text and images can be opened inside an archive.' })}`, meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).entry).toMatchObject({ preview: 'error', message: 'Only UTF-8 text and images can be opened inside an archive.' });
    await attachmentLocal(c, native, 'viewer:{"type":"entry-closed"}', meta.id, 0);
    expect((await attachmentView(c, native, meta, 1000)).entry.open).toBe(false);
    // The preview gone, its picture's file leaves the media cache.
    forgetPreview(c, meta.id);
    await until(() => !files.has(image.entry.url), "the entry picture's file removed");
  });

  test("a notebook's picture past 1 MiB is a file in the media cache, removed with the preview", async () => {
    const files = memoryAppFiles();
    const meta: AttachmentMeta = { id: 'nb2', name: 'plots.ipynb', mimeType: '', sizeBytes: 3_000_000 };
    const c = client([meta]);
    await attachmentView(c, native, meta, 1000);
    const big = `data:image/png;base64,${'A'.repeat(1_400_000)}`;
    await attachmentLocal(c, native, `viewer:${JSON.stringify({ type: 'notebook', language: 'python', sizeBytes: 3_000_000, cells: [{ kind: 'code', source: 'plot()', outputs: [{ kind: 'image', text: big }] }] })}`, meta.id, 0);
    const url = (await attachmentView(c, native, meta, 1000)).notebook.cells[0]!.outputs[0]!.url;
    expect([url.startsWith('app:/cache/media-preview/'), files.has(url)]).toEqual([true, true]);
    forgetPreview(c, meta.id);
    await until(() => !files.has(url), "the notebook picture's file removed");
  });
});
