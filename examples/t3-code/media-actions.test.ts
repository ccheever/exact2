// media-actions: MediaActions.tsx / OpenMediaLink.tsx / mediaContent.ts behaviour (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3) as media-actions.ts and media-views.ts port it.
import { describe, expect, test } from 'bun:test';
import { decodeMediaSource, encodeMediaSource, mediaFailureTitle, mediaFileName, mediaMenuItems, mediaTooltip, openMediaLink, resolveProtocolRelativeMediaUrl, showMediaMenu, type MediaActionSource } from './media-actions';
import { filesMediaView, markdownImageHref, markdownImages, markdownMedia, markdownMediaChips, markdownMediaUrls, mediaLocal, videoFailed } from './media-views';
import { toasts } from './toast';
import type { T3Client } from './client';
import type { Native } from './protocol';

const native: Native = { available: true, watch() {}, later: async () => ({}) };
const fileImage: MediaActionSource = { kind: 'image', name: 'logo.png', src: 'http://127.0.0.1:3/api/assets/x/logo.png', reference: { kind: 'file', path: '/repo/screens/logo.png', relativePath: 'screens/logo.png' },
  asset: { resource: { _tag: 'workspace-file', threadId: 't1', path: '/repo/screens/logo.png' } }, openFile: 'screens/logo.png' };
const urlVideo: MediaActionSource = { kind: 'video', name: 'clip', src: 'https://cdn.example/clip%20one.mp4', reference: { kind: 'url', url: 'https://cdn.example/clip%20one.mp4' } };

type Call = Record<string, unknown>;
function fakeClient(answers: (request: Call) => unknown, extra: Partial<Record<string, unknown>> = {}) {
  const calls: Call[] = [], rpcs: Call[] = [];
  const client = {
    connection: 'connected', origin: 'http://127.0.0.1:16260', threadId: 't1', environmentId: 'env', ready: true, generation: 1,
    projection: { visibleTurnItems: [], messages: [], thread: {} }, shell: { projects: [] },
    restAccess: () => ({ call: async (request: Call) => { calls.push(request); return answers(request); } }),
    rpc: async (_native: Native, method: string, payload: Call) => { rpcs.push({ method, ...payload }); return { relativeUrl: '/api/assets/fresh/logo.png?sig=2' }; },
    ...extra,
  } as unknown as T3Client;
  return { client, calls, rpcs };
}

describe('mediaMenuItems', () => {
  test('a workspace file: full and relative path, the file viewer, Save image, Copy image', () => {
    expect(mediaMenuItems(fileImage)).toEqual([
      { id: 'copy-full-path', label: 'Copy full path' }, { id: 'copy-relative-path', label: 'Copy relative path' }, { id: 'open-file', label: 'Open in file viewer' },
      { id: 'save', label: 'Save image', disabled: false }, { id: 'copy-image', label: 'Copy image', disabled: false },
    ]);
  });
  test('a file outside the workspace has no relative path; a URL video has Copy URL and Save video only', () => {
    expect(mediaMenuItems({ ...fileImage, reference: { kind: 'file', path: '/tmp/a.png' }, openFile: undefined }).map(item => item.id)).toEqual(['copy-full-path', 'save', 'copy-image']);
    expect(mediaMenuItems(urlVideo)).toEqual([{ id: 'copy-url', label: 'Copy URL' }, { id: 'save', label: 'Save video', disabled: false }]);
  });
  test('Save and Copy image are disabled with neither a source nor an asset; a clipboard-less host disables Copy image', () => {
    const none: MediaActionSource = { kind: 'image', name: 'image', src: null };
    expect(mediaMenuItems(none)).toEqual([{ id: 'save', label: 'Save image', disabled: true }, { id: 'copy-image', label: 'Copy image', disabled: true }]);
    expect(mediaMenuItems({ ...none, asset: { resource: { _tag: 'attachment', attachmentId: 'a' } } }).every(item => !item.disabled)).toBe(true);
    expect(mediaMenuItems(fileImage, false).at(-1)).toEqual({ id: 'copy-image', label: 'Copy image', disabled: true });
  });
  test('failure titles name the item in lower case', () => {
    const items = mediaMenuItems(fileImage);
    expect(mediaFailureTitle(items, 'save')).toBe('Could not save image');
    expect(mediaFailureTitle(items, 'copy-image')).toBe('Could not copy image');
    expect(mediaFailureTitle(items, 'missing')).toBe('Could not complete media action');
  });
});

