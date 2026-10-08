import { expect, test } from 'bun:test';
import { mobileWorkspaceEvent, mobileWorkspace, mobileWorkspaceEntries, mobileWorkspaceLocation, mobileWorkspaceThreadSelection,
  mobileWorkspaceFileSelection, mobileWorkspaceOverlay, type MobileWorkspaceOptions } from './mobile-workspace';
const home = { id: 0, name: 'home', url: '/', params: {} };
const thread = { id: 7, name: 'thread', url: '/threads/env/a', params: { threadEnvironment: 'env', threadId: 'a' } };
const files = { id: 12, name: 'threadFiles', url: '/threads/env/a/files', params: thread.params };
const settings = { id: 14, name: 'settingsAppearance', url: '/settings/appearance', params: {} };
const options: MobileWorkspaceOptions = { width: 1024, height: 768, primarySidebarPreferredVisible: true,
  supplementaryPanePreferredVisible: true, fileInspectorPreferredVisible: false,
  reducedMotion: false, appearance: 'dark', background: '#111111', hasWorkspace: true, selectedEnvironmentId: 'env', selectedThreadId: 'a' };
const inspector = { token: 'registration-1', routeId: '12', environmentId: 'env', threadId: 'a', generation: 3,
  role: 'inspector' as const, kind: 'files' as const, selectedPath: '', active: true, renderable: true };

test('projection preserves Exact zero/numeric ids and strips no legitimate path parameters', () => {
  expect(mobileWorkspaceEntries([home, thread])).toEqual([{ ...home, id: '0' }, { ...thread, id: '7' }]);
  expect(mobileWorkspaceEntries({ index: 1, routes: [home, thread] })).toEqual([]);
  expect(mobileWorkspaceEntries([{ id: 1, name: 'thread', url: '//example.test', params: {} }])).toHaveLength(0);
  expect(mobileWorkspace([] ,options)).toMatchObject({ ready: false, workspaceRouteId: '', inspectorTargetWidth: 0 });
});

test('source overlay families preserve underlying workspace even when they are native cards', () => {
  for (const name of ['settings', 'settingsLegal', 'newTask', 'newTaskBranches', 'addProject', 'addProjectRepository', 'addProjectDestination', 'addProjectLocal', 'addProjectNew', 'threadModel', 'threadDevices', 'threadBrowser', 'threadReviewComment', 'connections', 'environments', 'environmentDetail']) {
    expect(mobileWorkspaceLocation([home, thread, { ...settings, name }]).workspace?.id).toBe('7');
    expect(mobileWorkspaceOverlay(name)).toBe(true);
  }
  expect(mobileWorkspaceLocation([home, files, settings])).toMatchObject({ workspace: { id: '12' }, overlayCount: 1 });
  // Source ThreadAttachment is not in WORKSPACE_OVERLAY_ROUTES despite its modal presentation.
  expect(mobileWorkspaceOverlay('threadMedia')).toBe(false);
});

test('actual native geometry JSON matches scalar panes and reserves no invented inspector', () => {
  const projected = mobileWorkspace([home, thread], options), config = JSON.parse(projected.configuration);
  expect(projected).toMatchObject({ usesSplitView: true, sidebarTargetWidth: 328, contentSettledWidth: 696,
    workspaceRouteId: '7', environmentId: 'env', threadId: 'a', inspectorTargetWidth: 0 });
  expect(config).toEqual({ inspectorRouteKey: 't3-workspace-inspector', inspectorOwner: '', inspectorExitToken: '', inspectorContentWidth: 0, inspectorVisible: false, inspectorResizing: false, dividerColor: '#808080', dividerActiveColor: '#808080', sidebarRouteKey: 't3-workspace-sidebar', viewportWidth: 1024, viewportHeight: 768, usesSplitView: true, sidebarVisible: true,
    sidebarContentWidth: 328, sidebarTargetWidth: 328, contentSettledWidth: 696, inspectorTargetWidth: 0,
    reducedMotion: false, appearance: 'dark', background: '#111111' });
  expect(mobileWorkspace([home], { ...options, primarySidebarPreferredVisible: false })).toMatchObject({ sidebarVisible: true, emptyWorkspaceDetail: true });
  expect(mobileWorkspace([home, thread], { ...options, width: 932, height: 430 })).toMatchObject({ usesSplitView: false, sidebarTargetWidth: 0, contentSettledWidth: 932 });
});

