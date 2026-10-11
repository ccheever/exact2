import { describe, expect, test } from 'bun:test';
import { T3Client } from './shared/client';
import { obj, type Obj } from './shared/domain';
import { type Native } from './shared/protocol';
import { mobileMediaPrepare, mobileMediaShare, mobileMediaForget, mobileMediaURL } from './media-preview';
import { mobileHtmlRenderCanOpen, mobileHtmlRenderContext, mobileHtmlRenderDocumentSource, mobileHtmlRenderEvent, mobileHtmlRenderObserve, mobileHtmlRenderSnapshot, mobileHtmlRenderSourceKey } from './html-render-owner';
import { mobileHtmlRenderDocument, mobileHtmlRenderDocumentAction } from './html-render-document';
const imageId = '11111111-1111-4111-a111-111111111111';
function fixture() {
  const client = new T3Client(); client.environmentId = 'env'; client.threadId = 'thread'; client.projectId = 'project';
  client.origin = 'https://environment.example'; client.connection = 'connected';
  const item: Obj = { type: 'user_message', attachments: [{ id: 'pdf', type: 'file', name: 'report.pdf', mimeType: 'application/pdf' },
    { id: 'video', type: 'file', name: 'film.mov', mimeType: 'application/octet-stream' }] };
  client.thread = { sequence: 1, historyCursor: null, hasMore: false, latestLocalTurnOrdinal: 1,
    projection: { visibleTurnItems: [{ sourceThreadId: 'thread', sourceItemId: 'message', item }] } };
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: client.generation, value: request.method === 'assets.createUrl' ? { relativeUrl: '/api/assets/signed?cap=actual-server-result' } : {} };
  } };
  return { client, native, calls };
}
const prepare = (f: ReturnType<typeof fixture>, id: string, route = 'route', scope = 'transcript') => mobileMediaPrepare(scope, id, route, f.native, f.client);

