import { describe, expect, test } from 'bun:test';
import { adoptRightPanels, restoreRightPanel, savedPanel, syncRightPanels } from './r10-device-panels';
import { keepR11, prTargetOf, restoredEffects } from './r11-device-panels';
import { prSurfaceId } from './r5-panels-pr';

// Lane r11-device: every right-panel surface persists across launches, as rightPanelStore's
// `byThreadKey` does (migratePersistedRightPanelState), not only Files, file tabs and the list.
const pr = { projectId: 'p1', host: 'github.com', repository: 'acme/app', number: 101, url: 'https://github.com/acme/app/pull/101' };
const meta = { id: 'att-1', name: 'notes.md', mimeType: 'text/markdown', sizeBytes: 120 };
const device = { hostId: 'local', deviceId: 'SIM-1', platform: 'ios', name: 'iPhone 18 Pro' };
const surfaces = () => [
  { id: 'diff', kind: 'diff' as const, path: '', line: 0, reveal: 0 },
  { id: 'files', kind: 'files' as const, path: '', line: 0, reveal: 0 },
  { id: 'device', kind: 'device' as const, path: '', line: 0, reveal: 0, device },
  { id: prSurfaceId(pr), kind: 'pull-request' as const, path: '', line: 0, reveal: 1, pr },
  { id: 'attachment:att-1', kind: 'attachment' as const, path: 'notes.md', line: 0, reveal: 0, attachment: meta },
];

describe('Right panels keep every surface across launches (lane r11-device)', () => {
  test('the Diff, the device with its target, a pull request and an attachment are saved in order', () => {
    const saved = savedPanel({ visible: true, active: prSurfaceId(pr), surfaces: surfaces() });
    expect(saved).toEqual({ visible: true, active: 'pull-request:p1:github.com:acme%2Fapp:101', surfaces: [
      { id: 'diff', kind: 'diff', path: '', line: 0 },
      { id: 'files', kind: 'files', path: '', line: 0 },
      { id: 'device', kind: 'device', path: '', line: 0, device },
      { id: 'pull-request:p1:github.com:acme%2Fapp:101', kind: 'pull-request', path: '', line: 0, pr },
      { id: 'attachment:att-1', kind: 'attachment', path: 'notes.md', line: 0, attachment: meta },
    ] });
    // A Device surface before a device was chosen is the picker: kept without a target.
    expect(keepR11({ id: 'device', kind: 'device', path: '', line: 0 })).toEqual({ id: 'device', kind: 'device', path: '', line: 0 });
  });

  test('malformed entries are dropped as the migration drops them', () => {
    expect(prTargetOf({ projectId: 'p', repository: 'a/b', number: 0 })).toBeNull();
    expect(prTargetOf({ projectId: 'p', repository: 'a/b', number: 1.5 })).toBeNull();
    expect(prTargetOf({ repository: 'a/b', number: 3 })).toBeNull();
    expect(prTargetOf({ projectId: 'p', repository: 'a/b', number: 3, host: 'GitHub.com' })).toEqual({ projectId: 'p', host: 'github.com', repository: 'a/b', number: 3, url: '' });
    expect(keepR11({ id: 'attachment:other', kind: 'attachment', path: 'x', line: 0, attachment: meta })).toBeNull();
    expect(keepR11({ id: 'diff:2', kind: 'diff', path: '', line: 0 })).toBeNull();
    // A device target with an unknown platform is not a target: the picker comes back.
    expect(keepR11({ id: 'device', kind: 'device', path: '', line: 0, device: { ...device, platform: 'tv' } })).toEqual({ id: 'device', kind: 'device', path: '', line: 0 });
    const next = {} as { rightPanels?: Record<string, unknown> };
    adoptRightPanels(next, { rightPanels: { 'env:t1': { visible: true, active: 'pull-request:x', surfaces: [{ id: 'pull-request:x', kind: 'pull-request', pr: { projectId: 'p', repository: 'a/b', number: -1 } }, { id: 'diff', kind: 'diff' }] } } });
    // The active pull request was dropped: the first survivor while open (the Diff).
    expect(next.rightPanels).toEqual({ 'env:t1': { visible: true, active: 'diff', surfaces: [{ id: 'diff', kind: 'diff', path: '', line: 0 }] } });
  });

  test('a relaunch restores them with their targets; the open Diff and the device are reapplied', () => {
    const owner = { local: {} as object, preferencesLoaded: true };
    const panels = new Map([['env:t1', { surfaces: surfaces(), active: 'diff', visible: true, userRevision: 2 }]]);
    expect(syncRightPanels(owner, panels)).toBe(true);
    const relaunched = { local: JSON.parse(JSON.stringify(owner.local)), preferencesLoaded: false, ready: true };
    const next = {} as { rightPanels?: Record<string, unknown> };
    adoptRightPanels(next, JSON.parse(JSON.stringify(relaunched.local)));
    expect(next.rightPanels).toEqual(relaunched.local.rightPanels);
    relaunched.preferencesLoaded = true;
    const fresh = { surfaces: [] as ReturnType<typeof surfaces>, active: '', visible: false, userRevision: 0 };
    expect(restoreRightPanel(relaunched, 'env:t1', fresh)).toBe(true);
    expect(fresh.active).toBe('diff');
    expect(fresh.visible).toBe(true);
    expect(fresh.surfaces.map(entry => entry.id)).toEqual(['diff', 'files', 'device', 'pull-request:p1:github.com:acme%2Fapp:101', 'attachment:att-1']);
    expect(fresh.surfaces[2]!.device).toEqual(device);
    expect(fresh.surfaces[3]!.pr).toEqual(pr);
    expect(fresh.surfaces[4]!.attachment).toEqual(meta);
    expect(fresh.surfaces.every(entry => entry.reveal === 0)).toBe(true);
    expect(restoredEffects(fresh.surfaces, fresh.active, fresh.visible)).toEqual({ diff: true, device });
    // A hidden panel or another active surface does not reopen the Diff.
    expect(restoredEffects(fresh.surfaces, fresh.active, false).diff).toBe(false);
    expect(restoredEffects(fresh.surfaces, 'files', true).diff).toBe(false);
    // Closing the device tab forgets its target with it.
    panels.get('env:t1')!.surfaces = surfaces().filter(entry => entry.kind !== 'device');
    expect(syncRightPanels(owner, panels)).toBe(true);
    expect(JSON.stringify((owner.local as { rightPanels?: object }).rightPanels)).not.toContain('SIM-1');
  });
});

