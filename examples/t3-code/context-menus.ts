// context-menu-gaps: the items of three right-click menus (T3 Code 1e2ecbd975, MIT; see
// LICENSE-T3): fileContextMenu.ts (a workspace file: Open, the server-worded reveal, "Open
// with" the detected editors) with FileBrowserPanel.tsx's "Copy mention" / "Add to chat",
// components/pullRequest/pullRequestLinkContextMenu.ts (a change request's number),
// ChatMarkdown.tsx's file-link menu, components/preview/fileExplorerLabel.ts and
// editorLabels.ts openInEditorMenuLabel; diffFileActions.ts resolveDiffPathForWorkspace and
// packages/shared/src/path.ts for the absolute path. The module's `contextMenu` op
// (T3ContextMenu.swift) shows them as ElectronMenu.ts would: an item with children is a submenu.
import { obj, str, type Obj } from './domain';
import { EDITOR_DEFINITIONS } from './editors';
import type { MenuItem } from './sidebar-menu';
import { resolvePathLinkTarget } from './terminal-links';

const EDITOR_LABEL_BY_ID = new Map(EDITOR_DEFINITIONS.map(editor => [editor.id, editor.label]));

// ── Labels ────────────────────────────────────────────────────────────────

/** revealInFileExplorerLabelForOs: keyed by the environment's reported OS. */
export function revealInFileExplorerLabelForOs(os: string): string {
  if (os === 'darwin') return 'Reveal in Finder';
  if (os === 'windows') return 'Reveal in File Explorer';
  return 'Reveal in Files';
}
/** revealInFileExplorerLabelForKind: the server's wording (Windows File Explorer from WSL too). */
export function revealInFileExplorerLabelForKind(kind: string): string {
  if (kind === 'finder') return 'Reveal in Finder';
  if (kind === 'file-explorer') return 'Reveal in File Explorer';
  return 'Reveal in Files';
}
/**
 * The reveal item's label, or undefined when the environment cannot reveal: the server must
 * honor `reveal` (`shellRevealInFileManager`) and offer the file manager. The wording comes
 * from the server because on WSL the reveal can run through File Explorer on a Linux host.
 */
export function revealLabelFor(config: Obj, environmentId: string | null): string | undefined {
  const available = Array.isArray(config.availableEditors) ? config.availableEditors : [];
  if (!environmentId || config.shellRevealInFileManager !== true || !available.includes('file-manager')) return undefined;
  const kind = config.shellRevealInFileManagerKind;
  return typeof kind === 'string' ? revealInFileExplorerLabelForKind(kind) : revealInFileExplorerLabelForOs(str(obj(obj(config.environment).platform).os));
}
/** editorLabels.ts openInEditorMenuLabel. */
export function openInEditorMenuLabel(editorId: string | null): string {
  return editorId === null || editorId === '' || editorId === 'file-manager' ? 'Open in editor' : `Open in ${EDITOR_LABEL_BY_ID.get(editorId) ?? 'Editor'}`;
}
/** The editor ids the server reports that this client knows (EditorId), in the server's order. */
export function availableEditorIds(config: Obj): string[] {
  const raw: unknown[] = Array.isArray(config.availableEditors) ? config.availableEditors : [];
  return raw.filter((id): id is string => typeof id === 'string' && EDITOR_LABEL_BY_ID.has(id));
}

// ── Workspace file ───────────────────────────────────────────────────────

const isWindowsDrivePath = (value: string) => /^[a-zA-Z]:([/\\]|$)/.test(value);
const isUncPath = (value: string) => value.startsWith('\\\\');
const isWindowsAbsolutePath = (value: string) => isUncPath(value) || isWindowsDrivePath(value);
function trimTrailingPathSeparators(value: string): string {
  if (!value || value === '/' || value === '\\' || /^[a-zA-Z]:[/\\]$/.test(value)) return value;
  const trimmed = value.startsWith('/') ? value.replace(/\/+$/g, '') : value.replace(/[\\/]+$/g, '');
  if (!trimmed) return value;
  return /^[a-zA-Z]:$/.test(trimmed) ? `${trimmed}\\` : trimmed;
}
function normalizeProjectPathForComparison(value: string): string {
  const normalized = trimTrailingPathSeparators(value.trim());
  return isWindowsDrivePath(normalized) || isUncPath(normalized) ? normalized.split('/').join('\\').toLowerCase() : normalized;
}
function normalizedRelativePathSegments(filePath: string): string[] | null {
  if (filePath.startsWith('/') || isWindowsAbsolutePath(filePath) || /^[a-zA-Z]:/.test(filePath)) return null;
  const segments = filePath.split('\\').join('/').split('/').filter(segment => segment.length > 0 && segment !== '.');
  return segments.length === 0 || segments.includes('..') ? null : segments;
}
function repositoryRelativeWorkspaceSegments(workspaceRoot: string | undefined, repositoryRoot: string | undefined): string[] | null {
  if (!workspaceRoot || !repositoryRoot) return null;
  const workspace = normalizeProjectPathForComparison(workspaceRoot), repository = normalizeProjectPathForComparison(repositoryRoot);
  if (workspace === repository) return [];
  const separator = repository.includes('\\') ? '\\' : '/';
  const prefix = repository.endsWith(separator) ? repository : `${repository}${separator}`;
  if (!workspace.startsWith(prefix)) return null;
  return workspace.slice(prefix.length).split(/[\\/]+/).filter(Boolean);
}
/** diffFileActions.ts resolveDiffPathForWorkspace. */
export function resolveDiffPathForWorkspace(input: { filePath: string; workspaceRoot: string | undefined; repositoryRoot: string | undefined }): string | null {
  const fileSegments = normalizedRelativePathSegments(input.filePath);
  if (!fileSegments) return null;
  const workspaceSegments = repositoryRelativeWorkspaceSegments(input.workspaceRoot, input.repositoryRoot);
  if (!workspaceSegments || workspaceSegments.length === 0) return fileSegments.join('/');
  const caseInsensitive = input.repositoryRoot ? isWindowsAbsolutePath(input.repositoryRoot) : false;
  const belongs = workspaceSegments.every((segment, index) => {
    const candidate = fileSegments[index];
    return candidate !== undefined && (caseInsensitive ? candidate.toLowerCase() === segment : candidate === segment);
  });
  if (!belongs) return null;
  const relative = fileSegments.slice(workspaceSegments.length);
  return relative.length > 0 ? relative.join('/') : null;
}