describe('mobile media preview ownership', () => {
  test('fresh server authorization on open, first URL kept across resource refresh, explicit retry re-mints', async () => {
    const f = fixture(); const first = await prepare(f, 'pdf');
    expect(first).toMatchObject({ name: 'report.pdf', kind: 'file', ready: true, error: '' });
    expect(JSON.parse(first.sourceJSON)).toMatchObject({ source: 'remote', url: 'https://environment.example/api/assets/signed?cap=actual-server-result' });
    expect(f.calls[0]?.payload).toEqual({ resource: { _tag: 'attachment', attachmentId: 'pdf', fileName: 'report.pdf', mimeType: 'application/pdf', disposition: 'inline' } });
    expect(await prepare(f, 'pdf')).toEqual(first); expect(f.calls).toHaveLength(1);
    mobileMediaForget(first.identifier, f.client); await prepare(f, 'pdf'); expect(f.calls).toHaveLength(2);
    await prepare(f, 'pdf', 'another-route'); expect(f.calls).toHaveLength(3);
  });
  test('composer originals use owned ids and no path/base64/network substitute', async () => {
    const f = fixture(); f.client.connection = 'disconnected';
    f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: imageId, name: 'animation.gif', mimeType: 'image/gif', sizeBytes: 120 }];
    const value = await prepare(f, imageId, 'draft-preview', 'composer');
    expect(value.ready).toBe(true);
    expect(JSON.parse(value.sourceJSON)).toMatchObject({ id: imageId, source: 'draft-image', kind: 'image', url: '' });
    expect(value.sourceJSON).not.toContain('file:'); expect(f.calls).toHaveLength(0);
    f.client.projectId = 'another'; f.client.threadId = '';
    expect((await prepare(f, imageId, 'draft-preview', 'composer')).error).toContain('no longer available');
  });
  test('removed, disconnected and route-owner mismatches do not mint URLs', async () => {
    const f = fixture(); expect((await prepare(f, 'missing')).ready).toBe(false);
    f.client.connection = 'disconnected'; expect((await prepare(f, 'pdf')).error).toContain('Reconnect');
    expect((await mobileMediaPrepare('transcript', 'pdf', 'route', f.native, f.client, 'another', 'thread')).error).toContain('changed');
    expect(f.calls).toHaveLength(0);
  });
  test('late authorization cannot open media after selected thread changes', async () => {
    const f = fixture(), original = f.native.later;
    f.native.later = async input => { const result = await original(input); f.client.threadId = 'other'; return result; };
    expect((await prepare(f, 'pdf')).error).toContain('changed');
    expect(f.calls.some(call => call.op === 'mobileMediaShare')).toBe(false);
  });
  test('video type fallback follows shared source helper; share uses prepared original descriptor', async () => {
    const f = fixture(); expect((await prepare(f, 'video')).kind).toBe('video');
    expect(await mobileMediaShare('transcript', 'pdf', 'share-route', f.native, f.client)).toMatchObject({ message: '' });
    const share = f.calls.find(call => call.op === 'mobileMediaShare');
    expect(JSON.parse(String(share?.sourceJSON))).toMatchObject({ name: 'report.pdf', id: 'pdf', source: 'remote' });
  });
  test('local share rechecks the active draft before opening a native sheet', async () => {
    const f = fixture(); f.client.local.snapshotDrafts[f.client.draftKey] = [{ id: imageId, name: 'photo.jpg', mimeType: 'image/jpeg' }];
    const result = mobileMediaShare('composer', imageId, 'route', f.native, f.client);
    f.client.threadId = 'other'; expect((await result).message).toContain('changed'); expect(f.calls).toHaveLength(0);
  });
  test('abandoned native answers propagate without a misleading error snapshot', async () => {
    const f = fixture(); f.native.later = async () => { throw { name: 'FetchError', kind: 'Aborted' }; };
    await expect(prepare(f, 'pdf')).rejects.toMatchObject({ kind: 'superseded' });
  });
  test('URLs refuse embedded credentials and accept actual separately hosted signed assets', () => {
    expect(() => mobileMediaURL('https://environment.example', 'https://user:secret@example.test/media')).toThrow('invalid');
    expect(mobileMediaURL('https://environment.example', 'https://assets.example/file?cap=value')).toBe('https://assets.example/file?cap=value');
    expect(() => mobileMediaURL('https://environment.example', '')).toThrow('invalid');
  });
});

