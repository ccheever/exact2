import { mobileThreadAnswerFilesPrepare } from './thread';
import { mobileGitColors } from './git-colors';
import { mobileGitSnapshot, mobileGitRead, mobileGitAction } from './git-overview';
import { mobileGitBranchesSnapshot, mobileGitBranchesRead, mobileGitBranchAction } from './git-branches';
import { workspaceInspectorSnapshot } from './workspace-inspector';
import { mobileInspectorContext, mobileInspectorTransition, mobileInspectorPresentation } from './workspace-inspector-adapter';
import { mobileWorkspaceEvent, mobileWorkspace, mobileWorkspaceThreadSelection, mobileWorkspaceFileSelection, type MobileWorkspaceOptions } from './mobile-workspace';
import { workspaceOf } from './shared/r4-surfaces-panel';
import { mobileStreamingDescriptor } from './haptics';
import { mobileNewTaskFlowView, mobileNewTaskFlowAction, mobileNewTaskFlowOwns, mobileNewTaskFlowCurrent } from './new-task-flow';
import { mobileThreadPreferences, mobileThreadPreferencesCommand } from './settings-thread-preferences';
import { mobileProjectOverview, mobileProjectRename } from './settings-project';
import { mobileAccountRouteEntry } from './settings-account';
import { mobileNotificationsSettings } from './settings-notifications';
import { mobileAppLink } from './navigation-links';
import { mobilePreviewOwner, mobilePreviewPrepare, mobilePreviewStatus, mobilePreviewAction, mobilePreviewMenus, mobilePreviewMenuAction } from './mobile-preview-flow';
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
import { mobileFilesRead, mobileFilesSnapshot, mobileFilesAction, mobileFileRead, mobileFileSnapshot } from './file-data';
import { MOBILE_SERVER_ROUTES, mobileServerSettings, mobileServerSettingsCommand } from './settings-server';
import { settingsProviderNative, mobileProviderAccounts, mobileProviderAccountsSnapshot, mobileProviderCommand, mobileProviderField } from './settings-provider';
import { mobileMediaPrepare, mobileMediaShare, mobileMediaForget } from './media-preview';
import { mobileComposerAttachmentAction, mobileComposerAttachments, mobileComposerAttachmentPreviews } from './composer-attachments';
// @ref llp/1106.003-pairing-and-transport.decision.md#decision
import { mobileClient, mobileNative, mobileSnapshot, mobileCommand, mobilePairingFields } from './client';
import { mobileEnvironmentDetail, mobileEnvironmentDetailCommand } from './environment-detail';
import { arr, obj, str } from './shared/domain';
import { fleet } from './shared/settings-b-fleet';
import { mobileShelves, mobileToggleShelf, mobileHomeView } from './home-state';
import { mobileTheme, mobileHomeColors, mobileThreadColors, mobileComposerColors, mobileArchiveColors, mobileAgentColors } from './design';
import { mobilePreferencesResource, mobileSavePreference, mobileApplyAppearance, normalizeMobilePreferences, resolveMobileAppearance } from './settings-preferences';
import { settingsAppearanceView, settingsChoices } from './settings-appearance';
import { mobileArchive, mobileArchiveCommand } from './archive';
import { mobileAgentActivity } from './agent-activity';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTask, mobileNewTaskChooser, mobileNewTaskPrepare } from './new-task';
import { mobileThread, mobileThreadPrepare } from './thread';
import { mobileLayoutFacts, mobileScheduledHeader, homeChromeEvent, homeChromeView, settingsRoot, settingsScopeEvent } from './root-presentation';
import { connectionView } from './presentation';
import { bridgeReply, ClientError, nativeFiles, type Files, type Native } from './shared/protocol';

export const appId = 'com.exact.t3code.ios';
export const grants = 'device.camera purpose.camera device.microphone purpose.microphone';

