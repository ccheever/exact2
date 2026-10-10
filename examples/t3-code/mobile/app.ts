import { mobileComposerRootSnapshot, mobileComposerRootAction } from './composer-root';
import { mobileThreadSendRootSnapshot, mobileThreadSendRootAction } from './thread-send-root';
import { mobileThreadForkObserveRoute, mobileThreadForkAction } from './thread-fork';
import { mobileDraftRecoveryHandles } from './mobile-draft-recovery';
import { letGoAware } from './shared/let-go';
import { mobileKeyboardSnapshot, mobileKeyboardCopy, mobileKeyboardDismiss } from './mobile-keyboard';
import { archiveChromeView, archiveChromeEvent } from './archive-chrome';
import { mobileRootFaviconAdmission, mobileRootFavicons, mobileRootFaviconImages, mobileRootFaviconEvent } from './mobile-favicon-root';
import { mobileIncomingShareRead, mobileIncomingSharePresentation } from './incoming-share-inbox';
import { mobileOutboxThread } from './mobile-outbox-presentation';
import { mobileOutboxRootSnapshot, mobileOutboxRootAction, mobileOutboxRootEdit } from './mobile-outbox-root';
import { noteNow } from './shared/composer-controls';
import { homeArrangeActionValue } from './home-arrange';
import { mobileHomeAction } from './home-actions';
import { mobileAddProjectObserve, mobileAddProjectPrepare, mobileAddProjectEdit, mobileAddProjectBrowse, mobileAddProjectAction, type AddProjectRoute } from './add-project';
import { mobileNewTaskCloneObserve } from './new-task-clone';
import { mobileNewTaskCloneColors } from './add-project-presentation';
import { mobileAddProjectColors } from './add-project-presentation';
import { mobileThreadHeaderSnapshot, mobileThreadHeaderPrepare, mobileThreadHeaderAction } from './thread-header';
import { mobileThreadHeaderLaunch } from './thread-header-terminal';
import { mobileWorkingControl } from './working-control';
import type { Sources, Answer } from './app.contract.d.ts';
import { mobileComposerTarget } from './composer-target';
import { mobileQueueSnapshot, mobileQueueCommand } from './queue';
import { mobileQueuePrepare } from './queue-read';
import { mobileThreadAnswerFilesPrepare } from './thread';
import { mobileGitColors } from './git-colors';
import { mobileGitSnapshot, mobileGitRead, mobileGitAction } from './git-overview';
import { mobileGitFeedbackSnapshot, mobileGitFeedbackAction } from './git-feedback';
import { mobileGitBranchesSnapshot, mobileGitBranchesRead, mobileGitBranchAction } from './git-branches';
import { workspaceInspectorSnapshot } from './workspace-inspector';
import { mobileInspectorContext, mobileInspectorTransition, mobileInspectorPresentation } from './workspace-inspector-adapter';
import { mobileWorkspaceEvent, mobileWorkspace, mobileWorkspaceThreadSelection, mobileWorkspaceFileSelection, type MobileWorkspaceOptions } from './mobile-workspace';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { mobileStreamingDescriptor } from './haptics';
import { mobileNewTaskFlowView, mobileNewTaskFlowAction, mobileNewTaskFlowOwns, mobileNewTaskFileRouteCurrent, mobileNewTaskFlowCurrent } from './new-task-flow';
import { mobileThreadPreferences, mobileThreadPreferencesCommand } from './settings-thread-preferences';
import { mobileProjectOverview, mobileProjectRename } from './settings-project';
import { mobileAccountRouteEntry } from './settings-account';
import { mobileNotificationsSettings } from './settings-notifications';
import { mobileAppLink } from './navigation-links';
import { mobilePreviewOwner, mobilePreviewPrepare, mobileBrowserPreviewStatus, mobileDevicesPreviewStatus, mobilePreviewAction, mobilePreviewMenus, mobilePreviewMenuAction } from './mobile-preview-flow';
import { mobilePreviewColors } from './browser-mobile-colors';
import { MOBILE_INFORMATION_ROUTES, mobileInformationPrepare, mobileInformationSnapshot, mobileInformationCommand, mobileInformationLegalConfiguration } from './settings-information';
import { mobileAudioStatus, mobileAudioAction } from './attachment-audio';
import { mobileAutomationPrepare, mobileAutomationSnapshot, mobileAutomationCommand } from './settings-scheduled-flow';
import { mobileVoiceAction, mobileVoiceSnapshot, mobileVoiceStatus } from './voice-data';
import { mobileVoiceColors } from './voice-colors';
import { mobileAttachmentDocument, EMPTY_ATTACHMENT_DOCUMENT, type AttachmentDocumentSnapshot } from './attachment-document';
import { mobileAttachmentMenu, mobileAttachmentDocumentAction } from './attachment-document-actions';
import { mobileTerminalPrepare, mobileTerminalAction, mobileTerminalEvent, mobileTerminalCapture, mobileTerminalAttachOutput } from './terminal-mobile';
import { mobileReviewColors } from './review-colors';
import { mobileReviewRead, mobileReviewSnapshot, mobileReviewAction } from './review-data';
import { mobileNewTaskFileSnapshot, mobileNewTaskFileRead } from './new-task-file';
import { mobileFilesRead, mobileFilesSnapshot, mobileFilesAction, mobileFileRead, mobileFileSnapshot, mobileFileMenu, type FileSnapshot } from './file-data';
import { MOBILE_SERVER_ROUTES, mobileServerSettings, mobileServerSettingsCommand } from './settings-server';
import { settingsProviderNative, mobileProviderAccounts, mobileProviderAccountsSnapshot, mobileProviderCommand, mobileProviderField } from './settings-provider';
import { mobileMediaPrepare, mobileMediaForget } from './media-preview';
import { mobileComposerAttachmentAction, mobileComposerAttachmentPreviews } from './composer-attachments';
import {mobileComposerAttachmentMenuAction,mobileComposerAttachmentMenuSnapshot} from './composer-attachment-menu';
// @ref llp/1109.003-pairing-and-transport.decision.md#decision
import { mobileClient, mobileNative, mobileSnapshot, mobileCommand, mobilePairingFields } from './client';
import { mobileEnvironmentDetail, mobileEnvironmentDetailCommand } from './environment-detail';
import { arr, obj, str } from './shared/domain';
import { fleet } from './shared/settings-b-fleet';
import { machineKind } from './shared/connections';
import { mobileClearClientCaches } from './mobile-cache-controls';
import { mobileCacheFleetDisplays } from './mobile-client-cache-fleet';
import { mobileShelves, mobileToggleShelf, mobileHomeView } from './home-state';
import { mobileThreadLocalColors, mobileTheme, mobileHomeColors, mobileThreadColors, mobileComposerColors, mobileArchiveColors, mobileAgentColors } from './design';
import { mobilePreferencesResource, mobileSavePreference, mobileApplyAppearance, normalizeMobilePreferences, resolveMobileAppearance } from './settings-preferences';
import { settingsAppearanceView, settingsChoices } from './settings-appearance';
import { mobileArchive, mobileArchiveCommand } from './archive';
import { mobileAgentActivity } from './agent-activity';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTask, mobileNewTaskChooser, mobileNewTaskPrepare, mobileNewTaskCachedPrepare } from './new-task';
import { mobileThread, mobileThreadPrepare } from './thread';
import { mobileLayoutFacts, mobileScheduledHeader, homeChromeEvent, homeChromeView, settingsRoot, settingsScopeEvent } from './root-presentation';
import { connectionView } from './presentation';
import { bridgeReply, ClientError, nativeFiles, type Files, type Native } from './shared/protocol';

