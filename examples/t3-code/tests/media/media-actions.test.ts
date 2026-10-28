// MediaActions.tsx / OpenMediaLink.tsx / mediaContent.ts behaviour (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3) as media-actions.ts and media-views.ts port it.
import { describe, expect, test } from 'bun:test';
import { decodeMediaSource, encodeMediaSource, mediaFailureTitle, mediaFileName, mediaMenuItems, mediaTooltip, openMediaLink, resolveProtocolRelativeMediaUrl, showMediaMenu, type MediaActionSource } from '../../src/media/media-actions';
import { filesMediaView, markdownImageHref, markdownImages, markdownMedia, markdownMediaChips, markdownMediaUrls, mediaFailed, mediaLocal } from '../../src/media/media-views';
import { toasts } from '../../src/shell/toast';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { contractComponent } from '../support/contract-action-fixture';
import { contractFns } from '../support/contract-fn-fixture';
import { appText } from '../support/app-files';

const native: Native = { available: true, watch() {}, later: async () => ({}) };
const fileImage: MediaActionSource = { kind: 'image', name: 'logo.png', src: 'http://127.0.0.1:3/api/assets/x/logo.png', reference: { kind: 'file', path: '/repo/screens/logo.png', relativePath: 'screens/logo.png' },
  asset: { resource: { _tag: 'workspace-file', threadId: 't1', path: '/repo/screens/logo.png' } }, openFile: 'screens/logo.png' };
const urlVideo: MediaActionSource = { kind: 'video', name: 'clip', src: 'https://cdn.example/clip%20one.mp4', reference: { kind: 'url', url: 'https://cdn.example/clip%20one.mp4' } };

