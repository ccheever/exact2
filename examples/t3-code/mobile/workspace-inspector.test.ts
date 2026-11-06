import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { workspaceInspectorFocusOwner, workspaceInspectorSnapshot, workspaceInspectorTransition,
  type WorkspaceInspectorFocus, type WorkspaceInspectorInput } from './workspace-inspector';

const chat: WorkspaceInspectorFocus = { routeId: '7', kind: 'thread', environmentId: 'env', threadId: 'one',
  generation: 3, cwd: '/repo', selectedPath: '', renderable: true, candidateSupported: true };
const files: WorkspaceInspectorFocus = { ...chat, routeId: '8', kind: 'files' };
const overlay: WorkspaceInspectorFocus = { ...chat, routeId: '9', kind: 'other', renderable: false };
function input(focus: WorkspaceInspectorFocus | null = chat, now = 0, extra: Partial<WorkspaceInspectorInput> = {}): WorkspaceInspectorInput {
  return { focus, now, liveRouteIds: ['7', '8', '9'], reducedMotion: false, columnSupported: true, columnWidth: 320, resizing: false, ...extra };
}
function open(mode: 'files' | 'git' = 'files', now = 0) {
  return workspaceInspectorTransition('', input(chat, now), { kind: 'mode', owner: workspaceInspectorFocusOwner(chat), mode });
}
const registration = (value: { registrationJSON: string }) => JSON.parse(value.registrationJSON);

test('plain chat has a focus role but no invented content; committed snapshot is inert', () => {
  const first = workspaceInspectorTransition('', input());
  expect(first).toMatchObject({ mode: '', active: false, mounted: false, registrationJSON: '', revealInspector: false });
  expect(JSON.parse(first.roleJSON)).toMatchObject({ routeId: '7', value: 'inspector' });
  expect(workspaceInspectorTransition(first.serialized, input(chat, 10000))).toMatchObject({ serialized: first.serialized, changed: false });
  const { changed, revealInspector, ...snapshot } = first;
  expect(changed).toBe(true);
  expect(revealInspector).toBe(false);
  expect(workspaceInspectorSnapshot(first.serialized)).toEqual(snapshot);
});

test('Files opens only for its captured available workspace and has a350ms alternate deadline', () => {
  const opened = open('files', 20);
  expect(opened).toMatchObject({ mode: 'files', contentMode: 'files', mountedFiles: true, mountedGit: false, prewarmAt: 370,
    revealInspector: true, contentEnvironmentId: 'env', contentThreadId: 'one', contentGeneration: 3, cwd: '/repo' });
  expect(registration(opened)).toMatchObject({ kind: 'files', active: true, threadId: 'one' });
  const changed = { ...chat, threadId: 'two' };
  expect(workspaceInspectorTransition('', input(changed), { kind: 'mode', owner: opened.focusOwner, mode: 'files' }).registrationJSON).toBe('');
  const compact = { ...chat, candidateSupported: false };
  expect(workspaceInspectorTransition('', input(compact), { kind: 'mode', owner: workspaceInspectorFocusOwner(compact), mode: 'files' }).revealInspector).toBe(false);
  const noCwd = { ...chat, cwd: '' };
  expect(workspaceInspectorTransition('', input(noCwd), { kind: 'mode', owner: workspaceInspectorFocusOwner(noCwd), mode: 'files' }).registrationJSON).toBe('');
});

test('prewarm uses explicit time and token; same-mode content refresh does not restart it', () => {
  const opened = open('files', 20);
  const early = workspaceInspectorTransition(opened.serialized, input(chat, 369), { kind: 'deadline', token: opened.prewarmToken });
  expect(early.mountedGit).toBe(false);
  const otherCwd = { ...chat, cwd: '/other' };
  const refreshed = workspaceInspectorTransition(early.serialized, input(otherCwd, 300));
  expect(refreshed.prewarmToken).toBe(opened.prewarmToken);
  expect(refreshed.prewarmAt).toBe(370);
  expect(refreshed.contentOwner).not.toBe(opened.contentOwner);
  const warmed = workspaceInspectorTransition(refreshed.serialized, input(otherCwd, 370), { kind: 'deadline', token: opened.prewarmToken });
  expect(warmed).toMatchObject({ mountedFiles: true, mountedGit: true, prewarmToken: '', prewarmAt: 0 });
  expect(workspaceInspectorSnapshot(warmed.serialized).serialized).toBe(warmed.serialized);
});

