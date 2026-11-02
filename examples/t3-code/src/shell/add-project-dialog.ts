// AddProjectToMachineDialog, ported from T3 Code 74541bab17 (MIT; see LICENSE-T3):
// apps/web/src/components/AddProjectToMachineDialog.tsx. The Run on menus open it
// (`shelllocal:add-machine-open`, with the machine picked from the submenu or none), its
// fields are `shelllocal:add-machine-*`, and "Clone to <machine>" is the connection op
// `add-machine-submit` (connections.ts), because landing on the new machine's project moves
// the window there and T3Client adopts that connection. The dialog is drawn by
// add-project-machine.contract from addMachineView.
import type { T3Client } from '../client/client';
import { decodeIgnoredFiles, obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { fleet, focusFleetProject, type EnvironmentFleet } from '../connections/settings-b-fleet';
import { focusedOnPrimary } from '../connections/local-primary';
import { machineKind } from '../connections/connections';
import { environmentIndicator } from '../details/shell-details';
import { addMenuWidth } from '../details/git-env';
import { cloneDestination, cloneDirectoryName, initialBrowseQuery } from '../palette/palette-add';
import { formatBytes } from '../settings/storage-maintenance';
import { draftSelection, moveDraftSelection } from '../composer/auto-balance';
import { composerNow } from '../composer/composer-controls';
import { wakeShell } from '../client/command-waits';
import { letGo } from '../client/let-go';
import { addProjectTargets, addProjectToMachine, cloneUrlForProtocol, cloneUrlProtocol, COPYABLE_FILE_MAX_BYTES, defaultSelection, dismissProjectFileCopy,
  projectRemoteUrl, projectSubdirectory, type CloneProtocol, type IgnoredFile } from './add-project-machine';

type DialogState = {
  open: boolean; serial: number; environmentId: string; projectId: string; targetId: string; protocol: CloneProtocol | null; pathOverride: string | null;
  selected: Set<string> | null; files: { files: IgnoredFile[]; truncated: boolean } | null; filesError: string; submitting: boolean; error: string;
};
const states = new WeakMap<T3Client, DialogState>();
function stateOf(client: T3Client): DialogState {
  let state = states.get(client);
  if (!state) {
    state = { open: false, serial: 0, environmentId: '', projectId: '', targetId: '', protocol: null, pathOverride: null, selected: null, files: null, filesError: '', submitting: false, error: '' };
    states.set(client, state);
  }
  return state;
}

/** The project the dialog opened on, while the window still shows its machine; null once it is gone. */
function projectOf(client: T3Client, state: DialogState) {
  if (client.environmentId !== state.environmentId) return null;
  return client.shell.projects.find(project => project.id === state.projectId) ?? null;
}
const canListFiles = (client: T3Client) => obj(obj(client.config.environment).capabilities).ignoredFileListing === true;

const DEFAULT_TAIL = 10, MAX_SEGMENT_TAIL = 16, MIN_WORD_TAIL = 4;
const WORD_BREAK = /[-_./]/;
/**
 * ui/middle-truncate.tsx splitForMiddleTruncate: the tail that always shows (the last path segment up to 16 characters,
 * else 10, moved to start on a word), and the head that ellipsizes; null when the value is short enough to show whole.
 */
export function splitForMiddleTruncate(value: string, tail?: number): { head: string; tail: string } | null {
  const chars = Array.from(value);
  let keep = tail ?? DEFAULT_TAIL;
  if (tail === undefined) {
    const slash = chars.lastIndexOf('/');
    if (slash > 0 && slash < chars.length - 1) {
      const segment = chars.length - slash - 1;
      keep = segment <= MAX_SEGMENT_TAIL ? segment : DEFAULT_TAIL;
    }
  }
  if (keep <= 0 || chars.length <= keep + 4) return null;
  let cut = chars.length - keep;
  // An automatic tail starts on a word ("machine", not "er-machine"): a shorter tail first, so the head stays readable.
  if (tail === undefined && !WORD_BREAK.test(chars[cut - 1]!)) {
    const startsWord = (index: number) => WORD_BREAK.test(chars[index - 1]!);
    let snapped = -1;
    for (let index = cut + 1; index <= chars.length - MIN_WORD_TAIL; index++) if (startsWord(index)) { snapped = index; break; }
    for (let index = cut - 1; snapped < 0 && index >= chars.length - MAX_SEGMENT_TAIL && index > 4; index--) if (startsWord(index)) snapped = index;
    if (snapped > 0) cut = snapped;
  }
  return { head: chars.slice(0, cut).join(''), tail: chars.slice(cut).join('') };
}
const middle = (value: string) => { const split = splitForMiddleTruncate(value); return split ?? { head: value, tail: '' }; };

export const CLOSED_ADD_MACHINE = { open: false, key: '', description: '', sourceLabel: '', sourceMachine: 'server', sourcePath: '', sourceHead: '', sourceTail: '', fileSummary: '',
  targets: [] as { id: string; label: string; machine: string; selected: boolean }[], targetMenuWidth: 160, targetLabel: '', targetMachine: 'server', targetShown: false,
  destination: '', destinationHead: '', destinationTail: '', protocolShown: false, protocol: '', remoteUrl: '', folderLabel: '', canListFiles: false, countLabel: '', allLabel: '',
  files: [] as { path: string; size: string; checked: boolean; disabled: boolean }[], filesNote: '', filesError: false, truncatedNote: '', error: '', submitLabel: '', canSubmit: false, submitting: false };

/** Everything the dialog shows, as the reference derives it on each render. */
function derive(client: T3Client, state: DialogState, source: EnvironmentFleet) {
  const project = projectOf(client, state);
  const targets = project ? addProjectTargets(client, source) : [];
  const target = targets.find(candidate => candidate.environmentId === state.targetId) ?? targets[0] ?? null;
  const sourceUrl = projectRemoteUrl(project ?? undefined);
  const sshUrl = cloneUrlForProtocol(sourceUrl, 'ssh'), httpsUrl = cloneUrlForProtocol(sourceUrl, 'https');
  const remoteUrl = (state.protocol === 'ssh' ? sshUrl : state.protocol === 'https' ? httpsUrl : null) ?? sourceUrl;
  const targetEntry = target ? [...source.entries.values()].find(entry => entry.environmentId === target.environmentId) : undefined;
  const defaultPath = cloneDestination(initialBrowseQuery(targetEntry?.config ?? {}), cloneDirectoryName(sourceUrl));
  const destinationPath = state.pathOverride ?? defaultPath;
  const files = state.files?.files ?? [];
  const selected = state.selected ?? defaultSelection(files);
  const listing = canListFiles(client);
  // Submitting before the file list loads would silently skip the default .env copies.
  const unavailable = project === null || target === null || (listing && state.files === null && !state.filesError);
  return { project, targets, target, sourceUrl, sshUrl, httpsUrl, remoteUrl, destinationPath, files, selected, listing, unavailable };
}

/** The dialog for the shell answer; the reference closes it once its project is gone. */
export function addMachineView(client: T3Client, source: EnvironmentFleet = fleet): typeof CLOSED_ADD_MACHINE {
  const state = states.get(client);
  if (!state?.open) return CLOSED_ADD_MACHINE;
  const view = derive(client, state, source), project = view.project;
  if (!project) { state.open = false; return CLOSED_ADD_MACHINE; }
  const copyable = view.files.filter(file => file.byteLength <= COPYABLE_FILE_MAX_BYTES);
  const selectedBytes = view.files.filter(file => view.selected.has(file.path)).reduce((total, file) => total + file.byteLength, 0);
  const count = view.selected.size;
  const sourceLabel = environmentIndicator({ isPrimary: focusedOnPrimary(client), available: 2, environmentId: client.environmentId,
    runtimeLabel: str(obj(client.config.environment).label), savedLabel: '', machine: machineKind(client.config) });
  const identity = obj(project.repositoryIdentity), workspaceRoot = str(project.workspaceRoot);
  const allSelected = count === copyable.length;
  return {
    open: true, key: String(state.serial),
    description: `Clone ${str(identity.displayName) || str(project.title)} and bring the local files git does not track.`,
    sourceLabel: sourceLabel.envLabel || 'This machine', sourceMachine: sourceLabel.envKind, sourcePath: workspaceRoot, sourceHead: middle(workspaceRoot).head, sourceTail: middle(workspaceRoot).tail,
    fileSummary: count === 0 ? 'no files' : `+ ${count} ${count === 1 ? 'file' : 'files'}`,
    targets: view.targets.map(target => ({ id: target.environmentId, label: target.label, machine: target.machine, selected: target.environmentId === view.target?.environmentId })),
    targetMenuWidth: addMenuWidth(client, view.targets),
    targetLabel: view.target?.label ?? 'No machine available', targetMachine: view.target?.machine ?? 'server', targetShown: view.target !== null,
    destination: view.destinationPath, destinationHead: middle(view.destinationPath).head, destinationTail: middle(view.destinationPath).tail,
    protocolShown: !!view.sshUrl && !!view.httpsUrl, protocol: state.protocol ?? '', remoteUrl: view.remoteUrl,
    folderLabel: `Folder on ${view.target?.label ?? 'the other machine'}`, canListFiles: view.listing,
    countLabel: copyable.length > 0 ? `${count} of ${view.files.length}${selectedBytes > 0 ? ` · ${formatBytes(selectedBytes)}` : ''}` : '',
    allLabel: copyable.length > 0 ? (allSelected ? 'None' : 'All') : '',
    files: view.files.map(file => {
      const tooLarge = file.byteLength > COPYABLE_FILE_MAX_BYTES;
      return { path: file.path, size: tooLarge ? 'Over 1 MB' : formatBytes(file.byteLength), checked: view.selected.has(file.path), disabled: tooLarge };
    }),
    filesNote: state.filesError || (view.files.length === 0 ? (state.files ? 'No ignored files to copy.' : 'Finding ignored files…') : ''), filesError: !!state.filesError,
    truncatedNote: state.files?.truncated ? `Showing the first ${view.files.length} files.` : '',
    error: state.error, submitLabel: state.submitting ? 'Starting…' : `Clone to ${view.target?.label ?? 'machine'}`,
    canSubmit: !view.unavailable && !state.submitting, submitting: state.submitting,
  };
}

/** `shelllocal:add-machine-<op>`: open (value: the machine, or none), close, target, protocol, path, file (id: path, value: on), all, copy-dismiss. */
export async function addMachineLocal(client: T3Client, native: Native, op: string, id: string, value: string, source: EnvironmentFleet = fleet): Promise<string> {
  const state = stateOf(client);
  if (op === 'copy-dismiss') { if (client.projectId) dismissProjectFileCopy(client, client.environmentId, client.projectId); return ''; }
  if (op === 'open') {
    const project = client.shell.projects.find(entry => entry.id === client.projectId);
    if (!project || !client.environmentId) throw new ClientError('Open a project first.');
    Object.assign(state, { open: true, serial: state.serial + 1, environmentId: client.environmentId, projectId: client.projectId, targetId: value,
      protocol: cloneUrlProtocol(projectRemoteUrl(project)), pathOverride: null, selected: null, files: null, filesError: '', submitting: false, error: '' });
    if (!canListFiles(client)) return '';
    const serial = state.serial;
    await wakeShell(native); // "Finding ignored files…" while the list loads
    try {
      const listed = decodeIgnoredFiles(await client.restAccess(native).request('projects.listIgnoredFiles', { cwd: str(project.workspaceRoot) }));
      if (state.serial === serial) state.files = listed;
    } catch (error) {
      if (letGo(error)) throw error;
      if (state.serial === serial) state.filesError = error instanceof Error && error.message ? error.message : 'Could not list ignored files.';
    }
    return '';
  }
  if (!state.open) return '';
  // Cancel, Escape and the backdrop close it even while the clone starts; the clone still lands (the reference's
  // submit goes on after its dialog unmounts).
  if (op === 'close') { state.open = false; return ''; }
  if (op === 'target') { if (value) state.targetId = value; return ''; }
  if (op === 'protocol') { if (value === 'ssh' || value === 'https') state.protocol = value; return ''; }
  if (op === 'path') { state.pathOverride = value; return ''; }
  const view = derive(client, state, source);
  const copyable = view.files.filter(file => file.byteLength <= COPYABLE_FILE_MAX_BYTES);
  if (op === 'file') {
    const next = new Set(view.selected);
    if (value === 'true' && copyable.some(file => file.path === id)) next.add(id); else next.delete(id);
    state.selected = next;
    return '';
  }
  if (op === 'all') { state.selected = view.selected.size === copyable.length ? new Set() : new Set(copyable.map(file => file.path)); return ''; }
  throw new ClientError(`Unknown action: ${op}`);
}

/**
 * submit: starts the clone, then openClonedProject lands on the new machine right away; its composer
 * shows the clone and holds sends until it finishes. A draft open on the source project moves over
 * with whatever was typed in it. Returns the connection T3Client adopts (none while the dialog stays).
 */
export async function submitAddMachine(client: T3Client, native: Native, source: EnvironmentFleet = fleet): Promise<{ status: Obj | null; generation: number }> {
  const none = { status: null, generation: -1 };
  const state = stateOf(client);
  if (!state.open || state.submitting) return none;
  const view = derive(client, state, source), project = view.project, target = view.target;
  // The button is disabled until then, and Return in the folder field asks the same: before the file list loads a
  // clone would skip the default .env copies.
  if (!project || !target || view.unavailable) return none;
  if (view.destinationPath.trim().length === 0) { state.error = 'Choose a folder on the other machine.'; return none; }
  const serial = state.serial;
  state.submitting = true; state.error = '';
  await wakeShell(native); // "Starting…"
  const sourceEnvironmentId = state.environmentId, sourceProjectId = state.projectId;
  const mine = () => state.serial === serial;
  let projectId = '';
  try {
    [projectId = ''] = await client.restAccess(native).ids(1);
    const workspaceRoot = str(project.workspaceRoot);
    await addProjectToMachine(client, native, {
      source: { environmentId: sourceEnvironmentId, cwd: workspaceRoot },
      subdirectory: projectSubdirectory(str(obj(project.repositoryIdentity).rootPath) || undefined, workspaceRoot),
      target: { environmentId: target.environmentId, label: target.label }, projectId, title: str(project.title), remoteUrl: view.remoteUrl,
      destinationPath: view.destinationPath.trim(), files: view.files.filter(file => view.selected.has(file.path)).map(file => file.path),
      createdAt: new Date(composerNow(client)).toISOString(),
    });
  } catch (error) {
    if (letGo(error)) throw error;
    // A dialog closed while it started has nowhere to show the error, as the reference's unmounted one.
    if (mine()) state.submitting = false;
    if (mine() && state.open) { state.error = error instanceof Error && error.message ? error.message : 'Could not start the clone.'; }
    return none;
  }
  if (mine()) { state.open = false; state.submitting = false; }
  // openClonedProject: the draft open on the source project moves over; it moves back if the window cannot follow.
  const moved = !client.threadId && client.environmentId === sourceEnvironmentId && client.projectId === sourceProjectId
    ? moveDraft(client, `${target.environmentId}:new:${projectId}`) : null;
  const before = client.local.selections[target.environmentId];
  try {
    const focused = await focusFleetProject(client, native, target.environmentId, projectId, source);
    return { status: focused.value, generation: focused.generation };
  } catch (error) {
    moved?.();
    if (before) client.local.selections[target.environmentId] = before; else delete client.local.selections[target.environmentId];
    throw error;
  }
}

/** setDraftThreadContext onto the clone: the draft's text and workspace choices move, its machine choice is manual. Returns the undo. */
function moveDraft(client: T3Client, to: string): () => void {
  const from = client.draftKey, text = client.local.drafts[from];
  if (text) { client.local.drafts[to] = text; delete client.local.drafts[from]; }
  const contexts = (client.local.composerControls as { contexts?: Record<string, Record<string, unknown>> }).contexts ??= {};
  const context = contexts[from];
  if (context) { contexts[to] = { ...context, worktreePath: '' }; delete contexts[from]; }
  const previous = draftSelection(client, from);
  moveDraftSelection(client, from, to, { selection: 'manual', choice: '' });
  return () => {
    if (client.local.drafts[to] !== undefined && client.local.drafts[from] === undefined) { client.local.drafts[from] = client.local.drafts[to]!; delete client.local.drafts[to]; }
    if (context && !contexts[from]) { contexts[from] = context; delete contexts[to]; }
    moveDraftSelection(client, to, from, previous);
  };
}

/** For tests: the dialog's state. */
export const addMachineState = (client: T3Client) => stateOf(client);