describe("The Diff's preview for a project outside the server's root (DiffPanel shouldRetryBranchDiffAtEnvironmentCwd)", () => {
  test('a refused cwd is asked again at the server cwd; anything else is not retried', async () => {
    const { requestDiff } = await import('./r11-device-diff');
    const asked: string[] = [];
    const refuse = async (_method: string, payload: Record<string, unknown>) => {
      asked.push(String(payload.cwd));
      if (payload.cwd !== '/srv/root') throw new Error('Review diff preview cwd must stay within the configured workspace root.');
      return { cwd: payload.cwd, sources: [] };
    };
    const request = { method: 'review.getDiffPreview', payload: { cwd: '/elsewhere/repo', ignoreWhitespace: false } };
    expect(await requestDiff({ cwd: '/srv/root' }, request, refuse)).toEqual({ cwd: '/srv/root', sources: [] });
    expect(asked).toEqual(['/elsewhere/repo', '/srv/root']);
    // No server cwd, the same cwd, another error or a turn diff: the error stands.
    await expect(requestDiff({}, request, refuse)).rejects.toThrow('configured workspace root');
    await expect(requestDiff({ cwd: '/elsewhere/repo' }, request, refuse)).rejects.toThrow('configured workspace root');
    await expect(requestDiff({ cwd: '/srv/root' }, request, async () => { throw new Error('git failed'); })).rejects.toThrow('git failed');
    await expect(requestDiff({ cwd: '/srv/root' }, { method: 'orchestration.getTurnDiff', payload: {} }, async () => { throw new Error('configured workspace root'); })).rejects.toThrow();
  });
});