test('switching Files/Git retains visited content and makes the old prewarm callback inert', () => {
  const opened = open('files');
  const git = workspaceInspectorTransition(opened.serialized, input(chat, 100), { kind: 'mode', owner: opened.focusOwner, mode: 'git' });
  expect(git).toMatchObject({ mode: 'git', mountedFiles: true, mountedGit: true, prewarmToken: '', revealInspector: true });
  const late = workspaceInspectorTransition(git.serialized, input(chat, 400), { kind: 'deadline', token: opened.prewarmToken });
  expect(late.serialized).toBe(git.serialized);
});

test('plain Thread blur clears its mode and independently releases role while retaining captured content260ms', () => {
  const opened = open('git');
  const blurred = workspaceInspectorTransition(opened.serialized, input(overlay, 100));
  expect(blurred).toMatchObject({ mode: '', roleJSON: '', contentMode: 'git', contentThreadId: 'one', active: false,
    mounted: true, exitAt: 360 });
  expect(registration(blurred).token).toBe(registration(opened).token);
  const early = workspaceInspectorTransition(blurred.serialized, input(overlay, 359), { kind: 'deadline', token: blurred.exitToken });
  expect(early.mounted).toBe(true);
  const closed = workspaceInspectorTransition(early.serialized, input(overlay, 360), { kind: 'deadline', token: blurred.exitToken });
  expect(closed).toMatchObject({ registrationJSON: '', mounted: false, exitToken: '', prewarmToken: '' });
  const returned = workspaceInspectorTransition(closed.serialized, input(chat, 500));
  expect(returned).toMatchObject({ mode: '', registrationJSON: '', revealInspector: false });
});

test('replacement content wins over old deactivation, role release and native exit completions', () => {
  const opened = open();
  const oldRole = JSON.parse(opened.roleJSON).token;
  const blurred = workspaceInspectorTransition(opened.serialized, input(overlay, 50));
  const replaced = workspaceInspectorTransition(blurred.serialized, input(files, 70));
  expect(replaced).toMatchObject({ active: true, mode: 'route', contentMode: 'route', mountedRoute: true, exitToken: '', prewarmToken: '' });
  for (const event of [{ kind: 'deactivate', token: registration(opened).token }, { kind: 'release-role', token: oldRole },
    { kind: 'end-exit', token: blurred.exitToken }, { kind: 'deadline', token: blurred.exitToken }] as const) {
    expect(workspaceInspectorTransition(replaced.serialized, input(files, 1000), event).serialized).toBe(replaced.serialized);
  }
});

test('native completion needs current inactive exit token, including reenter then exit again', () => {
  const opened = open();
  expect(workspaceInspectorTransition(opened.serialized, input(), { kind: 'end-exit', token: registration(opened).token }).mounted).toBe(true);
  const firstExit = workspaceInspectorTransition(opened.serialized, input(overlay, 10));
  const returned = workspaceInspectorTransition(firstExit.serialized, input(chat, 20));
  const reopened = workspaceInspectorTransition(returned.serialized, input(chat, 30), { kind: 'mode', owner: returned.focusOwner, mode: 'files' });
  const secondExit = workspaceInspectorTransition(reopened.serialized, input(overlay, 40));
  expect(secondExit.exitToken).not.toBe(firstExit.exitToken);
  const stale = workspaceInspectorTransition(secondExit.serialized, input(overlay, 50), { kind: 'end-exit', token: firstExit.exitToken });
  expect(stale.mounted).toBe(true);
  expect(workspaceInspectorTransition(stale.serialized, input(overlay, 50), { kind: 'end-exit', token: secondExit.exitToken }).mounted).toBe(false);
});

test('role release is independent of content release and remains released until new focus', () => {
  const opened = open();
  const released = workspaceInspectorTransition(opened.serialized, input(), { kind: 'release-role', token: JSON.parse(opened.roleJSON).token });
  expect(released).toMatchObject({ roleJSON: '', active: true });
  expect(workspaceInspectorTransition(released.serialized, input(chat, 100)).roleJSON).toBe('');
  const deactivated = workspaceInspectorTransition(released.serialized, input(chat, 100), { kind: 'deactivate', token: registration(opened).token });
  expect(deactivated).toMatchObject({ active: false, exitAt: 360 });
  expect(workspaceInspectorTransition(deactivated.serialized, input(chat, 101)).active).toBe(false);
});

