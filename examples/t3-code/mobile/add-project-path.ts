// @ref llp/1107.005-composer-and-transcript.decision.md#new-task-ownership
// T3 Code 365aa87982, MIT (LICENSE-T3): AddProjectScreen.logic.ts,
// operations/projects.ts, state/{projects,filesystem}.ts and shared/path.ts.
import { isWindowsDrivePath, isUncPath, isWindowsAbsolutePath } from './shared/media-reference';
import { arr, obj, str, type Obj } from './shared/domain';

export function isExplicitRelativePath(value: string): boolean {
  return (
    value === "." ||
    value === ".." ||
    value.startsWith("./") ||
    value.startsWith("../") ||
    value.startsWith(".\\") ||
    value.startsWith("..\\")
  );
}

function isRootPath(value: string): boolean {
  // The drive separator is required: a bare `C:` is not the drive root (it
  // means "current directory on C:"), and treating it as already-canonical
  // would leave it as `C:` while `C:\` and `C:/` normalize to the drive root,
  // so the same location would fail project identity/dedup comparisons.
  return value === "/" || value === "\\" || /^[a-zA-Z]:[/\\]$/.test(value);
}

function trimTrailingPathSeparators(value: string): string {
  if (value.length === 0 || isRootPath(value)) {
    return value;
  }
  const trimmed = value.startsWith("/")
    ? value.replace(/\/+$/g, "")
    : value.replace(/[\\/]+$/g, "");
  if (trimmed.length === 0) {
    return value;
  }
  return /^[a-zA-Z]:$/.test(trimmed) ? `${trimmed}\\` : trimmed;
}

export function normalizeProjectPathForDispatch(value: string): string {
  return trimTrailingPathSeparators(value.trim());
}

export function normalizeProjectPathForComparison(value: string): string {
  const normalized = normalizeProjectPathForDispatch(value);
  if (isWindowsDrivePath(normalized) || isUncPath(normalized)) {
    return normalized.replaceAll("/", "\\").toLowerCase();
  }
  return normalized;
}

// Windows refuses these as file names, with or without an extension.
const WINDOWS_RESERVED_NAME = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/;

/**
 * Folder name for a project started from just a name ("Pinball Stats" becomes
 * "pinball-stats"). The server uses it for `projects.createNew`, and clients
 * use it to show the path before the server makes it.
 */
export function newProjectFolderName(name: string): string {
  const slug = name
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 64)
    .replace(/-+$/, "");
  if (slug.length === 0) return "project";
  return WINDOWS_RESERVED_NAME.test(slug) ? `${slug}-project` : slug;
}

export const isWindowsPlatform = (platform: string): boolean => {
  return /^win(dows)?/i.test(platform);
};

function getAbsolutePathKind(value: string): "unix" | "windows" | null {
  if (isWindowsDrivePath(value) || isUncPath(value)) {
    return "windows";
  }
  if (value.startsWith("/")) {
    return "unix";
  }
  return null;
}

function preferredPathSeparator(value: string): "/" | "\\" {
  const absolutePathKind = getAbsolutePathKind(value);
  if (absolutePathKind === "windows") return "\\";
  if (absolutePathKind === "unix") return "/";
  return value.includes("\\") ? "\\" : "/";
}

export function hasTrailingPathSeparator(value: string): boolean {
  return (getAbsolutePathKind(value) === "unix" ? /\/$/ : /[\\/]$/).test(value);
}

export { isExplicitRelativePath as isExplicitRelativeProjectPath };

function splitPathSegments(value: string, separator: "/" | "\\"): string[] {
  return value.split(separator === "/" ? /\/+/ : /[\\/]+/).filter(Boolean);
}

function getLastPathSeparatorIndex(value: string): number {
  if (getAbsolutePathKind(value) === "unix") {
    return value.lastIndexOf("/");
  }
  return Math.max(value.lastIndexOf("/"), value.lastIndexOf("\\"));
}

