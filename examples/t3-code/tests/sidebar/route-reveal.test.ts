// Ported from T3 Code 7c3b3b3605ef apps/web/src/components/Sidebar.logic.test.ts (pendingSidebarRouteReveal,
// resolveSidebarRouteReveal) and sidebar/routeReveal.browser.test.tsx (MIT, see LICENSE-T3). exact2: '' stands for
// the reference's null key; the scroll itself is the view's marker (sidebar.contract revealDue), so these read the
// snapshot's revealRow / revealPending.
import { describe, expect, it } from 'bun:test';
import type { Obj } from '../../src/client/domain';
import type { T3Client } from '../../src/client/client';
import type { Native } from '../../src/client/protocol';
import { completeRouteReveal, pendingRouteReveal, resolveRouteReveal } from '../../src/sidebar/route-reveal';
import { sidebarSnapshot } from '../../src/sidebar/sidebar-view';
import { sidebarLocal } from '../../src/sidebar/sidebar-commands';
import { sidebarPrefs, sidebarSession } from '../../src/sidebar/sidebar-state';

describe('pendingRouteReveal', () => {
  it('keeps a revealed thread settled across a frame with no thread open', () => {
    const reveal = { observedKey: '', pendingKey: '' };
    expect(pendingRouteReveal(reveal, 'env:a1')).toBe('env:a1');
    completeRouteReveal(reveal, 'env:a1');
    expect(pendingRouteReveal(reveal, '')).toBe('');
    expect(pendingRouteReveal(reveal, 'env:a1')).toBe('');
    expect(pendingRouteReveal(reveal, 'env:b1')).toBe('env:b1');
  });
  it('keeps a pending reveal through a frame with no thread open', () => {
    const reveal = { observedKey: '', pendingKey: '' };
    pendingRouteReveal(reveal, 'env:a1');
    expect(pendingRouteReveal(reveal, '')).toBe('');
    expect(pendingRouteReveal(reveal, 'env:a1')).toBe('env:a1');
  });
});

describe('resolveRouteReveal', () => {
  const hidden = (key: string) => key === 'env:hidden' ? true : undefined;
  it('widens the scope for a newly opened thread the scope hides', () => {
    expect(resolveRouteReveal({ reveal: { observedKey: 'env:other', pendingKey: '' }, routeKey: 'env:hidden', searching: false, scoped: true, hidden }))
      .toEqual({ pendingKey: 'env:hidden', widensScope: true });
  });
  it('never widens for a thread whose reveal already completed', () => {
    expect(resolveRouteReveal({ reveal: { observedKey: 'env:hidden', pendingKey: '' }, routeKey: 'env:hidden', searching: false, scoped: true, hidden }))
      .toEqual({ pendingKey: '', widensScope: false });
  });
  it('widens again for a thread reopened after one whose row never listed', () => {
    const reveal = { observedKey: 'env:hidden', pendingKey: '' };
    resolveRouteReveal({ reveal, routeKey: 'env:unlisted', searching: false, scoped: true, hidden });
    expect(resolveRouteReveal({ reveal, routeKey: 'env:hidden', searching: false, scoped: true, hidden })).toEqual({ pendingKey: 'env:hidden', widensScope: true });
  });
  it('holds a pending reveal without widening while searching', () => {
    expect(resolveRouteReveal({ reveal: { observedKey: 'env:hidden', pendingKey: 'env:hidden' }, routeKey: 'env:hidden', searching: true, scoped: true, hidden }))
      .toEqual({ pendingKey: 'env:hidden', widensScope: false });
  });
});