test('route-owned Files mode survives blur; route mode never prewarms and automatic reveal happens once per visit', () => {
  const opened = workspaceInspectorTransition('', input(files));
  expect(opened).toMatchObject({ contentMode: 'route', mountedRoute: true, mountedFiles: false, prewarmToken: '', revealInspector: true });
  const git = workspaceInspectorTransition(opened.serialized, input(files, 50), { kind: 'mode', owner: opened.focusOwner, mode: 'git' });
  const routeAgain = workspaceInspectorTransition(git.serialized, input(files, 60), { kind: 'mode', owner: git.focusOwner, mode: 'files' });
  expect(routeAgain).toMatchObject({ mode: 'route', contentMode: 'route', prewarmToken: '', revealInspector: true });
  const blurred = workspaceInspectorTransition(git.serialized, input(overlay, 100));
  const closed = workspaceInspectorTransition(blurred.serialized, input(overlay, 400), { kind: 'deadline', token: blurred.exitToken });
  const returned = workspaceInspectorTransition(closed.serialized, input(files, 500));
  expect(returned).toMatchObject({ mode: 'git', contentMode: 'git', mountedGit: true, mountedFiles: false, revealInspector: false, prewarmAt: 850 });
  const popped = workspaceInspectorTransition(returned.serialized, input(chat, 600, { liveRouteIds: ['7'] }));
  const newVisit = workspaceInspectorTransition(popped.serialized, input(files, 700));
  expect(newVisit).toMatchObject({ mode: 'route', revealInspector: true });
});

test('unsupported column unmounts active renderer while retaining its lease, then remounts fresh', () => {
  const opened = open();
  const warmed = workspaceInspectorTransition(opened.serialized, input(chat, 350), { kind: 'deadline', token: opened.prewarmToken });
  const compact = workspaceInspectorTransition(warmed.serialized, input(chat, 400, { columnSupported: false }));
  expect(compact).toMatchObject({ mode: 'files', active: true, mounted: false, exitToken: '', prewarmToken: '' });
  const wide = workspaceInspectorTransition(compact.serialized, input(chat, 500));
  expect(wide).toMatchObject({ mountedFiles: true, mountedGit: false, prewarmAt: 850 });
  const narrowOverlay = workspaceInspectorTransition(wide.serialized, input(overlay, 550, { columnSupported: false }));
  expect(narrowOverlay).toMatchObject({ mode: '', active: false, mounted: false, exitAt: 810 });
  expect(narrowOverlay.contentOwner).toBe(wide.contentOwner);
});

test('a missing cwd suppresses Files without clearing stored selection; new identity rejects old mode action', () => {
  const opened = open();
  const missing = { ...chat, cwd: '' };
  const suppressed = workspaceInspectorTransition(opened.serialized, input(missing, 10));
  expect(suppressed).toMatchObject({ mode: '', active: false });
  const changed = { ...chat, environmentId: 'env2', threadId: 'two', generation: 4, cwd: '/new' };
  const renewed = workspaceInspectorTransition(suppressed.serialized, input(changed, 20), { kind: 'mode', owner: opened.focusOwner, mode: 'git' });
  expect(renewed).toMatchObject({ mode: 'files', contentThreadId: 'two', contentEnvironmentId: 'env2', contentGeneration: 4 });
});

test('file-to-file replacement has a fresh content owner with no exit; file routes do not accept chat mode events', () => {
  const file = { ...files, kind: 'file' as const, selectedPath: 'one.ts' };
  const first = workspaceInspectorTransition('', input(file));
  const next = workspaceInspectorTransition(first.serialized, input({ ...file, selectedPath: 'two.ts' }, 100));
  expect(next).toMatchObject({ kind: 'files', active: true, selectedPath: 'two.ts', exitToken: '', prewarmToken: '', revealInspector: false });
  expect(registration(next).token).not.toBe(registration(first).token);
  expect(workspaceInspectorTransition(next.serialized, input({ ...file, selectedPath: 'two.ts' }), { kind: 'mode', owner: next.focusOwner, mode: 'git' }).contentMode).toBe('route');
});

test('Review role/reveal does not invent content while diff is loading or raw', () => {
  const review = { ...files, kind: 'review' as const, renderable: false };
  const loading = workspaceInspectorTransition('', input(review));
  expect(loading).toMatchObject({ registrationJSON: '', revealInspector: true });
  expect(workspaceInspectorTransition(loading.serialized, input(review, 100)).revealInspector).toBe(false);
  const ready = workspaceInspectorTransition(loading.serialized, input({ ...review, renderable: true }, 100));
  expect(ready).toMatchObject({ kind: 'changed-files', mountedRoute: true, prewarmToken: '', revealInspector: false });
});