function splitAbsolutePath(value: string): {
  root: string;
  separator: "/" | "\\";
  segments: string[];
} | null {
  if (isWindowsDrivePath(value)) {
    const root = `${value.slice(0, 2)}\\`;
    const segments = splitPathSegments(value.slice(root.length), "\\");
    return { root, separator: "\\", segments };
  }
  if (isUncPath(value)) {
    const segments = splitPathSegments(value, "\\");
    const [server, share, ...rest] = segments;
    if (!server || !share) return null;
    return {
      root: `\\\\${server}\\${share}\\`,
      separator: "\\",
      segments: rest,
    };
  }
  if (value.startsWith("/")) {
    return {
      root: "/",
      separator: "/",
      segments: splitPathSegments(value.slice(1), "/"),
    };
  }
  return null;
}

export function isFilesystemBrowseQuery(value: string, platform = ""): boolean {
  const allowWindowsPaths = isWindowsPlatform(platform);
  return (
    value.startsWith("./") ||
    value.startsWith("../") ||
    value.startsWith(".\\") ||
    value.startsWith("..\\") ||
    value.startsWith("/") ||
    value.startsWith("~/") ||
    (allowWindowsPaths && isWindowsAbsolutePath(value))
  );
}

export function isUnsupportedWindowsProjectPath(value: string, platform: string): boolean {
  return isWindowsAbsolutePath(value) && !isWindowsPlatform(platform);
}

export function resolveProjectPathForDispatch(value: string, cwd?: string | null): string {
  const trimmedValue = value.trim();
  if (!isExplicitRelativePath(trimmedValue) || !cwd) {
    return normalizeProjectPathForDispatch(trimmedValue);
  }

  const absoluteBase = splitAbsolutePath(normalizeProjectPathForDispatch(cwd));
  if (!absoluteBase) {
    return normalizeProjectPathForDispatch(trimmedValue);
  }

  const nextSegments = [...absoluteBase.segments];
  for (const segment of trimmedValue.split(/[\\/]+/)) {
    if (segment.length === 0 || segment === ".") continue;
    if (segment === "..") {
      nextSegments.pop();
      continue;
    }
    nextSegments.push(segment);
  }

  const joinedPath = nextSegments.join(absoluteBase.separator);
  return normalizeProjectPathForDispatch(
    joinedPath.length === 0 ? absoluteBase.root : `${absoluteBase.root}${joinedPath}`,
  );
}

export function findProjectByPath<T extends { workspaceRoot?: string; cwd?: string }>(
  projects: ReadonlyArray<T>,
  candidatePath: string,
): T | undefined {
  const normalizedCandidate = normalizeProjectPathForComparison(candidatePath);
  if (normalizedCandidate.length === 0) {
    return undefined;
  }
  return projects.find((project) => {
    const cwd = project.workspaceRoot ?? project.cwd;
    return cwd ? normalizeProjectPathForComparison(cwd) === normalizedCandidate : false;
  });
}

export function inferProjectTitleFromPath(value: string): string {
  const normalized = normalizeProjectPathForDispatch(value);
  const absolutePath = splitAbsolutePath(normalized);
  if (absolutePath) {
    return absolutePath.segments.findLast(Boolean) ?? normalized;
  }
  const segments = normalized.split(/[/\\]/);
  return segments.findLast(Boolean) ?? normalized;
}

export function appendBrowsePathSegment(currentPath: string, segment: string): string {
  const separator = preferredPathSeparator(currentPath);
  return `${getBrowseDirectoryPath(currentPath)}${segment}${separator}`;
}

export function getBrowseLeafPathSegment(currentPath: string): string {
  const lastSeparatorIndex = getLastPathSeparatorIndex(currentPath);
  return currentPath.slice(lastSeparatorIndex + 1);
}