type Call = Record<string, unknown>;
function fakeClient(answers: (request: Call) => unknown, extra: Partial<Record<string, unknown>> = {}) {
  const calls: Call[] = [], rpcs: Call[] = [];
  const client = {
    scopes: ['filesystem:read'],
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
    expect(mediaFailed(client, video.url)).toBe(true);
    expect((await filesMediaView(client, native, 'media/clip.mp4', '/repo/media/clip.mp4', '/repo', '', 1000)).state).toBe('failed');
    const before = rpcs.length;
    await mediaLocal(client, native, 'retry', video.retryKey, video.url);
    expect(rpcs.length).toBe(before + 1);
    expect(mediaFailed(client, video.url)).toBe(false);
    expect(await filesMediaView(client, native, 'notes.txt', '/repo/notes.txt', '/repo', '', 1000)).toMatchObject({ kind: '' });
    // workspaceAssetResource (#24627): a picture outside the workspace (an absolute link in the thread) is a media-file.
    await filesMediaView(client, native, '/Users/me/Desktop/shot.png', '/Users/me/Desktop/shot.png', '/repo', '', 1000);
    expect(rpcs.at(-1)).toEqual({ method: 'assets.createUrl', resource: { _tag: 'media-file', threadId: 't1', path: '/Users/me/Desktop/shot.png' } });
    // WorkspaceImagePreview's onError: the picture's `error` (exact2 #121) is the failure state, menu kept.
    await mediaLocal(client, native, 'image-error', image.url, 'HTTP 404');
    expect(await filesMediaView(client, native, 'screens/logo.png', '/repo/screens/logo.png', '/repo', '', 1000)).toMatchObject({ state: 'failed', failText: 'Unable to load workspace image.' });
  });
  // WorkspaceImagePreview at 7c3b3b3605ef: an image is not signed again for a workspace revision; the revision on its URL
  // reloads it.
  test('Files: an image keeps its signature and reloads by the workspace revision', async () => {
    let signature = 0;
    const { client } = fakeClient(() => ({}), { rpc: async () => ({ relativeUrl: `/api/assets/logo.png?sig=${++signature}`, fileRevision: 'r1' }) });
    const view = (workspace: string) => filesMediaView(client, native, 'screens/logo.png', '/repo/screens/logo.png', '/repo', workspace, 1000);
    expect((await view('w1')).url).toBe('http://127.0.0.1:16260/api/assets/logo.png?sig=1&workspace-revision=w1');
    expect((await view('w2')).url).toBe('http://127.0.0.1:16260/api/assets/logo.png?sig=1&workspace-revision=w2');
  });
  // FilePreviewPanel WorkspaceVideoPreview at 7c3b3b3605ef: each workspace change signs the video again; its URL names the
  // file's revision when the server reports one, else the workspace's, and the view says which (the player holds by it).
  test('Files: a video is signed again for each command and names its file revision, else the workspace one', async () => {
    let revision = 'dev:ino:10:1', signature = 0;
    const { client, rpcs } = fakeClient(() => ({}), { rpc: async (_native: Native, method: string, payload: Call) => {
      rpcs.push({ method, ...payload }); signature++;
      return { relativeUrl: `/api/assets/fresh/clip.mp4?sig=${signature}`, ...(revision ? { fileRevision: revision } : {}) };
    } });
    const view = async (workspace: string, now = 1000) => { const media = await filesMediaView(client, native, 'media/clip.mp4', '/repo/media/clip.mp4', '/repo', workspace, now); return [media.url, media.revision, rpcs.length]; };
    const url = (sig: number, name: string, value: string) => `http://127.0.0.1:16260/api/assets/fresh/clip.mp4?sig=${sig}&${name}=${encodeURIComponent(value)}`;
    expect(await view('["item-1",null]')).toEqual([url(1, 'file-revision', 'dev:ino:10:1'), 'dev:ino:10:1', 1]);
    // A command that left the file alone: signed again, the same revision (the player keeps what it loaded).
    expect(await view('["item-2",null]')).toEqual([url(2, 'file-revision', 'dev:ino:10:1'), 'dev:ino:10:1', 2]);
    // Five minutes on, the re-sign keeps the revision too.
    expect(await view('["item-2",null]', 1000 + 6 * 60_000)).toEqual([url(3, 'file-revision', 'dev:ino:10:1'), 'dev:ino:10:1', 3]);
    // The file changed: the next command's signature carries the new revision.
    revision = 'dev:ino:12:2';
    expect(await view('["item-3",null]', 1000 + 6 * 60_000)).toEqual([url(4, 'file-revision', 'dev:ino:12:2'), 'dev:ino:12:2', 4]);
    // An older server: the workspace revision, which each command moves (the player reloads once paused).
    revision = '';
    expect(await view('["item-4",null]', 1000 + 6 * 60_000)).toEqual([url(5, 'workspace-revision', '["item-4",null]'), '["item-4",null]', 5]);
    // No workspace revision yet: none named, none held.
    const fresh = fakeClient(() => ({}));
    expect((await filesMediaView(fresh.client, native, 'media/clip.mp4', '/repo/media/clip.mp4', '/repo', '', 1000)).revision).toBe('');
  });
  // MediaVideoPlayer at 7c3b3b3605ef: its `failed` reads the source it plays, so a re-signing that fails while a video
  // plays leaves it playing; the failure slot shows once playback pauses.
  test('Files: a failed re-sign leaves a playing video playing until it pauses', () => {
    const video = contractComponent('media-actions.contract', 'FilesVideo'), state = video.state();
    let media = { url: 'u1', revision: 'r1', name: 'clip.mp4', state: 'ready' };
    const scope = () => ({ media, local: () => undefined });
    video.call('started', state, scope(), []);
    media = { ...media, url: '', revision: 'w2', state: 'failed' };
    expect([video.drawn('video src', state, scope()), video.drawn('MediaVideoFailure(', state, scope()), video.derives(state, scope()).src]).toEqual([true, false, 'u1']);
    video.call('stopped', state, scope(), []);
    expect([video.drawn('video src', state, scope()), video.drawn('MediaVideoFailure(', state, scope())]).toEqual([false, true]);
  });
  // MediaVideoPlayer at 7c3b3b3605ef: playbackSource held while the revision holds, a new revision taken once paused or
  // ended, an error on a kept source falling back to the newest URL.
  test('Files: the video player holds the source it loaded while its revision holds, and takes a new one once playback stops', () => {
    const video = contractComponent('media-actions.contract', 'FilesVideo'), state = video.state(), sent: unknown[][] = [];
    let media = { url: 'u1', revision: 'r1', name: 'clip.mp4' };
    const scope = () => ({ media, local: (...args: unknown[]) => sent.push(args) });
    const src = () => video.derives(state, scope()).src;
    expect(src()).toBe('u1');
    video.call('loaded', state, scope(), []);
    media = { ...media, url: 'u2' }; // signed again, the same file
    expect(src()).toBe('u1');
    video.call('started', state, scope(), []);
    media = { ...media, url: 'u3', revision: 'r2' }; // the file changed while it plays
    expect(src()).toBe('u1');
    video.call('stopped', state, scope(), []);
    expect(src()).toBe('u3');
    video.call('loaded', state, scope(), []);
    media = { ...media, url: 'u4' };
    expect(src()).toBe('u3');
    // The kept source fails (its signature expired, say): the newest URL, no failure.
    video.call('broke', state, scope(), ['network']);
    expect([src(), sent]).toEqual(['u4', []]);
    // The newest one fails: the player's failure.
    video.call('broke', state, scope(), ['network']);
    expect(sent).toEqual([['media-video-error', 'u4', 'network']]);
    // With no revision nothing is held: each new URL loads.
    const plain = contractComponent('media-actions.contract', 'FilesVideo'), bare = plain.state();
    plain.call('loaded', bare, { media: { url: 'v1', revision: '', name: 'clip.mp4' }, local: () => undefined }, []);
    expect(plain.derives(bare, { media: { url: 'v2', revision: '', name: 'clip.mp4' } }).src).toBe('v2');
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
    // The image line's chip, then the same image's as a media run (markdownInlineImages).
    expect(markdownMediaChips('![Logo](screens/logo.png)\n![Logo](screens/logo.png)', '/repo').filter(chip => !chip.href.startsWith('t3-image:'))).toEqual([{ href: 't3-file:screens/logo.png', kind: 'media', label: 'Logo', size: 'image', tip: '/repo/screens/logo.png',
      detail: encodeMediaSource(markdownMedia('Logo', 'screens/logo.png', '', '/repo').source!), icon: 'environment', target: 'media:/repo/screens/logo.png' }]);
  });
  // ChatMarkdown's img renderer at 7c3b3b3605ef (#21363): a sound is its audio chip and a host file of no media kind its
  // file chip; MarkdownMedia draws them by the chip's `size`.
  test('Markdown: a sound plays in its audio chip, named by its file, and its name opens the host file', () => {
    expect(markdownMedia('Take', 'clips/take.mp3', 't3-file:clips/take.mp3', '/repo')).toMatchObject({ kind: 'audio', access: 'environment', key: 'media:/repo/clips/take.mp3',
      name: 'take.mp3', openPath: '/repo/clips/take.mp3', resource: { _tag: 'media-file', path: '/repo/clips/take.mp3' }, source: null });
    expect(markdownMedia('Take', 'https://cdn.example/a/take.ogg?x=1', '', '/repo')).toMatchObject({ kind: 'audio', access: 'direct', key: 'https://cdn.example/a/take.ogg?x=1', name: 'take.ogg' });
    expect(markdownMedia('Take', '~/take.wav', '', '/repo')).toMatchObject({ kind: 'audio', access: 'blocked', name: 'take.wav' });
    // An attachment's Markdown has no thread to sign through: "Audio unavailable".
    expect(markdownMedia('', 'take.m4a', '', '/repo', { thread: false })).toMatchObject({ kind: 'audio', access: 'blocked', name: 'take.m4a' });
    expect(markdownMediaChips('![Take](clips/take.mp3)', '/repo').filter(chip => !chip.href.startsWith('t3-image:'))).toEqual([
      { href: 't3-file:clips/take.mp3', kind: 'media', label: 'Take', size: 'audio', tip: 'take.mp3', detail: '/repo/clips/take.mp3', icon: 'environment', target: 'media:/repo/clips/take.mp3' }]);
    // Inside a line of text a sound plays in its audio chip too (ChatMarkdown's img renderer), a web URL's as written.
    expect(markdownMediaChips('Hear ![Take](clips/take.mp3) and ![Web](https://cdn.example/a.ogg).', '/repo').filter(chip => chip.href.startsWith('t3-image:')).map(chip => [chip.size, chip.icon, chip.tip, chip.target])).toEqual([
      ['audio', 'environment', 'take.mp3', 'media:/repo/clips/take.mp3'], ['audio', 'direct', 'a.ogg', 'https://cdn.example/a.ogg']]);
  });
  test('Markdown: a file of no media kind written as an image is its file chip', () => {
    const [chip] = markdownMediaChips('![report](docs/report.pdf)', '/repo', { cwd: '/repo' }).filter(entry => !entry.href.startsWith('t3-image:'));
    expect(chip).toMatchObject({ href: 't3-file:docs/report.pdf', kind: 'media', label: 'report.pdf', size: 'file', tip: '/repo/docs/report.pdf', detail: '', target: 'docs/report.pdf' });
    expect(chip!.icon).not.toBe('');
    expect(markdownMedia('', '/repo/a.pdf:3', '', '/repo').file).toMatchObject({ label: 'a.pdf · L3', tip: '/repo/a.pdf:3', target: '' });
    // A picture still loads as one; a name that is no file (no extension) stays an image, as the reference's does.
    expect(markdownMedia('', 'shots/a.png', '', '/repo').kind).toBe('image');
    expect(markdownMedia('', 'render', '', '/repo').kind).toBe('image');
  });
  // markdownImages.ts at 7c3b3b3605ef: a network path outside the project is blocked, so nothing asks the server to sign
  // it (its realpath would make Windows send the user's credentials to that host).
  test('Markdown: a network-path image is unavailable and is never signed', async () => {
    const text = '![cat](file://attacker.test/share/cat.jpg)\n![dog](\\\\attacker.test\\share\\dog.png)\n![own](/repo/own.png)';
    const { client, rpcs } = fakeClient(() => ({}), { projection: { visibleTurnItems: [{ item: { text } }], messages: [], thread: {} } });
    expect(markdownMediaChips(text, '/repo').filter(chip => !chip.href.startsWith('t3-image:')).map(chip => [chip.label, chip.icon])).toEqual([['cat', 'blocked'], ['dog', 'blocked'], ['own', 'environment']]);
    await markdownMediaUrls(client, native, '/repo', 5);
    expect(rpcs.map(rpc => rpc.resource)).toEqual([{ _tag: 'media-file', path: '/repo/own.png', threadId: 't1' }]);
  });
  test('Markdown: MarkdownMedia draws a sound\'s audio chip, a file chip or the unavailable label by the chip\'s kind', () => {
    const media = contractComponent('src/media/media-markdown.contract', 'MarkdownMedia');
    const scope = (kind: string, access: string) => ({ kind, access, target: 'media:/repo/a', urls: [], alt: '' });
    const drawn = (marker: string, kind: string, access: string) => media.drawn(marker, {}, scope(kind, access));
    const label = 'MediaUnavailableLabel(alt=alt, kind=kind';
    expect([drawn('AudioChip(', 'audio', 'environment'), drawn('AudioChip(', 'audio', 'direct'), drawn(label, 'audio', 'environment')]).toEqual([true, true, false]);
    expect([drawn(label, 'audio', 'blocked'), drawn('AudioChip(', 'audio', 'blocked'), drawn('MarkdownMediaBody(', 'audio', 'direct')]).toEqual([true, false, false]);
    expect([drawn('MarkdownFileChip(', 'file', 'pdf'), drawn(label, 'file', 'blocked'), drawn('MarkdownMediaBody(', 'file', 'pdf')]).toEqual([true, false, false]);
    expect([drawn('MarkdownFileChip(', 'image', 'blocked'), drawn(label, 'image', 'blocked')]).toEqual([false, true]);
    // Its source and failure come from audioSource, once per sound.
    const fns = contractFns('src/media/media-markdown.contract', ['AudioSource']) as unknown as { audioSource: (urls: unknown[], target: string, access: string) => unknown };
    const urls = [{ id: 'media:/repo/a.m4a', url: 'http://x/a?sig=1' }, { id: 'media-failed:/repo/b.m4a', url: '' }];
    expect([fns.audioSource(urls, 'media:/repo/a.m4a', 'environment'), fns.audioSource(urls, 'media:/repo/b.m4a', 'environment'), fns.audioSource([], 'https://x/c.ogg', 'direct')])
      .toEqual([{ src: 'http://x/a?sig=1', failed: false }, { src: '', failed: true }, { src: 'https://x/c.ogg', failed: false }]);
  });
  test('Markdown: a sound is signed as a media file, a file chip is not', async () => {
    const items = [{ item: { text: '![t](/tmp/take.mp3)\n![r](/tmp/report.pdf)' } }];
    const { client, rpcs } = fakeClient(() => ({}), { projection: { visibleTurnItems: items, messages: [], thread: {} } });
    expect((await markdownMediaUrls(client, native, '/repo', 5)).map(entry => entry.id)).toEqual(['media:/tmp/take.mp3']);
    expect(rpcs.map(rpc => rpc.resource)).toEqual([{ _tag: 'media-file', path: '/tmp/take.mp3', threadId: 't1' }]);
  });
  test('Markdown: a sound\'s name opens its file beside the thread; a web sound\'s opens nothing', async () => {
    const { client } = fakeClient(() => ({}));
    const opened: string[] = [];
    await mediaLocal(client, native, 'open-file', '/repo/clips/take.mp3', '', { openFile: async path => { opened.push(path); } });
    await mediaLocal(client, native, 'open-file', '', '', { openFile: async path => { opened.push(path); } });
    expect(opened).toEqual(['/repo/clips/take.mp3']);
  });
  test('Markdown: host-path media are signed for the shown thread; a refusal is listed', async () => {
    const items = [{ item: { text: '![a](/tmp/a.png)\n![b](/tmp/b.mp4)\n![c](https://x/c.png)' } }];
    const ok = fakeClient(() => ({}), { projection: { visibleTurnItems: items, messages: [], thread: {} } });
    expect(await markdownMediaUrls(ok.client, native, '/repo', 5)).toEqual([{ id: 'media:/tmp/a.png', url: 'http://127.0.0.1:16260/api/assets/fresh/logo.png?sig=2' }, { id: 'media:/tmp/b.mp4', url: 'http://127.0.0.1:16260/api/assets/fresh/logo.png?sig=2' }]);
    expect(ok.rpcs.map(rpc => rpc.resource)).toEqual([{ _tag: 'media-file', path: '/tmp/a.png', threadId: 't1' }, { _tag: 'media-file', path: '/tmp/b.mp4', threadId: 't1' }]);
    const refused = fakeClient(() => ({}), { projection: { visibleTurnItems: items, messages: [], thread: {} }, rpc: async () => { throw new Error('no'); } });
    expect((await markdownMediaUrls(refused.client, native, '/repo', 5)).map(entry => entry.id)).toEqual(['media-failed:/tmp/a.png', 'media-failed:/tmp/b.mp4']);
  });

  test('a rendered Markdown file signs its images again when the workspace changes or the connection returns (ChatMarkdownAssetImage)', async () => {
    let signature = 0;
    const { client, rpcs } = fakeClient(() => ({}), { rpc: async (_native: Native, method: string, payload: Call) => { rpcs.push({ method, ...payload }); return { relativeUrl: `/api/assets/f/a.png?sig=${++signature}` }; } });
    const text = ['![a](/tmp/a.png)\n![b](/tmp/b.mp4)'];
    const signed = async (mutationId: string | null) => (await markdownMediaUrls(client, native, '/repo', 5, text, { mutationId }))[0]!.url;
    expect(await signed('["c1",null]')).toBe('http://127.0.0.1:16260/api/assets/f/a.png?sig=1');
    expect(rpcs.length).toBe(2);
    // The same workspace: nothing is signed again.
    expect(await signed('["c1",null]')).toBe('http://127.0.0.1:16260/api/assets/f/a.png?sig=1');
    expect(rpcs.length).toBe(2);
    // A settled command changed the workspace: the image is signed again, the video is not.
    expect(await signed('["c2",null]')).toBe('http://127.0.0.1:16260/api/assets/f/a.png?sig=3');
    expect(rpcs.map(rpc => (rpc.resource as Call).path)).toEqual(['/tmp/a.png', '/tmp/b.mp4', '/tmp/a.png']);
    // Offline nothing signs; back online the image signs again.
    (client as unknown as { connection: string }).connection = 'reconnecting';
    await signed('["c2",null]');
    (client as unknown as { connection: string }).connection = 'connected';
    expect(await signed('["c2",null]')).toBe('http://127.0.0.1:16260/api/assets/f/a.png?sig=4');
  });
});