export const appId = 'com.exact.t3code.ios';
export const grants = 'device.camera purpose.camera\ndevice.microphone purpose.microphone';

function storageEnvironments() {
  const cached = mobileCacheFleetDisplays(fleet, mobileClient);
  return fleet.saved.map(row => {
    const environmentId = str(row.environmentId);
    const config = environmentId === mobileClient.environmentId ? mobileClient.config
      : [...fleet.entries.values()].find(entry => entry.environmentId === environmentId)?.config
        ?? cached.find(entry => entry.environmentId === environmentId)?.config ?? {};
    return { environmentId, label: str(row.mobileLabel) || str(obj(config.environment).label) || str(row.label) || environmentId,
      machine: machineKind(config) };
  });
}

// Each generated source has its own checked result type; no union assertion crosses the ABI.
const sources: Sources = {
  composerRoot: args => mobileComposerRootSnapshot(mobileClient, args[0], args[1]),
  composerRootAction: async (args, _store, suppliedStorage, nativeInput) => {
    const input = sourceNative('composerRootAction', args, nativeInput);
    const handle = input?.available ? letGoAware(mobileNative(input)) : input;
    const { native, storage } = mobileDraftRecoveryHandles(mobileClient, handle,
      handle?.available ? nativeFiles(handle) : suppliedStorage!);
    const op = (['event', 'rich', 'pick', 'dismiss', 'retry', 'prepare-files', 'cleanup-files', 'immediate', 'wake', 'claim-effect'] as const).find(value => value === args[1]);
    if (!native?.available || !op) return { revision: mobileClient.revision, message: 'This editor action is unavailable.',
      visit: args[0].visit, owner: '', editorId: args[0].editorId, admission: args[2], effectId: '', effectKind: '', alternate: false };
    return mobileComposerRootAction(mobileClient, { ...args[0], now: args[5] },
      { op, admission: args[2], key: args[3], payload: args[4] }, native, storage);
  },
  threadSubmission: args => mobileThreadSendRootSnapshot(mobileClient, args[0]),
  threadLocalColors: args => mobileThreadLocalColors(args[0], args[1]),
  threadSubmissionAction: async (args, _store, suppliedStorage, nativeInput) => {
    const input = sourceNative('threadSubmissionAction', args, nativeInput);
    const handle = input?.available ? letGoAware(mobileNative(input)) : input;
    const { native, storage } = mobileDraftRecoveryHandles(mobileClient, handle,
      handle?.available ? nativeFiles(handle) : suppliedStorage!);
    if (!native?.available) return { visit: args[5], owner: args[4], revision: mobileClient.revision, message: 'Open T3 Code on your iPhone or iPad.' };
    const op = (['send', 'send-alternate', 'recovery-read', 'recovery-action', 'feedback-dismiss', 'feedback-copy', 'usage-close', 'open-link', 'reset-credit'] as const).find(value => value === args[1]);
    if (!op) return { visit: args[5], owner: args[4], revision: mobileClient.revision, message: 'This composer action is unavailable.' };
    const result = await mobileThreadSendRootAction(mobileClient, native, storage,
      { ...args[0], now: args[6] }, { op, key: args[2], value: args[3], expectedOwner: args[4], visit: args[5] });
    return { visit: args[5], owner: args[4], revision: result.revision, message: result.stale ? '' : result.message };
  },
  keyboardSnapshot: args => mobileKeyboardSnapshot(mobileClient, str(args[0]), str(args[1])),
  keyboardCopy: (args, _store, _storage, native) => {
    mobileKeyboardSnapshot(mobileClient, str(args[1]), str(args[2]));
    return mobileKeyboardCopy(mobileClient, str(args[0]), native);
  },
  keyboardDismiss: args => mobileKeyboardDismiss(mobileClient, Number(args[0])),
  projectFaviconDemand: args => mobileRootFaviconAdmission(args),
  projectFavicons: (args, _store, _storage, native) => mobileRootFavicons(Number(args[0]), Number(args[1]), native),
  projectImages: () => mobileRootFaviconImages(),
  projectImageEvent: args => mobileRootFaviconEvent(str(args[0]), str(args[1]), str(args[2]), str(args[3])),
  incomingShareInbox: (_args, _store, _storage, native) => mobileIncomingShareRead(mobileClient, native),
  incomingSharePresentation: args => mobileIncomingSharePresentation(mobileClient, str(args[0]), args[1] === true, str(args[2])),
  outboxView: args => {
    const { initialized, complete, busy, count, next, delay } = mobileOutboxRootSnapshot(mobileClient, Number(args[1]));
    return { initialized, complete, busy, count, next, delay };
  },
  outboxEdit: args => ({ ...mobileOutboxRootEdit(mobileClient, str(args[0])), requestRoute: str(args[1]) }),
  outboxAction: async (args, _store, storage, nativeInput) => {
    const native = nativeInput?.available ? mobileNative(nativeInput) : nativeInput;
    return mobileOutboxRootAction(mobileClient, native, native?.available ? nativeFiles(native) : storage!,
      str(args[0]), str(args[1]), Number(args[2]));
  },
  threadHeader: args => mobileThreadHeaderSnapshot(args[0]),
  threadHeaderPrepare: (args, _store, _storage, native) => mobileThreadHeaderPrepare(args[0], native, args[1]),
  threadHeaderAction: (args, _store, storage, native) => mobileThreadHeaderAction(args[0], args[1], args[2], native, storage!),
  threadHeaderLaunch: async (args, _store, _storage, native) => ({
    ...await mobileThreadHeaderLaunch(args[0], args[1], native), requestRoute: args[2],
    navigation: '', location: '', sourceAction: '', key: '', pendingLaunch: '',
  }),
  workingControl: args => mobileWorkingControl(args[0], args[1], args[2]),
  inspectorState: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return workspaceInspectorSnapshot(str(args[0]));
  },
  inspectorContext: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileInspectorContext(args[0], args[1] === true, args[2] === true, Number(args[3]), args[4] === true, args[5] === true, args[6] === 'dark');
  },
  inspectorTransition: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileInspectorTransition(str(args[0]), str(args[1]), str(args[2]), str(args[3]), Number(args[4]), str(args[5]), args[6] === 'dark');
  },
  inspectorPresentation: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileInspectorPresentation(str(args[0]), str(args[1]), args[2] === true, str(args[3]), str(args[4]), str(args[5]), mobileClient, Number(args[6]));
  },
  workspaceEvent: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileWorkspaceEvent(str(args[0]), str(args[1]), str(args[2]), args[3] === true, Number(args[4]));
  },
  gitColors: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitColors(str(args[0]), str(args[1]));
  },
  gitSnapshot: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitSnapshot(Number(args[0]));
  },
  gitFeedback: args => mobileGitFeedbackSnapshot(Number(args[0]), mobileClient, args[2] === true),
  gitFeedbackAction: (args, _store, _storage, native) => mobileGitFeedbackAction(str(args[0]), str(args[1]), str(args[2]), Number(args[3]), native, mobileClient),
  gitRead: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitRead(str(args[0]), Number(args[1]), native);
  },
  gitBranchesSnapshot: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitBranchesSnapshot();
  },
  gitBranchesRead: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitBranchesRead(str(args[0]), Number(args[1]), native);
  },
  gitAction: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitAction(str(args[0]), str(args[1]), str(args[2]), str(args[3]), Number(args[4]), native).then(result => ({ ...result, requestRoute: str(args[5]) }));
  },
  gitBranchAction: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileGitBranchAction(str(args[0]), str(args[1]), str(args[2]), str(args[3]), Number(args[4]), native).then(result => ({ ...result, requestRoute: str(args[5]) }));
  },
  workspace: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return workspace(args);
  },
  workspaceThreadSelection: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileWorkspaceThreadSelection(args[0], args[1] === true, str(args[2]), str(args[3]));
  },
  workspaceFileSelection: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return mobileWorkspaceFileSelection(args[0], args[1] === true, str(args[2]), str(args[3]), str(args[4]));
  },
  addProjectObserve: args => mobileAddProjectObserve(str(args[0]), args[1] as AddProjectRoute[], str(args[2])),
  addProjectColors: args => mobileAddProjectColors(str(args[0]), str(args[1])),
  addProjectPrepare: (args, _store, _storage, native) => mobileAddProjectPrepare(str(args[0]), str(args[1]), sourceNative('addProjectPrepare', args, native)),
  addProjectEdit: args => mobileAddProjectEdit(str(args[0]), str(args[1]), str(args[2]), str(args[3])),
  addProjectBrowse: (args, _store, _storage, native) => mobileAddProjectBrowse(str(args[0]), str(args[1]), str(args[2]), str(args[3]), sourceNative('addProjectBrowse', args, native)),
  addProjectAction: (args, _store, _storage, native) => mobileAddProjectAction(str(args[0]), str(args[1]), str(args[2]), str(args[3]), sourceNative('addProjectAction', args, native)),
  newTaskClone: args => mobileNewTaskCloneObserve(str(args[0]), str(args[1]), str(args[2]), args[3] === true),
  newTaskCloneColors: args => mobileNewTaskCloneColors(str(args[0]), str(args[1])),
  newTaskFlow: (args, _store, storage, nativeInput) => {
    const native = nativeInput;
    return newTaskFlow(args, native);
  },
  newTaskAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('newTaskAction', args, nativeInput);
    return mobileNewTaskFlowAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), String(args[4]), native, storage!, mobileClient, fleet, Number(args[5]));
  },
  threadPreferences: (args, _store, storage, nativeInput) => {
    const native = sourceNative('threadPreferences', args, nativeInput);
    return mobileThreadPreferences(String(args[1]), args[2], args[3] === true, args[0] === true ? native : null);
  },
  threadPreferencesChange: (args, _store, storage, nativeInput) => {
    const native = sourceNative('threadPreferencesChange', args, nativeInput);
    const requestRoute = String(args[3]);
    return mobileThreadPreferencesCommand(String(args[0]), String(args[1]), String(args[2]), native).then(result => ({ ...result, requestRoute, saved: false }));
  },
  projectOverview: (args, _store, storage, nativeInput) => {
    const native = sourceNative('projectOverview', args, nativeInput);
    return mobileProjectOverview(String(args[1]), args[0] === true ? native : null);
  },
  projectRename: (args, _store, storage, nativeInput) => {
    const native = sourceNative('projectRename', args, nativeInput);
    const requestRoute = String(args[3]);
    return mobileProjectRename(String(args[0]), String(args[1]), String(args[2]), native).then(result => ({ ...result, requestRoute }));
  },
  streamingAssistant: (args, _store, storage, nativeInput) => {
    const native = sourceNative('streamingAssistant', args, nativeInput);
    return mobileStreamingDescriptor(String(args[1]), String(args[2]), String(args[3]), mobileClient);
  },
  notificationSettings: (args, _store, storage, nativeInput) => {
    const native = sourceNative('notificationSettings', args, nativeInput);
    return mobileNotificationsSettings();
  },
  accountEntry: (args, _store, storage, nativeInput) => {
    const native = sourceNative('accountEntry', args, nativeInput);
    const entry = mobileAccountRouteEntry(String(args[0]), args[1] === true);
    return { kind: entry?.kind ?? '', route: entry?.kind === 'replace' ? entry.route : '', requestRoute: String(args[2]) };
  },
  appLink: (args, _store, storage, nativeInput) => {
    const native = sourceNative('appLink', args, nativeInput);
    return mobileAppLink(String(args[0]), String(args[1]), args[2] === true);
  },
  previewOwner: (args, _store, storage, nativeInput) => {
    const native = sourceNative('previewOwner', args, nativeInput);
    return mobilePreviewOwner(args[0] === true);
  },
  previewPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('previewPrepare', args, nativeInput);
    return mobilePreviewPrepare(String(args[0]), native);
  },
  previewColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('previewColors', args, nativeInput);
    return mobilePreviewColors(String(args[0]), String(args[1]));
  },
  browserStatus: (args, _store, storage, nativeInput) => {
    const native = sourceNative('browserStatus', args, nativeInput);
    return mobileBrowserPreviewStatus(String(args[1]), args[2] === true, native);
  },
  devicesStatus: (args, _store, storage, nativeInput) => {
    const native = sourceNative('devicesStatus', args, nativeInput);
    return mobileDevicesPreviewStatus(String(args[1]), args[2] === true, native);
  },
  browserMenus: (args, _store, storage, nativeInput) => {
    const native = sourceNative('browserMenus', args, nativeInput);
    return mobilePreviewMenus(String(args[0]), args[1]);
  },
  devicesMenus: (args, _store, storage, nativeInput) => {
    const native = sourceNative('devicesMenus', args, nativeInput);
    return mobilePreviewMenus(String(args[0]), args[1]);
  },
  previewAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('previewAction', args, nativeInput);
    return mobilePreviewAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), native).then(result => ({ ...result, requestRoute: String(args[4]) }));
  },
  previewMenuAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('previewMenuAction', args, nativeInput);
    return mobilePreviewMenuAction(String(args[0]), String(args[1]), String(args[2]), native).then(result => ({ ...result, requestRoute: String(args[3]) }));
  },
  informationPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('informationPrepare', args, nativeInput);
    return mobileInformationPrepare(String(args[0]), native, str(args[1]));
  },
  informationSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('informationSnapshot', args, nativeInput);
    return mobileInformationSnapshot(str(args[0]), str(args[1]), storageEnvironments(), args[3] === true, mobileTheme(str(args[4]), str(args[5])).colors.dangerForeground);
  },
  informationCommand: (args, _store, storage, nativeInput) => {
    const native = sourceNative('informationCommand', args, nativeInput);
    return mobileInformationCommand(String(args[0]), String(args[1]), native, { routeKey: str(args[2]), environments: storageEnvironments(),
      clearCache: (handle, environmentId) => mobileClearClientCaches(mobileClient, handle, environmentId) }).then(result => ({ ...result, requestRoute: String(args[2]) }));
  },
  informationLegal: (args, _store, storage, nativeInput) => {
    const native = sourceNative('informationLegal', args, nativeInput);
    return { configuration: mobileInformationLegalConfiguration(String(args[0]), String(args[1]), args[2]) };
  },
  informationRoute: (args, _store, storage, nativeInput) => {
    const native = sourceNative('informationRoute', args, nativeInput);
    return { active: MOBILE_INFORMATION_ROUTES.includes(String(args[0])), title: ({ settingsAbout: 'About T3 Code', settingsClientStorage: 'Client Storage', settingsDiagnostics: 'Diagnostics', settingsOpenSourceLicenses: 'Open source licenses', settingsOpenSourceLicense: 'License notice', settingsLegal: 'Legal' } as Record<string, string>)[String(args[0])] ?? '' };
  },
  audioStatus: (args, _store, storage, nativeInput) => {
    const native = sourceNative('audioStatus', args, nativeInput);
    return mobileAudioStatus(String(args[0] ?? ''), String(args[1] ?? ''));
  },
  audioAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('audioAction', args, nativeInput);
    return mobileAudioAction(String(args[0] ?? ''), String(args[1] ?? ''), native);
  },
  automationPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('automationPrepare', args, nativeInput);
    return mobileAutomationPrepare(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), String(args[4] ?? ''), Number(args[5]), native);
  },
  automationSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('automationSnapshot', args, nativeInput);
    const snapshot = mobileAutomationSnapshot();
    return args[1] === true ? { ...snapshot, editor: { ...snapshot.editor, busy: true, canSave: false, webhookCopyable: false } } : snapshot;
  },
  automationCommand: (args, _store, storage, nativeInput) => {
    const native = sourceNative('automationCommand', args, nativeInput);
    return mobileAutomationCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), Number(args[4]), native).then(result => ({ ...result, requestRoute: String(args[5] ?? '') }));
  },
  scheduledHeader: (args, _store, storage, nativeInput) => {
    const native = sourceNative('scheduledHeader', args, nativeInput);
    return mobileScheduledHeader(args);
  },
  voiceFocus: (args, _store, storage, nativeInput) => {
    if (newTaskGuard('voiceFocus', args)?.() === false) return { owner: '', label: String(args[1] || 'Draft') };
    const native = sourceNative('voiceFocus', args, nativeInput);
    return { owner: args[0] === true ? mobileComposerTarget(mobileClient).editorOwner : '', label: String(args[1] || 'Draft') };
  },
  voiceColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('voiceColors', args, nativeInput);
    return mobileVoiceColors(String(args[0]), String(args[1]));
  },
  voiceStatus: (args, _store, storage, nativeInput) => {
    const native = sourceNative('voiceStatus', args, nativeInput);
    return native?.available ? mobileVoiceStatus(mobileNative(native)) : { available: false, locale: '', reason: '', session: '', event: 0, eventKind: '', error: '', uri: '', elapsed: 0, levels: [], phase: 'idle' };
  },
  voiceSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('voiceSnapshot', args, nativeInput);
    const snapshot = mobileVoiceSnapshot(String(args[0]), mobileClient, Number(args[1]));
    return args[4] === true ? { ...snapshot, confirmationEnabled: false } : snapshot;
  },
  voiceAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('voiceAction', args, nativeInput);
    if (!native?.available) return { revision: mobileClient.revision, message: 'Voice input is unavailable.', data: mobileVoiceSnapshot(String(args[2])) };
    const handle = mobileNative(native);
    return mobileVoiceAction(String(args[0]), -1, -1, String(args[1]), String(args[2]), handle, nativeFiles(handle));
  },
  terminalView: (args, _store, storage, nativeInput) => {
    const native = sourceNative('terminalView', args, nativeInput);
    return mobileTerminalPrepare(String(args[0] ?? ''), String(args[1]), String(args[2]), Number(args[3]), Number(args[4]), args[5] === true ? native : null);
  },
  terminalAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('terminalAction', args, nativeInput);
    return mobileTerminalAction(String(args[0]), String(args[1]), String(args[2] ?? ''), native, storage!);
  },
  terminalEvent: (args, _store, storage, nativeInput) => {
    const native = sourceNative('terminalEvent', args, nativeInput);
    return mobileTerminalEvent(String(args[0]), String(args[1]));
  },
  terminalCapture: (args, _store, storage, nativeInput) => {
    const native = sourceNative('terminalCapture', args, nativeInput);
    return mobileTerminalCapture(String(args[0]), Number(args[1]), Number(args[2]));
  },
  terminalAttach: (args, _store, storage, nativeInput) => {
    const native = sourceNative('terminalAttach', args, nativeInput);
    return mobileTerminalAttachOutput(String(args[0]), String(args[1]), Number(args[2]), Number(args[3]), Number(args[4]), native, storage!, mobileClient, String(args[5]));
  },
  reviewColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('reviewColors', args, nativeInput);
    return mobileReviewColors(String(args[0]), String(args[1]));
  },
  reviewSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('reviewSnapshot', args, nativeInput);
    return mobileReviewSnapshot(args[0] === 'dark', mobileClient, args[1] === true);
  },
  reviewPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('reviewPrepare', args, nativeInput);
    return mobileReviewRead(native, String(args[0] ?? ''), args[1] === 'dark');
  },
  reviewAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('reviewAction', args, nativeInput);
    return mobileReviewAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), Number(args[4]), native, storage!, args[5] === 'dark', mobileClient, str(args[6])).then(result => ({ ...result, requestRoute: str(args[6]) }));
  },
  filesSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('filesSnapshot', args, nativeInput);
    return mobileFilesSnapshot(String(args[0] ?? ''));
  },
  filesPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('filesPrepare', args, nativeInput);
    return mobileFilesRead(String(args[0] ?? ''), String(args[1] ?? ''), native);
  },
  filesAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('filesAction', args, nativeInput);
    return mobileFilesAction(String(args[0]), String(args[1]), String(args[2]), String(args[3] ?? ''), native);
  },
  fileSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('fileSnapshot', args, nativeInput);
    if (String(args[4]).startsWith('/new/draft/files/')) return mobileNewTaskFileSnapshot(String(args[0] ?? ''), String(args[4]), String(args[6]), String(args[5]), args[1] === 'dark');
    return mobileFileSnapshot(String(args[0] ?? ''), args[1] === 'dark', Number(args[2] ?? 0));
  },
  filePrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('filePrepare', args, nativeInput);
    if (String(args[4]).startsWith('/new/draft/files/')) return mobileNewTaskFileRead(String(args[0] ?? ''), String(args[4]), String(args[6]), String(args[5]), args[1] === 'dark', native);
    return mobileFileRead(String(args[0] ?? ''), native, args[1] === 'dark', Number(args[2] ?? 0), args[3] === true);
  },
  fileMenu: args => mobileFileMenu(args[0] as FileSnapshot, args[1], args[2], args[3], args[4]),
  serverSettings: (args, _store, storage, nativeInput) => {
    const native = sourceNative('serverSettings', args, nativeInput);
    return mobileServerSettings(MOBILE_SERVER_ROUTES[String(args[0])] ?? 'new-threads', String(args[1]), args[2] === true ? native : null);
  },
  serverSettingChange: (args, _store, storage, nativeInput) => {
    const native = sourceNative('serverSettingChange', args, nativeInput);
    return mobileServerSettingsCommand(MOBILE_SERVER_ROUTES[String(args[0])] ?? 'new-threads', String(args[1]), String(args[2]), String(args[3]), native);
  },
  providerSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('providerSnapshot', args, nativeInput);
    const snapshot = mobileProviderAccountsSnapshot(String(args[0]), args[1] === true);
    return snapshot.ready || !obj(args[2]).error ? snapshot : { ...snapshot, error: str(obj(args[2]).error) };
  },
  providerAccounts: (args, _store, storage, nativeInput) => {
    const native = sourceNative('providerAccounts', args, nativeInput);
    return mobileProviderAccounts(String(args[0]), args[1] === true, native, native?.available === true);
  },
  providerAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('providerAction', args, nativeInput);
    return mobileProviderCommand(String(args[0]), String(args[1]), String(args[2]), native);
  },
  providerField: (args, _store, storage, nativeInput) => {
    const native = sourceNative('providerField', args, nativeInput);
    return mobileProviderField(String(args[0]), String(args[1]), String(args[2]));
  },
  attachmentDocument: (args, _store, storage, nativeInput) => {
    if (newTaskGuard('attachmentDocument', args)?.() === false) return EMPTY_ATTACHMENT_DOCUMENT;
    const native = sourceNative('attachmentDocument', args, nativeInput);
    if (args[5] !== true || !args[6]) return EMPTY_ATTACHMENT_DOCUMENT;
    return mobileAttachmentDocument(String(args[0]), String(args[1]), String(args[2]), args[3] === true, args[4] === 'dark', Number(args[7]), false, native);
  },
  attachmentNativePreview: (args, _store, storage, nativeInput) => {
    const native = sourceNative('attachmentNativePreview', args, nativeInput);
    let value; try { value = obj(JSON.parse(String(args[0]))); } catch { value = {}; }
    return { identifier: String(value.identifier ?? ''), name: String(value.name ?? ''), kind: String(value.kind ?? ''), sourceJSON: String(args[0]), ready: !!value.identifier, error: '' };
  },
  attachmentNativeEvent: (args, _store, storage, nativeInput) => {
    const native = sourceNative('attachmentNativeEvent', args, nativeInput);
    let value; try { value = obj(JSON.parse(String(args[0]))); } catch { value = {}; }
    return { identifier: value.identifier === args[1] ? String(value.identifier) : '', message: String(value.message ?? ''), operation: 'native-error', removed: false, sourceJSON: '' };
  },
  attachmentMenu: (args, _store, storage, nativeInput) => {
    const native = sourceNative('attachmentMenu', args, nativeInput);
    return mobileAttachmentMenu(args[0] as AttachmentDocumentSnapshot, args[1] === true);
  },
  attachmentDocumentAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('attachmentDocumentAction', args, nativeInput);
    return mobileAttachmentDocumentAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), args[4] as AttachmentDocumentSnapshot, native, storage!);
  },
  mediaPreview: (args, _store, storage, nativeInput) => {
    if (newTaskGuard('mediaPreview', args)?.() === false) return { identifier: '', name: '', kind: '', sourceJSON: '', ready: false, error: '' };
    const native = sourceNative('mediaPreview', args, nativeInput);
    if (args[5] !== true) return { identifier: '', name: '', kind: '', sourceJSON: '', ready: false, error: '' };
    return mobileMediaPrepare(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, mobileClient, String(args[3] ?? ''), String(args[4] ?? ''), String(args[11] ?? ''));
  },
  mediaCompletion: (args, _store, storage, nativeInput) => {
    const native = sourceNative('mediaCompletion', args, nativeInput);
    let value: Record<string, unknown> = {};
    try { value = obj(JSON.parse(String(args[0] ?? ''))); } catch { /* Ignore malformed or stale native events. */ }
    const identifier = String(value.identifier ?? '');
    if (!identifier || identifier !== args[1]) return { identifier: '', message: '' };
    mobileMediaForget(identifier);
    return { identifier, message: String(value.message ?? '') };
  },
  attachmentAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('attachmentAction', args, nativeInput);
    if(args[0]==='choice')return mobileComposerAttachmentMenuAction(str(args[1]),{visit:str(args[5]),url:str(args[6]),active:args[7]===true},str(args[4]),native,storage!);
    return mobileComposerAttachmentAction(String(args[0] ?? ''), String(args[1] ?? ''), native, storage!, mobileClient, str(args[4]));
  },
  composerAttachments: (args, _store, storage, nativeInput) => {
    if (newTaskGuard('composerAttachments', args)?.() === false) return { menuConfiguration:'', contentOwner: '', previewRequest: '', items: [], canPick: false, supportsFiles: false, remaining: 0, error: '' };
    sourceNative('composerAttachments', args, nativeInput);
    return mobileComposerAttachmentMenuSnapshot(mobileClient,{visit:str(args[4]),url:str(args[5]),active:args[6]===true},Number(args[3]));
  },
  composerPreviewsPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('composerPreviewsPrepare', args, nativeInput);
    return mobileComposerAttachmentPreviews(native, mobileClient, Number(args[2]), str(args[0]), str(args[1]));
  },
  preferences: (args, _store, storage, nativeInput) => {
    const native = sourceNative('preferences', args, nativeInput);
    return mobilePreferencesResource(native);
  },
  preferenceChange: (args, _store, storage, nativeInput) => {
    const native = sourceNative('preferenceChange', args, nativeInput);
    return mobileSavePreference(String(args[0] ?? ''), String(args[1] ?? ''), native);
  },
  applyAppearance: (args, _store, storage, nativeInput) => {
    const native = sourceNative('applyAppearance', args, nativeInput);
    return mobileApplyAppearance(args[0], native);
  },
  appearance: (args, _store, storage, nativeInput) => {
    const native = sourceNative('appearance', args, nativeInput);
    const preferences = normalizeMobilePreferences(args[0]);
    const resolved = resolveMobileAppearance(preferences, String(args[1] ?? 'light'));
    return { scheme: resolved.scheme, themeId: resolved.themeId, baseFontSize: resolved.baseFontSize, terminalFontSize: resolved.terminalFontSize,
      themeMode: preferences.themeMode, enterBehavior: preferences.composerEnterBehavior, groupingMode: preferences.projectGroupingMode };
  },
  settingsRoot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('settingsRoot', args, nativeInput);
    return settingsRoot(args);
  },
  settingsScopeEvent: (args, _store, storage, nativeInput) => {
    const native = sourceNative('settingsScopeEvent', args, nativeInput);
    return settingsScopeEvent(args);
  },
  settingsAppearance: (args, _store, storage, nativeInput) => {
    const native = sourceNative('settingsAppearance', args, nativeInput);
    return settingsAppearanceView(args[0], String(args[1] ?? 'light'), args[2] === true, Number(args[3]));
  },
  settingsChoices: (args, _store, storage, nativeInput) => {
    const native = sourceNative('settingsChoices', args, nativeInput);
    return settingsChoices(String(args[0] ?? ''), normalizeMobilePreferences(args[1]), args[2] === true);
  },
  pairingFields: (args, _store, storage, nativeInput) => {
    const native = sourceNative('pairingFields', args, nativeInput);
    return mobilePairingFields(String(args[0] ?? ''));
  },
  theme: (args, _store, storage, nativeInput) => {
    const native = sourceNative('theme', args, nativeInput);
    return mobileTheme(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  },
  environmentDetail: (args, _store, storage, nativeInput) => {
    const native = sourceNative('environmentDetail', args, nativeInput);
    return mobileEnvironmentDetail(String(args[0] ?? ''), native, storage);
  },
  environmentDetailCommand: (args, _store, storage, nativeInput) => {
    const native = sourceNative('environmentDetailCommand', args, nativeInput);
    return mobileEnvironmentDetailCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage);
  },
  layoutFacts: (args, _store, storage, nativeInput) => {
    const native = sourceNative('layoutFacts', args, nativeInput);
    return mobileLayoutFacts(String(args[0] ?? ''));
  },
  homeChromeEvent: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homeChromeEvent', args, nativeInput);
    return homeChromeEvent(String(args[0] ?? ''));
  },
  homeChrome: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homeChrome', args, nativeInput);
    return homeChromeView(args);
  },
  archiveChrome: (args, _store, storage, nativeInput) => {
    const native = sourceNative('archiveChrome', args, nativeInput);
    return archiveChromeView(args);
  },
  archiveChromeEvent: (args, _store, storage, nativeInput) => {
    const native = sourceNative('archiveChromeEvent', args, nativeInput);
    return archiveChromeEvent(String(args[0] ?? ''), String(args[1] ?? ''), args[2]);
  },
  archiveColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('archiveColors', args, nativeInput);
    return mobileArchiveColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  },
  agentColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('agentColors', args, nativeInput);
    return mobileAgentColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  },
  archiveView: (args, _store, storage, nativeInput) => {
    const native = sourceNative('archiveView', args, nativeInput);
    return mobileArchive(Number(args[0]), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? 'newest'), args[4] === true ? native : null);
  },
  archiveAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('archiveAction', args, nativeInput);
    return mobileArchiveCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), native, storage);
  },
  agentActivity: (args, _store, storage, nativeInput) => {
    const native = sourceNative('agentActivity', args, nativeInput);
    return mobileAgentActivity(String(args[0] ?? ''), String(args[1] ?? ''), Number(args[2]));
  },
  composerColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('composerColors', args, nativeInput);
    return mobileComposerColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  },
  composerSettings: (args, _store, storage, nativeInput) => {
    if (newTaskGuard('composerSettings', args)?.() === false) return { ...mobileComposerSettings('', '', false), open: false, canEdit: false, canSave: false, models: [], filters: [], options: [], runtimes: [], error: '' };
    const native = sourceNative('composerSettings', args, nativeInput);
    return mobileComposerSettings(String(args[0] ?? ''), String(args[1] ?? ''), args[2] === true);
  },
  composerAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('composerAction', args, nativeInput);
    const requestRoute = String(args[5] ?? '');
    return mobileComposerSettingsAction(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage!).then(result => ({ ...result, requestRoute }));
  },
  newTaskView: (args, _store, storage, nativeInput) => {
    const native = sourceNative('newTaskView', args, nativeInput);
    return args[4] === true ? mobileNewTaskChooser(String(args[0] ?? ''), String(args[5] ?? 'repository')) : mobileNewTask(String(args[0] ?? ''));
  },
  newTaskCachedPrepare: (args, _store, storage, nativeInput) => {
    const current = newTaskGuard('newTaskCachedPrepare', args) ?? (() => true);
    if (!current()) return { revision: mobileClient.revision, loaded: false };
    const native = sourceNative('newTaskCachedPrepare', args, nativeInput);
    return args[1] === true ? mobileNewTaskCachedPrepare(String(args[0] ?? ''), native, mobileClient, current)
      : { revision: mobileClient.revision, loaded: false };
  },
  newTaskPrepare: (args, _store, storage, nativeInput) => {
    const current = newTaskGuard('newTaskPrepare', args) ?? (() => true);
    if (!current()) return { revision: mobileClient.revision, loaded: false };
    const native = sourceNative('newTaskPrepare', args, nativeInput);
    return args[1] === true ? mobileNewTaskPrepare(String(args[0] ?? ''), native, mobileClient, current)
      : { revision: mobileClient.revision, loaded: false };
  },
  threadColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('threadColors', args, nativeInput);
    return mobileThreadColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  },
  selectThread: (args, _store, storage, nativeInput) => {
    const native = sourceNative('selectThread', args, nativeInput);
    return mobileCommand(['select-thread', String(args[0] ?? ''), '', 0], native, storage!).then(change =>
    ({ ...change, environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, requestRoute: String(args[1] ?? '') }));
  },
  threadView: (args, _store, storage, nativeInput) => {
    const native = sourceNative('threadView', args, nativeInput);
    return threadView(args, native);
  },
  threadFork: (args, _store, storage, nativeInput) => {
    const handle = nativeInput?.available ? letGoAware(mobileNative(sourceNative('threadFork', args, nativeInput)!)) : nativeInput;
    const handles = mobileDraftRecoveryHandles(mobileClient, handle, handle?.available ? nativeFiles(handle) : storage!);
    return mobileThreadForkAction(mobileClient, str(args[0]), str(args[1]), str(args[2]), handles.native, handles.storage);
  },
  answerFilesPrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('answerFilesPrepare', args, nativeInput);
    return mobileThreadAnswerFilesPrepare(str(args[0]), str(args[1]), Number(args[2]), native);
  },
  homeColors: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homeColors', args, nativeInput);
    return mobileHomeColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  },
  homePreferences: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homePreferences', args, nativeInput);
    return mobileShelves(native);
  },
  toggleShelf: (args, _store, storage, nativeInput) => {
    const native = sourceNative('toggleShelf', args, nativeInput);
    return mobileToggleShelf(String(args[0] ?? ''), native);
  },
  homeArrangeAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homeArrangeAction', args, nativeInput);
    const value = homeArrangeActionValue(str(args[1]), str(args[4]), str(args[5]), str(args[6]));
    return mobileHomeAction(str(args[0]), str(args[2]), str(args[3]), 'arrange', value, Number(args[7]), native);
  },
  homeAction: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homeAction', args, nativeInput);
    return mobileHomeAction(str(args[0]), str(args[1]), str(args[2]), str(args[3]), str(args[4]), Number(args[5]), native, mobileClient, fleet, native?.available ? nativeFiles(native) : storage);
  },
  homeView: (args, _store, storage, nativeInput) => {
    const native = sourceNative('homeView', args, nativeInput);
    return mobileHomeView(args);
  },
  snapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('snapshot', args, nativeInput);
    noteNow(mobileClient, Number(args[2]));
    return mobileSnapshot(native, storage!, Number(args[2])).then(snapshot => connectionView(snapshot, String(args[0] ?? 'light'), String(args[1] ?? 't3-code')));
  },
  cameraPermission: (args, _store, storage, nativeInput) => {
    const native = sourceNative('cameraPermission', args, nativeInput);
    if (!native?.available) return { status: 'unavailable' };
    return bridgeReply(native, { op: 'mobileCameraPermission' }).then(reply =>
      ({ status: reply.ok ? String(obj(reply.value).status ?? 'unavailable') : 'error:Camera access could not be checked.' }));
  },
  alert: (args, _store, storage, nativeInput) => {
    const native = sourceNative('alert', args, nativeInput);
    if (!native?.available) return { choice: 'cancel' };
    return bridgeReply(native, { op: 'mobileAlert', kind: String(args[0] ?? 'info'),
      title: String(args[1] ?? ''), message: String(args[2] ?? '') }).then(reply =>
      ({ choice: reply.ok ? String(obj(reply.value).choice ?? 'cancel') : 'cancel' }));
  },
  queueSnapshot: (args, _store, storage, nativeInput) => {
    const native = sourceNative('queueSnapshot', args, nativeInput);
    return mobileQueueSnapshot(str(args[0]), args[1] === true, Number(args[2]));
  },
  queuePrepare: (args, _store, storage, nativeInput) => {
    const native = sourceNative('queuePrepare', args, nativeInput);
    return mobileQueuePrepare(str(args[0]), str(args[1]), str(args[2]), Number(args[3]), native);
  },
  command: (args, _store, storage, nativeInput) => {
    const native = sourceNative('command', args, nativeInput);
    const operation = String(args[0] ?? ''), key = String(args[1] ?? '');
    if (operation.startsWith('queue:')) return mobileQueueCommand(args, native, storage!);
    const separator = key.indexOf('\n');
    const origin = separator < 0 ? key : key.slice(0, separator);
    const environmentId = separator < 0 ? '' : key.slice(separator + 1);
    if (operation === 'forget') return mobileCommand(['forget', origin, environmentId, 0], native, storage!);
    if (operation === 'save-environment') return mobileCommand([operation, environmentId,
      JSON.stringify({ label: String(args[2] ?? ''), url: String(args[3] ?? '') }), 0], native, storage!);
    return mobileCommand(args, native, storage!);
  },
};
export const answer: Answer = (source, args, store, storage, native) => {
  if (!Object.prototype.hasOwnProperty.call(sources, source)) throw new Error(`Unknown mobile source: ${source}`);
  return sources[source](args, store, storage, native);
};

