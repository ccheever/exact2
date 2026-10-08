// Project paths as T3 Code compares, titles and browses them (20261005-app-activation). Ported from
// T3 Code (MIT, see LICENSE-T3), reference 1e2ecbd975: packages/shared/src/path.ts and
// packages/client-runtime/src/state/projects.ts, as apps/web/src/lib/projectPaths.ts re-exports them.
// `t3 app <dir>` (desktop-activation.ts) finds an existing project with `findProjectByPath` and
// names a new one with `inferProjectTitleFromPath`; the rest is the same module, kept whole so its
// tests port by name.

// ── packages/shared/src/path.ts ──────────────────────────────────────────
export function isWindowsDrivePath(value: string): boolean {
  return /^[a-zA-Z]:([/\\]|$)/.test(value);
}
export function isUncPath(value: string): boolean {
  return value.startsWith('\\\\');
}
export function isWindowsAbsolutePath(value: string): boolean {
  return isUncPath(value) || isWindowsDrivePath(value);
}
export function isExplicitRelativeProjectPath(value: string): boolean {
  return value === '.' || value === '..' || value.startsWith('./') || value.startsWith('../') || value.startsWith('.\\') || value.startsWith('..\\');
}
// A bare `C:` is not the drive root ("current directory on C:"), so it is never canonical as it stands.
function isRootPath(value: string): boolean {
  return value === '/' || value === '\\' || /^[a-zA-Z]:[/\\]$/.test(value);
}
function trimTrailingPathSeparators(value: string): string {
  if (value.length === 0 || isRootPath(value)) return value;
  const trimmed = value.startsWith('/') ? value.replace(/\/+$/g, '') : value.replace(/[\\/]+$/g, '');
  if (trimmed.length === 0) return value;
  return /^[a-zA-Z]:$/.test(trimmed) ? `${trimmed}\\` : trimmed;
}
export function normalizeProjectPathForDispatch(value: string): string {
  return trimTrailingPathSeparators(value.trim());
}
export function normalizeProjectPathForComparison(value: string): string {
  const normalized = normalizeProjectPathForDispatch(value);
  if (isWindowsDrivePath(normalized) || isUncPath(normalized)) return normalized.replaceAll('/', '\\').toLowerCase();
  return normalized;
}

// ── packages/client-runtime/src/state/projects.ts ────────────────────────
export const isWindowsPlatform = (platform: string): boolean => /^win(dows)?/i.test(platform);

function getAbsolutePathKind(value: string): 'unix' | 'windows' | null {
  if (isWindowsDrivePath(value) || isUncPath(value)) return 'windows';
  if (value.startsWith('/')) return 'unix';
  return null;
}
function preferredPathSeparator(value: string): '/' | '\\' {
  const kind = getAbsolutePathKind(value);
  if (kind === 'windows') return '\\';
  if (kind === 'unix') return '/';
  return value.includes('\\') ? '\\' : '/';
}
export function hasTrailingPathSeparator(value: string): boolean {
  return (getAbsolutePathKind(value) === 'unix' ? /\/$/ : /[\\/]$/).test(value);
}
function splitPathSegments(value: string, separator: '/' | '\\'): string[] {
  return value.split(separator === '/' ? /\/+/ : /[\\/]+/).filter(Boolean);
}
function getLastPathSeparatorIndex(value: string): number {
  if (getAbsolutePathKind(value) === 'unix') return value.lastIndexOf('/');
  return Math.max(value.lastIndexOf('/'), value.lastIndexOf('\\'));
}
function splitAbsolutePath(value: string): { root: string; separator: '/' | '\\'; segments: string[] } | null {
  if (isWindowsDrivePath(value)) {
    const root = `${value.slice(0, 2)}\\`;
    return { root, separator: '\\', segments: splitPathSegments(value.slice(root.length), '\\') };
  }
  if (isUncPath(value)) {
    const [server, share, ...rest] = splitPathSegments(value, '\\');
    if (!server || !share) return null;
    return { root: `\\\\${server}\\${share}\\`, separator: '\\', segments: rest };
  }
  if (value.startsWith('/')) return { root: '/', separator: '/', segments: splitPathSegments(value.slice(1), '/') };
  return null;
}

