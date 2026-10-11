// device-player-compact-and-rail-keys (T3 Code 1e2ecbd975, MIT, see LICENSE-T3): DP-1 the thread details card's
// compact and essential rows (ThreadDetailsCard's density, ThreadDetailsPanel's rows per density), DP-2 the card as the
// header's popover where the floating player leaves it no placement, DR-1 the device rail menus' keys at a focused row.
// The canvas cases run chat-canvas-view.ts; the rows and the wiring are read from the Contract sources, as
// menu-keys.test.ts reads its menus. The behaviour is driven on the macOS bundle (the task record's drive).
import { beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { chatCanvasView, type ChatCanvasArgs } from './chat-canvas-view';
import { DETAILS_FRAMES, measureThreadDetailsCard, resolveThreadDetailsCardDensity } from './thread-details-card-layout';
import { floatMiniDevice, miniStoreOf } from './r6-media-device';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();
async function component(file: string, name: string): Promise<string> {
  const lines = (await source(file)).split('\n');
  const start = lines.findIndex(line => line === `component ${name}`);
  if (start < 0) throw new Error(`${file}: no component ${name}`);
  const end = lines.findIndex((line, index) => index > start && /^\S/.test(line) && !line.startsWith('//'));
  return lines.slice(start, end < 0 ? undefined : end).join('\n');
}
/** The lines nested under the first line of `body` containing `head` (deeper indent, until the indent comes back). */
function under(body: string, head: string): string[] {
  const lines = body.split('\n');
  const at = lines.findIndex(line => line.includes(head));
  if (at < 0) throw new Error(`no line with ${head}`);
  const depth = lines[at]!.length - lines[at]!.trimStart().length;
  const out: string[] = [];
  for (const line of lines.slice(at + 1)) {
    if (line.trim() && line.length - line.trimStart().length <= depth) break;
    out.push(line);
  }
  return out;
}

const iphone = { hostId: 'local', deviceId: 'IPHONE', platform: 'ios', name: 'iPhone 18 Pro' };
const KEY = 'device:local:IPHONE';
// 1584 × 1040 with the panel closed: the canvas is 1584 × 988 under the 52 pt header; the card sits at x 1292, 280 wide.
const chat = [0, 0, 1584, 1040], overlay = [400, 860, 736, 180];
// The card's full layout: 325 pt of content; the parts only `full` shows (the workspace row 32, Automations and Lineage
// 50, the Git action group at rest 32) and the Version Control section (its 1 pt hairline only in `full`).
const fullLayout = { chat, overlay, [DETAILS_FRAMES.content]: [1292, 64, 280, 325], [DETAILS_FRAMES.fullTop]: [1300, 72, 264, 32],
  [DETAILS_FRAMES.fullTail]: [1300, 300, 264, 50], [DETAILS_FRAMES.fullGit]: [1300, 200, 264, 32], [DETAILS_FRAMES.vcs]: [1292, 150, 280, 120] };
const FULL = 325 + 2, COMPACT = FULL - 32 - 50 - 32 - 1;
const native = null as unknown as Native;
let client: T3Client;
const args = (extra: Partial<ChatCanvasArgs> = {}): ChatCanvasArgs => ({ width: 1584, viewportHeight: 1040, detailsInline: true, chatMax: 736, overlaid: true, gesture: '', wide: true, popoverOpen: false, ...extra });
/** The player resized (so the layout pass keeps it where it is) to `y`, over the card's columns. */
const playerAt = (y: number) => miniStoreOf(client).resize('t1', KEY, 240, { x: 1300, y });

beforeEach(() => {
  client = { environmentId: 'env', threadId: 't1', draftKey: 'env:t1', generation: 1, diffOpen: false, presentation: { frames: fullLayout }, local: {}, shell: { projects: [], threads: [] } } as unknown as T3Client;
});

describe('DP-1: ThreadDetailsCard measures its content per density', () => {
  test('a full layout gives the full height and the compact one without the parts only full shows; a compact layout the compact one', () => {
    expect(measureThreadDetailsCard(fullLayout)).toEqual({ full: FULL, compact: COMPACT });
    // Without Automations, Lineage, a resting Git group or a Version Control section, compact is full less the workspace rows.
    expect(measureThreadDetailsCard({ [DETAILS_FRAMES.content]: [0, 0, 280, 200], [DETAILS_FRAMES.fullTop]: [0, 0, 264, 64] })).toEqual({ full: 202, compact: 138 });
    expect(measureThreadDetailsCard({ [DETAILS_FRAMES.content]: [0, 0, 280, 180], [DETAILS_FRAMES.compact]: [0, 0, 0, 0] })).toEqual({ compact: 182 });
    // The essential rows, or a layout caught between densities (no marker), are not measured; nor is a card not shown.
    expect(measureThreadDetailsCard({ [DETAILS_FRAMES.content]: [0, 0, 280, 120] })).toEqual({});
    expect(measureThreadDetailsCard({ chat })).toEqual({});
  });

  test('resolveThreadDetailsCardDensity: full while it fits, then compact, then essential', () => {
    expect(resolveThreadDetailsCardDensity(400, { full: FULL, compact: COMPACT })).toBe('full');
    expect(resolveThreadDetailsCardDensity(250, { full: FULL, compact: COMPACT })).toBe('compact');
    expect(resolveThreadDetailsCardDensity(200, { full: FULL, compact: COMPACT })).toBe('essential');
    // Before a measurement the card tries the next density and measures it there.
    expect(resolveThreadDetailsCardDensity(200, { full: 0, compact: 0 })).toBe('full');
    expect(resolveThreadDetailsCardDensity(200, { full: FULL, compact: 0 })).toBe('compact');
  });

  test('a player resized into the card folds its rows to compact, then essential, and more room restores them', async () => {
    floatMiniDevice(client, 't1', iphone);
    const open = await chatCanvasView(client, native, args());
    expect([open.density, open.popover, open.cardShown, open.cardHeight]).toEqual(['full', false, true, 988 - 24]);
    // The card reports itself to the canvas as tall as its full content (ThreadDetailsCard cardBottom), whatever it shows.
    playerAt(300);
    const compact = await chatCanvasView(client, native, args());
    expect([compact.density, compact.popover, compact.cardShown, compact.cardHeight, compact.overlapsDetailsCard]).toEqual(['compact', false, true, 300 - 24, true]);
    // Laid out compact, the card measures its compact rows (a marker frame, no full-only parts); the full height is kept.
    client.presentation = { frames: { chat, overlay, [DETAILS_FRAMES.content]: [1292, 64, 280, COMPACT - 2], [DETAILS_FRAMES.compact]: [1292, 64, 0, 0] } };
    expect((await chatCanvasView(client, native, args())).density).toBe('compact');
    playerAt(220);
    const essential = await chatCanvasView(client, native, args());
    expect([essential.density, essential.cardHeight]).toEqual(['essential', 220 - 24]);
    // Essential is not measured: the heights stay, so a smaller player brings the compact and then the full rows back.
    client.presentation = { frames: { chat, overlay, [DETAILS_FRAMES.content]: [1292, 64, 280, 120] } };
    playerAt(300);
    expect((await chatCanvasView(client, native, args())).density).toBe('compact');
    playerAt(600);
    expect((await chatCanvasView(client, native, args())).density).toBe('full');
  });

  test('the popover takes the canvas height less 52 pt, measured under its own key when chat is narrow', async () => {
    client.presentation = { frames: { chat: [0, 0, 900, 300], overlay, ...{ [DETAILS_FRAMES.content]: [600, 60, 264, 325], [DETAILS_FRAMES.fullTop]: [608, 68, 248, 32] } } };
    const narrow = await chatCanvasView(client, native, args({ width: 900, viewportHeight: 300, detailsInline: false, wide: false, popoverOpen: true }));
    // 300 − 52 header − 52 = 196: the full content (327) does not fit; its compact height (327 − 32 = 295) does not either.
    expect([narrow.popover, narrow.density]).toEqual([true, 'essential']);
    // The docked card's measurements are another key: a wide canvas starts again from its own.
    client.presentation = { frames: { chat: [0, 0, 1584, 300], overlay } };
    expect((await chatCanvasView(client, native, args({ viewportHeight: 300, popoverOpen: false }))).density).toBe('full');
  });
});

describe('DP-2: the card becomes the header\'s popover where no placement is left', () => {
  test('the canvas answers the mode and counts its switches; the inline mode coming back closes the popover', async () => {
    floatMiniDevice(client, 't1', iphone);
    const first = await chatCanvasView(client, native, args());
    expect([first.popover, first.cardShown]).toEqual([false, true]);
    // Resized up into the card until under 160 pt is left above it: the card leaves the canvas for the popover.
    playerAt(150);
    const popover = await chatCanvasView(client, native, args());
    expect([popover.popover, popover.cardShown, popover.popoverEpoch]).toEqual([true, false, first.popoverEpoch + 1]);
    // The popover's rows: the canvas height less 52 pt, so the full rows.
    expect(popover.density).toBe('full');
    expect((await chatCanvasView(client, native, args({ popoverOpen: true }))).popoverEpoch).toBe(popover.popoverEpoch);
    playerAt(300);
    const back = await chatCanvasView(client, native, args({ popoverOpen: true }));
    expect([back.popover, back.cardShown, back.popoverEpoch]).toEqual([false, true, popover.popoverEpoch + 1]);
    // A narrow chat is the popover too; with the docked card closed and room left the mode stays inline.
    expect((await chatCanvasView(client, native, args({ width: 900, wide: false }))).popover).toBe(true);
    playerAt(600);
    expect((await chatCanvasView(client, native, args({ detailsInline: false }))).popover).toBe(false);
  });

  test('the window opens the popover in popover mode and shows it only within the mode it opened in', async () => {
    const root = await source('app.contract');
    expect(root).toContain('derive detailsPopover = canvas.ready ? canvas.popover : conversationWidth < 984');
    expect(root).toContain('derive detailsShown = detailsOpen and detailsPopover and detailsEpoch == canvas.popoverEpoch');
    expect(root).toContain('detailsOpen = op == "toggle" ? (not detailsShown and detailsPopover) :');
    expect(root).toContain('detailsEpoch = op == "toggle" ? canvas.popoverEpoch : detailsEpoch');
    // Only the inline mode's toggle remembers the thread's docked card; the window's popover is the shown one.
    expect(root).toContain('if (op != "toggle" or not detailsPopover) and op != "editors"');
    expect(root).toContain('narrowDiff=narrowDiff, detailsOpen=detailsShown, details=details,');
    expect(root).toContain('shell.panel.deviceMini.show or shell.panel.browserMini.show, conversationWidth >= 984, detailsOpen) as shape ChatCanvasView');
    const overlays = await component('app-overlays.contract', 'WindowOverlays');
    expect(overlays).toContain('when detailsOpen and scheme == "dark"\n      ThreadDetailsPanel(details=details, density=detailsDensity,');
    expect(overlays).not.toContain('conversationWidth < 984');
    // The header's button is pressed for the presentation it toggles (selectThreadPanelOpen).
    const column = await component('app-main.contract', 'ChatColumn');
    expect(column.split('detailsOpen=(detailsOpen or (details.inline and canvas.cardShown))').length - 1).toBe(2);
    expect(column).toContain('ThreadDetailsInline(details=details, density=canvas.density,');
    // The popover's menus take its inner width, not the docked card's.
    const body = await component('shell-details.contract', 'DetailsBody');
    expect(body).toContain('R4EnvRow(details=details, inline=inline, act=act)');
    expect(await component('shell-details.contract', 'ThreadDetailsPanel')).toContain('DetailsBody(details=details, density=density, inline=false,');
    expect(await component('shell-details.contract', 'ThreadDetailsInline')).toContain('DetailsBody(details=details, density=density, inline=true,');
    const git = await component('r4-git.contract', 'R4GitRows');
    expect(git).not.toContain('details.inline');
    expect(git).toContain('width=((inline ? 262 : 246) * rem / 16)');
  });
});

describe('DP-1: the rows each density shows (ThreadDetailsPanel)', () => {
  test('full only: the Run on and workspace rows, Automations and Lineage; Open in leaves in essential', async () => {
    const body = await component('shell-details.contract', 'DetailsBody');
    const top = under(body, 'when density == "full"');
    expect(top[0]).toContain('column width="100%" hatch="t3-frame" data-frame="details-full-top"');
    const topText = top.join('\n');
    for (const row of ['R4EnvRow(', 'testId="details-workspace"', 'testId="details-folder"']) expect(topText).toContain(row);
    expect(under(body, 'when details.editorShow and density != "essential"').join('\n')).toContain('testId="details-open-editor"');
    const tail = under(body, 'data-frame="details-full-tail"').join('\n');
    expect(tail).toContain('ThreadAutomationsSection(');
    expect(tail).toContain('LineageGroups(');
    expect(under(body, 'when details.cwd != ""').join('\n')).toContain('DetailsSection(title="Version Control", separated=(density == "full"), heading=false)');
    expect(body).toContain('R4GitRows(details=details, compact=(density != "full"), inline=inline, act=act, local=local)');
    expect(under(body, 'when density == "compact"')[0]).toContain('data-frame="details-density-compact"');
    // The scripts row stays in every density; its menu and Open in's hang under the rows each density shows.
    expect(body).toContain('top=((44 + (density == "full" ? (details.envShow ? 64 : 32) : 0) + (details.editorShow and density != "essential" ? 32 : 0)) * rem / 16)');
    expect(body).toContain('top=(density != "full" ? "2.75rem" : details.envShow ? "6.75rem" : "4.75rem")');
  });

  test('compact: the Git action group shows only while an action runs or just finished; Changes always', async () => {
    const git = await component('r4-git.contract', 'R4GitRows');
    const group = under(git, 'when not compact or details.git.progress or details.git.success').join('\n');
    expect(group).toContain('role="group" aria-label="Git actions"');
    expect(group).toContain('when not details.git.progress and not details.git.success\n            box position="absolute" left=0 top=0 bottom=0 width=0 pointer-events="none" aria-hidden=true hatch="t3-frame" data-frame="details-full-git"');
    expect(group).not.toContain('testId="details-changes"');
    expect(git).toContain('testId="details-changes"');
  });
});

describe('found while driving: Float device over chat closes the right panel (DevicePanel floatActive)', () => {
  test('the window lets go of the panel it holds open, as for a floated Browser tab', async () => {
    const root = await source('app.contract');
    const rule = root.split('\n').find(line => line.includes('op == "surface-browser-float" and not shell.panel.browser.capture.floating'))!;
    expect(rule).toContain('or op == "surface-r6dev-float" or');
    const next = root.split('\n')[root.split('\n').indexOf(rule) + 1]!;
    expect(next.trim()).toBe('rightPanelAt = ""');
  });
});

describe('DR-1: the device rail menus hear the keys at a focused row (Base UI Menu)', () => {
  test('each rail menu draws its rows inside its KeyMenu', async () => {
    const workspace = await component('r6-device.contract', 'R6DeviceWorkspaceView');
    for (const [menu, rows] of [['text', ['device-text-small', 'device-text-default', 'device-text-large', 'device-text-extra-large']],
      ['rotate', ['device-rotate-portrait', 'device-rotate-landscape']], ['more', ['device-float', 'device-close-panel', 'device-power-off']]] as const) {
      const popup = under(workspace, `when ws.rail.menu == "${menu}"`);
      expect(popup[0]).toContain(`role="menu"`);
      const keyMenu = under(workspace, `KeyMenu(menuId="device-${menu}-keys"`).join('\n');
      for (const row of rows) expect(keyMenu).toMatch(new RegExp(`R6MenuItem\\(keysId="device-${menu}-keys".*testId="${row}"\\)`));
      // The KeyMenu sits inside the popup, the rows inside the KeyMenu (its `slot`).
      expect(popup[1]).toContain(`KeyMenu(menuId="device-${menu}-keys"`);
    }
  });
});
