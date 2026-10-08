import { mobileOutboxDraftRecoveryBlocked } from './mobile-outbox-draft-handoff';
// Source365aa87982 new-task-flow-provider and NewTaskDraftScreen capture inputs.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { MobileDraftClient } from './mobile-draft-recovery';
import { mobileNewTaskContextProject } from './mobile-new-task-context';
import { mobileNewTaskDraftPresentation, mobileNewTaskDraftSelectedBranch } from './mobile-new-task-drafts';
import type { MobileOutboxCaptureFacts, MobileOutboxDraftPresentation } from './mobile-outbox-capture';
import type { MobileOutboxRuntimeMode } from './mobile-outbox-model';
import { mobileNewTaskDefaultModel } from './new-task-model';
import { mobileParseProjectFile } from './new-task-project-file';
import { mobileQueuedEditOrigin, queuedEditRefreshOrigin } from './queued-edit-origin';
import { mobileSessionGrants } from './mobile-grants';
import { normalizeMobilePreferences } from './settings-preferences';
import { mobileComposerTarget } from './composer-target';
import { mobileComposerAttachmentPicking } from './composer-attachments';
import { mobileVoiceBlocksSubmission } from './voice-data';
import { mobileNewTaskCloneBlocks } from './new-task-clone';
import { arr, obj, str, type Obj } from './shared/domain';
import { ClientError, bridgeReply, reply, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { resolveProjectSettings } from './shared/scoped-settings-plan';
import { branchState } from './shared/r4-git-branch';
import { isScratch, scratchRootOf } from './shared/r12-threads-scratch';
import { peekCurrentVcsStatus, watchVcsStatus } from './shared/shell-vcs';
import { contextReferences } from './shared/composer-editor-menu';

const FILE_BYTES = 1024 * 1024; // WorkspaceFileSystem.ts PROJECT_READ_FILE_MAX_BYTES.
const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const object = (value: unknown): value is Obj => !!value && typeof value === 'object' && !Array.isArray(value);
const stale = () => new ClientError('The draft or environment changed before capture.', 'superseded');
const runtimeModes = ['approval-required', 'auto-accept-edits', 'auto', 'full-access'];

/** Source resolves legacy aggregate defaults only before projectSettingsFolded,
 * then stored project overrides, environment, optional t3.json and built-ins. */
function projectSettings(config: Obj, project: Obj, file: Obj | null) {
  const settings = obj(config.settings), id = str(project.id);
  const legacy = settings.projectSettingsFolded ? {} : {
    ...(project.defaultModelSelection == null ? {} : { defaultModelSelection: project.defaultModelSelection }),
    ...(project.defaultThreadEnvMode == null ? {} : { defaultThreadEnvMode: project.defaultThreadEnvMode }),
  };
  return resolveProjectSettings({ ...settings, projectSettingsOverrides: { ...obj(settings.projectSettingsOverrides),
    [id]: { ...legacy, ...obj(obj(settings.projectSettingsOverrides)[id]) } } }, id, file).settings;
}
function fileContents(value: Obj): Obj | null {
  // The server caps raw bytes; replacement characters may expand if re-encoded.
  if (value.truncated !== false || typeof value.contents !== 'string' || !Number.isSafeInteger(value.byteLength)
    || Number(value.byteLength) < 0 || Number(value.byteLength) > FILE_BYTES || value.contents.includes('\0')
    || value.contents.length > FILE_BYTES) return null;
  return mobileParseProjectFile(value.contents);
}

/** Only this draft's stored payload is capture authority, including after retarget/restart. */
function contextError(draft: MobileOutboxDraftPresentation): string {
  const projected = mobileNewTaskContextProject(draft.text, draft.context);
  if (!projected.ok) return projected.error;
  const records = projected.context?.records ?? [];
  for (const reference of contextReferences(draft.text)) {
    if (['image', 'file'].includes(reference.kind)) continue;
    const record = records.find(record => record.contextId === reference.id && record.kind === reference.kind);
    if (!record || record.kind === 'terminal' && !str(record.text).trim())
      return 'Restore the saved context for this draft before queuing it.';
  }
  return '';
}

/** One foreground invocation. No native handle, Promise, timer or cache survives
 * its caller. The coordinator invokes prepare only after durable recovery lookup.
 * now is the caller's action-time wall clock, not a cached render clock. */
export function mobileNewTaskCaptureFacts(client: MobileDraftClient, draftKey: string, routeCurrent: () => boolean, now: number) {
  const initial = mobileNewTaskDraftPresentation(client, draftKey);
  const project = () => client.shell.projects.find(value => value.id === client.projectId) ?? null;
  const identity = () => JSON.stringify([client.origin, client.environmentId, client.generation, client.projectId, client.threadId, client.threadEpoch,
    client.draftKey, mobileComposerTarget(client).owner, client.connection, client.config, project(), mobileNewTaskDraftPresentation(client, draftKey)]);
  const captured = identity(), origin = client.origin, generation = client.generation, cwd = str(project()?.workspaceRoot);
  let prepared = false, preparing = false, abandoned = false, originVerified = false;
  let session: Obj | null = null, projectFile: Obj | null = null, preferencesLoaded = false, planEnabled = false, readError = '';
  const current = () => !abandoned && routeCurrent() && !!initial && client.preferencesLoaded && !client.threadId
    && client.draftKey === draftKey && initial.environmentId === client.environmentId && initial.projectId === client.projectId
    && mobileComposerTarget(client).kind === 'ordinary' && identity() === captured
    && (!originVerified || mobileQueuedEditOrigin(client) === initial.origin);
  const assertCurrent = () => { if (!current()) throw stale(); };
  function facts(): MobileOutboxCaptureFacts {
    assertCurrent();
    const draft = mobileNewTaskDraftPresentation(client, draftKey)!;
    const selectedProject = project(), config = object(client.config.settings) && Array.isArray(client.config.providers) ? client.config : null;
    const connected = client.connection === 'connected';
    const settings = config && selectedProject ? projectSettings(config, selectedProject, projectFile) : {};
    const scratch = !!selectedProject && !!config && isScratch(selectedProject, scratchRootOf(true, config));
    const mode = scratch ? 'local' : draft.workspace?.envMode === 'worktree' || draft.workspace?.envMode === 'local'
      ? draft.workspace.envMode : settings.defaultThreadEnvMode === 'worktree' ? 'worktree' : 'local';
    const worktreePath = scratch ? null : draft.workspace?.worktreePath || null;
    const vcs = connected && cwd ? peekCurrentVcsStatus(client, cwd) : null;
    const currentCheckoutBranch = vcs?.origin === origin && vcs.environmentId === draft.environmentId && vcs.generation === generation
      && vcs.status.isRepo === true ? str(vcs.status.refName) || null : null;
    const context = contextError(draft);
    const runtime = runtimeModes.includes(str(settings.defaultRuntimeMode)) ? str(settings.defaultRuntimeMode) as MobileOutboxRuntimeMode : 'full-access';
    let blockReason = !prepared ? 'Wait for the task settings to load.' : readError;
    if (!config || !client.shellLoaded || !selectedProject || selectedProject.archivedAt != null || !cwd) blockReason ||= 'Wait for this environment and project to load.';
    if (connected && !client.configLive) blockReason ||= 'Wait for the current environment configuration.';
    if (!originVerified || mobileQueuedEditOrigin(client) !== draft.origin) blockReason ||= 'The draft environment identity is not verified.';
    if (client.busy || client.pending || mobileComposerAttachmentPicking(client) || mobileVoiceBlocksSubmission(client)) blockReason ||= 'Wait for the current composer operation to finish.';
    if (mobileNewTaskCloneBlocks(client)) blockReason ||= 'Wait for this repository to finish cloning.';
    if (mobileOutboxDraftRecoveryBlocked(client, draftKey)) blockReason ||= 'Finish restoring this draft before sending it.';
    blockReason ||= context;
    const selectedModel = config && selectedProject ? mobileNewTaskDefaultModel(client) : null;
    return { key: draftKey, origin: draft.origin, environmentId: draft.environmentId, projectId: draft.projectId,
      ...(selectedProject ? { projectTitle: str(selectedProject.title), projectCwd: cwd } : {}),
      config: config ? clone(config) : null, selectedModel: selectedModel ? clone(selectedModel) as MobileOutboxCaptureFacts['selectedModel'] : null,
      defaultRuntimeMode: runtime, connected, canOperate: connected && mobileSessionGrants(session, 'orchestration:operate'),
      planPreferenceLoaded: preferencesLoaded, planModeEnabled: preferencesLoaded && planEnabled,
      workspace: { canChoose: !scratch, mode, worktreePath, explicitBranch: scratch ? null : mobileNewTaskDraftSelectedBranch(draft),
        currentCheckoutBranch, startFromOrigin: branchState(client).origin.get(draftKey) ?? settings.newWorktreesStartFromOrigin !== false },
      // No app-owned independent background upload producer or persisted origin
      // adoption exists yet. A remote ID alone must fail the pure capture check.
      uploadOwners: {}, uploadStates: {}, ...(blockReason ? { blockReason } : {}) };
  }
  async function prepareFacts(native: Native): Promise<void> {
    assertCurrent();
    if (preparing || prepared) throw new ClientError('Task facts have already been prepared for this invocation.');
    preparing = true;
    // This wrapper is local to the call, including the let-go marker. In
    // particular watchVcsStatus's tolerated errors cannot revive a let-go read.
    const scoped: Native = { available: native.available, watch(topic) { assertCurrent(); native.watch(topic); }, async later(request) {
      assertCurrent();
      try {
        const response = await native.later(request);
        if (obj(request).generation !== undefined && reply(response).generation !== generation) { abandoned = true; throw stale(); }
        assertCurrent(); return response;
      }
      catch (error) { if (letGo(error)) { abandoned = true; throw stale(); } assertCurrent(); throw error; }
    } };
    try {
      await queuedEditRefreshOrigin(scoped, client); assertCurrent();
      originVerified = true; assertCurrent();
      try {
        const reply = await bridgeReply(scoped, { op: 'mobilePreferences' }); assertCurrent();
        if (reply.ok && (object(reply.value) || typeof reply.value === 'string')) {
          planEnabled = normalizeMobilePreferences(reply.value).planModeEnabled; preferencesLoaded = true;
        }
      } catch (error) { if (letGo(error)) throw error; assertCurrent(); }
      if (client.connection === 'connected') {
        try { session = await client.http(scoped, '/api/auth/session', generation); assertCurrent(); }
        catch (error) { if (letGo(error)) throw error; assertCurrent(); readError = 'The current environment permissions could not be verified.'; }
        if (mobileSessionGrants(session, 'filesystem:read') && cwd) {
          try { projectFile = fileContents(await client.request(scoped, 'projects.readFile', { cwd, relativePath: 't3.json' }, generation)); assertCurrent(); }
          catch (error) { if (letGo(error)) throw error; assertCurrent(); projectFile = null; }
        }
        if (mobileSessionGrants(session, 'orchestration:operate') && cwd) {
          await watchVcsStatus(client, scoped, cwd, now); assertCurrent();
        }
      }
      prepared = true;
    } finally { preparing = false; }
  }
  return { draftKey, current, now, prepareFacts, facts };
}