describe('names, tooltips and the open link', () => {
  test('the file name is the reference\'s, else the display name, else the kind', () => {
    expect(mediaFileName(fileImage)).toBe('logo.png');
    expect(mediaFileName(urlVideo)).toBe('clip one.mp4');
    expect(mediaFileName({ kind: 'video', name: '', src: null })).toBe('video');
    expect(mediaFileName({ kind: 'image', name: 'Shot', src: null })).toBe('Shot');
  });
  test('the tooltip is the path, the URL or the name', () => {
    expect(mediaTooltip(fileImage)).toBe('/repo/screens/logo.png');
    expect(mediaTooltip(urlVideo)).toBe('https://cdn.example/clip%20one.mp4');
    expect(mediaTooltip({ kind: 'image', name: 'Pasted image', src: null })).toBe('Pasted image');
  });
  test('OpenMediaLink: Open original, Download video, Open in browser, or nothing', () => {
    expect(openMediaLink({ originalUrl: 'https://cdn.example/a.mp4', src: 'http://h/api/assets/a' })).toEqual({ url: 'https://cdn.example/a.mp4', label: 'Open original', icon: 'external-link' });
    expect(openMediaLink({ src: 'blob:https://app/1' })).toEqual({ url: 'blob:https://app/1', label: 'Download video', icon: 'download' });
    expect(openMediaLink({ originalUrl: 'ftp://x/a.mp4', src: 'http://h/api/assets/a' })).toEqual({ url: 'http://h/api/assets/a', label: 'Open in browser', icon: 'external-link' });
    expect(openMediaLink({ src: 'data:video/mp4;base64,AA' })).toBeNull();
    expect(openMediaLink({ src: null })).toBeNull();
    expect(resolveProtocolRelativeMediaUrl('//cdn.example/a.png')).toBe('https://cdn.example/a.png');
  });
  test('a source is its JSON or a sent attachment by id', () => {
    const { client } = fakeClient(() => ({}), { projection: { visibleTurnItems: [{ item: { attachments: [{ id: 'img1', type: 'image', name: 'shot.png' }, { id: 'v1', type: 'file', name: 'clip.mov', mimeType: 'video/quicktime' }] } }], messages: [] } });
    expect(decodeMediaSource(client, encodeMediaSource(fileImage))).toEqual(fileImage);
    expect(decodeMediaSource(client, 'attachment:img1', () => 'http://h/api/assets/i')).toEqual({ kind: 'image', name: 'shot.png', src: 'http://h/api/assets/i' });
    expect(decodeMediaSource(client, 'attachment:v1')).toEqual({ kind: 'video', name: 'clip.mov', src: null, asset: { resource: { _tag: 'attachment', attachmentId: 'v1', fileName: 'clip.mov', mimeType: 'video/quicktime' } } });
    expect(decodeMediaSource(client, 'attachment:missing')).toBeNull();
    expect(decodeMediaSource(client, '{not json')).toBeNull();
  });
});

