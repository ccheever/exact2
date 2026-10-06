// The chat canvas source: the lane, the card and the floating player's frame and gestures
// (chat-canvas-view.ts), and closing the right panel on a device floating it (ChatView
// closePreviewPanel). The numbers are the ported functions' (chat-canvas-layout.ts).
import { beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { chatCanvasView, emptyChatCanvas, parseGesture, type ChatCanvasArgs } from './chat-canvas-view';
import { resolveChatCanvasLayout } from './chat-canvas-layout';
import { resolveDeviceMiniPlayerSourceSize } from './previewMiniPlayerLayout';
import { closeMiniDevice, floatMiniDevice, miniDeviceOf, miniStoreOf, selectDeviceTarget } from './r6-media-device';
import { panelKey, panelState, surfaceLocal } from './r4-surfaces-panel';
import { toggleInline } from './shell-prefs';

const iphone = { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' };
const pixel = { hostId: 'local', deviceId: 'emulator-5554', platform: 'android', name: 'Pixel 9' };
// 1280 × 840 with a 256 pt sidebar and the panel closed: the canvas is 1024 × 788 under the 52 pt header.
const frames = { chat: [256, 0, 1024, 840], overlay: [400, 668, 736, 172], 'details-content': [988, 64, 280, 300] };
const args = (extra: Partial<ChatCanvasArgs> = {}): ChatCanvasArgs => ({ width: 1024, viewportHeight: 840, detailsInline: false, chatMax: 736, overlaid: true, gesture: '', ...extra });
let client: T3Client;
const native = null as unknown as Native;

beforeEach(() => {
  client = { environmentId: 'env', threadId: 't1', draftKey: 'env:t1', generation: 1, diffOpen: false, presentation: { frames }, local: {}, shell: { projects: [], threads: [] } } as unknown as T3Client;
});

describe('the chat canvas (ChatCanvas, chatCanvasLayout)', () => {
  test('without a player chat is centred and the answer says so', async () => {
    expect(emptyChatCanvas().ready).toBe(false);
    const canvas = await chatCanvasView(client, native, args());
    expect(canvas).toMatchObject({ ready: true, containerWidth: 1024, containerHeight: 788, left: 144, width: 736, insetEnd: 0, overlapsChat: false, mini: { show: false } });
  });

  test('a new player sits bottom-right beside the composer at the minimum width and chat moves left (rewrites placeMini)', async () => {
    floatMiniDevice(client, 't1', iphone);
    const canvas = await chatCanvasView(client, native, args());
    // resolveDeviceMiniPlayerSourceSize before the first frame: a 9:19.5 phone, 240 wide (the minimum), not the 320 box.
    expect(canvas.mini).toMatchObject({ show: true, key: 'local\u0000IPHONE', x: 772, top: 52 + 256, width: 240, height: 520, radius: 12, pillInset: 8 });
    expect([canvas.left, canvas.width, canvas.insetEnd, canvas.overlapsChat]).toEqual([24, 736, 240, false]);
    const expected = resolveChatCanvasLayout({ container: { width: 1024, height: 788 }, preview: { key: 'device:local:IPHONE', width: null, position: null, source: resolveDeviceMiniPlayerSourceSize('ios', null), lastInteraction: 'drag' },
      padding: 20, maxChatWidth: 736, minChatWidth: 640, composerHeight: 172 });
    expect(expected.frame).toEqual({ x: 772, y: 256, width: 240, height: 520 });
    // A short canvas fits the height above the composer and the player overlaps chat (allows message overlap).
    client.presentation = { frames: { chat: [0, 0, 600, 552], overlay: [0, 400, 600, 100] } };
    const short = await chatCanvasView(client, native, args({ width: 600, viewportHeight: 552 }));
    expect(short.overlapsChat).toBe(true);
    expect(short.mini.top - 52 + short.mini.height).toBeLessThanOrEqual(500 - 100 - 12);
    client.presentation = { frames };
    // A landscape stream floats a wide box; an Android player rounds its corners like the phone.
    closeMiniDevice(client, 't1');
    floatMiniDevice(client, 't1', pixel);
    (client.presentation as { deviceStreams?: object }).deviceStreams = { 'local\u0000emulator-5554': { status: 'streaming', width: 2424, height: 1080, orientation: 'landscape_left' } };
    const android = await chatCanvasView(client, native, args());
    expect(android.mini.width / android.mini.height).toBeCloseTo(2424 / 1080, 1);
    expect([android.mini.radius, android.mini.pillInset]).toEqual([Math.max(12, Math.round(android.mini.height * 0.14)), Math.max(8, Math.round(android.mini.radius * 0.55))]);
  });

  test('a drag moves the store from the frame on screen; the lane stops it 672 from the canvas edge and keeps the stored x', async () => {
    floatMiniDevice(client, 't1', iphone);
    await chatCanvasView(client, native, args());
    const key = 'device:local:IPHONE';
    const drag = await chatCanvasView(client, native, args({ gesture: `1|${key}|move|-500|-100` }));
    expect(miniStoreOf(client).get('t1')).toMatchObject({ position: { x: 272, y: 156 }, width: null, lastInteraction: 'drag' });
    expect([drag.mini.x, drag.mini.top]).toEqual([672, 52 + 156]);
    expect([drag.left, drag.width]).toEqual([20, 640]);
    // Asking again with the same gesture changes nothing; the next move is from the start, not from the last answer.
    expect(await chatCanvasView(client, native, args({ gesture: `1|${key}|move|-500|-100` }))).toEqual(drag);
    await chatCanvasView(client, native, args({ gesture: `1|${key}|move|-10|0` }));
    expect(miniStoreOf(client).get('t1')!.position).toEqual({ x: 762, y: 256 });
    // Release, then a second drag starts from the frame now on screen.
    await chatCanvasView(client, native, args());
    await chatCanvasView(client, native, args({ gesture: `2|${key}|move|0|-40` }));
    expect(miniStoreOf(client).get('t1')!.position).toEqual({ x: 762, y: 216 });
    // A gesture that began on another source changes nothing (the stale-source guard).
    await chatCanvasView(client, native, args({ gesture: `3|device:local:OTHER|move|-50|0` }));
    expect(miniStoreOf(client).get('t1')!.position).toEqual({ x: 762, y: 216 });
  });

  test('every edge and corner resizes at the source aspect with the opposite edge held', async () => {
    const results: Record<string, { x: number; top: number; width: number; height: number }> = {};
    for (const [direction, dx, dy] of [['north', 0, -40], ['south', 0, 40], ['west', -40, 0], ['east', 40, 0], ['northwest', -40, -40], ['northeast', 40, -40], ['southwest', -40, 40], ['southeast', 40, 40]] as const) {
      miniStoreOf(client).close('t1');
      floatMiniDevice(client, 't1', iphone);
      await chatCanvasView(client, native, args({ gesture: '' }));
      // Drag it to the middle first so every edge has room, then resize from there.
      await chatCanvasView(client, native, args({ gesture: `m-${direction}|device:local:IPHONE|move|-100|-200` }));
      const before = await chatCanvasView(client, native, args());
      const after = await chatCanvasView(client, native, args({ gesture: `r-${direction}|device:local:IPHONE|${direction}|${dx}|${dy}` }));
      results[direction] = after.mini;
      expect(Math.abs(after.mini.width / after.mini.height - 9 / 19.5)).toBeLessThan(1 / after.mini.height + 0.005);
      expect(miniStoreOf(client).get('t1')!.lastInteraction).toBe('resize');
      // The store keeps the resize's frame (resizePreviewMiniPlayer); the layout pass then lifts it clear of the composer.
      const stored = miniStoreOf(client).get('t1')!;
      if (direction.includes('west')) expect(stored.position!.x + stored.width!).toBe(before.mini.x + before.mini.width);
      if (direction.includes('east')) expect(stored.position!.x).toBe(before.mini.x);
      if (direction.startsWith('north')) expect(stored.position!.y + Math.round(stored.width! / (9 / 19.5))).toBeCloseTo(before.mini.top - 52 + before.mini.height, -0.5);
    }
    expect(results.north!.width).toBeGreaterThan(240);
    expect(results.southeast!.width).toBeGreaterThan(240);
  });

  test('resizing never goes below 240 wide and never past the canvas edge', async () => {
    floatMiniDevice(client, 't1', iphone);
    await chatCanvasView(client, native, args());
    const small = await chatCanvasView(client, native, args({ gesture: '1|device:local:IPHONE|southeast|-300|-300' }));
    expect(small.mini.width).toBe(240);
    await chatCanvasView(client, native, args());
    const big = await chatCanvasView(client, native, args({ gesture: '2|device:local:IPHONE|northwest|-2000|-2000' }));
    expect(big.mini.x).toBeGreaterThanOrEqual(12);
    expect(big.mini.top - 52).toBeGreaterThanOrEqual(12);
    expect(big.mini.top - 52 + big.mini.height).toBeLessThanOrEqual(788 - 12);
  });

  test('the stored width survives a narrow window and returns when it grows (resolved on every pass)', async () => {
    floatMiniDevice(client, 't1', iphone);
    miniStoreOf(client).resize('t1', 'device:local:IPHONE', 330, { x: 600, y: 40 });
    const before = await chatCanvasView(client, native, args());
    client.presentation = { frames: { chat: [0, 0, 700, 452], overlay: [0, 300, 700, 100] } };
    const narrow = await chatCanvasView(client, native, args({ width: 700, viewportHeight: 452 }));
    expect(narrow.mini.width).toBeLessThan(before.mini.width);
    expect(narrow.mini.x + narrow.mini.width).toBeLessThanOrEqual(700 - 12);
    expect(miniStoreOf(client).get('t1')).toMatchObject({ width: 330, position: { x: 600, y: 40 } });
    client.presentation = { frames };
    expect(await chatCanvasView(client, native, args())).toEqual(before);
  });

  test('with the inline card the player tucks below it and the card folds only when no slot is left', async () => {
    floatMiniDevice(client, 't1', iphone);
    client.presentation = { frames: { chat: [0, 0, 1584, 1040], overlay: [400, 860, 736, 180], 'details-content': [1292, 64, 280, 325] } };
    const wide = client;
    const canvas = await chatCanvasView(wide, native, args({ width: 1584, viewportHeight: 1040, detailsInline: true }));
    expect([canvas.cardShown, canvas.overlapsDetailsCard, canvas.cardHeight]).toEqual([true, false, 988 - 24]);
    // A drag up into the card's columns slides out beside the card (the shorter move than below its 12 + 327),
    // chat moves left for it, and the card keeps its full height: moving alone never folds it.
    await chatCanvasView(wide, native, args({ width: 1584, viewportHeight: 1040, detailsInline: true, gesture: '1|device:local:IPHONE|move|0|-600' }));
    const dragged = await chatCanvasView(wide, native, args({ width: 1584, viewportHeight: 1040, detailsInline: true }));
    expect(dragged.mini).toMatchObject({ x: 1292 - 12 - 240, top: 52 + 12 });
    expect([dragged.left, dragged.overlapsDetailsCard, dragged.cardHeight]).toEqual([292, false, 988 - 24]);
    // A corner resize grows it; the card then takes the height resolveThreadDetailsCardLayout leaves it.
    await chatCanvasView(wide, native, args({ width: 1584, viewportHeight: 1040, detailsInline: true, gesture: '2|device:local:IPHONE|southeast|200|400' }));
    const grown = await chatCanvasView(wide, native, args({ width: 1584, viewportHeight: 1040, detailsInline: true }));
    expect(grown.mini.width).toBeGreaterThan(240);
    expect(grown.cardHeight).toBeLessThanOrEqual(988 - 24);
    // A thread whose card is closed reports none.
    toggleInline(wide, 'env:t1');
    expect((await chatCanvasView(wide, native, args({ width: 1584, viewportHeight: 1040, detailsInline: true }))).overlapsDetailsCard).toBe(false);
  });

  test('the player hides while the panel shows the same device, and its gesture is parsed or refused', async () => {
    floatMiniDevice(client, 't1', iphone);
    const state = panelState(client);
    state.surfaces = [{ id: 'device', kind: 'device', path: '', line: 0, reveal: 0 }]; state.active = 'device'; state.visible = true;
    selectDeviceTarget(client, panelKey(client), iphone);
    expect((await chatCanvasView(client, native, args())).mini.show).toBe(false);
    expect(parseGesture('4|device:a:b|southwest|-3|5')).toEqual({ serial: '4', sourceKey: 'device:a:b', direction: 'southwest', dx: -3, dy: 5 });
    expect([parseGesture(''), parseGesture('4|k|sideways|1|1'), parseGesture('4|k|move|x|1')]).toEqual([null, null, null]);
  });
});

describe('closing the right panel on a device floats it (ChatView closePreviewPanel)', () => {
  const deviceTab = () => {
    const state = panelState(client);
    state.surfaces = [{ id: 'device', kind: 'device', path: '', line: 0, reveal: 0 }, { id: 'files', kind: 'files', path: '', line: 0, reveal: 0 }];
    state.active = 'device'; state.visible = true;
    selectDeviceTarget(client, panelKey(client), iphone);
    return state;
  };
  test('⌘⌥B, the header button and the sheet backdrop (surface-hide) float the active device and hide the panel', async () => {
    const state = deviceTab();
    await surfaceLocal(client, native, 'hide', '', '');
    expect(state.visible).toBe(false);
    expect(miniDeviceOf(client, 't1')).toMatchObject({ hostId: 'local', deviceId: 'IPHONE' });
  });
  test('closing the tab or another surface active does not float', async () => {
    const state = deviceTab();
    await surfaceLocal(client, native, 'close', 'device', '');
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
    state.active = 'files'; state.visible = true;
    await surfaceLocal(client, native, 'hide', '', '');
    expect(miniDeviceOf(client, 't1')).toBeUndefined();
  });
});
