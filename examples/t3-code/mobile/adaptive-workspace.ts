// @ref llp/1107.011-responsive-workspace.decision.md#source-helpers
// Adapted from pinned365aa87982 AdaptiveWorkspaceLayout.tsx and workspace-inspector-pane.tsx.
// Pure geometry only: root retains preferences, search, route/inspector ownership and animations.
import { deriveLayout, deriveWorkspacePaneLayout, deriveFileInspectorPaneLayout,
  constrainAuxiliaryPaneWidth, CHAT_CONTENT_MAX_WIDTH, type WorkspaceAuxiliaryPaneRole } from './layout-mobile';

export interface AdaptiveWorkspaceInput {
  readonly width: number;
  readonly height: number;
  /** Underlying workspace path, excluding sheet overlays. */
  readonly pathname: string;
  readonly primarySidebarPreferredVisible: boolean;
  readonly supplementaryPanePreferredVisible: boolean;
  readonly supplementaryPanePreferredWidth?: number;
  readonly fileInspectorPreferredVisible: boolean;
  readonly fileInspectorPreferredWidth?: number;
  readonly focusedAuxiliaryPaneRole?: WorkspaceAuxiliaryPaneRole | null;
  /** Root retains content through its exit transition, guarded by its registration token. */
  readonly inspectorRegistered: boolean;
  readonly inspectorActive: boolean;
}
export function adaptiveWorkspace(input: AdaptiveWorkspaceInput) {
  const layout = deriveLayout(input);
  const showPrimarySidebar = input.pathname === '/' || input.primarySidebarPreferredVisible;
  const auxiliaryPaneRole = input.focusedAuxiliaryPaneRole ?? (/\/files(?:\/|$)/.test(input.pathname) ? 'inspector' : 'supplementary');
  const fileInspector = deriveFileInspectorPaneLayout({ layout, viewportWidth: input.width,
    preferredWidth: input.fileInspectorPreferredWidth,
    reservedLeadingWidth: layout.usesSplitView && showPrimarySidebar ? (layout.listPaneWidth ?? 0) : 0 });
  const panes = deriveWorkspacePaneLayout({ layout, viewportWidth: input.width,
    primarySidebarPreferredVisible: showPrimarySidebar,
    auxiliaryPaneRole,
    auxiliaryPanePreferredVisible: auxiliaryPaneRole === 'inspector' ? input.fileInspectorPreferredVisible : input.supplementaryPanePreferredVisible,
    auxiliaryPanePreferredWidth: auxiliaryPaneRole === 'inspector' ? input.fileInspectorPreferredWidth : input.supplementaryPanePreferredWidth });
  const inspectorSupported = input.inspectorRegistered && panes.auxiliaryPaneWidth !== null;
  const inspectorVisible = inspectorSupported && panes.auxiliaryPaneVisible && input.inspectorActive;
  const inspectorTargetWidth = inspectorVisible ? (panes.auxiliaryPaneWidth ?? 0) : 0;
  return {
    variant: layout.variant, usesSplitView: layout.usesSplitView, shellPadding: layout.shellPadding,
    sidebarMounted: layout.usesSplitView, sidebarVisible: panes.primarySidebarVisible,
    sidebarSuppressedByAuxiliary: panes.primarySidebarSuppressedByAuxiliary,
    sidebarContentWidth: layout.listPaneWidth ?? 0,
    sidebarTargetWidth: panes.primarySidebarVisible ? (layout.listPaneWidth ?? 0) : 0,
    emptyWorkspaceDetail: layout.usesSplitView && input.pathname === '/',
    contentPaneWidth: panes.contentPaneWidth,
    // Settle once per pane change; clip/reveal around this box during motion.
    contentSettledWidth: layout.usesSplitView ? Math.max(0, panes.contentPaneWidth - inspectorTargetWidth) : input.width,
    auxiliaryPaneRole, auxiliaryPaneSupported: panes.supportsAuxiliaryPane,
    auxiliaryPaneVisible: panes.auxiliaryPaneVisible, auxiliaryPaneWidth: panes.auxiliaryPaneWidth ?? 0,
    fileInspectorSupported: fileInspector.supported, fileInspectorWidth: fileInspector.width ?? 0,
    inspectorMounted: inspectorSupported, inspectorVisible, inspectorTargetWidth,
    chatContentMaxWidth: CHAT_CONTENT_MAX_WIDTH,
  };
}

/** Source toggle changes preference only; Home always retains its primary list. */
export function adaptiveSidebarToggle(input: AdaptiveWorkspaceInput) {
  const panes = adaptiveWorkspace(input);
  if (input.pathname === '/') return { primarySidebarPreferredVisible: input.primarySidebarPreferredVisible,
    fileInspectorPreferredVisible: input.fileInspectorPreferredVisible };
  if (!panes.sidebarVisible && panes.sidebarSuppressedByAuxiliary) return {
    primarySidebarPreferredVisible: true, fileInspectorPreferredVisible: false };
  return { primarySidebarPreferredVisible: !input.primarySidebarPreferredVisible,
    fileInspectorPreferredVisible: input.fileInspectorPreferredVisible };
}

/** Pointer translation is cumulative from the width captured at resize start. */
export function adaptiveInspectorResize(startWidth: number, translationX: number, contentPaneWidth: number) {
  return constrainAuxiliaryPaneWidth({ preferredWidth: startWidth - translationX, availableWidth: contentPaneWidth });
}
/** Source VoiceOver delta is already directed: increment widens the trailing pane. */
export function adaptiveInspectorAccessibilityResize(startWidth: number, action: 'increment' | 'decrement', contentPaneWidth: number) {
  return constrainAuxiliaryPaneWidth({ preferredWidth: startWidth + (action === 'increment' ? 24 : -24), availableWidth: contentPaneWidth });
}