describe('showMediaMenu', () => {
  test('Copy full / relative path writes the text and toasts "Path copied"; Copy URL toasts "URL copied"', async () => {
    for (const [pick, source, text, title] of [['copy-full-path', fileImage, '/repo/screens/logo.png', 'Path copied'], ['copy-relative-path', fileImage, 'screens/logo.png', 'Path copied'], ['copy-url', urlVideo, 'https://cdn.example/clip%20one.mp4', 'URL copied']] as const) {
      const { client, calls } = fakeClient(request => (request.op === 'mediaMenu' ? { id: pick } : { ok: true }));
      expect(await showMediaMenu(client, native, encodeMediaSource(source), 'pointer')).toBe(pick);
      expect(calls[0]).toMatchObject({ op: 'mediaMenu' });
      expect(calls[0]!.anchor).toBeUndefined();
      expect(calls[1]).toEqual({ op: 'mediaCopyText', text });
      expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title });
    }
  });
  test('the keyboard opens the menu at the bottom-left corner; a dismissed or disabled pick does nothing', async () => {
    const { client, calls } = fakeClient(() => ({ id: null }));
    expect(await showMediaMenu(client, native, encodeMediaSource(fileImage), 'key')).toBe('');
    expect(calls).toEqual([{ op: 'mediaMenu', items: mediaMenuItems(fileImage), anchor: 'bottom-left' }]);
    const disabled = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'save' } : { ok: true }));
    expect(await showMediaMenu(disabled.client, native, encodeMediaSource({ kind: 'image', name: 'x', src: null }), 'pointer')).toBe('');
    expect(disabled.calls).toHaveLength(1);
  });
  test('Save signs the asset again at action time, then "Preparing image download…" becomes "Download started"', async () => {
    const { client, calls, rpcs } = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'save' } : { ok: true, started: true }));
    expect(await showMediaMenu(client, native, encodeMediaSource(fileImage), 'pointer')).toBe('save');
    expect(rpcs).toEqual([{ method: 'assets.createUrl', resource: fileImage.asset!.resource }]);
    expect(calls[1]).toEqual({ op: 'mediaSave', url: 'http://127.0.0.1:16260/api/assets/fresh/logo.png?sig=2', name: 'logo.png' });
    expect(toasts(client).at(-1)).toMatchObject({ kind: 'success', title: 'Download started' });
  });
  test('Copy image uses the shown URL when there is no asset; a media-file asset takes the shown thread', async () => {
    const plain = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'copy-image' } : { ok: true }));
    await showMediaMenu(plain.client, native, encodeMediaSource({ kind: 'image', name: 'shot.png', src: '//cdn.example/shot.png' }), 'pointer');
    expect(plain.rpcs).toEqual([]);
    expect(plain.calls[1]).toEqual({ op: 'mediaCopyImage', url: 'https://cdn.example/shot.png' });
    expect(toasts(plain.client).at(-1)).toMatchObject({ kind: 'success', title: 'Image copied' });
    const md = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'copy-image' } : { ok: true }));
    await showMediaMenu(md.client, native, encodeMediaSource({ kind: 'image', name: 'a', src: null, asset: { resource: { _tag: 'media-file', path: '/tmp/a.png' } } }), 'pointer');
    expect(md.rpcs[0]).toEqual({ method: 'assets.createUrl', resource: { _tag: 'media-file', path: '/tmp/a.png', threadId: 't1' } });
  });
  test('failures keep the progress toast and say what failed, in the reference\'s words', async () => {
    const refused = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'copy-image' } : { ok: false, message: 'This image is too large or has no usable dimensions. Try saving it instead.' }));
    await showMediaMenu(refused.client, native, encodeMediaSource(fileImage), 'pointer');
    expect(toasts(refused.client)).toHaveLength(1);
    expect(toasts(refused.client)[0]).toMatchObject({ kind: 'error', title: 'Could not copy image', description: 'This image is too large or has no usable dimensions. Try saving it instead.' });
    const offline = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'save' } : { ok: true }), { connection: 'reconnecting' });
    await showMediaMenu(offline.client, native, encodeMediaSource(fileImage), 'pointer');
    expect(toasts(offline.client)[0]).toMatchObject({ kind: 'error', title: 'Could not save image', description: 'Reconnect to this environment and try again.' });
    const invalid = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'save' } : { ok: true }), { rpc: async () => ({ relativeUrl: '' }) });
    await showMediaMenu(invalid.client, native, encodeMediaSource(fileImage), 'pointer');
    expect(toasts(invalid.client)[0]).toMatchObject({ description: 'The environment returned an invalid media URL.' });
    const unavailable = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'save' } : { ok: true }));
    await showMediaMenu(unavailable.client, native, encodeMediaSource({ ...urlVideo, src: null, reference: undefined, asset: undefined }), 'pointer');
    expect(toasts(unavailable.client)).toHaveLength(0);
    const menu = fakeClient(() => { throw new Error(''); });
    await showMediaMenu(menu.client, native, encodeMediaSource(fileImage), 'pointer');
    expect(toasts(menu.client)[0]).toMatchObject({ kind: 'error', title: 'Could not open media menu', description: 'The media action failed.' });
  });
  test('one menu at a time', async () => {
    let release: (value: unknown) => void = () => {};
    const { client, calls } = fakeClient(request => (request.op === 'mediaMenu' ? new Promise(resolve => { release = resolve; }) : { ok: true }));
    const first = showMediaMenu(client, native, encodeMediaSource(fileImage), 'pointer');
    expect(await showMediaMenu(client, native, encodeMediaSource(fileImage), 'pointer')).toBe('');
    release({ id: null });
    await first;
    expect(calls.filter(call => call.op === 'mediaMenu')).toHaveLength(1);
  });
  test('Open in file viewer opens the relative path in Files', async () => {
    const opened: string[] = [];
    const { client } = fakeClient(request => (request.op === 'mediaMenu' ? { id: 'open-file' } : { ok: true }));
    await showMediaMenu(client, native, encodeMediaSource(fileImage), 'pointer', { openFile: async path => { opened.push(path); } });
    expect(opened).toEqual(['screens/logo.png']);
  });
});