export function getBrowseDirectoryPath(currentPath: string): string {
  if (hasTrailingPathSeparator(currentPath)) {
    return currentPath;
  }
  const lastSeparatorIndex = getLastPathSeparatorIndex(currentPath);
  return lastSeparatorIndex < 0 ? currentPath : currentPath.slice(0, lastSeparatorIndex + 1);
}

export function ensureBrowseDirectoryPath(currentPath: string): string {
  const trimmed = currentPath.trim();
  if (trimmed.length === 0 || hasTrailingPathSeparator(trimmed)) {
    return trimmed;
  }
  return `${trimmed}${preferredPathSeparator(trimmed)}`;
}

export function getBrowseParentPath(currentPath: string): string | null {
  const trimmed = normalizeProjectPathForDispatch(currentPath);
  const absolutePath = splitAbsolutePath(trimmed);
  if (absolutePath) {
    if (absolutePath.segments.length === 0) return null;
    if (absolutePath.segments.length === 1) return absolutePath.root;
    const parentSegments = absolutePath.segments.slice(0, -1).join(absolutePath.separator);
    return `${absolutePath.root}${parentSegments}${absolutePath.separator}`;
  }

  const separator = preferredPathSeparator(currentPath);
  const lastSeparatorIndex = getLastPathSeparatorIndex(trimmed);
  if (lastSeparatorIndex < 0) return null;
  if (lastSeparatorIndex === 2 && /^[a-zA-Z]:/.test(trimmed)) {
    return `${trimmed.slice(0, 2)}${separator}`;
  }
  return trimmed.slice(0, lastSeparatorIndex + 1);
}

export function canNavigateUp(currentPath: string): boolean {
  return hasTrailingPathSeparator(currentPath) && getBrowseParentPath(currentPath) !== null;
}


export type AddProjectRemoteProviderKind = "github" | "gitlab" | "forgejo" | "bitbucket" | "azure-devops";
export type AddProjectRemoteSource = AddProjectRemoteProviderKind | "url";

export function canCreateProjectInEnvironment(
  connectionPhase: string | null | undefined,
): boolean {
  return connectionPhase === "connected";
}

export type AddProjectRemoteSourceReadiness = Record<
  AddProjectRemoteSource,
  { readonly ready: boolean; readonly hint: string | null }
>;

const ADD_PROJECT_REMOTE_SOURCES: ReadonlyArray<AddProjectRemoteSource> = [
  "url",
  "github",
  "gitlab",
  "forgejo",
  "bitbucket",
  "azure-devops",
];

const ADD_PROJECT_REMOTE_PROVIDER_SOURCES: ReadonlyArray<AddProjectRemoteProviderKind> = [
  "github",
  "gitlab",
  "forgejo",
  "bitbucket",
  "azure-devops",
];

export function addProjectRemoteSourceLabel(source: AddProjectRemoteSource): string {
  switch (source) {
    case "github":
      return "GitHub";
    case "forgejo":
      return "Forgejo / Gitea";
    case "gitlab":
      return "GitLab";
    case "bitbucket":
      return "Bitbucket";
    case "azure-devops":
      return "Azure DevOps";
    case "url":
      return "Git URL";
  }
}

export function addProjectRemoteSourcePathHint(source: AddProjectRemoteSource): string {
  switch (source) {
    case "forgejo":
    case "github":
      return "owner/repo";
    case "gitlab":
      return "group/project";
    case "bitbucket":
      return "workspace/repository";
    case "azure-devops":
      return "project/repository";
    case "url":
      return "URL";
  }
}

export function addProjectRemoteSourceProvider(
  source: AddProjectRemoteSource,
): AddProjectRemoteProviderKind | null {
  return source === "url" ? null : source;
}

const GITHUB_REPOSITORY_SHORTHAND =
  /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})\/[A-Za-z0-9._-]+(?:\.git)?$/;

