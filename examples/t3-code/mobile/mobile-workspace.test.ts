import { expect, test } from 'bun:test';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { contractComponent } from '../tests/support/contract-action-fixture';
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

type ContractTerm = string | ContractTerm[];
// Read the compiler's expanded view, including component and slot expansion.
// A source-text match cannot tell which box UIKit receives as a route's first child.
function expandedAppView(): ContractTerm[] {
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const compiler = process.env.CONTRACT_BIN ?? resolve(root, process.env.CARGO_TARGET_DIR ?? 'target', 'debug/contract');
  const lean = execFileSync(compiler, ['lean', resolve(import.meta.dir, 'app.contract'), '--name', 'WorkspaceStructure'],
    { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  let at = lean.indexOf('  view := ') + '  view := '.length;
  if (at < '  view := '.length) throw new Error('Contract compiler emitted no expanded view.');
  function read(): ContractTerm {
    while (/\s|,/.test(lean[at] ?? '') && at < lean.length) at++;
    const first = lean[at++];
    if (first === '[' || first === '(') {
      const end = first === '[' ? ']' : ')', values: ContractTerm[] = [];
      while (at < lean.length) {
        while (/\s|,/.test(lean[at] ?? '') && at < lean.length) at++;
        if (lean[at] === end) { at++; return values; }
        values.push(read());
      }
      throw new Error('Unterminated expanded Contract node.');
    }
    if (first === '"') {
      const start = at - 1;
      while (at < lean.length) { if (lean[at++] === '\\') at++; else if (lean[at - 1] === '"') break; }
      return JSON.parse(lean.slice(start, at).replace(/\\x([0-9a-f]{2})/g, '\\u00$1'));
    }
    const start = at - 1;
    while (at < lean.length && !/[\s,\]\)]/.test(lean[at]!)) at++;
    return lean.slice(start, at);
  }
  return terms(read());
}
function expandedWorkspaceRoute(): ContractTerm[] {
  const view = expandedAppView(), main = terms(view[0]), children = terms(main[5]);
  function findEntry(values: ContractTerm[]): ContractTerm[] | undefined {
    for (const value of values) {
      if (!Array.isArray(value)) continue;
      const node = terms(value);
      if (node[0] === '.each' && node[2] === 'entry') return node;
      for (const child of node) if (Array.isArray(child)) {
        const found = findEntry(child);
        if (found) return found;
      }
    }
    return undefined;
  }
  const entry = findEntry(children);
  if (!entry) throw new Error('Expanded root has no actual router-entry loop.');
  return terms(terms(entry[6])[0]);
}
function elementAttribute(node: ContractTerm, name: string): ContractTerm | undefined {
  const parts = terms(node);
  if (parts[0] !== '.element') return undefined;
  return terms(parts[3]).map(terms).find(attribute => attribute[0] === name)?.[1];
}
function literalString(value: ContractTerm | undefined): string | undefined {
  const node = Array.isArray(value) ? terms(value) : [];
  return node[0] === '.str' && typeof node[1] === 'string' ? node[1] : undefined;
}
function expandedElements(value: ContractTerm): ContractTerm[][] {
  if (!Array.isArray(value)) return [];
  const node = terms(value), own = node[0] === '.element' ? [node] : [];
  return own.concat(node.flatMap(child => expandedElements(child)));
}
function terms(value: ContractTerm | undefined): ContractTerm[] {
  if (!Array.isArray(value)) throw new Error('Expected an expanded Contract node/list.');
  return value;
}
function contractValue(value: ContractTerm | undefined, scope: Record<string, unknown>): unknown {
  const node = terms(value), kind = node[0];
  if (kind === '.typed') return contractValue(node[1], scope);
  if (kind === '.str') return node[1];
  if (kind === '.bool') return node[1] === 'true';
  if (kind === '.var') return scope[String(node[1])];
  if (kind === '.member') return (contractValue(node[1], scope) as Record<string, unknown>)[String(node[2])];
  if (kind === '.template') return terms(node[1]).map(part => String(contractValue(part, scope))).join('');
  if (kind === '.unary' && node[1] === '.not') return !contractValue(node[2], scope);
  if (kind === '.binary') {
    const left = contractValue(node[2], scope);
    if (node[1] === '.and') return left && contractValue(node[3], scope);
    if (node[1] === '.or') return left || contractValue(node[3], scope);
    if (node[1] === '.eq') return left === contractValue(node[3], scope);
    if (node[1] === '.ne') return left !== contractValue(node[3], scope);
  }
  if (kind === '.call' && node[1] === 'startsWith') {
    const args = terms(node[2]).map(arg => contractValue(arg, scope));
    return String(args[0]).startsWith(String(args[1]));
  }
  throw new Error(`Unsupported structural condition ${kind}: ${JSON.stringify(node).slice(0, 160)}`);
}
function routeParts(nodes: ContractTerm[], scope: Record<string, unknown>): ContractTerm[][] {
  return nodes.flatMap(value => {
    const node = terms(value);
    if (node[0] === '.when') return routeParts(terms(node[contractValue(node[2], scope) ? 3 : 4]), scope);
    return [node];
  });
}

