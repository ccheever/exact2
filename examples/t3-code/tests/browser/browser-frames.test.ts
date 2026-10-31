// A Browser tab's frame (browser-frames.ts): the page state its records give (Manager.ts `computeNavStatus`, `failed`),
// the commands it takes, its names and partitions, and the layer's projection; and Annotate's pure helpers
// (browser-annotate.ts: Manager.ts `normalizeCaptureRect`, `cropAnnotationScreenshot`'s geometry, react-grab's merge).
import { describe, expect, it } from 'bun:test';
import {
  adoptFrameEvent, commandFrame, failureOf, frameCommandDone, frameCommandView, frameEntry, frameIdOf, frameLayerView, frameNav, frames, navigateFrame,
  partitionOf, setFrame, steppedZoom, syncFrames,
} from '../../src/browser/browser-frames';
import { cropRegion, mergeReact, normalizeCaptureRect, targetRects } from '../../src/browser/browser-annotate';

const tab = (id: string, url = '') => ({ runtimeId: id, url, environment: 'env', profile: 'default', zoom: 1, colorScheme: 'system' as const });
const q = (fields: Record<string, string | number | boolean>) => Object.entries(fields).map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`).join('&');

describe('a frame\'s records (computeNavStatus)', () => {
  it('Loading while a navigation is in flight, Success after it, and the committed URL and title', () => {
    const owner = {};
    syncFrames(owner, [tab('a', 'https://a.test/')]);
    const id = frameIdOf('a'), entry = () => frameEntry(owner, 'a')!;
    expect(frameNav(entry())).toEqual({ kind: 'Loading', url: 'https://a.test/', title: '' }); // the mount URL is its first load
    expect(adoptFrameEvent(owner, id, q({ event: 'navigate', id: 1, destination: 'https://a.test/', loading: true }), 0)).toBe(true);
    adoptFrameEvent(owner, id, q({ event: 'currententrychange', id: 1, url: 'https://a.test/', loading: true }), 0);
    adoptFrameEvent(owner, id, q({ event: 'navigatesuccess', id: 1, url: 'https://a.test/', title: '', back: false }), 0);
    expect(frameNav(entry())).toEqual({ kind: 'Success', url: 'https://a.test/', title: '' });
    // WebKit's title comes after the load, as its own currententrychange (docs/agent-pitfalls.md).
    adoptFrameEvent(owner, id, q({ event: 'currententrychange', id: 1, url: 'https://a.test/', title: 'A', icons: 'https://a.test/i.png\nhttps://a.test/favicon.ico' }), 5);
    expect(frameNav(entry())).toEqual({ kind: 'Success', url: 'https://a.test/', title: 'A' });
    expect(entry().favicon).toEqual({ dataUrl: 'https://a.test/i.png', pageUrl: 'https://a.test/', capturedAt: 5 });
    // Another origin's document drops the old icon until it names one.
    adoptFrameEvent(owner, id, q({ event: 'currententrychange', id: 2, url: 'https://b.test/', loading: true }), 6);
    expect(entry().favicon).toBeNull();
    expect(adoptFrameEvent(owner, 'browser-frame-missing', q({ event: 'navigate' }), 0)).toBe(false);
  });

  it('a network failure is LoadFailed at the failed URL, worded as Chromium\'s; an abort or a policy stop is none', () => {
    const owner = {};
    syncFrames(owner, [tab('a')]);
    const id = frameIdOf('a'), entry = () => frameEntry(owner, 'a')!;
    expect(frameNav(entry()).kind).toBe('Idle');
    adoptFrameEvent(owner, id, q({ event: 'navigate', id: 1, destination: 'http://localhost:9/', loading: true }), 0);
    adoptFrameEvent(owner, id, q({ event: 'navigateerror', id: 1, error: 'NetworkError', message: 'NSURLErrorDomain -1004: Could not connect to the server.' }), 0);
    expect(frameNav(entry())).toEqual({ kind: 'LoadFailed', url: 'http://localhost:9/', title: '' });
    expect([entry().failure, entry().failures]).toEqual([{ url: 'http://localhost:9/', code: -1004, description: 'ERR_CONNECTION_REFUSED' }, 1]);
    adoptFrameEvent(owner, id, q({ event: 'navigate', id: 2, destination: 'https://ok.test/', loading: true }), 0);
    expect(entry().failure).toBeNull(); // the next navigation starts clean
    adoptFrameEvent(owner, id, q({ event: 'navigateerror', id: 2, error: 'AbortError', message: 'superseded by a new navigation' }), 0);
    expect([frameNav(entry()).kind, entry().failures]).toEqual(['Idle', 1]);
    expect(failureOf('u', 'WebKitErrorDomain 204: Plug-in handled load')).toBeNull();
    expect(failureOf('u', 'NSURLErrorDomain -1202: The certificate is not trusted.')?.description).toBe('ERR_CERT_AUTHORITY_INVALID');
    expect(failureOf('u', 'WebKitErrorDomain 300: Something else')).toEqual({ url: 'u', code: 300, description: 'Something else' });
    // A crash that will not recover leaves the page as it was until Refresh.
    adoptFrameEvent(owner, id, q({ event: 'navigateerror', id: 3, error: 'crashed', recovering: false }), 0);
    expect(entry().crashed).toBe(true);
  });
});

describe('a frame\'s commands', () => {
  it('queues navigate, traverse, reload and stop, one at a time, and refuses what a frame cannot load', () => {
    const owner = {};
    syncFrames(owner, [tab('a', 'https://a.test/')]);
    const id = frameIdOf('a');
    adoptFrameEvent(owner, id, q({ event: 'navigatesuccess', id: 1, url: 'https://a.test/', back: true }), 0);
    expect(navigateFrame(owner, 'a', 'javascript:alert(1)')).toBe(false);
    expect(navigateFrame(owner, 'a', 'https://b.test/')).toBe(true);
    commandFrame(owner, 'a', 'back');
    commandFrame(owner, 'a', 'forward'); // no forward entry: nothing
    commandFrame(owner, 'a', 'stop');
    commandFrame(owner, 'a', 'devtools');
    expect(frames(owner).queue.map(command => [command.kind, command.url, command.delta])).toEqual([['navigate', 'https://b.test/', 0], ['traverse', '', -1], ['stop', '', 0], ['devtools', '', 0]]);
    expect(frameCommandView(owner)).toMatchObject({ kind: 'navigate', id });
    frameCommandDone(owner, frameCommandView(owner).serial);
    expect(frameCommandView(owner).kind).toBe('traverse');
    // Refresh retries a failed load at its own URL; with none in flight it reloads, Hard reload past the cache.
    frames(owner).queue = [];
    adoptFrameEvent(owner, id, q({ event: 'navigate', id: 2, destination: 'http://down.test/', loading: true }), 0);
    adoptFrameEvent(owner, id, q({ event: 'navigateerror', id: 2, error: 'NetworkError', message: 'NSURLErrorDomain -1003: A server with the specified hostname could not be found.' }), 0);
    commandFrame(owner, 'a', 'refresh');
    expect(frames(owner).queue.at(-1)).toMatchObject({ kind: 'navigate', url: 'http://down.test/' });
    adoptFrameEvent(owner, id, q({ event: 'navigatesuccess', id: 3, url: 'http://down.test/' }), 0);
    commandFrame(owner, 'a', 'hard-reload');
    expect(frames(owner).queue.at(-1)).toMatchObject({ kind: 'reload', flag: true });
    // A tab whose session ended drops its frame and its commands.
    syncFrames(owner, []);
    expect([frames(owner).entries.size, frames(owner).queue.length]).toEqual([0, 0]);
  });

  it('steps the zoom ladder and sets the appearance', () => {
    const owner = {};
    syncFrames(owner, [tab('a')]);
    setFrame(owner, 'a', { zoomStep: 'in' });
    expect(frameEntry(owner, 'a')!.zoom).toBe(1.1);
    setFrame(owner, 'a', { zoom: 1.3 });
    expect(frameEntry(owner, 'a')!.zoom).toBe(1.25); // a value off the ladder snaps to its nearest step
    setFrame(owner, 'a', { colorScheme: 'sepia' });
    setFrame(owner, 'a', { colorScheme: 'dark' });
    expect(frameEntry(owner, 'a')!.colorScheme).toBe('dark');
    expect([steppedZoom(5, true), steppedZoom(0.25, false), steppedZoom(1.2, false)]).toEqual([5, 0.25, 1]); // 1.2 steps down from the 1.1 below it
  });
});

describe('names and the layer', () => {
  it('a partition per environment and profile, Incognito in memory; ids are the runtime id\'s hash', () => {
    expect(partitionOf('env', 'default')).toMatch(/^persist:t3-preview-[0-9a-f]{16}$/);
    expect(partitionOf('env', 'default')).not.toBe(partitionOf('env', 'work'));
    expect(partitionOf('env', 'default')).not.toBe(partitionOf('other', 'default'));
    expect(partitionOf('env', 'incognito')).toMatch(/^t3-incognito-[0-9a-f]{16}$/);
    expect(frameIdOf('["env","t","e","tab"]')).toMatch(/^browser-frame-[0-9a-f]{16}$/);
  });
  it('places the panel\'s tab in its slot at its fixed viewport, the floating player\'s at the player, and keeps a held tab drawing', () => {
    const owner = {};
    syncFrames(owner, [tab('a', 'https://a.test/'), tab('b'), tab('c'), tab('d')]);
    const layer = frameLayerView(owner, { runtimeId: 'a', pageWidth: 390, pageHeight: 844 }, 'c', runtimeId => runtimeId === 'd' ? { width: 900, height: 700 } : null);
    expect(layer.frames.map(frame => [frame.frameId, frame.place, frame.pageWidth, frame.src, frame.held, frame.heldWidth])).toEqual([
      [frameIdOf('a'), 'panel', 390, 'https://a.test/', false, 1280], [frameIdOf('b'), '', 0, 'about:blank', false, 1280],
      [frameIdOf('c'), 'mini', 0, 'about:blank', false, 1280], [frameIdOf('d'), '', 0, 'about:blank', true, 900]]);
  });
});

describe('Annotate\'s crop and React read', () => {
  it('normalizeCaptureRect keeps finite rects with a size', () => {
    expect(normalizeCaptureRect({ x: 1, y: 2, width: 3, height: 4 })).toEqual({ x: 1, y: 2, width: 3, height: 4 });
    expect(normalizeCaptureRect({ x: 1, y: 2, width: 0, height: 4 })).toBeNull();
    expect(normalizeCaptureRect({ x: 'a', y: 2, width: 3, height: 4 })).toBeNull();
  });
  it('crops to the rect clipped to the viewport, the whole viewport without one, nothing when no target is in view', () => {
    const size = { width: 800, height: 600 };
    expect(cropRegion({ x: -10.5, y: 20.2, width: 100, height: 50 }, [{ x: 0, y: 30, width: 10, height: 10 }], size)).toEqual({ x: 0, y: 20, width: 90, height: 51 });
    expect(cropRegion(null, [], size)).toEqual({ x: 0, y: 0, width: 800, height: 600 });
    expect(cropRegion({ x: 0, y: 700, width: 10, height: 10 }, [{ x: 0, y: 700, width: 10, height: 10 }], size)).toBeNull();
    expect(cropRegion({ x: 900, y: 0, width: 10, height: 10 }, [], size)).toBeNull();
    expect(cropRegion({ x: 0, y: 0, width: 10, height: 10 }, [{ x: 5, y: 5, width: 0, height: 20 }], size)).toEqual({ x: 0, y: 0, width: 10, height: 10 });
    expect(targetRects({ elements: [{ rect: { x: 1, y: 1, width: 2, height: 2 } }], strokes: [{ bounds: { x: 3, y: 3, width: 1, height: 1 } }] })).toHaveLength(2);
  });
  it('merges the page world\'s React findings by target id, bounded', () => {
    const annotation = { elements: [{ id: 't1', element: { selector: 'a' } }, { id: 't2', element: { selector: 'b' } }] };
    const merged = mergeReact(annotation, { t1: { componentName: 'Button', stack: [{ functionName: 'Button', fileName: 'b.tsx', lineNumber: 3, columnNumber: true }], source: null } });
    expect(merged.elements).toEqual([
      { id: 't1', element: { selector: 'a', componentName: 'Button', stack: [{ functionName: 'Button', fileName: 'b.tsx', lineNumber: 3, columnNumber: null }], source: { functionName: 'Button', fileName: 'b.tsx', lineNumber: 3, columnNumber: null } } },
      { id: 't2', element: { selector: 'b' } },
    ]);
  });
});