export function answer(source: string, args: unknown[], _store?: unknown, storage?: Files, native?: Native | null) {
  if (source === 'inspectorState') return workspaceInspectorSnapshot(str(args[0]));
  if (source === 'inspectorContext') return mobileInspectorContext(args[0], args[1] === true, args[2] === true, Number(args[3]), args[4] === true, args[5] === true, args[6] === 'dark');
  if (source === 'inspectorTransition') return mobileInspectorTransition(str(args[0]), str(args[1]), str(args[2]), str(args[3]), Number(args[4]), str(args[5]), args[6] === 'dark');
  if (source === 'inspectorPresentation') return mobileInspectorPresentation(str(args[0]), str(args[1]), args[2] === true, str(args[3]), str(args[4]), str(args[5]), mobileClient, Number(args[6]));
  if (source === 'workspaceEvent') return mobileWorkspaceEvent(str(args[0]), str(args[1]), str(args[2]), args[3] === true, Number(args[4]));
  if (source === 'gitColors') return mobileGitColors(str(args[0]), str(args[1]));
  if (source === 'gitSnapshot') return mobileGitSnapshot(Number(args[0]));
  if (source === 'gitRead') return mobileGitRead(str(args[0]), Number(args[1]), native);
  if (source === 'gitBranchesSnapshot') return mobileGitBranchesSnapshot();
  if (source === 'gitBranchesRead') return mobileGitBranchesRead(str(args[0]), Number(args[1]), native);
  if (source === 'gitAction') return mobileGitAction(str(args[0]), str(args[1]), str(args[2]), str(args[3]), Number(args[4]), native).then(result => ({ ...result, requestRoute: str(args[5]) }));
  if (source === 'gitBranchAction') return mobileGitBranchAction(str(args[0]), str(args[1]), str(args[2]), str(args[3]), Number(args[4]), native).then(result => ({ ...result, requestRoute: str(args[5]) }));
  if (source === 'workspace') return workspace(args);
  if (source === 'workspaceThreadSelection') return mobileWorkspaceThreadSelection(args[0], args[1] === true, str(args[2]), str(args[3]));
  if (source === 'workspaceFileSelection') return mobileWorkspaceFileSelection(args[0], args[1] === true, str(args[2]), str(args[3]), str(args[4]));
  if (source === 'newTaskFlow') return newTaskFlow(args, native);
  if (native) native = settingsProviderNative(native);
  const guarded = newTaskGuard(source, args);
  if (guarded && !guarded()) return unavailableTaskSource(source, args);
  if (guarded && native) {
    const base = native;
    native = { available: base.available, watch: topic => base.watch(topic), later: async input => {
      if (!guarded()) throw new ClientError('The new task route changed.', 'superseded');
      const result = await base.later(input);
      if (!guarded()) throw new ClientError('The new task route changed.', 'superseded'); return result;
    } };
  }
  if (source === 'newTaskAction') return mobileNewTaskFlowAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), String(args[4]), native, storage!);
  if (source === 'threadPreferences') return mobileThreadPreferences(String(args[1]), args[2], args[3] === true, args[0] === true ? native : null);
  if (source === 'threadPreferencesChange') {
    const requestRoute = String(args[3]);
    return mobileThreadPreferencesCommand(String(args[0]), String(args[1]), String(args[2]), native).then(result => ({ ...result, requestRoute, saved: false }));
  }
  if (source === 'projectOverview') return mobileProjectOverview(String(args[1]), args[0] === true ? native : null);
  if (source === 'projectRename') {
    const requestRoute = String(args[3]);
    return mobileProjectRename(String(args[0]), String(args[1]), String(args[2]), native).then(result => ({ ...result, requestRoute }));
  }
  if (source === 'streamingAssistant') return mobileStreamingDescriptor(String(args[1]), String(args[2]), String(args[3]), mobileClient);
  if (source === 'notificationSettings') return mobileNotificationsSettings();
  if (source === 'accountEntry') {
    const entry = mobileAccountRouteEntry(String(args[0]), args[1] === true);
    return { kind: entry?.kind ?? '', route: entry?.kind === 'replace' ? entry.route : '', requestRoute: String(args[2]) };
  }
  if (source === 'appLink') return mobileAppLink(String(args[0]), String(args[1]), args[2] === true);
  if (source === 'previewOwner') return mobilePreviewOwner(args[0] === true);
  if (source === 'previewPrepare') return mobilePreviewPrepare(String(args[0]), native);
  if (source === 'previewColors') return mobilePreviewColors(String(args[0]), String(args[1]));
  if (source === 'previewStatus') return mobilePreviewStatus(String(args[0]), String(args[1]), args[2] === true, native);
  if (source === 'previewMenus') return mobilePreviewMenus(String(args[0]), args[1]);
  if (source === 'previewAction') return mobilePreviewAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), native).then(result => ({ ...result, requestRoute: String(args[4]) }));
  if (source === 'previewMenuAction') return mobilePreviewMenuAction(String(args[0]), String(args[1]), String(args[2]), native).then(result => ({ ...result, requestRoute: String(args[3]) }));
  if (source === 'informationPrepare') return mobileInformationPrepare(String(args[0]), native);
  if (source === 'informationSnapshot') return mobileInformationSnapshot(String(args[0] ?? ''), String(args[1] ?? ''));
  if (source === 'informationCommand') return mobileInformationCommand(String(args[0]), String(args[1]), native).then(result => ({ ...result, requestRoute: String(args[2]) }));
  if (source === 'informationLegal') return { configuration: mobileInformationLegalConfiguration(String(args[0]), String(args[1]), args[2]) };
  if (source === 'informationRoute') return { active: MOBILE_INFORMATION_ROUTES.includes(String(args[0])), title: ({ settingsAbout: 'About T3 Code', settingsClientStorage: 'Client Storage', settingsDiagnostics: 'Diagnostics', settingsOpenSourceLicenses: 'Open source licenses', settingsOpenSourceLicense: 'License notice', settingsLegal: 'Legal' } as Record<string, string>)[String(args[0])] ?? '' };
  if (source === 'audioStatus') return mobileAudioStatus(String(args[0] ?? ''), String(args[1] ?? ''));
  if (source === 'audioAction') return mobileAudioAction(String(args[0] ?? ''), String(args[1] ?? ''), native);
  if (source === 'automationPrepare') return mobileAutomationPrepare(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), String(args[4] ?? ''), Number(args[5]), native);
  if (source === 'automationSnapshot') {
    const snapshot = mobileAutomationSnapshot();
    return args[1] === true ? { ...snapshot, editor: { ...snapshot.editor, busy: true, canSave: false, webhookCopyable: false } } : snapshot;
  }
  if (source === 'automationCommand') return mobileAutomationCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), Number(args[4]), native).then(result => ({ ...result, requestRoute: String(args[5] ?? '') }));
  if (source === 'scheduledHeader') return mobileScheduledHeader(args);
  if (source === 'voiceFocus') return { owner: args[0] === true ? mobileClient.draftKey : '', label: String(args[1] || 'Draft') };
  if (source === 'voiceColors') return mobileVoiceColors(String(args[0]), String(args[1]));
  if (source === 'voiceStatus') return native?.available ? mobileVoiceStatus(mobileNative(native)) : { available: false, locale: '', reason: '', session: '', event: 0, eventKind: '', error: '', uri: '', elapsed: 0, levels: [], phase: 'idle' };
  if (source === 'voiceSnapshot') {
    const snapshot = mobileVoiceSnapshot(String(args[0]), mobileClient, Number(args[1]));
    return args[4] === true ? { ...snapshot, confirmationEnabled: false } : snapshot;
  }
  if (source === 'voiceAction') {
    if (!native?.available) return { revision: mobileClient.revision, message: 'Voice input is unavailable.', data: mobileVoiceSnapshot(String(args[2])) };
    const handle = mobileNative(native);
    return mobileVoiceAction(String(args[0]), -1, -1, String(args[1]), String(args[2]), handle, nativeFiles(handle));
  }
  if (source === 'terminalView') return mobileTerminalPrepare(String(args[0] ?? ''), String(args[1]), String(args[2]), Number(args[3]), Number(args[4]), args[5] === true ? native : null);
  if (source === 'terminalAction') return mobileTerminalAction(String(args[0]), String(args[1]), String(args[2] ?? ''), native, storage!);
  if (source === 'terminalMenu') return mobileTerminalAction('menu', String(args[0]), JSON.stringify({ tabs: args[1], readOnly: args[2] === true, fontSize: Number(args[3]) }), native, storage!);
  if (source === 'terminalEvent') return mobileTerminalEvent(String(args[0]), String(args[1]));
  if (source === 'terminalCapture') return mobileTerminalCapture(String(args[0]), Number(args[1]), Number(args[2]));
  if (source === 'terminalAttach') return mobileTerminalAttachOutput(String(args[0]), String(args[1]), Number(args[2]), Number(args[3]), Number(args[4]), native, storage!);
  if (source === 'reviewColors') return mobileReviewColors(String(args[0]), String(args[1]));
  if (source === 'reviewSnapshot') return mobileReviewSnapshot(args[0] === 'dark', mobileClient, args[1] === true);
  if (source === 'reviewPrepare') return mobileReviewRead(native, String(args[0] ?? ''), args[1] === 'dark');
  if (source === 'reviewAction') return mobileReviewAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), Number(args[4]), native, storage!, args[5] === 'dark').then(result => ({ ...result, requestRoute: str(args[6]) }));
  if (source === 'filesSnapshot') return mobileFilesSnapshot(String(args[0] ?? ''));
  if (source === 'filesPrepare') return mobileFilesRead(String(args[0] ?? ''), String(args[1] ?? ''), native);
  if (source === 'filesAction') return mobileFilesAction(String(args[0]), String(args[1]), String(args[2]), String(args[3] ?? ''), native);
  if (source === 'fileSnapshot') return mobileFileSnapshot(String(args[0] ?? ''), args[1] === 'dark', Number(args[2] ?? 0));
  if (source === 'filePrepare') return mobileFileRead(String(args[0] ?? ''), native, args[1] === 'dark', Number(args[2] ?? 0), args[3] === true);
  if (source === 'serverSettings') return mobileServerSettings(MOBILE_SERVER_ROUTES[String(args[0])] ?? 'new-threads', String(args[1]), args[2] === true ? native : null);
  if (source === 'serverSettingChange') return mobileServerSettingsCommand(MOBILE_SERVER_ROUTES[String(args[0])] ?? 'new-threads', String(args[1]), String(args[2]), String(args[3]), native);
  if (source === 'providerSnapshot') {
    const snapshot = mobileProviderAccountsSnapshot(String(args[0]), args[1] === true);
    return snapshot.ready || !obj(args[2]).error ? snapshot : args[2];
  }
  if (source === 'providerAccounts') return mobileProviderAccounts(String(args[0]), args[1] === true, native, native?.available === true);
  if (source === 'providerAction') return mobileProviderCommand(String(args[0]), String(args[1]), String(args[2]), native);
  if (source === 'providerField') return mobileProviderField(String(args[0]), String(args[1]), String(args[2]));
  if (source === 'attachmentDocument') {
    if (args[5] !== true || !args[6]) return EMPTY_ATTACHMENT_DOCUMENT;
    return mobileAttachmentDocument(String(args[0]), String(args[1]), String(args[2]), args[3] === true, args[4] === 'dark', Number(args[7]), false, native);
  }
  if (source === 'attachmentNativePreview') {
    let value; try { value = obj(JSON.parse(String(args[0]))); } catch { value = {}; }
    return { identifier: String(value.identifier ?? ''), name: String(value.name ?? ''), kind: String(value.kind ?? ''), sourceJSON: String(args[0]), ready: !!value.identifier, error: '' };
  }
  if (source === 'attachmentNativeEvent') {
    let value; try { value = obj(JSON.parse(String(args[0]))); } catch { value = {}; }
    return { identifier: value.identifier === args[1] ? String(value.identifier) : '', message: String(value.message ?? ''), operation: 'native-error', removed: false, sourceJSON: '' };
  }
  if (source === 'attachmentMenu') return mobileAttachmentMenu(args[0] as AttachmentDocumentSnapshot, args[1] === true);
  if (source === 'attachmentDocumentAction') return mobileAttachmentDocumentAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), args[4] as AttachmentDocumentSnapshot, native, storage!);
  if (source === 'mediaPreview') {
    if (args[5] !== true) return { identifier: '', name: '', kind: '', sourceJSON: '', ready: false, error: '' };
    return mobileMediaPrepare(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, mobileClient, String(args[3] ?? ''), String(args[4] ?? ''));
  }
  if (source === 'mediaCompletion') {
    let value: Record<string, unknown> = {};
    try { value = obj(JSON.parse(String(args[0] ?? ''))); } catch { /* Ignore malformed or stale native events. */ }
    const identifier = String(value.identifier ?? '');
    if (!identifier || identifier !== args[1]) return { identifier: '', message: '' };
    mobileMediaForget(identifier);
    return { identifier, message: String(value.message ?? '') };
  }
  if (source === 'shareMedia') return mobileMediaShare(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native);
  if (source === 'attachmentAction') return mobileComposerAttachmentAction(String(args[0] ?? 'menu'), String(args[1] ?? ''), native, storage!);
  if (source === 'composerAttachments') return mobileComposerAttachmentPreviews(native).then(() => mobileComposerAttachments());
  if (source === 'preferences') return mobilePreferencesResource(native);
  if (source === 'preferenceChange') return mobileSavePreference(String(args[0] ?? ''), String(args[1] ?? ''), native);
  if (source === 'applyAppearance') return mobileApplyAppearance(args[0], native);
  if (source === 'appearance') {
    const preferences = normalizeMobilePreferences(args[0]);
    const resolved = resolveMobileAppearance(preferences, String(args[1] ?? 'light'));
    return { scheme: resolved.scheme, themeId: resolved.themeId, baseFontSize: resolved.baseFontSize, terminalFontSize: resolved.terminalFontSize,
      themeMode: preferences.themeMode, enterBehavior: preferences.composerEnterBehavior, groupingMode: preferences.projectGroupingMode };
  }
  if (source === 'settingsRoot') return settingsRoot(args);
  if (source === 'settingsScopeEvent') return settingsScopeEvent(args);
  if (source === 'settingsAppearance') return settingsAppearanceView(args[0], String(args[1] ?? 'light'), args[2] === true, Number(args[3]));
  if (source === 'settingsChoices') return settingsChoices(String(args[0] ?? ''), normalizeMobilePreferences(args[1]), args[2] === true);
  if (source === 'pairingFields') return mobilePairingFields(String(args[0] ?? ''));
  if (source === 'theme') return mobileTheme(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'environmentDetail') return mobileEnvironmentDetail(String(args[0] ?? ''), native, storage);
  if (source === 'environmentDetailCommand') return mobileEnvironmentDetailCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage);
  if (source === 'layoutFacts') return mobileLayoutFacts(String(args[0] ?? ''));
  if (source === 'homeChromeEvent') return homeChromeEvent(String(args[0] ?? ''));
  if (source === 'homeChrome') return homeChromeView(args);
  if (source === 'archiveColors') return mobileArchiveColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'agentColors') return mobileAgentColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'archiveView') return mobileArchive(Number(args[0]), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? 'newest'), args[4] === true ? native : null);
  if (source === 'archiveAction') return mobileArchiveCommand(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), native, storage);
  if (source === 'agentActivity') return mobileAgentActivity(String(args[0] ?? ''), String(args[1] ?? ''), Number(args[2]));
  if (source === 'composerColors') return mobileComposerColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'composerSettings') return mobileComposerSettings(String(args[0] ?? ''), String(args[1] ?? ''), args[2] === true);
  if (source === 'composerAction') {
    const requestRoute = String(args[5] ?? '');
    return mobileComposerSettingsAction(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage!).then(result => ({ ...result, requestRoute }));
  }
  if (source === 'newTaskView') return args[4] === true ? mobileNewTaskChooser(String(args[0] ?? ''), String(args[5] ?? 'repository')) : mobileNewTask(String(args[0] ?? ''));
  if (source === 'newTaskPrepare') return args[1] === true ? mobileNewTaskPrepare(String(args[0] ?? ''), native) : {revision: mobileClient.revision, loaded: false};
  if (source === 'threadColors') return mobileThreadColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'selectThread') return mobileCommand(['select-thread', String(args[0] ?? ''), '', 0], native, storage!).then(change =>
    ({ ...change, environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, requestRoute: String(args[1] ?? '') }));
  if (source === 'threadView') return threadView(args, native);
  if (source === 'answerFilesPrepare') return mobileThreadAnswerFilesPrepare(str(args[0]), str(args[1]), Number(args[2]), native);
  if (source === 'homeColors') return mobileHomeColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'homePreferences') return mobileShelves(native);
  if (source === 'toggleShelf') return mobileToggleShelf(String(args[0] ?? ''), native);
  if (source === 'homeView') return mobileHomeView(args);
  if (source === 'snapshot') return mobileSnapshot(native, storage!).then(snapshot => connectionView(snapshot, String(args[0] ?? 'light'), String(args[1] ?? 't3-code')));
  if (source === 'cameraPermission') {
    if (!native?.available) return { status: 'unavailable' };
    return bridgeReply(native, { op: 'mobileCameraPermission' }).then(reply =>
      ({ status: reply.ok ? String(obj(reply.value).status ?? 'unavailable') : 'error:Camera access could not be checked.' }));
  }
  if (source === 'alert') {
    if (!native?.available) return { choice: 'cancel' };
    return bridgeReply(native, { op: 'mobileAlert', kind: String(args[0] ?? 'info'),
      title: String(args[1] ?? ''), message: String(args[2] ?? '') }).then(reply =>
      ({ choice: reply.ok ? String(obj(reply.value).choice ?? 'cancel') : 'cancel' }));
  }
  if (source === 'command') {
    const operation = String(args[0] ?? ''), key = String(args[1] ?? '');
    const separator = key.indexOf('\n');
    const origin = separator < 0 ? key : key.slice(0, separator);
    const environmentId = separator < 0 ? '' : key.slice(separator + 1);
    if (operation === 'forget') return mobileCommand(['forget', origin, environmentId, 0], native, storage!);
    if (operation === 'save-environment') return mobileCommand([operation, environmentId,
      JSON.stringify({ label: String(args[2] ?? ''), url: String(args[3] ?? '') }), 0], native, storage!);
    return mobileCommand(args, native, storage!);
  }
  throw new Error(`Unknown mobile source: ${source}`);
}