describe('HTML render native demand ownership', () => {
  const theme = { appearance: 'dark' as const, variables: { '--background': '#151515' } };
  function htmlFixture() {
    const f = fixture();
    f.client.configLive = f.client.shellLive = f.client.threadLive = true;
    f.client.scopes = [];
    const row = { visibility: 'inherited', sourceThreadId: 'parent', sourceItemId: 'render', item: {
      type: 'dynamic_tool', status: 'completed', toolName: 't3-code.html_render', outputOmitted: true,
      output: { htmlRender: { attachmentId: 'parent-page', title: 'Revenue / FY26', height: 420 } },
    } };
    f.client.projection.visibleTurnItems = [row];
    const key = mobileHtmlRenderSourceKey(row), context = mobileHtmlRenderObserve(f.client, 'thread-visit', true);
    const snapshot = () => mobileHtmlRenderSnapshot(f.client, context, key, true, 390, theme, '#999999');
    const descriptor = () => obj(JSON.parse(snapshot().sourceJSON || '{}'));
    const event = (kind: string, fields: Obj = {}, mountId = 'mount-a') => {
      const view = snapshot(), value = JSON.stringify({ identifier: view.identifier, attempt: descriptor().attempt, mountId, kind, ...fields });
      return mobileHtmlRenderEvent(f.client, context, key, true, view.identifier, value, f.native);
    };
    const snapshotAt = (activeContext: string) => mobileHtmlRenderSnapshot(f.client, activeContext, key, true, 390, theme, '#999999');
    const eventAt = (view: ReturnType<typeof mobileHtmlRenderSnapshot>, kind: string, mountId: string, fields: Obj = {}) =>
      mobileHtmlRenderEvent(f.client, view.context, key, true, view.identifier,
        JSON.stringify({ identifier: view.identifier, attempt: obj(JSON.parse(view.sourceJSON)).attempt, mountId, kind, ...fields }), f.native);
    const cover = () => mobileHtmlRenderContext(f.client, 'settings', 'settings', '', '', '', '', 0);
    const back = () => mobileHtmlRenderContext(f.client, 'thread', 'thread-visit', f.client.environmentId, f.client.threadId, '', '', 0);
    return { ...f, row, key, context, snapshot, descriptor, event, snapshotAt, eventAt, cover, back };
  }
  test('metadata projection signs nothing; a read-only mounted inherited render requests its own asset', async () => {
    const f = htmlFixture(), before = structuredClone(f.client.projection);
    for (let i = 0; i < 100; i++) expect(f.snapshot().available).toBe(true);
    expect(f.descriptor()).toMatchObject({ phase: 'pending', url: '', attempt: 0, mode: 'html-render' });
    expect(f.calls).toHaveLength(0);
    await f.event('mount', { width: 390 });
    expect(f.calls).toHaveLength(1);
    expect(f.calls[0]?.payload).toEqual({ resource: { _tag: 'attachment', attachmentId: 'parent-page',
      fileName: 'Revenue FY26.html', mimeType: 'text/html', disposition: 'inline' } });
    expect(f.descriptor()).toMatchObject({ phase: 'ready', url: 'https://environment.example/api/assets/signed?cap=actual-server-result' });
    expect(f.client.projection).toEqual(before);
    expect(await f.event('open')).toMatchObject({ context: f.context, openKey: f.key });
  });
  test('ordinary revisions, width and theme changes retain the first accepted URL', async () => {
    const f = htmlFixture(); await f.event('mount', { width: 390 });
    const first = f.descriptor().url;
    f.client.revision++; f.row.item.outputOmitted = true;
    mobileHtmlRenderObserve(f.client, 'thread-visit', true);
    await f.event('width', { width: 728 }); await f.event('mount', { width: 728 });
    const changed = mobileHtmlRenderSnapshot(f.client, f.context, f.key, true, 728,
      { appearance: 'light', variables: { '--background': '#fff' } }, '#999999');
    expect(JSON.parse(changed.sourceJSON)).toMatchObject({ url: first, attempt: 0, theme: { appearance: 'light' } });
    expect(f.calls).toHaveLength(1);
  });
  test('a load failure refreshes once, ignores old attempt events, then needs explicit retry', async () => {
    const f = htmlFixture(); await f.event('mount');
    const old = f.descriptor(); await f.event('error');
    expect(f.calls).toHaveLength(2); expect(f.descriptor().attempt).toBe(1);
    await f.event('error', { attempt: old.attempt }); expect(f.calls).toHaveLength(2);
    await f.event('error'); expect(f.descriptor().phase).toBe('failed');
    expect((await f.event('open')).openKey).toBe('');
    await f.event('retry'); expect(f.descriptor()).toMatchObject({ phase: 'ready', attempt: 2 });
    expect(f.calls).toHaveLength(2);
    await f.event('error'); expect(f.calls).toHaveLength(3);
  });
  test.each(['removed page', 'replacement title', 'generation', 'thread epoch', 'thread', 'environment', 'origin', 'route'])
  ('%s retires captured signing and Open ownership', async change => {
    const f = htmlFixture(), original = f.native.later, captured = f.snapshot();
    let release!: () => void;
    f.native.later = async value => { await new Promise<void>(resolve => { release = resolve; }); return original(value); };
    const pending = f.event('mount');
    switch (change) {
      case 'removed page': f.client.projection.visibleTurnItems = []; break;
      case 'replacement title': f.row.item.output.htmlRender.title = 'Replacement'; break;
      case 'generation': f.client.generation++; break;
      case 'thread epoch': f.client.threadEpoch++; break;
      case 'thread': f.client.threadId = 'other'; break;
      case 'environment': f.client.environmentId = 'other'; break;
      case 'origin': f.client.origin = 'https://other.example'; break;
      case 'route': mobileHtmlRenderObserve(f.client, 'settings-visit', false); break;
    }
    release();
    await expect(pending).rejects.toMatchObject({ kind: 'superseded' });
    const current = f.snapshot();
    if (change === 'replacement title') {
      expect(current.available).toBe(true);
      expect(current.identifier).not.toBe(captured.identifier);
      expect(JSON.parse(current.sourceJSON)).toMatchObject({ phase: 'pending', url: '' });
    } else expect(current.available).toBe(false);
    const staleOpen = await mobileHtmlRenderEvent(f.client, f.context, f.key, true, captured.identifier,
      JSON.stringify({ identifier: captured.identifier, attempt: 0, mountId: 'mount-a', kind: 'open' }), f.native);
    expect(staleOpen.openKey).toBe('');
    expect((await f.event('open')).openKey).toBe('');
  });
  test('a newer native instance owns its result even when the previous request finishes last', async () => {
    const f = htmlFixture(), releases: ((value: Obj) => void)[] = [];
    f.native.later = async input => {
      f.calls.push(obj(input));
      return new Promise<Obj>(resolve => { releases.push(resolve); });
    };
    const old = f.event('mount', {}, 'old-view'), newer = f.event('mount', {}, 'new-view');
    releases[1]!({ ok: true, generation: f.client.generation, value: { relativeUrl: '/new-page' } }); await newer;
    releases[0]!({ ok: true, generation: f.client.generation, value: { relativeUrl: '/old-page' } });
    await expect(old).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.descriptor().url).toBe('https://environment.example/new-page');
    await f.event('mount', {}, 'old-view');
    expect((await f.event('open', {}, 'old-view')).openKey).toBe('');
    expect((await f.event('open', {}, 'new-view')).openKey).toBe(f.key);
    expect(releases).toHaveLength(2);
  });
  test('remount width refreshes layout without resigning, and root Open rechecks the exact reference', async () => {
    const f = htmlFixture();
    Object.assign(f.row.item.output.htmlRender, { height: 1000, heights: [[390, 300], [728, 600]] });
    await f.event('mount', { width: 390 });
    expect(f.snapshot().height).toBe(300);
    const opened = await f.event('open');
    const remounted = await f.event('mount', { width: 728 }, 'mount-b');
    expect(remounted.revision).toBeGreaterThan(opened.revision);
    expect(f.snapshot().height).toBe(600); expect(f.calls).toHaveLength(1);
    expect(mobileHtmlRenderCanOpen(f.client, opened.context, opened.openKey, opened.identifier)).toBe(true);
    f.row.item.output.htmlRender.title = 'Replacement';
    expect(mobileHtmlRenderCanOpen(f.client, opened.context, opened.openKey, opened.identifier)).toBe(false);
  });
  test('full-page and covering routes retain accepted inline content under fresh Back admission', async () => {
    const f = htmlFixture(); await f.event('mount', { width: 390 });
    const old = f.snapshot(), firstURL = f.descriptor().url;
    const full = mobileHtmlRenderContext(f.client, 'threadMedia', 'full-page', 'env', 'thread', f.key, 'parent-page', 0);
    const document = await mobileHtmlRenderDocumentSource(f.client, full.context, f.key, 'parent-page', f.native);
    expect(document.ready).toBe(true); expect(document.identifier).not.toBe(old.identifier); expect(f.calls).toHaveLength(2);
    expect(f.snapshot().available).toBe(false);
    await f.eventAt(old, 'error', 'mount-a'); await f.eventAt(old, 'mount', 'mount-a');
    await f.eventAt(old, 'external', 'mount-a', { url: 'https://example.test' });
    expect(f.calls).toHaveLength(2);
    const returned = f.snapshotAt(f.back().context);
    expect(returned.context).not.toBe(old.context); expect(JSON.parse(returned.sourceJSON)).toMatchObject({ phase: 'ready', url: firstURL, attempt: 0 });
    f.client.connection = 'disconnected';
    await f.eventAt(returned, 'mount', 'after-back');
    await f.eventAt(old, 'mount', 'mount-a'); await f.eventAt(old, 'error', 'mount-a');
    await f.eventAt(old, 'external', 'mount-a', { url: 'https://example.test' });
    expect((await f.eventAt(returned, 'open', 'after-back')).openKey).toBe(f.key); expect(f.calls).toHaveLength(2);
    f.cover(); const again = f.snapshotAt(f.back().context); await f.eventAt(again, 'mount', 'second-back');
    expect(JSON.parse(again.sourceJSON).url).toBe(firstURL); expect(f.calls).toHaveLength(2);
    expect((await mobileHtmlRenderDocumentSource(f.client, full.context, f.key, 'parent-page', f.native)).ready).toBe(false);
  });
  test('a never-admitted old mount cannot claim a page after covering and Back', async () => {
    const f = htmlFixture(), old = f.snapshot(); f.cover();
    await f.eventAt(old, 'mount', 'delayed-first-mount'); expect(f.calls).toHaveLength(0);
    const returned = f.snapshotAt(f.back().context);
    expect(returned.identifier).not.toBe(old.identifier);
    await f.eventAt(old, 'mount', 'delayed-first-mount');
    await mobileHtmlRenderEvent(f.client, returned.context, f.key, true, old.identifier,
      JSON.stringify({ identifier: old.identifier, attempt: 0, mountId: 'delayed-first-mount', kind: 'mount' }), f.native);
    expect(f.calls).toHaveLength(0);
    await f.eventAt(returned, 'mount', 'current-mount'); expect(f.calls).toHaveLength(1);
  });
  test.each(['covered', 'after Back'])('initial signing completing %s cannot populate retained content', async timing => {
    const f = htmlFixture(), original = f.native.later;
    let entered!: () => void, release!: () => void;
    const started = new Promise<void>(resolve => { entered = resolve; }), paused = new Promise<void>(resolve => { release = resolve; });
    f.native.later = async request => { const result = await original(request); entered(); await paused; return result; };
    const old = f.snapshot(), signing = f.eventAt(old, 'mount', 'old-sign'); await started; f.cover();
    if (timing === 'after Back') f.back();
    release(); await expect(signing).rejects.toMatchObject({ kind: 'superseded' });
    const returned = f.snapshotAt(f.back().context);
    expect(JSON.parse(returned.sourceJSON)).toMatchObject({ phase: 'pending', url: '', attempt: 0 });
    f.native.later = original; await f.eventAt(old, 'mount', 'old-sign'); expect(f.calls).toHaveLength(1);
    await f.eventAt(returned, 'mount', 'new-sign'); expect(f.calls).toHaveLength(2);
  });
  test.each(['covered', 'after Back'])('automatic refresh completing %s cannot replace the retained first URL', async timing => {
    const f = htmlFixture(); await f.event('mount'); const old = f.snapshot(), firstURL = f.descriptor().url, original = f.native.later;
    let entered!: () => void, release!: () => void;
    const started = new Promise<void>(resolve => { entered = resolve; }), paused = new Promise<void>(resolve => { release = resolve; });
    f.native.later = async request => { const result = await original(request); entered(); await paused;
      return { ...obj(result), value: { relativeUrl: '/retired-refresh' } }; };
    const refreshing = f.eventAt(old, 'error', 'mount-a'); await started; f.cover();
    if (timing === 'after Back') f.back();
    release(); await expect(refreshing).rejects.toMatchObject({ kind: 'superseded' });
    const returned = f.snapshotAt(f.back().context);
    expect(JSON.parse(returned.sourceJSON)).toMatchObject({ phase: 'ready', url: firstURL, attempt: 0 });
    f.native.later = original; await f.eventAt(returned, 'mount', 'new-view');
    await f.eventAt(old, 'error', 'mount-a'); await f.eventAt(old, 'mount', 'mount-a');
    expect(f.calls).toHaveLength(2);
  });
  test.each(['title', 'source tuple', 'generation', 'thread epoch', 'thread', 'environment', 'origin'])
  ('%s replacement while covered invalidates the retained inline URL', async change => {
    const f = htmlFixture(); await f.event('mount'); f.cover();
    switch (change) {
      case 'title': f.row.item.output.htmlRender.title = 'Replacement'; break;
      case 'source tuple': f.row.sourceThreadId = 'different-parent'; break;
      case 'generation': f.client.generation++; break;
      case 'thread epoch': f.client.threadEpoch++; break;
      case 'thread': f.client.threadId = 'other-thread'; break;
      case 'environment': f.client.environmentId = 'other-environment'; break;
      case 'origin': f.client.origin = 'https://other.example'; break;
    }
    const returned = f.snapshotAt(f.back().context);
    expect(returned.sourceJSON === '' || obj(JSON.parse(returned.sourceJSON)).url === '').toBe(true);
    expect(f.calls).toHaveLength(1);
  });
  test('a retired native event cannot open a link and arbitrary schemes never reach native', async () => {
    const f = htmlFixture(); await f.event('mount');
    const count = f.calls.length;
    await f.event('external', { url: 'file:///secret' });
    await f.event('external', { url: 'https://user:pass@example.test' });
    await f.event('external', { url: 'https://example.test' }, 'retired-view');
    expect(f.calls).toHaveLength(count);
    f.native.later = async input => { f.calls.push(obj(input)); return { ok: true, generation: f.client.generation, value: { opened: true } }; };
    expect((await f.event('external', { url: 'https://example.test/page#part' })).message).toBe('');
    expect(f.calls.at(-1)).toMatchObject({ op: 'mobileOpenURL', url: 'https://example.test/page#part' });
  });
});

