// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): the thread details card's
// compact and essential rows (ThreadDetailsCard's density, ThreadDetailsPanel's rows per density), the card as the
// header's popover where the floating player leaves it no placement, the device rail menus' keys at a focused row.
// The canvas cases run chat-canvas-view.ts; the rows and the wiring are read from the Contract sources, as
// menu-keys.test.ts reads its menus. The behaviour is checked by driving the macOS bundle.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from './client';
import type { Native } from './protocol';
import { chatCanvasView, type ChatCanvasArgs } from './chat-canvas-view';
import { DETAILS_FRAMES, measureThreadDetailsCard, resolveThreadDetailsCardDensity } from './thread-details-card-layout';
import { floatMiniDevice, miniStoreOf } from './r6-media-device';
import { shellDetails } from './shell-details';
import { forgetDetails } from './r6-pr-actions';
import { resetPrimary, primaryAt } from './local-primary-fixture';

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

describe('ThreadDetailsCard measures its content per density', () => {
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

describe('the card becomes the header\'s popover where no placement is left', () => {
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
    expect(root).toContain('if (op != "toggle" or not detailsPopover) and op != "dismiss"');
    expect(root).toContain('narrowDiff=narrowDiff, detailsOpen=detailsShown, details=details,');
    expect(root).toContain('shell.panel.deviceMini.show or shell.panel.browserMini.show, conversationWidth >= 984, detailsOpen) as shape ChatCanvasView');
    // The popover lies in the chat column under the floating player (the reference's popover sits below the player;
    // z-index orders siblings here), not in the window's overlays.
    const column = await component('app-main.contract', 'ChatColumn');
    const popover = column.indexOf('if detailsOpen and not (narrowDiff and (rightPanel or data.diffOpen))\n        ThreadDetailsPanel(details=details, density=canvas.density, right=13,');
    expect(popover).toBeGreaterThan(0);
    expect(column.indexOf('R6DeviceMiniPlayer(')).toBeGreaterThan(popover);
    expect(column.indexOf('BrowserMiniPlayer(')).toBeGreaterThan(popover);
    // Over a right panel sheet (a narrow window) the window draws it, above the sheet, as the base did.
    const overlays = await component('app-overlays.contract', 'WindowOverlays');
    expect(overlays).toContain('if detailsSheet\n      ThreadDetailsPanel(details=details, density=detailsDensity, right=13,');
    const window = await component('app-window.contract', 'T3Window');
    expect(window).toContain('derive detailsSheet = detailsOpen and narrowDiff and (rightPanel or shell.panel.open or data.diffOpen)');
    expect(window).toContain('detailsSheet=detailsSheet, detailsDensity=canvas.density');
    const layer = await component('shell-details.contract', 'ThreadDetailsPanel');
    expect(layer).toContain('z-index=35 testId="thread-details-layer"');
    expect(await component('r6-device.contract', 'R6DeviceMiniPlayer')).toContain('z-index=40 pointerdown=outside pointerup=outside role="region" aria-label="Floating device preview"');
    // Outside presses close it, as Base UI's: the backdrop takes the column's presses and, `below` the column, the terminal
    // drawer's; the floating player, which lies above the backdrop, closes it on a primary press of its own (detailsPlayerPointer) (its pill buttons,
    // its screen, its resize edges); the window on a press beside the chat column (the sidebar, the right panel). A drag
    // on the player closes it too, as its gesture starts. Over a right panel sheet the window's popover has a window-wide
    // backdrop, so the window's rule stands aside.
    expect(root).toContain('    canvasGestureValue = value\n    detailsOpen = detailsOpen and value == ""');
    const press = under(window, 'action outsidePressDown(e: PointerEvent)').join('\n');
    expect(press).toContain('    if detailsOpen and not detailsSheet and e.buttons == 1 and (e.clientX < (data.sidebarOpen ? sidebarWidth : 0) or e.clientX >= (data.sidebarOpen ? sidebarWidth : 0) + conversationWidth)\n      titleMenuOpen = false\n      detailsActRoot("dismiss", "", "")');
    // The right panel's details button is drawn only on a sheet, under the window's backdrop: no exception names it.
    expect(press).not.toContain('panel-toggle-details');
    for (const file of ['r4-surfaces.contract', 'shell-panels.contract']) expect(await source(file)).not.toContain('id="panel-toggle-details"');
    expect(column).toContain('R6DeviceMiniPlayer(mini=shell.panel.deviceMini, frame=canvas.mini, still=viewport.prefersReducedMotion, local=chatLocal, gesture=canvasGesture, outside=detailsOutside)');
    expect(column).toContain('BrowserMiniPlayer(mini=shell.panel.browserMini, frame=canvas.mini, canvasWidth=canvas.containerWidth, local=chatLocal, gesture=canvasGesture, outside=detailsOutside)');
    expect(window).toContain('canvas=canvas, canvasGesture=canvasGesture, detailsOutside=detailsPlayerPointer, ');
    // It closes as the press ends (armed at the primary button's down), so a pill button's own press still lands.
    // A skill chip's details close on it too (a citation's comment settles, as the window's outside press): the player
    // takes the press from the window's `main`.
    expect(under(window, 'action detailsPlayerPointer(e: PointerEvent)').join('\n')).toBe('    if e.buttons == 1\n      detailsPlayerArmed = true\n    else\n      if detailsPlayerArmed and detailsOpen\n        detailsAct("dismiss", "", "")\n      if detailsPlayerArmed and chipShown\n        if chip.kind == "citation"\n          citeSettle()\n        else\n          chipClose(false)\n      detailsPlayerArmed = false');
    expect(await component('r6-device.contract', 'R6DeviceMiniPlayer')).toContain('z-index=40 pointerdown=outside pointerup=outside role="region" aria-label="Floating device preview"');
    expect(await component('browser-capture.contract', 'BrowserMiniPlayer')).toContain('z-index=40 hover=overPlayer pointermove=movePlayer pointerdown=outside pointerup=outside role="region" aria-label="Floating browser preview"');
    expect(root).toContain('op != "ui:changes" and op != "dismiss")');
    // The column's z-index lays it, and so the popover, over the terminal drawer under it (the reference's popover is
    // portaled over the drawer), at the reference's height (100dvh − 6.5rem); its backdrop reaches the window's foot.
    expect(column).toContain('    column flex=1 min-width=0 min-height=0 height="100%" position="relative" z-index=1 inert=pageCover hatch="t3-frame" data-frame="chat"');
    expect(column.split('ThreadDetailsPanel(details=details, density=canvas.density, right=13, below=(canvas.ready ? max(0, viewport.height - 52 - canvas.containerHeight) : 0), maxHeight=(viewport.height - 104),').length - 1).toBe(1);
    expect(overlays.split('ThreadDetailsPanel(details=details, density=detailsDensity, right=13, below=0,').length - 1).toBe(1);
    expect(layer).toContain('box position="absolute" left=0 top=0 right=0 bottom=(0 - below) z-index=35 testId="thread-details-layer"');
    const stack = under(window, 'column flex=1 min-width=0 height="100%" // the drawer sits below the chat column');
    expect([stack.findIndex(line => line.includes('ChatColumn(')), stack.findIndex(line => line.includes('TerminalDrawer('))]).toEqual([0, 1]);
    // The header's button is pressed for the presentation it toggles (selectThreadPanelOpen).
    expect(column.split('detailsOpen=(detailsOpen or (details.inline and canvas.cardShown))').length - 1).toBe(1);
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

describe('the pull request rows place their cards for the presentation shown', () => {
  afterEach(resetPrimary);
  const URL104 = 'https://github.com/t3-fixture/pr-demo/pull/104';
  const github = { provider: 'github', canonicalKey: 'github.com/t3-fixture/pr-demo', displayName: 't3-fixture/pr-demo', owner: 't3-fixture', name: 'pr-demo', locator: { remoteUrl: 'https://github.com/t3-fixture/pr-demo.git' } };
  const checks = ['build', 'test', 'lint'].map(name => ({ name, status: 'success', description: null, url: `https://github.com/t3-fixture/pr-demo/actions/runs/${name.length}` }));
  const detail = { provider: 'github', projectId: 'p1', projectTitle: 'pr-demo', workspaceRoot: '/repos/pr-demo', repository: 't3-fixture/pr-demo', number: 104, title: 'Update README badges', body: '',
    url: URL104, state: 'open', isDraft: false, mergeability: 'mergeable', additions: 24, deletions: 6, changedFiles: 3, headBranch: 'feature/clean', baseBranch: 'main',
    capabilities: { actions: ['merge', 'ready', 'draft', 'close'], mergeMethods: ['merge', 'squash', 'rebase'] }, viewerPermissions: { actions: ['merge', 'ready', 'draft', 'close'] },
    mergeCapabilities: { merge: true, squash: true, rebase: true }, checks };
  test('an open popover on a wide chat (the player took the docked card\'s place) lays its rows out for the popover', async () => {
    primaryAt('http://127.0.0.1:3773', 'env');
    const native = { available: true, watch() {}, later: async () => ({}) } as unknown as Native;
    const prClient = { ready: true, projectId: 'p1', threadId: 't1', environmentId: 'env', origin: 'http://127.0.0.1:3773', local: { composerControls: { contexts: {} } }, draftKey: 'env:t1', generation: 1, revision: 0,
      config: { availableEditors: [], environment: { label: 'Local', capabilities: { pullRequests: true, threadPullRequests: true, pullRequestChecks: true } }, providers: [] },
      projection: { thread: { id: 't1', lineage: {} }, runs: [], subagents: [], contextTransfers: [] },
      shell: { projects: [{ id: 'p1', title: 'pr-demo', workspaceRoot: '/repos/pr-demo', repositoryIdentity: github }],
        threads: [{ id: 't1', projectId: 'p1', title: 'T t1', status: 'idle', createdAt: '2026-10-04T10:00:00.000Z', branch: 'main', lineage: { parentThreadId: null, relationshipToParent: null },
          pullRequests: [{ host: 'github.com', repository: 't3-fixture/pr-demo', number: 104, url: URL104, source: 'manual', linkedAt: '2026-10-04T10:00:00.000Z',
            snapshot: { title: 'Update README badges', state: 'open', isDraft: false, syncedAt: '2026-10-04T10:00:00.000Z' }, watch: null }] }] },
      rpc: async (_native: unknown, method: string) => method === 'pullRequests.detail' ? detail : method === 'pullRequests.checks' ? { state: 'open', checks } : {},
      restAccess: () => ({ call: async (request: { op?: string }) => (request.op === 'subscribe' ? { id: 'sub-1' } : {}) }),
    } as unknown as T3Client;
    // The docked card (open by default on a wide chat): 262 pt rows 12 pt in; the trigger ends 83.3 pt from the window's edge.
    const docked = (await shellDetails(prClient, native, false, 't1', true, 1, 0)).prRows.rows[0]!;
    expect([docked.r6.tail, docked.r6.card.maxWidth]).toEqual([83.3, 262 + 8]);
    // The popover shown on the same wide chat: 246 pt rows 13 pt in, as on a narrow one.
    const shown = (await shellDetails(prClient, native, true, 't1', true, 1, 0)).prRows.rows[0]!;
    const narrow = (await shellDetails(prClient, native, true, 't1', false, 1, 0)).prRows.rows[0]!;
    expect([shown.r6.tail, shown.r6.card.maxWidth]).toEqual([84.3, 246 + 8]);
    expect(shown.r6).toEqual(narrow.r6);
    forgetDetails(prClient);
  });
  test('the window passes the popover shown, not the toggle it remembers; the canvas reads nothing of the details', async () => {
    const root = await source('app.contract');
    expect(root).toContain('  resource details = shellDetails(detailsShown, data.threadId,');
    expect(await source('shell-details.ts')).toContain('rightGap, inline: inline && !open }),');
    // detailsShown reads the canvas's mode, so a canvas that read `details` would close a cycle the runner refuses at the
    // toggle ("press … (detailsAct#1) refused: Cycle", found in a drive); the docked card's state is its own.
    const canvas = root.split('\n').find(line => line.startsWith('  resource canvas = chatCanvas('))!;
    expect(canvas).not.toContain('details.');
    expect(canvas).toContain('chatCanvas(data.revision, conversationWidth, viewport.height, conversationWidth >= 984 and data.projectId != "", data.look.chatMax,');
  });
});

describe('the rows each density shows (ThreadDetailsPanel)', () => {
  test('full only: the Run on and workspace rows, Automations and Lineage; Open in leaves in essential', async () => {
    const body = await component('shell-details.contract', 'DetailsBody');
    const top = under(body, 'if density == "full"');
    expect(top[0]).toContain('column width="100%" hatch="t3-frame" data-frame="details-full-top"');
    const topText = top.join('\n');
    for (const row of ['R4EnvRow(', 'testId="details-workspace"', 'testId="details-folder"']) expect(topText).toContain(row);
    expect(under(body, 'if details.editorShow and density != "essential"').join('\n')).toContain('OpenInRow(details=details, menuWidth=((inline ? 262 : 246) * rem / 16), act=act)');
    const tail = under(body, 'data-frame="details-full-tail"').join('\n');
    expect(tail).toContain('ThreadAutomationsSection(');
    expect(tail).toContain('LineageGroups(');
    expect(under(body, 'if details.cwd != ""').join('\n')).toContain('DetailsSection(title="Version Control", separated=(density == "full"), heading=false)');
    expect(body).toContain('R4GitRows(details=details, compact=(density != "full"), inline=inline, act=act, local=local)');
    expect(under(body, 'if density == "compact"')[0]).toContain('data-frame="details-density-compact"');
    // The scripts and Open in rows' menus are popovers anchored at their chevrons, as wide as the row.
    expect(body).toContain('R6ScriptRow(details=details, menuWidth=((inline ? 262 : 246) * rem / 16)');
    expect(body).not.toContain('details-editors');
  });

  test('compact: the Git action group shows only while an action runs or just finished; Changes always', async () => {
    const git = await component('r4-git.contract', 'R4GitRows');
    const group = under(git, 'if not compact or details.git.progress or details.git.success').join('\n');
    expect(group).toContain('role="group" aria-label="Git actions"');
    expect(group).toContain('if not details.git.progress and not details.git.success\n            box position="absolute" left=0 top=0 bottom=0 width=0 pointer-events="none" aria-hidden=true hatch="t3-frame" data-frame="details-full-git"');
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

describe('the device rail menus hear the keys at a focused row (Base UI Menu)', () => {
  test('each rail menu is a popover beside its trigger, its rows inside its KeyMenu', async () => {
    const workspace = await component('r6-device.contract', 'R6DeviceWorkspaceView');
    const button = await component('r6-device.contract', 'R6RailMenuButton');
    // The KeyMenu sits inside the popup, the rows inside the KeyMenu (the button's `slot`).
    expect(button).toContain('        column width="100%" padding="0.25rem"');
    expect(button).toContain('role="menu" aria-label=menuLabel testId=menuTestId\n          KeyMenu(menuId=keysId, items=items, keyed=0, gap="0px", modal=true)\n            children');
    // DeviceControlsRail's MenuPopups: side left, the text size and rotate menus centred, More end-aligned.
    for (const [menu, area, rows] of [['text', 'left', ['device-text-small', 'device-text-default', 'device-text-large', 'device-text-extra-large']],
      ['rotate', 'left', ['device-rotate-portrait', 'device-rotate-landscape']], ['more', 'left span-top', ['device-float', 'device-close-panel', 'device-power-off']]] as const) {
      const call = workspace.split('\n').find(line => line.includes(`menuId="device-${menu}-popup"`) && line.includes('R6RailMenuButton('))!;
      expect(call).toContain(`area="${area}", keysId="device-${menu}-keys"`);
      const rowsUnder = under(workspace, call.trim()).join('\n');
      for (const row of rows) expect(rowsUnder).toMatch(new RegExp(`R6MenuItem\\(keysId="device-${menu}-keys", menuId="device-${menu}-popup".*testId="${row}"\\)`));
    }
  });
});