const taskGuardIndices: Record<string, number> = { command: 4, composerAction: 3, composerSettings: 4,
  attachmentAction: 2, composerAttachments: 1, voiceFocus: 3, voiceAction: 3,
  mediaPreview: 9, attachmentDocument: 8, attachmentDocumentAction: 5, shareMedia: 3, newTaskPrepare: 8 };
function newTaskGuard(source: string, args: unknown[]): (() => boolean) | null {
  // Existing recordings retain their captured draft and cleanup owner offscreen.
  if (source === 'voiceAction' && args[0] !== 'start') return null;
  const index = taskGuardIndices[source];
  if (index === undefined || !args[index]) return null;
  const owner = String(args[index]), route = String(args[index + 1] ?? '');
  return () => mobileNewTaskFlowOwns(owner, route);
}
function unavailableTaskSource(source: string, args: unknown[]) {
  if (source === 'voiceFocus') return { owner: '', label: String(args[1] || 'Draft') };
  if (source === 'composerAttachments') return { items: [], canPick: false, supportsFiles: false, remaining: 0, error: '' };
  if (source === 'newTaskPrepare') return { revision: mobileClient.revision, loaded: false };
  if (source === 'mediaPreview') return { identifier: '', name: '', kind: '', sourceJSON: '', ready: false, error: '' };
  if (source === 'attachmentDocument') return EMPTY_ATTACHMENT_DOCUMENT;
  if (source === 'composerSettings') return { ...mobileComposerSettings('', '', false), open: false, canEdit: false, canSave: false,
    models: [], filters: [], options: [], runtimes: [], error: '' };
  throw new ClientError('Choose a project in the current new task before continuing.', 'superseded');
}

async function newTaskFlow(args: unknown[], native?: Native | null) {
  const session = String(args[0]), visit = String(args[1]), location = String(args[2]), active = args[3] === true;
  // Invalidate departing actions synchronously, before the catalog read or any
  // dependent resource can use the previous flow's composer owner.
  const initial = mobileNewTaskFlowView(session, visit, location, active, false);
  if (!active || !native?.available) return initial;
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
  const [_revision, time, scheme, environmentId, threadId, active] = args;
  const matched = active === true && environmentId === mobileClient.environmentId && threadId === mobileClient.threadId;
  if (matched) await mobileThreadPrepare(native, Number(time));
  const view = mobileThread(Number(time), scheme === 'dark');
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