function sourceNative(source: string, args: unknown[], nativeInput?: Native | null): Native | null | undefined {
  const native = nativeInput ? settingsProviderNative(nativeInput) : nativeInput;
  const guarded = newTaskGuard(source, args);
  if (guarded && !guarded()) throw new ClientError('Choose a project in the current new task before continuing.', 'superseded');
  if (!guarded || !native) return native;
  return { available: native.available, watch: topic => native.watch(topic), later: async input => {
    if (!guarded()) throw new ClientError('The new task route changed.', 'superseded');
    const result = await native.later(input);
    if (!guarded()) throw new ClientError('The new task route changed.', 'superseded');
    return result;
  } };
}

const taskGuardIndices: Record<string, number> = { filePrepare: 5, command: 4, composerAction: 3, composerSettings: 4,
  attachmentAction: 2, composerAttachments: 1, voiceFocus: 3, voiceAction: 3,
  mediaPreview: 9, attachmentDocument: 8, attachmentDocumentAction: 5, newTaskPrepare: 8, newTaskCachedPrepare: 8 };
function newTaskGuard(source: string, args: unknown[]): (() => boolean) | null {
  // Existing recordings retain their captured draft and cleanup owner offscreen.
  if (source === 'voiceAction' && args[0] !== 'start') return null;
  if (source === 'filePrepare' && String(args[4] ?? '').startsWith('/new/draft/files/'))
    return () => mobileNewTaskFileRouteCurrent(String(args[5] ?? ''), String(args[6] ?? ''), String(args[4]));
  const index = taskGuardIndices[source];
  if (index === undefined || !args[index]) return null;
  const owner = String(args[index]), route = String(args[index + 1] ?? '');
  return () => mobileNewTaskFlowOwns(owner, route);
}

