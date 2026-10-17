// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): the thread details card's
// compact and essential rows (ThreadDetailsCard's density, ThreadDetailsPanel's rows per density), the card as the
// header's popover where the floating player leaves it no placement, the device rail menus' keys at a focused row.
// The canvas cases run chat-canvas-view.ts; the rows and the wiring are read from the Contract sources, as
// menu-keys.test.ts reads its menus. The behaviour is checked by driving the macOS bundle.
import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { chatCanvasView, type ChatCanvasArgs } from '../../src/shell/chat-canvas-view';
import { DETAILS_FRAMES, measureThreadDetailsCard, resolveThreadDetailsCardDensity } from '../../src/details/thread-details-card-layout';
import { floatMiniDevice, miniStoreOf } from '../../src/device/device-session';
import { shellDetails } from '../../src/details/shell-details';
import { forgetDetails } from '../../src/details/pr-row-actions';
import { resetPrimary, primaryAt } from '../support/local-primary-fixture';

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
    expect([docked.extra.tail, docked.extra.card.maxWidth]).toEqual([83.3, 262 + 8]);
    // The popover shown on the same wide chat: 246 pt rows 13 pt in, as on a narrow one.
    const shown = (await shellDetails(prClient, native, true, 't1', true, 1, 0)).prRows.rows[0]!;
    const narrow = (await shellDetails(prClient, native, true, 't1', false, 1, 0)).prRows.rows[0]!;
    expect([shown.extra.tail, shown.extra.card.maxWidth]).toEqual([84.3, 246 + 8]);
    expect(shown.extra).toEqual(narrow.extra);
    forgetDetails(prClient);
  });
});
