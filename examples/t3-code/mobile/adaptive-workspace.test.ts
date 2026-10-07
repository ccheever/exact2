// @ref llp/1106-t3-code-ios.rfc.md#parity-method
import { expect, test } from 'bun:test';
import { adaptiveWorkspace, adaptiveSidebarToggle, adaptiveInspectorResize, adaptiveInspectorAccessibilityResize,
  type AdaptiveWorkspaceInput } from './adaptive-workspace';
const base: AdaptiveWorkspaceInput = { width: 1024, height: 768, pathname: '/threads/env/thread',
  primarySidebarPreferredVisible: true, supplementaryPanePreferredVisible: true,
  fileInspectorPreferredVisible: true, inspectorRegistered: false, inspectorActive: false };

test('Home forces its sidebar without overwriting preference, compact preserves input state', () => {
  const input = { ...base, pathname: '/', primarySidebarPreferredVisible: false };
  expect(adaptiveWorkspace(input)).toMatchObject({ sidebarMounted: true, sidebarVisible: true, sidebarContentWidth: 328, emptyWorkspaceDetail: true });
  expect(adaptiveSidebarToggle(input).primarySidebarPreferredVisible).toBe(false);
  expect(adaptiveWorkspace({ ...input, width: 932, height: 430 })).toMatchObject({ sidebarMounted: false, sidebarVisible: false, emptyWorkspaceDetail: false, contentSettledWidth: 932 });
  expect(input.primarySidebarPreferredVisible).toBe(false);
});
test('narrow iPad inspector suppresses sidebar; showing sidebar closes inspector without changing its width preference', () => {
  const input = { ...base, pathname: '/threads/env/thread/files', inspectorRegistered: true, inspectorActive: true, fileInspectorPreferredWidth: 420 };
  expect(adaptiveWorkspace(input)).toMatchObject({ sidebarVisible: false, sidebarSuppressedByAuxiliary: true, inspectorVisible: true, inspectorTargetWidth: 260, contentSettledWidth: 764 });
  const next = { ...input, ...adaptiveSidebarToggle(input) };
  expect(adaptiveWorkspace(next)).toMatchObject({ sidebarVisible: true, inspectorVisible: false, contentSettledWidth: 696 });
  expect(next.fileInspectorPreferredWidth).toBe(420);
});
test('focused role overrides path and supplementary preference/width remains distinct', () => {
  const input = { ...base, width: 1600, height: 1000, pathname: '/threads/env/thread/files', focusedAuxiliaryPaneRole: 'supplementary' as const,
    supplementaryPanePreferredWidth: 300, supplementaryPanePreferredVisible: false, fileInspectorPreferredWidth: 450,
    inspectorRegistered: true, inspectorActive: true };
  expect(adaptiveWorkspace(input)).toMatchObject({ auxiliaryPaneRole: 'supplementary', auxiliaryPaneWidth: 300, auxiliaryPaneVisible: false, inspectorVisible: false });
  expect(adaptiveWorkspace({ ...input, focusedAuxiliaryPaneRole: null })).toMatchObject({ auxiliaryPaneRole: 'inspector', auxiliaryPaneWidth: 450, inspectorVisible: true });
});
test('inspector exit retains mounted content while settled center immediately uses final geometry', () => {
  const input = { ...base, width: 1366, height: 1024, pathname: '/threads/env/thread/files', inspectorRegistered: true, inspectorActive: false };
  expect(adaptiveWorkspace(input)).toMatchObject({ inspectorMounted: true, inspectorVisible: false, inspectorTargetWidth: 0, contentSettledWidth: 986 });
  expect(adaptiveWorkspace({ ...input, inspectorRegistered: false }).inspectorMounted).toBe(false);
});
test('divider uses captured starting width and source accessibility increment direction', () => {
  expect(adaptiveInspectorResize(300, -80, 1000)).toBe(380);
  expect(adaptiveInspectorResize(300, 80, 1000)).toBe(260);
  expect(adaptiveInspectorResize(300, -400, 1000)).toBe(440);
  expect(adaptiveInspectorAccessibilityResize(300, 'increment', 1000)).toBe(324);
  expect(adaptiveInspectorAccessibilityResize(300, 'decrement', 1000)).toBe(276);
});