async function newTaskFlow(args: unknown[], native?: Native | null) {
  noteNow(mobileClient, Number(args[5]));
  const session = String(args[0]), visit = String(args[1]), location = String(args[2]), active = args[3] === true;
  // Invalidate departing actions synchronously, before the catalog read or any
  // dependent resource can use the previous flow's composer owner.
  const initial = mobileNewTaskFlowView(session, visit, location, active, false);
  if (!active || initial.status === 'add-project' || initial.status === 'file' || !native?.available) return initial;
  const catalog = await bridgeReply(native, { op: 'environments' });
  if (!mobileNewTaskFlowCurrent(initial.owner, visit, location)) return initial;
  const saved = catalog.ok ? arr(obj(catalog.value).saved).filter(entry => entry.enabled !== false) : [];
  const pending = saved.some(entry => {
    const id = str(entry.environmentId);
    if (id === mobileClient.environmentId) return !mobileClient.shellLoaded && ['connected', 'connecting', 'reconnecting'].includes(mobileClient.connection);
    const entryState = [...fleet.entries.values()].find(entry => entry.environmentId === id);
    return !entryState || ['connecting', 'reconnecting'].includes(entryState.phase)
      || entryState.phase === 'connected' && entryState.synchronized !== entryState.generation;
  });
  return mobileNewTaskFlowView(session, visit, location, active, catalog.ok && mobileClient.preferencesLoaded && !pending);
}