export type FileContextMenuTarget = { environmentId: string | null; filePath: string; workspaceRoot: string | undefined; repositoryRoot?: string | undefined };
/** Absolute path on the environment host, or null: callers then offer no file actions. */
export function resolveFileContextMenuAbsolutePath(target: FileContextMenuTarget): string | null {
  const workspaceFilePath = resolveDiffPathForWorkspace({ filePath: target.filePath, workspaceRoot: target.workspaceRoot, repositoryRoot: target.repositoryRoot });
  if (workspaceFilePath === null) return null;
  if (target.workspaceRoot === undefined) return workspaceFilePath.startsWith('/') || /^[a-zA-Z]:/.test(workspaceFilePath) ? workspaceFilePath : null;
  return resolvePathLinkTarget(workspaceFilePath, target.workspaceRoot);
}

export type FileContextMenuCapabilities = { revealLabel: string | undefined; canOpenDefault: boolean; editorIds: readonly string[] };
/** useFileContextMenu's capabilities, from the environment's server config. */
export function fileContextMenuCapabilities(config: Obj, environmentId: string | null): FileContextMenuCapabilities {
  const editorIds = availableEditorIds(config);
  return { revealLabel: revealLabelFor(config, environmentId), canOpenDefault: editorIds.includes('file-manager'), editorIds };
}
/**
 * buildFileContextMenuItems: only what the environment advertises — default-app open, reveal
 * (server-worded) and an "Open with" submenu of detected editors. Empty when nothing can act.
 */
export function buildFileContextMenuItems(input: { hasAbsolutePath: boolean; capabilities: FileContextMenuCapabilities }): MenuItem[] {
  if (!input.hasAbsolutePath) return [];
  const items: MenuItem[] = [];
  if (input.capabilities.canOpenDefault) items.push({ id: 'open', label: 'Open' });
  if (input.capabilities.revealLabel !== undefined) items.push({ id: 'reveal-in-folder', label: input.capabilities.revealLabel });
  const editorIds = input.capabilities.editorIds.filter(id => id !== 'file-manager');
  if (editorIds.length > 0) {
    items.push({ id: 'open-with', label: 'Open with', children: editorIds.map(editorId => ({ id: `editor:${editorId}`, label: EDITOR_LABEL_BY_ID.get(editorId) ?? editorId })) });
  }
  return items;
}
/** FileBrowserPanel showEntryContextMenu: the file actions, then the panel's mention actions. */
export function fileTreeContextMenuItems(fileItems: MenuItem[]): MenuItem[] {
  return [...fileItems, { id: 'copy-mention', label: 'Copy mention' }, { id: 'add-to-chat', label: 'Add to chat' }];
}
/** The toast title when a file action fails (useFileContextMenu activate). */
export function fileActionFailureTitle(action: string): string {
  if (action === 'open') return 'Could not open file';
  if (action === 'reveal-in-folder') return 'Unable to reveal file';
  const editor = action.slice('editor:'.length);
  return `Could not open in ${EDITOR_LABEL_BY_ID.get(editor) ?? editor}`;
}

// ── Pull request number ──────────────────────────────────────────────────

/** Named for the host rather than "externally": the point is where you will land. */
const OPEN_ON_HOST_LABELS: Record<string, string> = { github: 'Open on GitHub', gitlab: 'Open on GitLab', forgejo: 'Open on Forgejo', bitbucket: 'Open on Bitbucket', 'azure-devops': 'Open on Azure DevOps' };
export const openOnHostLabel = (provider: string): string => OPEN_ON_HOST_LABELS[provider] ?? 'Open on host';
/** Copy first: it is the reason to right-click a number rather than click it. */
export function pullRequestLinkContextMenuItems(openLabel: string): MenuItem[] {
  return [{ id: 'copy-link', label: 'Copy link' }, { id: 'open-external', label: openLabel }];
}

// ── Chat file link ───────────────────────────────────────────────────────

/**
 * ChatMarkdown's file-link menu: "Preview media" for media in a thread, the open item whenever
 * shell actions are allowed (named for the preferred editor), the reveal item with the server's
 * wording, then the two copies. "Open in integrated browser" is the Browser surface's (X1).
 */
export function markdownFileMenuItems(input: { canPreviewMedia: boolean; canOpen: boolean; preferredEditor: string | null; revealLabel: string | undefined }): MenuItem[] {
  return [
    ...(input.canPreviewMedia ? [{ id: 'preview-media', label: 'Preview media' }] : []),
    ...(input.canOpen ? [{ id: 'open', label: openInEditorMenuLabel(input.preferredEditor) }] : []),
    ...(input.canOpen && input.revealLabel ? [{ id: 'reveal', label: input.revealLabel }] : []),
    { id: 'copy-relative', label: 'Copy relative path' }, { id: 'copy-full', label: 'Copy full path' },
  ];
}
/** All ids a menu can return, a submenu's children included (a child is what gets picked). */
export function menuItemIds(items: MenuItem[]): string[] {
  return items.flatMap(item => item.children?.length ? menuItemIds(item.children) : [item.id]);
}
