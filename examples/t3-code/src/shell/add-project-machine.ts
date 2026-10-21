// Adding a project to another connected machine, ported from T3 Code 74541bab17 (MIT; see
// LICENSE-T3): apps/web/src/lib/addProjectToMachine.ts (clone URL protocols, the default env-file
// pick, the clone start and the ignored-file relay) and hooks/useAddProjectTargets.ts (which
// machines can take the project). The two servers never talk to each other, so the client reads
// the chosen ignored files from the source, starts the tracked clone on the target, and writes
// the files there once the clone is done. The reference waits in a detached promise; here each
// shell pass checks the target's clone list (project-clones-live.ts cloneToasts calls
// fileCopyPass, which starts the writes without holding the pass), because data sources and
// commands have no timers.
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { cloneTracking, liveEnvironment, type LiveEnvironment } from '../client/live-streams';
import { fleet, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { isPrimaryEnvironment } from '../connections/local-primary';
import { machineKind } from '../connections/connections';
import { environmentIndicator } from '../details/shell-details';
import { environmentOptions } from '../details/git-env';
import { letGo } from '../client/let-go';
import { wakeShell } from '../client/command-waits';

export type CloneProtocol = 'ssh' | 'https';

/** Matches the server's read cap: a larger file would arrive truncated. */
export const COPYABLE_FILE_MAX_BYTES = 1024 * 1024;

const SCP_LIKE_URL = /^(?:[^@/\s]+@)?([^:/\s]+):(?!\/)(.+)$/;
const SSH_URL = /^ssh:\/\/(?:[^@/\s]+@)?([^:/\s]+)(:\d+)?\/(.+)$/;
const HTTPS_URL = /^https?:\/\/(?:[^@/\s]+@)?([^:/\s]+)(:\d+)?\/(.+)$/;

function parseCloneUrl(url: string): { protocol: CloneProtocol; host: string; path: string; customPort: boolean } | null {
  const trimmed = url.trim();
  const ssh = SSH_URL.exec(trimmed);
  if (ssh) return { protocol: 'ssh', host: ssh[1]!, path: ssh[3]!, customPort: !!ssh[2] };
  const https = HTTPS_URL.exec(trimmed);
  if (https) return { protocol: 'https', host: https[1]!, path: https[3]!, customPort: !!https[2] };
  const scp = SCP_LIKE_URL.exec(trimmed);
  if (scp) return { protocol: 'ssh', host: scp[1]!, path: scp[2]!, customPort: false };
  return null;
}

export function cloneUrlProtocol(url: string): CloneProtocol | null {
  return parseCloneUrl(url)?.protocol ?? null;
}

/**
 * Rewrites a clone URL to the other protocol, because the target machine may
 * only have SSH keys or only an HTTPS credential helper. Returns null when the
 * URL cannot be rewritten safely, such as a custom port.
 */
export function cloneUrlForProtocol(url: string, protocol: CloneProtocol): string | null {
  const parsed = parseCloneUrl(url);
  if (!parsed) return null;
  if (parsed.protocol === protocol) return url.trim();
  if (parsed.customPort) return null;
  return protocol === 'ssh' ? `git@${parsed.host}:${parsed.path}` : `https://${parsed.host}/${parsed.path}`;
}

const DEFAULT_COPIED_FILE_NAME = /^(?:\.env(?:\..+)?|\.envrc|\.dev\.vars)$/;

/** Env files are the ignored files people almost always want on a new machine. */
export function isDefaultCopiedFile(path: string): boolean {
  const name = path.split('/').at(-1) ?? path;
  return DEFAULT_COPIED_FILE_NAME.test(name);
}

export type IgnoredFile = { path: string; byteLength: number };

/** AddProjectToMachineDialog defaultSelection: the env files small enough to copy. */
export function defaultSelection(files: readonly IgnoredFile[]): Set<string> {
  return new Set(files.filter(file => file.byteLength <= COPYABLE_FILE_MAX_BYTES && isDefaultCopiedFile(file.path)).map(file => file.path));
}

/**
 * A project can be rooted below its repository's top folder. The clone lands
 * at the top, so copied files keep the same path below it.
 */
export function projectSubdirectory(repositoryRoot: string | undefined, workspaceRoot: string): string {
  if (!repositoryRoot) return '';
  const prefix = repositoryRoot.replace(/[\\/]+$/, '');
  const rest = workspaceRoot.slice(prefix.length);
  return workspaceRoot.startsWith(prefix) && /^[\\/]/.test(rest) ? rest.slice(1).replaceAll('\\', '/') : '';
}

/** The project's git remote, which a target clones (repositoryIdentity.locator.remoteUrl). */
export const projectRemoteUrl = (project: Obj | undefined) => str(obj(obj(project?.repositoryIdentity).locator).remoteUrl);

export type AddProjectTarget = { environmentId: string; label: string; machine: string };

/**
 * useAddProjectTargets: connected machines that do not have the project yet and can clone it (the
 * source-control write scope, a connection that can create projects, and clone tracking). Projects
 * without a git remote get none. The project is the focused client's; a background machine counts
 * once its shell is synchronized, as the Run on options do (git-env.ts environmentOptions).
 */
export function addProjectTargets(client: T3Client, source: EnvironmentFleet = fleet): AddProjectTarget[] {
  const project = client.shell.projects.find(entry => entry.id === client.projectId);
  if (!client.environmentId || !projectRemoteUrl(project)) return [];
  const hasProject = new Set([client.environmentId, ...environmentOptions(client, source).map(option => option.id)]);
  const targets: AddProjectTarget[] = [];
  for (const entry of source.entries.values()) {
    if (!entry.environmentId || hasProject.has(entry.environmentId) || targets.some(target => target.environmentId === entry.environmentId)) continue;
    if (entry.phase !== 'connected' || entry.synchronized !== entry.generation) continue;
    if (!entry.scopes.includes('source-control:write') || !cloneTracking(entry.config)) continue;
    const saved = source.saved.find(item => str(item.environmentId) === entry.environmentId);
    const label = environmentIndicator({ isPrimary: isPrimaryEnvironment(entry.environmentId), available: 2, environmentId: entry.environmentId,
      runtimeLabel: str(obj(entry.config.environment).label), savedLabel: str(saved?.label), machine: machineKind(entry.config) }).envLabel;
    targets.push({ environmentId: entry.environmentId, label, machine: machineKind(entry.config, str(saved?.machine)) });
  }
  return targets.sort((left, right) => left.label.localeCompare(right.label));
}

// ── The ignored-file copies a cloned project is still owed ───────────────────

type Read = { relativePath: string; contents: string } | { relativePath: string; error: string };
/**
 * One copy, keyed by the target's scoped project key. `copying` holds the reads made when the user
 * asked, until the clone is done; `failed` shows in the composer until dismissed.
 */
export type ProjectFileCopy =
  | { phase: 'copying'; environmentId: string; projectId: string; label: string; cwd: string; subdirectory: string; reads: Read[]; seen: boolean; writing: boolean }
  | { phase: 'failed'; title: string; failures: string[] };

type MachineState = { copies: Map<string, ProjectFileCopy>; silenced: Set<string> };
const states = new WeakMap<T3Client, MachineState>();
function stateOf(client: T3Client): MachineState {
  let state = states.get(client);
  if (!state) { state = { copies: new Map(), silenced: new Set() }; states.set(client, state); }
  return state;
}
const scopedKey = (environmentId: string, projectId: string) => `${environmentId}:${projectId}`;

/** The composer holds sends until the copy lands, and shows a failed copy until dismissed. */
export function projectFileCopy(client: T3Client, environmentId: string, projectId: string): ProjectFileCopy | null {
  return stateOf(client).copies.get(scopedKey(environmentId, projectId)) ?? null;
}
export function dismissProjectFileCopy(client: T3Client, environmentId: string, projectId: string): void {
  stateOf(client).copies.delete(scopedKey(environmentId, projectId));
}

/**
 * state/projectClones.ts silenceProjectCloneToasts: keeps a clone's progress out of toasts once
 * the user is looking at its project, whose composer shows the same progress. Failures still toast
 * whenever the project is not on screen.
 */
export function silenceProjectCloneToasts(client: T3Client, environmentId: string, projectId: string): void {
  stateOf(client).silenced.add(scopedKey(environmentId, projectId));
}
export function isProjectCloneToastSilenced(client: T3Client, environmentId: string, projectId: string): boolean {
  return stateOf(client).silenced.has(scopedKey(environmentId, projectId));
}

export type AddProjectToMachineInput = {
  source: { environmentId: string; cwd: string };
  /** Where the source project sits below its repository root, or "". */
  subdirectory: string;
  target: { environmentId: string; label: string };
  projectId: string;
  title: string;
  remoteUrl: string;
  destinationPath: string;
  files: readonly string[];
  /** The clone's createdAt (data sources cannot read the clock). */
  createdAt: string;
};
const failureMessage = (error: unknown) => error instanceof Error && error.message ? error.message : 'An error occurred.';
function environmentOf(client: T3Client, native: Native, environmentId: string): LiveEnvironment {
  const environment = liveEnvironment(client, native, environmentId);
  if (!environment?.connected) throw new ClientError('That environment is not connected.');
  return environment;
}

/**
 * Clones a project onto another machine, then copies the chosen ignored files
 * from the source. Resolves once the clone starts. The caller opens the new
 * project, whose composer shows the clone and copy progress, so nothing toasts.
 */
export async function addProjectToMachine(client: T3Client, native: Native, input: AddProjectToMachineInput): Promise<void> {
  const sourceEnvironment = environmentOf(client, native, input.source.environmentId);
  // Read first, so the copy reflects the files as they were when the user asked.
  const reads: Read[] = await Promise.all(input.files.map(async (relativePath): Promise<Read> => {
    try {
      const result = await sourceEnvironment.request('projects.readFile', { cwd: input.source.cwd, relativePath });
      if (result.truncated === true) return { relativePath, error: 'Too large to copy.' };
      return { relativePath, contents: str(result.contents) };
    } catch (error) {
      if (letGo(error)) throw error;
      return { relativePath, error: failureMessage(error) };
    }
  }));
  const state = stateOf(client), key = scopedKey(input.target.environmentId, input.projectId);
  // Marked before the start request, since the clone list can arrive before its reply.
  silenceProjectCloneToasts(client, input.target.environmentId, input.projectId);
  const copy: ProjectFileCopy = { phase: 'copying', environmentId: input.target.environmentId, projectId: input.projectId, label: input.target.label,
    cwd: '', subdirectory: input.subdirectory, reads, seen: false, writing: false };
  if (reads.length > 0) state.copies.set(key, copy);
  let started: Obj;
  try {
    started = await environmentOf(client, native, input.target.environmentId).request('projectClone.start', { projectId: input.projectId, title: input.title,
      createdAt: input.createdAt, remoteUrl: input.remoteUrl, destinationPath: input.destinationPath }, true);
  } catch (error) {
    state.copies.delete(key);
    throw error;
  }
  copy.cwd = str(started.cwd, input.destinationPath);
}

/**
 * The reference's waitForClone and the copy after it, checked on each shell pass: a copy whose clone is done
 * writes its files to the target, detached from the pass (the `writing` guard keeps a later pass from starting it
 * again), and wakes the shell when it is through; a clone that left the list after it was seen drops its copy. A
 * failed or cancelled clone keeps waiting, since the composer offers a retry. Resolves when the copies it started end.
 */
export function fileCopyPass(client: T3Client, native: Native): Promise<void> {
  const state = stateOf(client), started: Promise<void>[] = [];
  for (const [key, copy] of [...state.copies]) {
    if (copy.phase !== 'copying' || copy.writing || !copy.cwd) continue;
    const environment = liveEnvironment(client, native, copy.environmentId);
    const clones = environment?.connected ? environment.clones.value : null;
    if (!environment || !clones) continue;
    const clone = clones.find(candidate => candidate.projectId === copy.projectId);
    if (clone) copy.seen = true;
    if (!clone) { if (copy.seen) state.copies.delete(key); continue; }
    if (clone.phase !== 'done') continue;
    copy.writing = true;
    started.push(writeCopy(client, native, environment, key, copy).finally(() => wakeShell(native)));
  }
  return Promise.all(started).then(() => undefined);
}

async function writeCopy(client: T3Client, native: Native, environment: LiveEnvironment, key: string, copy: Extract<ProjectFileCopy, { phase: 'copying' }>): Promise<void> {
  const state = stateOf(client), failed: string[] = [];
  for (const read of copy.reads) {
    if (!('contents' in read)) { failed.push(`${read.relativePath}: ${read.error}`); continue; }
    try {
      await environment.request('projects.writeFile', { cwd: copy.cwd, relativePath: copy.subdirectory ? `${copy.subdirectory}/${read.relativePath}` : read.relativePath, contents: read.contents }, true);
    } catch (error) {
      // A connection that went away leaves the copy owed; the next pass on a live connection tries again.
      if (letGo(error)) { copy.writing = false; return; }
      failed.push(`${read.relativePath}: ${failureMessage(error)}`);
    }
  }
  if (state.copies.get(key) !== copy) return;
  if (failed.length === 0) state.copies.delete(key);
  else state.copies.set(key, { phase: 'failed', title: `Could not copy ${failed.length} ${failed.length === 1 ? 'file' : 'files'} to ${copy.label}`, failures: failed });
}