describe('owned HTML document routes', () => {
  const theme = { appearance: 'light' as const, variables: { '--background': '#ffffff' } };
  function documentFixture() {
    const f = fixture(); f.client.configLive = f.client.shellLive = f.client.threadLive = true;
    const row = { visibility: 'inherited', sourceThreadId: 'parent', sourceItemId: 'page', item: {
      type: 'dynamic_tool', status: 'completed', toolName: 't3-code.html_render',
      output: { htmlRender: { attachmentId: 'owned-html', title: 'Revenue', height: 420 } },
    } };
    f.client.projection.visibleTurnItems = [row];
    const key = mobileHtmlRenderSourceKey(row);
    const route = mobileHtmlRenderContext(f.client, 'threadMedia', 'preview', 'env', 'thread', key, 'owned-html', 0);
    f.native.later = async input => {
      const request = obj(input); f.calls.push(request);
      const value = request.method === 'assets.createUrl' ? { relativeUrl: `/signed?attempt=${f.calls.length}` }
        : request.op === 'mobileDocumentRead' ? { identifier: obj(JSON.parse(String(request.sourceJSON))).identifier, base64: btoa('<h1>Revenue</h1>') } : {};
      return { ok: true, generation: f.client.generation, value };
    };
    const read = (rendered = true, now = 1000, covered = false) => mobileHtmlRenderDocument(f.client,
      route.context, key, 'owned-html', rendered, theme, '#777777', now, f.native, covered);
    return { ...f, row, key, route, read };
  }
  test('Source can open before any native mount; its URL survives theme and mode changes', async () => {
    const f = documentFixture(), source = await f.read(false);
    expect(source).toMatchObject({ ready: true, activeMode: 'source', text: '<h1>Revenue</h1>', htmlRenderSource: '' });
    expect(f.calls.map(call => call.op)).toEqual(['request', 'mobileDocumentRead']);
    const preview = await f.read(), descriptor = obj(JSON.parse(preview.htmlRenderSource));
    expect(descriptor).toMatchObject({ mode: 'html-render', nested: false, url: source.uri });
    const dark = await mobileHtmlRenderDocument(f.client, f.route.context, f.key, 'owned-html', true,
      { appearance: 'dark', variables: { '--background': '#151515' } }, '#999999', 1000, f.native);
    expect(JSON.parse(dark.htmlRenderSource)).toMatchObject({ url: source.uri, theme: { appearance: 'dark' } });
    expect(f.calls).toHaveLength(2);
    expect((await f.read(true, 1000, true)).htmlRenderSource).toBe('');
  });
  test('HTML route identity must match source tuple and metadata; generic IDs never authorize it', async () => {
    const f = documentFixture();
    expect((await mobileHtmlRenderDocumentSource(f.client, f.route.context, f.key, 'other-html', f.native)).ready).toBe(false);
    expect((await prepare(f, 'owned-html')).ready).toBe(false);
    expect(mobileHtmlRenderContext(f.client, 'threadMedia', 'other', 'env', 'thread', f.key, 'other-html', 0).context).toBe('');
    expect(f.calls).toHaveLength(0);
    const route = mobileHtmlRenderContext(f.client, 'threadMedia', 'valid', 'env', 'thread', f.key, 'owned-html', 0);
    f.row.item.output.htmlRender.title = 'Replaced';
    expect((await mobileHtmlRenderDocumentSource(f.client, route.context, f.key, 'owned-html', f.native)).name).toBe('Replaced.html');
  });
  test('Source and covering native viewer retire old HTML callbacks while retaining its first URL', async () => {
    const f = documentFixture(), preview = await f.read(), descriptor = obj(JSON.parse(preview.htmlRenderSource));
    const event = (kind: string, mountId: string) => mobileHtmlRenderEvent(f.client, f.route.context, f.key, false, preview.identifier,
      JSON.stringify({ identifier: preview.identifier, attempt: descriptor.attempt, mountId, kind, url: 'https://example.test' }), f.native);
    await event('mount', 'old-view'); await f.read(false);
    const count = f.calls.length; await event('external', 'old-view'); await event('mount', 'old-view');
    expect(f.calls).toHaveLength(count);
    const again = await f.read(); expect(JSON.parse(again.htmlRenderSource).url).toBe(descriptor.url);
    await event('mount', 'new-view'); await f.read(true, 1000, true); await event('external', 'new-view');
    expect(f.calls).toHaveLength(count);
  });
  test('share signs a fresh validated source and cannot publish after an owner change', async () => {
    const f = documentFixture(), data = await f.read(), first = data.uri;
    const event = JSON.stringify({ identifier: data.identifier, operation: 'share' });
    expect((await mobileHtmlRenderDocumentAction(f.client, f.route.context, f.key, 'owned-html', event, data, f.native)).message).toBe('');
    const share = f.calls.find(call => call.op === 'mobileMediaShare');
    expect(JSON.parse(String(share?.sourceJSON))).toMatchObject({ id: 'owned-html', name: 'Revenue.html' });
    expect(JSON.parse(String(share?.sourceJSON)).url).not.toBe(first);
    expect((await f.read()).uri).toBe(first);
    const original = f.native.later;
    f.native.later = async input => { const result = await original(input); if (obj(input).method === 'assets.createUrl') f.client.threadEpoch++; return result; };
    const count = f.calls.filter(call => call.op === 'mobileMediaShare').length;
    await expect(mobileHtmlRenderDocumentAction(f.client, f.route.context, f.key, 'owned-html', event, data, f.native)).rejects.toMatchObject({ kind: 'superseded' });
    expect(f.calls.filter(call => call.op === 'mobileMediaShare')).toHaveLength(count);
  });
  test('a recovered full preview pairs its new render URL with fresh Source authorization', async () => {
    const f = documentFixture(), first = await f.read(), initial = obj(JSON.parse(first.htmlRenderSource));
    const event = (kind: string) => mobileHtmlRenderEvent(f.client, f.route.context, f.key, false, first.identifier,
      JSON.stringify({ identifier: first.identifier, attempt: initial.attempt, mountId: 'full-view', kind }), f.native);
    await event('mount'); await event('error');
    const recovered = await f.read(true, 301_000), display = obj(JSON.parse(recovered.htmlRenderSource));
    expect(display.url).not.toBe(first.uri); expect(recovered.uri).toBe(display.url); expect(display.attempt).toBe(1);
    await f.read(true, 302_000); // A clock-only refresh never re-signs the displayed page.
    const source = await f.read(false, 302_001);
    expect(source.text).toBe('<h1>Revenue</h1>');
    expect(f.calls.filter(call => call.method === 'assets.createUrl')).toHaveLength(2);
    const reads = f.calls.filter(call => call.op === 'mobileDocumentRead'); expect(reads).toHaveLength(1);
    expect(JSON.parse(String(reads[0]!.sourceJSON)).url).toBe(display.url);
  });
  test('a recovered render URL clears a cached Source failure from the old URL', async () => {
    const f = documentFixture(), first = await f.read(), original = f.native.later;
    f.native.later = async input => {
      if (obj(input).op !== 'mobileDocumentRead') return original(input);
      f.calls.push(obj(input));
      return { ok: false, generation: f.client.generation, error: { kind: 'Attachment', message: 'Expired first URL' } };
    };
    expect((await f.read(false)).error).toBe('Expired first URL');
    f.native.later = original;
    const rendered = await f.read(), initial = obj(JSON.parse(rendered.htmlRenderSource));
    const event = (kind: string) => mobileHtmlRenderEvent(f.client, f.route.context, f.key, false, first.identifier,
      JSON.stringify({ identifier: first.identifier, attempt: initial.attempt, mountId: 'recovery-view', kind }), f.native);
    await event('mount'); await event('error');
    const recovered = await f.read(); expect(recovered.uri).not.toBe(first.uri);
    const source = await f.read(false);
    expect(source).toMatchObject({ error: '', text: '<h1>Revenue</h1>' });
    const reads = f.calls.filter(call => call.op === 'mobileDocumentRead'); expect(reads).toHaveLength(2);
    expect(JSON.parse(String(reads[1]!.sourceJSON)).url).toBe(recovered.uri);
  });
  test('refreshing the render URL retains successfully decoded same-file Source', async () => {
    const f = documentFixture(), source = await f.read(false), rendered = await f.read(), initial = obj(JSON.parse(rendered.htmlRenderSource));
    const event = (kind: string) => mobileHtmlRenderEvent(f.client, f.route.context, f.key, false, rendered.identifier,
      JSON.stringify({ identifier: rendered.identifier, attempt: initial.attempt, mountId: 'decoded-view', kind }), f.native);
    await event('mount'); await event('error'); await f.read();
    expect((await f.read(false)).text).toBe(source.text);
    expect(f.calls.filter(call => call.op === 'mobileDocumentRead')).toHaveLength(1);
  });
  test.each(['metadata', 'route'])('%s replaced during a source read rejects the late native bytes', async change => {
    const f = documentFixture(), original = f.native.later;
    f.native.later = async input => {
      const result = await original(input);
      if (obj(input).op === 'mobileDocumentRead') {
        if (change === 'metadata') f.row.item.output.htmlRender.title = 'Replacement';
        else mobileHtmlRenderContext(f.client, 'settings', 'settings', '', '', '', '', 0);
      }
      return result;
    };
    await expect(f.read(false)).rejects.toMatchObject({ kind: 'superseded' });
  });
});
