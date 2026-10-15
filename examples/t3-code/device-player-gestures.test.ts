// device-player-gestures (T3 Code 1e2ecbd975, MIT, see LICENSE-T3: ThreadPreviewMiniPlayer.tsx MiniPlayerShell's
// beginGesture / handlePointerMove and its pill, chatCanvasLayout.ts, previewMiniPlayerLayout.ts). The floating device
// player's gestures as a hand makes them: R6DeviceMiniPlayer's `pan` runs from r6-device.contract with the clock moving
// between moves (contract-action-fixture.ts), and every gesture it sends goes to the canvas source (chat-canvas-view.ts).
// DP-2: one serial per gesture, so a drag moves by the pointer's total once. DP-4a, DPC-3, DPC-4: the resizes from it.
// DP-3: the pill's drag is a grab box behind its buttons. DP-10: the 840 × 620 cells.
import { beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { chatCanvasView, type ChatCanvasArgs } from './chat-canvas-view';
import { resolveChatCanvasLayout } from './chat-canvas-layout';
import { resizePreviewMiniPlayer, resolveDeviceMiniPlayerSourceSize, type BrowserViewportResizeDirection } from './previewMiniPlayerLayout';
import { floatMiniDevice, miniStoreOf } from './r6-media-device';
import { DETAILS_FRAMES } from './thread-details-card-layout';
import { contractComponent } from './contract-action-fixture';

const iphone = { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 17 Pro' };
const KEY = 'device:local:IPHONE';
const native = null as unknown as Native;
// 1280 × 840 with the 256 pt sidebar and the right panel closed: the canvas is 1024 × 788 under the 52 pt header, the
// composer overlay 172 pt; the lane's drag stop is 20 + 640 + 12 = 672 from the canvas edge.
const frames = { chat: [256, 0, 1024, 840], overlay: [400, 668, 736, 172] };
const args = (extra: Partial<ChatCanvasArgs> = {}): ChatCanvasArgs => ({ width: 1024, viewportHeight: 840, detailsInline: false, chatMax: 736, overlaid: true, gesture: '', wide: true, popoverOpen: false, ...extra });
const player = contractComponent('r6-device.contract', 'R6DeviceMiniPlayer');
let client: T3Client;
let canvasArgs: Partial<ChatCanvasArgs>;
// The player's slots and the clock live across the gestures of a test, as the mounted component's do.
let state: Record<string, unknown>;
let clock: number;

beforeEach(() => {
  client = { environmentId: 'env', threadId: 't1', draftKey: 'env:t1', generation: 1, diffOpen: false, presentation: { frames }, local: {}, shell: { projects: [], threads: [] } } as unknown as T3Client;
  canvasArgs = {};
  state = player.state();
  clock = 5000;
  floatMiniDevice(client, 't1', iphone);
});

/** The player's frame in canvas points (its top less the 52 pt header). */
async function frame(gesture = '') {
  const { mini } = await chatCanvasView(client, native, args({ ...canvasArgs, gesture }));
  return { x: mini.x, y: mini.top - 52, width: mini.width, height: mini.height };
}

/** A hand's gesture: `moves` pans 40 ms apart (a real trackpad's 10 pt every 40 ms), each sent to the canvas, then the release. */
async function gesture(direction: string, dx: number, dy: number, moves: number) {
  const sent: string[] = [];
  const scope = { mini: { sourceKey: KEY }, floor: Math.floor, performanceNow: () => clock, gesture: (value: string) => sent.push(value) };
  await frame();
  for (let i = 0; i < moves; i++) {
    clock += 40;
    player.call('pan', state, scope, [direction, dx / moves, dy / moves]);
    await frame(sent.at(-1));
  }
  const held = await frame(sent.at(-1));
  player.call('release', state, scope, [0, 0]);
  expect(sent.at(-1)).toBe('');
  return { sent: sent.slice(0, -1), held, after: await frame() };
}

describe('DP-2: a drag moves the player by the pointer\'s total, once', () => {
  test('every move of one drag carries the serial of its first pan, and the totals add up', async () => {
    const { sent } = await gesture('move', -320, -160, 20);
    const parts = sent.map(value => value.split('|'));
    expect(new Set(parts.map(([serial]) => serial)).size).toBe(1);
    expect(parts.map(([, source, direction]) => `${source}|${direction}`)).toEqual(Array(20).fill(`${KEY}|move`));
    expect(parts.at(-1)!.slice(3).map(Number)).toEqual([-320, -160]);
    // The next drag is another serial (the count goes on), so it starts from the frame then on screen.
    const next = await gesture('move', 0, 40, 2);
    expect(next.sent[0]!.split('|')[0]).not.toBe(parts[0]![0]);
  });

  test('a (-320, -160) drag from the default frame: x stops at the readable lane, y moves exactly -160 (reference: 964,184)', async () => {
    expect(await frame()).toEqual({ x: 772, y: 256, width: 240, height: 520 });
    const { held, after } = await gesture('move', -320, -160, 20);
    expect(held).toEqual({ x: 672, y: 96, width: 240, height: 520 });
    expect(after).toEqual(held);
    expect(miniStoreOf(client).get('t1')).toMatchObject({ position: { x: 452, y: 96 }, lastInteraction: 'drag' });
    // The padding drag after it (DP-3's +60, +200): x +60 and y to the bottom clamp, as the reference's 1024,296.
    expect((await gesture('move', 60, 200, 20)).after).toEqual({ x: 732, y: 256, width: 240, height: 520 });
  });
});

describe('DP-4a: the eight zones resize from the frame at the press', () => {
  // The canvas pass after one resize of the total from `start` (MiniPlayerShell's handlePointerMove, then ChatCanvas).
  const once = (start: { x: number; y: number; width: number; height: number }, direction: BrowserViewportResizeDirection, dx: number, dy: number) => {
    const source = resolveDeviceMiniPlayerSourceSize('ios', null), container = { width: 1024, height: 788 };
    const next = resizePreviewMiniPlayer({ start, direction, delta: { x: dx, y: dy }, source, container });
    return resolveChatCanvasLayout({ container, preview: { key: KEY, width: next.width, position: { x: next.x, y: next.y }, source, lastInteraction: 'resize' }, padding: 20, maxChatWidth: 736, minChatWidth: 640, composerHeight: 172 }).frame;
  };
  const OUT: Record<BrowserViewportResizeDirection, [number, number]> = { north: [0, -60], south: [0, 60], east: [60, 0], west: [-60, 0], northwest: [-60, -60], northeast: [60, -60], southwest: [-60, 60], southeast: [60, 60] };

  for (const [direction, [dx, dy]] of Object.entries(OUT) as [BrowserViewportResizeDirection, [number, number]][]) {
    test(`${direction}: six moves of 10 pt equal one resize of 60 pt, from the lane stop at the top`, async () => {
      await gesture('move', -500, -400, 10); // to the lane stop and the top (964,102 on screen)
      const stop = await frame();
      expect(stop).toEqual({ x: 672, y: 12, width: 240, height: 520 });
      expect((await gesture(direction, dx, dy, 6)).after).toEqual(once(stop, direction, dx, dy)!);
    });
  }

  test('W: the reference\'s numbers: out 60 from the lane stop at the top ends at 904 on screen, the right edge held', async () => {
    await gesture('move', -500, -400, 10);
    expect((await gesture('west', -60, 0, 6)).after).toEqual({ x: 612, y: 12, width: 273, height: 592 });
    expect((await gesture('west', 200, 0, 5)).after).toEqual({ x: 645, y: 12, width: 240, height: 520 }); // the right edge at 885
  });

  test('W: out 60 from the reference\'s own start (1024,280 on screen) ends at its 964,102 (it recorded 272 × 592)', async () => {
    await gesture('move', -40, -66, 10);
    expect(await frame()).toEqual({ x: 732, y: 190, width: 240, height: 520 });
    expect((await gesture('west', -60, 0, 6)).after).toEqual({ x: 672, y: 12, width: 273, height: 592 });
  });
});

describe('DPC-3 and DPC-4: the docked details card while the top edge grows and shrinks the player', () => {
  // The card at the canvas's top right (732 + 256 = 988 in the window, 280 wide, 12 below the header): 229 pt of content
  // (231 with its borders); the workspace row and the resting Git group show only in `full` (compact = 231 − 64).
  const card = { ...frames, [DETAILS_FRAMES.content]: [988, 64, 280, 229], [DETAILS_FRAMES.fullTop]: [996, 72, 264, 32], [DETAILS_FRAMES.fullGit]: [996, 200, 264, 32] };
  beforeEach(() => {
    client.presentation = { frames: card };
    canvasArgs = { detailsInline: true };
  });
  const canvas = async () => chatCanvasView(client, native, args(canvasArgs));

  test('37 pt up grows the player by 37 and the card folds to compact; 32 more, essential; 37 more, the popover; 150 down, the card is back', async () => {
    const read = async () => { const c = await canvas(); return [await frame(), c.density, c.popover, c.cardShown]; };
    expect(await read()).toEqual([{ x: 772, y: 256, width: 240, height: 520 }, 'full', false, true]);
    // The top edge follows the pointer (bottom held at 776); the wider player stays 12 pt inside the canvas's right edge.
    await gesture('north', 0, -37, 5);
    expect(await read()).toEqual([{ x: 755, y: 219, width: 257, height: 557 }, 'compact', false, true]);
    await gesture('north', 0, -32, 5);
    expect(await read()).toEqual([{ x: 740, y: 187, width: 272, height: 589 }, 'essential', false, true]);
    // DPC-4: 37 more leaves the card under 160 pt: it becomes the header's popover. Opened, then 150 pt down: the
    // popover's mode ends and the docked card is back in full (the top at 256 leaves it 244 pt).
    await gesture('north', 0, -37, 5);
    expect(await read()).toEqual([{ x: 723, y: 150, width: 289, height: 626 }, 'full', true, false]);
    canvasArgs = { detailsInline: true, popoverOpen: true };
    await gesture('north', 0, 150, 10);
    expect(await read()).toEqual([{ x: 723, y: 256, width: 240, height: 520 }, 'full', false, true]);
  });
});

describe('DP-3: a press on a pill button starts no drag', () => {
  const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
  async function component(name: string): Promise<string[]> {
    const lines = (await source('r6-device.contract')).split('\n');
    const start = lines.indexOf(`component ${name}`);
    const end = lines.findIndex((line, index) => index > start && line.startsWith('component '));
    return lines.slice(start, end < 0 ? undefined : end);
  }

  test('the pill holds no pan; a grab box behind its buttons does, and the buttons lie over it', async () => {
    const lines = await component('R6DeviceMiniPlayer');
    const pill = lines.findIndex(line => line.includes('testId="device-mini-pill"'));
    expect(lines[pill]).not.toContain('pan=');
    expect(lines[pill]).toContain('hover=overPill');
    // Its first child: the grab box over the padding, the gaps and the 1 pt border, before (under) the two buttons.
    expect(lines[pill + 1]).toContain('box position="absolute" left=-1 top=-1 right=-1 bottom=-1 pan=pan("move") panrelease=release touch-action="none"');
    expect(lines[pill + 1]).toContain('testId="device-mini-grab"');
    expect(lines.slice(pill + 2, pill + 4).every(line => line.trim().startsWith('R6MiniPillButton('))).toBe(true);
    // Only the grab box, the handle and the eight zones pan.
    expect(lines.filter(line => line.includes('pan=pan(')).length).toBe(10);
    const button = (await component('R6MiniPillButton')).find(line => line.trim().startsWith('button '))!;
    expect(button).not.toContain('pan=');
    expect(button).toContain('position="relative" flex-shrink=0 width="1.5rem" height="1.5rem"');
  });
});

describe('DP-10: the player at 840 × 620', () => {
  // The sidebar is 208 at 840 wide: the canvas is 632 × 568.
  const small = { chat: [208, 0, 632, 620], overlay: [228, 451, 592, 165] };
  beforeEach(() => { client.presentation = { frames: small }; });

  test('a draft (the hero composer, not overlaid) keeps the 240 pt minimum at the reference\'s cell (624,124 on screen)', async () => {
    const { mini } = await chatCanvasView(client, native, args({ width: 632, viewportHeight: 620, overlaid: false }));
    expect([mini.x, mini.top - 52, mini.width, mini.height]).toEqual([380, 36, 240, 520]);
  });

  test('a thread (the composer docked) fits above the composer, as the reference\'s layout does with its composer\'s height', async () => {
    const { mini } = await chatCanvasView(client, native, args({ width: 632, viewportHeight: 620 }));
    const reference = resolveChatCanvasLayout({ container: { width: 632, height: 568 }, preview: { key: KEY, width: null, position: null, source: resolveDeviceMiniPlayerSourceSize('ios', null), lastInteraction: 'drag' },
      padding: 20, maxChatWidth: 736, minChatWidth: 640, composerHeight: 165 }).frame!;
    expect([mini.x, mini.top - 52, mini.width, mini.height]).toEqual([reference.x, reference.y, reference.width, reference.height]);
    expect(mini.width).toBeLessThan(240);
  });
});