describe('media views', () => {
  test('Files: an image signs a workspace-file asset, a video a media-file asset with its failure slot', async () => {
    const { client, rpcs } = fakeClient(() => ({}));
    const image = await filesMediaView(client, native, 'screens/logo.png', '/repo/screens/logo.png', '/repo', '', 1000);
    expect(rpcs.at(-1)).toEqual({ method: 'assets.createUrl', resource: { _tag: 'workspace-file', threadId: 't1', path: '/repo/screens/logo.png' } });
    expect(image).toMatchObject({ kind: 'image', state: 'ready', url: 'http://127.0.0.1:16260/api/assets/fresh/logo.png?sig=2', tip: '/repo/screens/logo.png', failText: 'Unable to load workspace image.', retryKey: '' });
    expect(JSON.parse(image.source)).toMatchObject({ reference: { kind: 'file', path: '/repo/screens/logo.png', relativePath: 'screens/logo.png' } });
    const video = await filesMediaView(client, native, 'media/clip.mp4', '/repo/media/clip.mp4', '/repo', '', 1000);
    expect(rpcs.at(-1)).toMatchObject({ resource: { _tag: 'media-file', path: '/repo/media/clip.mp4' } });
    expect(video).toMatchObject({ kind: 'video', state: 'ready', failText: 'Video unavailable · media/clip.mp4', openLabel: 'Open in browser' });
    await mediaLocal(client, native, 'video-error', video.url, 'decode');
    expect(videoFailed(client, video.url)).toBe(true);
    expect((await filesMediaView(client, native, 'media/clip.mp4', '/repo/media/clip.mp4', '/repo', '', 1000)).state).toBe('failed');
    const before = rpcs.length;
    await mediaLocal(client, native, 'retry', video.retryKey, video.url);
    expect(rpcs.length).toBe(before + 1);
    expect(videoFailed(client, video.url)).toBe(false);
    expect(await filesMediaView(client, native, 'notes.txt', '/repo/notes.txt', '/repo', '', 1000)).toMatchObject({ kind: '' });
  });
  test('Files: a refused signature is the failure state, menu kept', async () => {
    const { client } = fakeClient(() => ({}), { rpc: async () => { throw new Error('denied'); } });
    expect(await filesMediaView(client, native, 'gone.png', '/repo/gone.png', '/repo', '', 1)).toMatchObject({ state: 'failed', failText: 'Unable to load workspace image.' });
  });
  test('Markdown: image lines resolve as ChatMarkdown\'s img renderer does', () => {
    expect(markdownImages('intro\n![Logo](screens/logo.png "title")\n  ![](https://cdn.example/a.mp4)\ntext ![inline](x.png) here\n![ctx](t3-context://v1/image/a)')).toEqual([
      { alt: 'Logo', src: 'screens/logo.png', href: 't3-file:screens/logo.png' }, { alt: '', src: 'https://cdn.example/a.mp4', href: 'https://cdn.example/a.mp4' },
    ]);
    expect(markdownImageHref('data:image/png;base64,AA')).toBe('');
    expect(markdownMedia('Logo', 'screens/logo.png', 't3-file:screens/logo.png', '/repo')).toMatchObject({ access: 'environment', kind: 'image', key: 'media:/repo/screens/logo.png',
      source: { name: 'Logo', reference: { kind: 'file', path: '/repo/screens/logo.png', relativePath: 'screens/logo.png' }, openFile: 'screens/logo.png', asset: { resource: { _tag: 'media-file', path: '/repo/screens/logo.png' } } } });
    expect(markdownMedia('', '//cdn.example/a.mp4', '', '/repo')).toMatchObject({ access: 'direct', kind: 'video', key: 'https://cdn.example/a.mp4', source: { name: 'video', reference: { kind: 'url', url: '//cdn.example/a.mp4' } } });
    expect(markdownMedia('Home', '~/a.png', 't3-file:~/a.png', '/repo')).toMatchObject({ access: 'blocked', source: null });
    expect(markdownMediaChips('![Logo](screens/logo.png)\n![Logo](screens/logo.png)', '/repo')).toEqual([{ href: 't3-file:screens/logo.png', kind: 'media', label: 'Logo', size: 'image', tip: '/repo/screens/logo.png',
      detail: encodeMediaSource(markdownMedia('Logo', 'screens/logo.png', '', '/repo').source!), icon: 'environment', target: 'media:/repo/screens/logo.png' }]);
  });
  test('Markdown: host-path media are signed for the shown thread; a refusal is listed', async () => {
    const items = [{ item: { text: '![a](/tmp/a.png)\n![b](/tmp/b.mp4)\n![c](https://x/c.png)' } }];
    const ok = fakeClient(() => ({}), { projection: { visibleTurnItems: items, messages: [], thread: {} } });
    expect(await markdownMediaUrls(ok.client, native, '/repo', 5)).toEqual([{ id: 'media:/tmp/a.png', url: 'http://127.0.0.1:16260/api/assets/fresh/logo.png?sig=2' }, { id: 'media:/tmp/b.mp4', url: 'http://127.0.0.1:16260/api/assets/fresh/logo.png?sig=2' }]);
    expect(ok.rpcs.map(rpc => rpc.resource)).toEqual([{ _tag: 'media-file', path: '/tmp/a.png', threadId: 't1' }, { _tag: 'media-file', path: '/tmp/b.mp4', threadId: 't1' }]);
    const refused = fakeClient(() => ({}), { projection: { visibleTurnItems: items, messages: [], thread: {} }, rpc: async () => { throw new Error('no'); } });
    expect((await markdownMediaUrls(refused.client, native, '/repo', 5)).map(entry => entry.id)).toEqual(['media-failed:/tmp/a.png', 'media-failed:/tmp/b.mp4']);
  });
});