export function isFilesystemBrowseQuery(value: string, platform = ''): boolean {
  return value.startsWith('./') || value.startsWith('../') || value.startsWith('.\\') || value.startsWith('..\\') || value.startsWith('/')
    || value.startsWith('~/') || (isWindowsPlatform(platform) && isWindowsAbsolutePath(value));
}
export function isUnsupportedWindowsProjectPath(value: string, platform: string): boolean {
  return isWindowsAbsolutePath(value) && !isWindowsPlatform(platform);
}
export function resolveProjectPathForDispatch(value: string, cwd?: string | null): string {
  const trimmedValue = value.trim();
  if (!isExplicitRelativeProjectPath(trimmedValue) || !cwd) return normalizeProjectPathForDispatch(trimmedValue);
  const base = splitAbsolutePath(normalizeProjectPathForDispatch(cwd));
  if (!base) return normalizeProjectPathForDispatch(trimmedValue);
  const next = [...base.segments];
  for (const segment of trimmedValue.split(/[\\/]+/)) {
    if (segment.length === 0 || segment === '.') continue;
    if (segment === '..') { next.pop(); continue; }
    next.push(segment);
  }
  const joined = next.join(base.separator);
  return normalizeProjectPathForDispatch(joined.length === 0 ? base.root : `${base.root}${joined}`);
}

/** The project whose workspace root is this path, however its separators and trailing slash are written. */
export function findProjectByPath<T extends { workspaceRoot?: string; cwd?: string }>(projects: ReadonlyArray<T>, candidatePath: string): T | undefined {
  const candidate = normalizeProjectPathForComparison(candidatePath);
  if (candidate.length === 0) return undefined;
  return projects.find(project => {
    const cwd = project.workspaceRoot ?? project.cwd;
    return cwd ? normalizeProjectPathForComparison(cwd) === candidate : false;
  });
}

// Array.prototype.findLast(Boolean), spelled out for the app's JavaScript engine.
const lastNonEmpty = (segments: string[]): string | undefined => { for (let i = segments.length - 1; i >= 0; i--) if (segments[i]) return segments[i]; return undefined; };
/** A new project's title: the path's last segment. */
export function inferProjectTitleFromPath(value: string): string {
  const normalized = normalizeProjectPathForDispatch(value);
  const absolute = splitAbsolutePath(normalized);
  if (absolute) return lastNonEmpty(absolute.segments) ?? normalized;
  return lastNonEmpty(normalized.split(/[/\\]/)) ?? normalized;
}

export function appendBrowsePathSegment(currentPath: string, segment: string): string {
  return `${getBrowseDirectoryPath(currentPath)}${segment}${preferredPathSeparator(currentPath)}`;
}
export function getBrowseLeafPathSegment(currentPath: string): string {
  return currentPath.slice(getLastPathSeparatorIndex(currentPath) + 1);
}
export function getBrowseDirectoryPath(currentPath: string): string {
  if (hasTrailingPathSeparator(currentPath)) return currentPath;
  const last = getLastPathSeparatorIndex(currentPath);
  return last < 0 ? currentPath : currentPath.slice(0, last + 1);
}
export function ensureBrowseDirectoryPath(currentPath: string): string {
  const trimmed = currentPath.trim();
  if (trimmed.length === 0 || hasTrailingPathSeparator(trimmed)) return trimmed;
  return `${trimmed}${preferredPathSeparator(trimmed)}`;
}
export function getBrowseParentPath(currentPath: string): string | null {
  const trimmed = normalizeProjectPathForDispatch(currentPath);
  const absolute = splitAbsolutePath(trimmed);
  if (absolute) {
    if (absolute.segments.length === 0) return null;
    if (absolute.segments.length === 1) return absolute.root;
    return `${absolute.root}${absolute.segments.slice(0, -1).join(absolute.separator)}${absolute.separator}`;
  }
  const separator = preferredPathSeparator(currentPath);
  const last = getLastPathSeparatorIndex(trimmed);
  if (last < 0) return null;
  if (last === 2 && /^[a-zA-Z]:/.test(trimmed)) return `${trimmed.slice(0, 2)}${separator}`;
  return trimmed.slice(0, last + 1);
}
export function canNavigateUp(currentPath: string): boolean {
  return hasTrailingPathSeparator(currentPath) && getBrowseParentPath(currentPath) !== null;
}