async function threadView(args: unknown[], native?: Native | null) {
  const [_revision, time, scheme, environmentId, threadId, active, visit, routeName] = args;
  mobileThreadForkObserveRoute(mobileClient, str(visit), active === true && routeName === 'thread'
    && environmentId === mobileClient.environmentId && threadId === mobileClient.threadId);
  const queued = mobileOutboxThread(active === true ? str(environmentId) : '', active === true ? str(threadId) : '', Number(time), scheme === 'dark');
  const matched = active === true && environmentId === mobileClient.environmentId && threadId === mobileClient.threadId;
  if (queued && !matched) return queued;
  if (matched) await mobileThreadPrepare(native, Number(time));
  if (queued) {
    const preparing = mobileOutboxThread(str(environmentId), str(threadId), Number(time), scheme === 'dark');
    if (preparing) return preparing;
  }
  const view = mobileThread(Number(time), scheme === 'dark', mobileClient, matched && routeName === 'thread' ? str(visit) : '');
  if (matched) return view;
  return { ...view, title: '', loaded: false, loading: active === true, rows: [], approvals: [],
    emptyTitle: active ? 'Loading thread' : '', emptyDetail: '', readsNeeded: false,
    composer: { ...view.composer, canSend: false, canStop: false, canOperate: false, draft: '' } };
}