test('real file registration reserves inspector; leaving focus retains content only for exit', () => {
  const visible = mobileWorkspace([home, files], { ...options, inspector, fileInspectorPreferredVisible: true });
  expect(visible).toMatchObject({ inspectorMounted: true, inspectorVisible: true, inspectorTargetWidth: 260,
    contentSettledWidth: 764, sidebarSuppressedByAuxiliary: true, inspectorMain: 'chat', inspectorCandidate: 'files' });
  const exiting = mobileWorkspace([home, files, settings], { ...options, inspector, fileInspectorPreferredVisible: true });
  expect(exiting).toMatchObject({ workspaceRouteId: '12', topRouteId: '14', inspectorMounted: true, inspectorVisible: false, inspectorTargetWidth: 0, inspectorToken: 'registration-1' });
  expect(mobileWorkspace([home, files], { ...options, inspector: { ...inspector, renderable: false } })).toMatchObject({ inspectorMounted: false, inspectorToken: '', inspectorTargetWidth: 0 });
  const file = { ...files, name: 'threadFile', url: '/threads/env/a/files/src%2Fa.ts' };
  expect(mobileWorkspace([home, file], { ...options, inspector: { ...inspector, selectedPath: 'src/a.ts' } })).toMatchObject({ inspectorMain: 'file', inspectorSelectedPath: 'src/a.ts' });
});

test('focused role ownership is distinct from retained inspector content', () => {
  const projected = mobileWorkspace([home, thread], { ...options, width: 1600,
    role: { token: 'role-1', routeId: '7', value: 'supplementary' }, supplementaryPanePreferredWidth: 310,
    inspector: { ...inspector, routeId: '7', role: 'supplementary' } });
  expect(projected).toMatchObject({ auxiliaryPaneRole: 'supplementary', inspectorTargetWidth: 310 });
  expect(mobileWorkspace([home, thread], { ...options, role: { token: 'stale', routeId: '12', value: 'supplementary' } }).auxiliaryPaneRole).toBe('inspector');
});

test('thread selection uses source push/set-params/replace decisions and actual Exact anchor id', () => {
  expect(mobileWorkspaceThreadSelection([home], true, 'env', 'b')).toMatchObject({ operation: 'push', anchorRouteId: '0', hideFileInspector: false });
  expect(mobileWorkspaceThreadSelection([home, thread], false, 'env', 'b')).toMatchObject({ operation: 'push', anchorRouteId: '7' });
  expect(mobileWorkspaceThreadSelection([home, thread], true, 'env', 'b')).toMatchObject({ operation: 'replace', sourceAction: 'set-params', anchorRouteId: '7', hideFileInspector: true });
  expect(mobileWorkspaceThreadSelection([home, thread], true, 'env', 'a')).toMatchObject({ operation: 'none', hideFileInspector: false });
  expect(mobileWorkspaceThreadSelection([home, thread, files], true, 'env', 'b')).toMatchObject({ operation: 'replace', sourceAction: 'replace', anchorRouteId: '12' });
});

test('overlay selection truncates to actual underlying anchor then mutates once, including same-thread dismissal', () => {
  expect(mobileWorkspaceThreadSelection([home, thread, settings], true, 'env', 'a')).toMatchObject({ operation: 'replace', sourceAction: 'set-params', requestRoute: '14', anchorRouteId: '7', anchorLocation: '/threads/env/a', dismissOverlays: true, hideFileInspector: true });
  expect(mobileWorkspaceThreadSelection([home, settings], true, 'e/x', 't y')).toMatchObject({ operation: 'push', anchorRouteId: '0', location: '/threads/e%2Fx/t%20y', dismissOverlays: true });
  expect(mobileWorkspaceThreadSelection([home, thread, { ...settings, url: thread.url }], true, 'env', 'b')).toMatchObject({ operation: 'none', message: 'The workspace route changed.' });
});

test('file selection preserves compact browser and replaces supported wide browser with encoded real path', () => {
  expect(mobileWorkspaceFileSelection([home, thread, files], true, 'env', 'a', 'src/a.ts')).toMatchObject({ operation: 'replace', anchorRouteId: '12', location: '/threads/env/a/files/src%2Fa.ts' });
  expect(mobileWorkspaceFileSelection([home, thread, files], false, 'env', 'a', 'src/a.ts')).toMatchObject({ operation: 'push' });
  expect(mobileWorkspaceFileSelection([home], true, '', '', '')).toMatchObject({ operation: 'none' });
});


test('replacement identity and reconnect cannot reuse an active inspector registration', () => {
  const changed = { ...files, params: { threadEnvironment: 'env', threadId: 'b' }, url: '/threads/env/b/files' };
  expect(mobileWorkspace([home, changed], { ...options, inspector, fileInspectorPreferredVisible: true })).toMatchObject({ inspectorVisible: false, inspectorTargetWidth: 0 });
  expect(mobileWorkspace([home, files], { ...options, inspector, generation: 4 })).toMatchObject({ inspectorMounted: false, inspectorToken: '' });
  const automatic = mobileWorkspace([home, files], { ...options, inspector, fileInspectorPreferredVisible: true, width: 1600, fileInspectorPreferredWidth: 0 });
  expect(automatic.auxiliaryPaneWidth).toBe(mobileWorkspace([home, files], { ...options, inspector, fileInspectorPreferredVisible: true, width: 1600 }).auxiliaryPaneWidth);
  expect(JSON.parse(mobileWorkspace([home], { ...options, width: NaN, height: Infinity }).configuration).contentSettledWidth).toBe(0);
});