test('reduced motion makes fallback exit immediately due; projection and malformed input never allocate async work', () => {
  const opened = open();
  const exit = workspaceInspectorTransition(opened.serialized, input(overlay, 100, { reducedMotion: true }));
  expect(exit.exitAt).toBe(100);
  expect(workspaceInspectorTransition(exit.serialized, input(overlay, 100, { reducedMotion: true }), { kind: 'deadline', token: exit.exitToken }).mounted).toBe(false);
  expect(workspaceInspectorSnapshot('not JSON')).toMatchObject({ registrationJSON: '', roleJSON: '', exitToken: '', prewarmToken: '' });
});


test('an unsupported outgoing lease remounts and restarts the source width-dependent closure', () => {
  const opened = open();
  const blurred = workspaceInspectorTransition(opened.serialized, input(overlay, 100, { columnSupported: false }));
  expect(blurred).toMatchObject({ active: false, mounted: false, contentMode: 'files', exitAt: 360, mountedFiles: false, prewarmToken: '' });
  const resized = workspaceInspectorTransition(blurred.serialized, input(overlay, 340));
  expect(resized).toMatchObject({ active: false, mounted: true, mountedFiles: true, mountedGit: false, exitAt: 600 });
  expect(resized.exitToken).not.toBe(blurred.exitToken);
  const stale = workspaceInspectorTransition(resized.serialized, input(overlay, 360), { kind: 'end-exit', token: blurred.exitToken });
  expect(stale.contentOwner).toBe(resized.contentOwner);
  const closed = workspaceInspectorTransition(resized.serialized, input(overlay, 600), { kind: 'deadline', token: resized.exitToken });
  expect(closed).toMatchObject({ contentOwner: '', registrationJSON: '', prewarmToken: '' });
});


test('inactive width changes restart close but repeated width does not', () => {
  const opened = open();
  const blurred = workspaceInspectorTransition(opened.serialized, input(overlay, 100));
  const resized = workspaceInspectorTransition(blurred.serialized, input(overlay, 200, { columnWidth: 360 }));
  expect(resized.exitAt).toBe(460);
  expect(resized.exitToken).not.toBe(blurred.exitToken);
  const repeated = workspaceInspectorTransition(resized.serialized, input(overlay, 300, { columnWidth: 360 }));
  expect(repeated.serialized).toBe(resized.serialized);
  expect(workspaceInspectorTransition(repeated.serialized, input(overlay, 360, { columnWidth: 360 }),
    { kind: 'deadline', token: blurred.exitToken }).contentOwner).toBe(resized.contentOwner);
});


test('ending resize while inactive restarts source close completion', () => {
  const opened = open();
  const dragging = workspaceInspectorTransition(opened.serialized, input(chat, 20, { resizing: true }));
  const blurred = workspaceInspectorTransition(dragging.serialized, input(overlay, 100, { resizing: true }));
  const ended = workspaceInspectorTransition(blurred.serialized, input(overlay, 120));
  expect(ended.exitAt).toBe(380);
  expect(ended.exitToken).not.toBe(blurred.exitToken);
  expect(workspaceInspectorTransition(ended.serialized, input(overlay, 360),
    { kind: 'end-exit', token: blurred.exitToken }).contentOwner).toBe(ended.contentOwner);
});

// Exact rejects extra producer fields even when structural TypeScript accepts them.
test('inspector producer snapshots match declared Contract keys without transition flags', () => {
  const contract = readFileSync(new URL('./workspace-inspector.contract', import.meta.url), 'utf8');
  const declaration = contract.split('shape InspectorState\n')[1]?.split('shape InspectorContext')[0] ?? '';
  const expected = [...declaration.matchAll(/^  (\w+):/gm)].map(match => match[1]).sort();
  expect(expected.length).toBe(23);
  const opened = open();
  const exiting = workspaceInspectorTransition(opened.serialized, input(overlay, 100));
  for (const serialized of ['', 'not JSON', opened.serialized, exiting.serialized]) {
    const snapshot = workspaceInspectorSnapshot(serialized);
    expect(Object.keys(snapshot).sort()).toEqual(expected);
    expect(workspaceInspectorSnapshot(snapshot.serialized)).toEqual(snapshot);
  }
  expect(opened).toMatchObject({ changed: true, revealInspector: true });
  expect(exiting).toMatchObject({ changed: true, revealInspector: false, exitAt: 360 });
});