/** Only UI preferences cross from Contract. Workspace/connection facts remain
 * the actual shared client's, so stale or fabricated option fields cannot
 * establish an inspector owner. No read or native handle is retained here. */
function workspace(args: unknown[]) {
  const raw = obj(args[1]);
  const record = (value: unknown) => { try { return typeof value === 'string' ? obj(JSON.parse(value)) : obj(value); } catch { return {}; } };
  const pane = record(raw.inspectorRegistration ?? raw.inspector), role = record(raw.inspectorRole ?? raw.role);
  const dimension = (value: unknown) => typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : 0;
  const preference = (value: unknown) => typeof value === 'boolean' ? value : true;
  const appearance = raw.appearance === 'dark' ? 'dark' : 'light';
  const selectedThread = mobileClient.shell.threads.some(thread => thread.id === mobileClient.threadId);
  const hasWorkspace = mobileClient.ready && selectedThread && !!mobileClient.threadId && !!workspaceOf(mobileClient).cwd;
  const paneRole = pane.role === 'inspector' || pane.role === 'supplementary' ? pane.role : null;
  const paneKind = (['files', 'git', 'route', 'changed-files'] as const).find(kind => kind === pane.kind);
  const paneGeneration = typeof pane.generation === 'number' && Number.isSafeInteger(pane.generation) ? pane.generation : -1;
  const inspector: MobileWorkspaceOptions['inspector'] = str(pane.token) && str(pane.routeId) && paneRole && paneKind
    ? { token: str(pane.token), routeId: str(pane.routeId), environmentId: str(pane.environmentId), threadId: str(pane.threadId),
      generation: paneGeneration, role: paneRole, kind: paneKind, selectedPath: str(pane.selectedPath), active: pane.active === true,
      renderable: pane.renderable === true && hasWorkspace && pane.environmentId === mobileClient.environmentId && pane.threadId === mobileClient.threadId } : null;
  return mobileWorkspace(args[0], { width: dimension(raw.width), height: dimension(raw.height),
    primarySidebarPreferredVisible: preference(raw.primarySidebarPreferredVisible),
    supplementaryPanePreferredVisible: preference(raw.supplementaryPanePreferredVisible),
    fileInspectorPreferredVisible: preference(raw.fileInspectorPreferredVisible),
    supplementaryPanePreferredWidth: dimension(raw.supplementaryPanePreferredWidth), fileInspectorPreferredWidth: dimension(raw.fileInspectorPreferredWidth),
    hasWorkspace, selectedEnvironmentId: mobileClient.environmentId, selectedThreadId: mobileClient.threadId, generation: mobileClient.generation,
    threadMode: raw.threadMode === 'files' || raw.threadMode === 'git' ? raw.threadMode : '', threadModeOwner: str(raw.threadModeOwner),
    inspector, role: str(role.token) && str(role.routeId) && (role.value === 'inspector' || role.value === 'supplementary')
      ? { token: str(role.token), routeId: str(role.routeId), value: role.value } : null,
    inspectorExitToken: str(raw.inspectorExitToken), inspectorResizing: raw.inspectorResizing === true,
    dividerColor: str(raw.dividerColor), dividerActiveColor: str(raw.dividerActiveColor),
    reducedMotion: raw.reducedMotion === true, appearance, background: str(raw.background) || mobileTheme(appearance).screen });
}