test('only the actual focused route proposes inspector content and plain chat needs an explicit current mode', () => {
  const plain = mobileWorkspace([home, thread], { ...options, generation: 3 });
  expect(plain).toMatchObject({ inspectorCandidate: 'none', inspectorAutomatic: false });
  expect(mobileWorkspace([home, thread], { ...options, generation: 3, threadMode: 'files', threadModeOwner: plain.threadOwner })).toMatchObject({ inspectorCandidate: 'files', inspectorAutomatic: false });
  expect(mobileWorkspace([home, thread], { ...options, generation: 4, threadMode: 'files', threadModeOwner: plain.threadOwner }).inspectorCandidate).toBe('none');
  expect(mobileWorkspace([home, files, settings], { ...options, inspector }).inspectorCandidate).toBe('none');
  expect(mobileWorkspace([home, files], { ...options, hasWorkspace: false }).inspectorCandidate).toBe('none');
  expect(mobileWorkspace([home, files], { ...options, selectedThreadId: 'other' }).inspectorCandidate).toBe('none');
});


test('file navigation retains source path segmentation and refuses an old selected thread', () => {
  expect(mobileWorkspaceFileSelection([home, files], true, 'env', 'a', '/src//a.ts').location).toBe('/threads/env/a/files/src%2Fa.ts');
  expect(mobileWorkspaceFileSelection([home, files], true, 'env', 'b', 'src/a.ts')).toMatchObject({ operation: 'none', message: 'The workspace route changed.' });
});


test('Files opened from plain chat retain chat, while a current file preview updates its own visit', () => {
  expect(mobileWorkspaceFileSelection([home, thread], true, 'env', 'a', 'src/a.ts').operation).toBe('push');
  const file = { ...files, name: 'threadFile', url: '/threads/env/a/files/src%2Fa.ts' };
  expect(mobileWorkspaceFileSelection([home, thread, file], false, 'env', 'a', 'src/b.ts')).toMatchObject({ operation: 'replace', anchorRouteId: '12' });
});


test('inspector chrome and divider events belong to the current visible registration', () => {
  const send = (event: object, visible = true) => mobileWorkspaceEvent(JSON.stringify(event), 'pane:4', 'exit:8', visible, 1100);
  expect(send({ kind: 'search', owner: 'pane:old', value: 'secret' }).kind).toBe('');
  expect(send({ kind: 'search', owner: 'pane:4', value: 'src' }, false).kind).toBe('');
  expect(send({ kind: 'search', owner: 'pane:4', value: 'src' }).value).toBe('src');
  expect(send({ kind: 'resize', owner: 'pane:4', phase: 'update', startWidth: 300, translationX: -40 }).width).toBe(340);
  expect(send({ kind: 'resize', owner: 'pane:4', phase: 'step', startWidth: 300, translationX: 24 }).width).toBe(276);
  expect(send({ kind: 'resize', owner: 'pane:4', phase: 'end', startWidth: 300, translationX: 0 }).phase).toBe('end');
  expect(send({ kind: 'inspector-closed', owner: 'pane:4', exitToken: 'exit:7' }, false).kind).toBe('');
  expect(send({ kind: 'inspector-closed', owner: 'pane:4', exitToken: 'exit:8' }).kind).toBe('');
  expect(send({ kind: 'inspector-closed', owner: 'pane:4', exitToken: 'exit:8' }, false).kind).toBe('end-exit');
});


test('global Arrange preserves the workspace and retained New Task visit beneath its fullscreen route', () => {
  const arrange = { id: 23, name: 'homeArrange', url: '/arrange', params: {} };
  const newTask = { id: 20, name: 'newTask', url: '/new', params: {} };
  const draft = { id: 21, name: 'newTaskDraft', url: '/new/draft?environmentId=env&projectId=p', params: {} };
  for (const prefix of [[home], [home, thread], [home, thread, newTask, draft]]) {
    const before = mobileWorkspace(prefix, options);
    const during = mobileWorkspace([...prefix, arrange], options);
    expect(during).toMatchObject({ workspaceRouteId: before.workspaceRouteId, environmentId: before.environmentId,
      threadId: before.threadId, topRouteId: '23',
      sidebarContentWidth: before.sidebarContentWidth, contentSettledWidth: before.contentSettledWidth,
      overlayCount: before.overlayCount + 1 });
    const retained = mobileWorkspaceLocation([...prefix, arrange]).entries;
    expect(retained.slice(0, -1)).toEqual(mobileWorkspaceEntries(prefix));
    expect(mobileWorkspace(retained.slice(0, -1), options)).toEqual(before);
  }
  expect(mobileWorkspaceOverlay('homeArrange')).toBe(true);
});