/** Treat the common owner/repository shorthand as a public GitHub HTTPS URL. */
export function normalizePastedCloneUrl(input: string): string {
  const trimmed = input.trim();
  if (!GITHUB_REPOSITORY_SHORTHAND.test(trimmed)) return trimmed;
  const repository = trimmed.endsWith(".git") ? trimmed : `${trimmed}.git`;
  return `https://github.com/${repository}`;
}

/** GitHub and Forgejo default to HTTPS; other providers retain their existing SSH default. */
export function getDefaultCloneUrl(
  repository: { provider: string; url: string; sshUrl: string },
): string {
  return repository.provider === "github" || repository.provider === "forgejo"
    ? repository.url
    : repository.sshUrl;
}

export function sortAddProjectProviderSources(
  readinessBySource: AddProjectRemoteSourceReadiness,
): ReadonlyArray<AddProjectRemoteProviderKind> {
  return [...ADD_PROJECT_REMOTE_PROVIDER_SOURCES].sort((a, b) => {
    const ready = Number(readinessBySource[b].ready) - Number(readinessBySource[a].ready);
    const left = addProjectRemoteSourceLabel(a), right = addProjectRemoteSourceLabel(b);
    return ready || (left < right ? -1 : left > right ? 1 : 0);
  });
}

export function buildAddProjectRemoteSourceReadiness(
  discovery: Obj | null,
): AddProjectRemoteSourceReadiness {
  const unavailable = {
    ready: false,
    hint: "Provider status unavailable. Open Source Control settings and rescan.",
  } as const;
  const readiness: AddProjectRemoteSourceReadiness = {
    url: { ready: true, hint: null },
    github: unavailable,
    gitlab: unavailable,
    forgejo: unavailable,
    bitbucket: unavailable,
    "azure-devops": unavailable,
  };

  if (!discovery) {
    return readiness;
  }

  const providerByKind = new Map(
    arr(discovery.sourceControlProviders).map((provider) => [str(provider.kind), provider]),
  );
  for (const source of ADD_PROJECT_REMOTE_SOURCES) {
    const kind = addProjectRemoteSourceProvider(source);
    if (!kind) continue;
    const provider = providerByKind.get(kind);
    if (!provider) {
      readiness[source] = unavailable;
      continue;
    }
    if (provider.status !== "available") {
      readiness[source] = { ready: false, hint: str(provider.installHint) };
      continue;
    }
    if (obj(provider.auth).status === "unauthenticated") {
      readiness[source] = {
        ready: false,
        hint:
          optionString(obj(provider.auth).detail) ??
          `${provider.label} is not authenticated. Open Source Control settings for setup guidance.`,
      };
      continue;
    }
    readiness[source] = { ready: true, hint: null };
  }
  return readiness;
}

export function getAddProjectInitialQuery(baseDirectory: string | null | undefined): string {
  const trimmed = baseDirectory?.trim() ?? "";
  return trimmed.length === 0 ? "~/" : ensureBrowseDirectoryPath(trimmed);
}

/**
 * Folder name `git clone` would pick, from either a looked-up repository or a
 * pasted clone URL. Providers report `owner/repo`, Azure DevOps reports
 * `org/project/repo`, and a URL can arrive in any form: `https://host/owner/
 * repo.git`, `ssh://git@host:22/owner/repo`, `git@host:owner/repo.git`, with
 * or without a query, a fragment or a trailing slash. The repository is always
 * the last segment, minus the `.git` suffix.
 */