const helpers = { projectIdentity: () => ({ projectMark: '', projectInk: '', projectSurface: '' }), providerBadge: () => ({ providerBadge: '', providerBadgeColor: '' }) };
const native = { available: true, watch() {}, later: async () => ({ ok: true, generation: 0, value: {} }) } as Native;
const NOW = Date.parse('2026-10-27T12:00:00.000Z');
function shell(id: string, projectId: string, extra: Obj = {}): Obj {
  return { id, projectId, title: `Thread ${id}`, createdAt: '2026-10-27T10:00:00.000Z', updatedAt: '2026-10-27T10:00:00.000Z', latestRunId: null, status: 'idle',
    archivedAt: null, deletedAt: null, lineage: { relationshipToParent: null }, ...extra };
}
function window(threads: Obj[], extra: Obj = {}) {
  return {
    shell: { projects: [{ id: 'pa', title: 'Alpha', workspaceRoot: '/a' }, { id: 'pb', title: 'Beta', workspaceRoot: '/b' }], threads, sequence: 1 },
    config: { environment: { capabilities: {} }, providers: [], keybindings: [] }, environmentId: 'env', threadId: '', projectId: '', query: '', presentation: {},
    local: { drafts: {}, clientSettings: {}, deviceSettings: {} },
    projectGroups: () => [{ key: 'ga', name: 'Alpha', members: [{ id: 'pa', title: 'Alpha', workspaceRoot: '/a' }] }, { key: 'gb', name: 'Beta', members: [{ id: 'pb', title: 'Beta', workspaceRoot: '/b' }] }],
    ...extra,
  } as unknown as T3Client;
}

describe('the sidebar\'s route reveal', () => {
  it('reveals the open row on a route change, once; the view\'s mount reveal reads revealRow', async () => {
    const client = window([shell('a', 'pa'), shell('b', 'pa')]);
    (client as unknown as { threadId: string }).threadId = 'a';
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ revealRow: 'a', revealPending: 'a' });
    await sidebarLocal(client, native, 'reveal-done', 'a', '');
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ revealRow: 'a', revealPending: '' });
    (client as unknown as { threadId: string }).threadId = 'b';
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ revealRow: 'b', revealPending: 'b' });
  });

  it('a thread the project filter hides resets it to All projects and clears the selection; a mount alone keeps the filter', async () => {
    const client = window([shell('a', 'pa'), shell('b', 'pb')]);
    (client as unknown as { threadId: string }).threadId = 'a';
    sidebarSnapshot(client, NOW, helpers);
    await sidebarLocal(client, native, 'reveal-done', 'a', '');
    sidebarPrefs(client).scopes = ['ga'];
    sidebarSession(client).selection = ['a'];
    // The same route under the chosen scope: nothing widens.
    expect(sidebarSnapshot(client, NOW, helpers).sidebar.scopeKey).toBe('ga');
    // ⌘K opens a Beta thread: the first frame already lists it under All projects.
    (client as unknown as { threadId: string }).threadId = 'b';
    const view = sidebarSnapshot(client, NOW, helpers);
    expect(view.sidebar).toMatchObject({ scopeKey: '', revealRow: 'b', revealPending: 'b', selectionCount: 0 });
    expect(view.threads.map(row => row.id)).toContain('b');
    expect(sidebarPrefs(client).scopes).toEqual([]);
  });

  it('search defers the reveal; the row reveals when the list returns', () => {
    const client = window([shell('a', 'pa'), shell('b', 'pb')]);
    (client as unknown as { query: string }).query = 'Thread';
    (client as unknown as { threadId: string }).threadId = 'b';
    sidebarPrefs(client).scopes = ['ga'];
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ revealRow: '', revealPending: '', scopeKey: 'ga' });
    (client as unknown as { query: string }).query = '';
    expect(sidebarSnapshot(client, NOW, helpers).sidebar).toMatchObject({ revealRow: 'b', revealPending: 'b', scopeKey: '' });
  });
});

describe('a saved legacy sidebar opt-in', () => {
  it('opens the default sidebar: its rows, and its reveal of the open row', () => {
    // v0.1.5 removed the legacy sidebar; the setting it saved is still read there, but the clone reads nothing from it.
    const client = window([shell('a', 'pa'), shell('b', 'pb')], { threadId: 'b' });
    Object.assign(client.local.clientSettings!, { legacySidebarEnabled: true, sidebarThreadPreviewCount: 3 });
    const view = sidebarSnapshot(client, NOW, helpers);
    expect(view.threads.map(row => row.id).sort()).toEqual(['a', 'b']);
    expect(view.sidebar).toMatchObject({ revealRow: 'b', revealPending: 'b' });
  });
});