test('the app root exposes the three workspace tab panels to the native container seam', () => {
  const elements = expandedElements(expandedAppView());
  const ids = elements.map(node => literalString(elementAttribute(node, 'id'))).filter((value): value is string => typeof value === 'string');
  expect(ids.filter(id => ['sidebar-panel', 'workspace-panel', 'inspector-panel'].includes(id))).toEqual(['sidebar-panel', 'workspace-panel', 'inspector-panel']);
  const marker = elements.find(node => literalString(elementAttribute(node, 'role')) === 'tablist');
  expect(marker).toBeDefined();
  const markerChildren = expandedElements(marker!).filter(node => literalString(elementAttribute(node, 'role')) === 'tab');
  expect(markerChildren.map(tab => literalString(elementAttribute(tab, 'aria-controls')))).toEqual(['sidebar-panel', 'workspace-panel', 'inspector-panel']);
});

test('expanded routes put native headers first and ordinary inspector columns below the bar', () => {
  const route = expandedWorkspaceRoute();
  for (const name of ['home', 'thread', 'threadFiles', 'threadFile', 'threadReview', 'newTask', 'newTaskDraft', 'settings', 'settingsAppearance', 'connect']) {
    for (const mode of [{ fileInspectorSupported: false, inspectorMounted: false, inspectorRouteId: '7' },
      { fileInspectorSupported: true, inspectorMounted: true, inspectorRouteId: '7' },
      { fileInspectorSupported: true, inspectorMounted: true, inspectorRouteId: '99' }]) {
      const scope = { entry: { id: 7, name }, workspace: { sidebarMounted: false, ...mode }, filesData: { error: '' } };
      const children = routeParts(terms(route[5]), scope);
      expect(children[0]?.slice(0, 2), `${name}: UIKit needs a direct header`).toEqual(['.element', 'header']);
      expect(children[1]?.slice(0, 2), `${name}: content and inspector share the body row`).toEqual(['.element', 'row']);
      const columns = routeParts(terms(children[1]![5]), scope);
      expect(columns[0]?.slice(0, 2)).toEqual(['.element', 'column']);
      // The inspector is a separate public tab panel; the route stack stays a
      // single column so UIKit can move it without reparenting route content.
      expect(columns).toHaveLength(1);
    }
  }
});

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