export function getCloneDirectoryName(repositoryOrRemoteUrl: string | null | undefined): string {
  const withoutQuery = (repositoryOrRemoteUrl ?? "").split(/[?#]/)[0]?.trim() ?? "";
  const schemeIndex = withoutQuery.indexOf("://");
  // A remote URL carries a host before the repository path. The host is never
  // the repository, so a link that stops at the host, or at a port, names
  // nothing and the destination falls back to the browsed folder.
  const hasHost = schemeIndex >= 0 || /^[^/\\:]+@[^/\\:]+:/.test(withoutQuery);
  const pathPart = schemeIndex >= 0 ? withoutQuery.slice(schemeIndex + "://".length) : withoutQuery;
  const segments = pathPart.split(/[/\\:]+/).filter((segment) => segment.trim().length > 0);
  if (hasHost && segments.length < 2) {
    return "";
  }

  const lastSegment = segments.at(-1)?.trim() ?? "";
  // A port can only sit directly behind the authority, so it is a port only
  // when nothing follows it. Deeper segments are path, even when numeric: the
  // repository in `https://host/acme/123` really is named `123`.
  if (hasHost && segments.length === 2 && /^\d+$/.test(lastSegment)) {
    return "";
  }
  return lastSegment.endsWith(".git") ? lastSegment.slice(0, -".git".length) : lastSegment;
}

/**
 * Clone destination proposed for a directory: the directory the user picked
 * plus the repository folder inside it. Without a name the directory is the
 * destination, which is what the raw clone URL flow keeps doing.
 */
export function getCloneDestinationPath(
  directoryPath: string,
  directoryName: string | null | undefined,
): string {
  const name = directoryName?.trim() ?? "";
  if (name.length === 0) {
    return directoryPath;
  }
  return `${ensureBrowseDirectoryPath(directoryPath)}${name}`;
}

/**
 * Where `projects.createNew` will put a project named `name`. The server adds
 * `-2`, `-3`, ... when that folder is taken, so this is a preview.
 */
export function getNewProjectPathPreview(newProjectsRoot: string, name: string): string {
  return getCloneDestinationPath(newProjectsRoot, newProjectFolderName(name));
}

/**
 * The GitHub account a new project would be published under, or null when
 * GitHub is not ready on that environment. A ready GitHub with an unknown
 * account still publishes; `gh` picks the signed-in user.
 */
export function getNewProjectGitHubTarget(
  discovery: Obj | null,
): { readonly account: string | null } | null {
  if (!buildAddProjectRemoteSourceReadiness(discovery).github.ready) return null;
  const github = arr(discovery?.sourceControlProviders).find((provider) => provider.kind === "github");
  return { account: github ? optionString(obj(github.auth).account) : null };
}

/** `owner/folder` for publishing a new project, or just the folder for `gh` to place. */
export function getNewProjectGitHubRepository(
  target: { readonly account: string | null },
  workspaceRoot: string,
): string {
  const folderName = workspaceRoot.split(/[\\/]/).filter(Boolean).at(-1) ?? "";
  return target.account ? `${target.account}/${folderName}` : folderName;
}

/**
 * Destination query after choosing a directory while the clone folder is
 * pinned in the path input. Selecting an existing directory with the pinned
 * name uses that directory directly instead of producing `repo/repo`.
 */
export function getCloneDestinationBrowsePath(input: {
  readonly browseDirectoryPath: string;
  readonly selectedDirectoryName: string;
  readonly cloneDirectoryName: string;
  readonly caseSensitive: boolean;
}): string {
  const selectedDirectoryPath = appendBrowsePathSegment(
    input.browseDirectoryPath,
    input.selectedDirectoryName,
  );
  const selectedDirectoryMatches = input.caseSensitive
    ? input.selectedDirectoryName === input.cloneDirectoryName
    : input.selectedDirectoryName.toLowerCase() === input.cloneDirectoryName.toLowerCase();
  return selectedDirectoryMatches
    ? selectedDirectoryPath
    : getCloneDestinationPath(selectedDirectoryPath, input.cloneDirectoryName);
}

export function resolveAddProjectPath(input: {
  readonly rawPath: string;
  readonly currentProjectCwd?: string | null;
  readonly platform: string;
}): { readonly ok: true; readonly path: string } | { readonly ok: false; readonly error: string } {
  const rawPath = input.rawPath.trim();
  if (rawPath.length === 0) {
    return { ok: false, error: "Enter a project path." };
  }
  if (isUnsupportedWindowsProjectPath(rawPath, input.platform)) {
    return { ok: false, error: "Windows-style paths are only supported on Windows environments." };
  }
  if (isExplicitRelativePath(rawPath) && !input.currentProjectCwd) {
    return { ok: false, error: "Relative paths require an active project in this environment." };
  }
  const path = resolveProjectPathForDispatch(rawPath, input.currentProjectCwd);
  return path.length === 0 ? { ok: false, error: "Enter a project path." } : { ok: true, path };
}

export function findExistingAddProject<T extends { environmentId: string; workspaceRoot?: string; cwd?: string }>(input: {
  readonly projects: ReadonlyArray<T>;
  readonly environmentId: string;
  readonly path: string;
}): T | null {
  return findProjectByPath(input.projects.filter(project => project.environmentId === input.environmentId), input.path) ?? null;
}

export function buildProjectCreateCommand(input: { commandId: string; projectId: string; workspaceRoot: string }): Obj {
  return { type: 'project.create', commandId: input.commandId, projectId: input.projectId,
    title: inferProjectTitleFromPath(input.workspaceRoot), workspaceRoot: input.workspaceRoot,
    createWorkspaceRootIfMissing: true, defaultModelSelection: null };
}

// Shared native replies use JSON strings/null; decoded Effect Options are accepted too.
function optionString(value: unknown): string | null {
  if (typeof value === 'string') return value;
  const option = obj(value);
  return option._tag === 'Some' && typeof option.value === 'string' ? option.value : null;
}

export function resolveAddProjectEnvironment<T extends { environmentId: string; connectionState: string }>(
  environmentOptions: ReadonlyArray<T>, requestedEnvironmentId: string | null,
): T | null {
  return environmentOptions.find(environment => canCreateProjectInEnvironment(environment.connectionState)
    && (requestedEnvironmentId === null || environment.environmentId === requestedEnvironmentId)) ?? null;
}

export function platformFromOs(os: string | null | undefined): string {
  return os === 'windows' ? 'Win32' : os === 'darwin' ? 'MacIntel' : os === 'linux' ? 'Linux' : '';
}
export function stringParam(value: string | string[] | undefined): string | null {
  return Array.isArray(value) ? value[0] ?? null : value ?? null;
}
export function sourceFromParam(value: string | string[] | undefined): AddProjectRemoteSource {
  const source = stringParam(value);
  return ADD_PROJECT_REMOTE_SOURCES.find(candidate => candidate === source) ?? 'url';
}
export function getFilesystemBrowsePath(query: string, platform = '', enabled = true) {
  const isBrowsing = enabled && isFilesystemBrowseQuery(query, platform);
  const directoryPath = isBrowsing ? getBrowseDirectoryPath(query) : '';
  const filterQuery = isBrowsing && !hasTrailingPathSeparator(query) ? getBrowseLeafPathSegment(query) : '';
  return { isBrowsing, directoryPath, filterQuery, parentPath: isBrowsing ? getBrowseParentPath(directoryPath) : null,
    canBrowseUp: isBrowsing && canNavigateUp(directoryPath) };
}
export function filterFilesystemBrowseEntries<T extends { name: string }>(entries: ReadonlyArray<T>, query: string) {
  const lowerQuery = query.toLowerCase(), showHidden = query.startsWith('.');
  const visibleEntries = entries.filter(entry => entry.name.toLowerCase().startsWith(lowerQuery) && (showHidden || !entry.name.startsWith('.')));
  return { visibleEntries, exactEntry: query.length > 0 ? visibleEntries.find(entry => entry.name === query) ?? null : null };
}
export function getPinnedBrowseFilter(query: string, pinned: string, platform: string): string {
  return (isWindowsPlatform(platform) ? query.toLowerCase() === pinned.toLowerCase() : query === pinned) ? '' : query;
}