test('split geometry reserves a persistent sidebar while compact geometry stays one column', () => {
  const projected = mobileWorkspace([home, thread], options), config = JSON.parse(projected.configuration);
  expect(projected).toMatchObject({ usesSplitView: true, sidebarMounted: true, sidebarVisible: true,
    sidebarTargetWidth: 328, sidebarContentWidth: 328, contentPaneWidth: 696, contentSettledWidth: 696,
    workspaceRouteId: '7', environmentId: 'env', threadId: 'a', inspectorTargetWidth: 0 });
  expect(config).toEqual({ inspectorRouteKey: 't3-workspace-inspector', inspectorOwner: '', inspectorExitToken: '',
    inspectorContentWidth: 0, inspectorVisible: false, inspectorResizing: false,
    dividerColor: '#808080', dividerActiveColor: '#808080', sidebarRouteKey: 't3-workspace-sidebar',
    viewportWidth: 1024, viewportHeight: 768, usesSplitView: true, sidebarVisible: true,
    sidebarContentWidth: 328, sidebarTargetWidth: 328, contentSettledWidth: 696, inspectorTargetWidth: 0,
    reducedMotion: false, appearance: 'dark', background: '#111111' });
  for (const width of [402, 719, 720, 744, 819, 820, 1366]) {
    const compact = mobileWorkspace([home], { ...options, width, primarySidebarPreferredVisible: false });
    const split = width >= 720;
    expect(compact).toMatchObject({ sidebarVisible: split, emptyWorkspaceDetail: split,
      contentSettledWidth: split ? width - compact.sidebarTargetWidth : width });
    expect(compact.sidebarMounted).toBe(width >= 720);
  }
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

test('Home and sidebar route first: a fleet row names its own environment, a bare row the focused one', () => {
  expect(mobileWorkspaceThreadSelection([home], false, 'env', 'fleet:remote:t:1')).toMatchObject({ operation: 'push', location: '/threads/remote/t%3A1', message: '' });
  expect(mobileWorkspaceThreadSelection([home, thread], true, 'env', 'fleet:env:a')).toMatchObject({ operation: 'none', message: '' });
  expect(mobileWorkspaceThreadSelection([home, thread], true, 'env', 'fleet:remote:a')).toMatchObject({ operation: 'replace', sourceAction: 'set-params', location: '/threads/remote/a' });
  expect(mobileWorkspaceThreadSelection([home], false, 'env', 'b')).toMatchObject({ operation: 'push', location: '/threads/env/b' });
  expect(mobileWorkspaceThreadSelection([home], false, '', 'b')).toMatchObject({ operation: 'none', message: 'Choose an available thread.' });
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
  expect(mobileWorkspace([home], { ...options, width: NaN, height: Infinity }).contentSettledWidth).toBe(0);
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


test('ordinary inspector support follows the viewport without changing route or preference owners', () => {
  const routes = [home, files], before = structuredClone(routes);
  const preferences = { ...options, inspector, fileInspectorPreferredVisible: true, fileInspectorPreferredWidth: 300 };
  for (const [width, height, supported] of [[719, 600, false], [720, 599, false], [720, 600, false],
    [819, 900, false], [820, 900, true], [1366, 1024, true], [932, 430, false]] as const) {
    const projected = mobileWorkspace(routes, { ...preferences, width, height });
    expect(projected).toMatchObject({ workspaceRouteId: '12', topRouteId: '12', environmentId: 'env', threadId: 'a',
      inspectorToken: 'registration-1', inspectorRouteId: '12', inspectorMounted: supported, inspectorVisible: supported });
    expect(projected.contentSettledWidth + projected.inspectorTargetWidth + projected.sidebarTargetWidth).toBe(width);
  }
  expect(routes).toEqual(before);
  expect(preferences.fileInspectorPreferredWidth).toBe(300);
  expect(preferences.primarySidebarPreferredVisible).toBe(true);
  expect(mobileWorkspace(routes, { ...preferences, width: 1366 })).toMatchObject({ inspectorTargetWidth: 300, contentSettledWidth: 686,
    sidebarTargetWidth: 380 });
});


test('inline inspector controls execute only for the current interactive owner and preserve draft/navigation state', () => {
  const root = contractComponent('mobile/app.contract', 'App');
  const nav = { id: 'thread-visit', url: '/threads/env/a' };
  const state = { fileUi: { query: '', preview: 'src/a.ts' }, fileInspectorPreferredVisible: true, inspectorResizing: true,
    draft: 'unsent draft', nav };
  const scope = { workspace: { inspectorToken: 'content:4' }, inspectorPresentation: { interactive: true },
    FileUi: (current: object, changes: object) => ({ ...current, ...changes }) };
  root.call('inspectorControl', state, scope, ['content:old', 'search', 'stale']);
  expect(state.fileUi.query).toBe('');
  root.call('inspectorControl', state, { ...scope, inspectorPresentation: { interactive: false } }, ['content:4', 'close', '']);
  expect(state.fileInspectorPreferredVisible).toBe(true);
  root.call('inspectorControl', state, scope, ['content:4', 'search', 'src']);
  expect(state.fileUi).toEqual({ query: 'src', preview: 'src/a.ts' });
  root.call('inspectorControl', state, scope, ['content:4', 'close', '']);
  expect(state).toMatchObject({ fileInspectorPreferredVisible: false, inspectorResizing: false, draft: 'unsent draft', nav });
  expect(state.nav).toBe(nav);
});

test('inspector timing follows the source cubic within subpixel error and settles for reduced motion or direct resize', () => {
  const projected = mobileWorkspace([home, files], { ...options, inspector });
  expect(projected.inspectorMotion.startsWith('260ms linear(')).toBe(true);
  const values = projected.inspectorMotion.slice('260ms linear('.length, -1).split(',').map(Number);
  expect(values.length).toBeLessThanOrEqual(64);
  let largestError = 0;
  for (let millisecond = 0; millisecond <= 260; millisecond++) {
    const progress = millisecond / 260, position = progress * (values.length - 1), index = Math.min(values.length - 2, Math.floor(position));
    const shown = values[index]! + (values[index + 1]! - values[index]!) * (position - index);
    const source = progress <= 0.5 ? Math.pow(progress * 2, 3) / 2 : 1 - Math.pow((1 - progress) * 2, 3) / 2;
    largestError = Math.max(largestError, Math.abs(shown - source) * 480);
  }
  expect(largestError).toBeLessThan(0.2);
  for (const override of [{ reducedMotion: true }, { inspectorResizing: true }]) {
    const settled = mobileWorkspace([home, files], { ...options, inspector, ...override });
    expect(settled).toMatchObject({ inspectorMotion: 'none', inspectorTransition: 'none',
      inspectorToken: projected.inspectorToken, contentSettledWidth: projected.contentSettledWidth });
  }
});
