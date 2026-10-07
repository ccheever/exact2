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
import { obj } from './shared/domain';
import { mobileShelves, mobileToggleShelf, mobileHomeView } from './home-state';
import { mobileTheme, mobileHomeColors, mobileThreadColors, mobileComposerColors, mobileArchiveColors, mobileAgentColors } from './design';
import { mobilePreferencesResource, mobileSavePreference, mobileApplyAppearance, normalizeMobilePreferences, resolveMobileAppearance } from './settings-preferences';
import { settingsAppearanceView, settingsChoices } from './settings-appearance';
import { mobileArchive, mobileArchiveCommand } from './archive';
import { mobileAgentActivity } from './agent-activity';
import { mobileComposerSettings, mobileComposerSettingsAction } from './composer-settings';
import { mobileNewTask, mobileNewTaskPrepare, mobileNewTaskAction } from './new-task';
import { mobileThread, mobileThreadPrepare } from './thread';
import { mobileLayoutFacts, mobileScheduledHeader, homeChromeEvent, homeChromeView, settingsRoot, settingsScopeEvent } from './root-presentation';
import { connectionView } from './presentation';
import { bridgeReply, nativeFiles, type Files, type Native } from './shared/protocol';

export const appId = 'com.exact.t3code.ios';
export const grants = 'device.camera purpose.camera device.microphone purpose.microphone';

export function answer(source: string, args: unknown[], _store?: unknown, storage?: Files, native?: Native | null) {
  if (native) native = settingsProviderNative(native);
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
  if (source === 'reviewAction') return mobileReviewAction(String(args[0]), String(args[1]), String(args[2]), String(args[3]), Number(args[4]), native, storage!, args[5] === 'dark');
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
  if (source === 'composerAction') return mobileComposerSettingsAction(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage!);
  if (source === 'newTaskView') return mobileNewTask(String(args[0] ?? ''));
  if (source === 'newTaskPrepare') return args[1] === true ? mobileNewTaskPrepare(String(args[0] ?? ''), native) : {revision: mobileClient.revision, loaded: false};
  if (source === 'newTaskAction') return mobileNewTaskAction(String(args[0] ?? ''), String(args[1] ?? ''), String(args[2] ?? ''), native, storage!);
  if (source === 'threadColors') return mobileThreadColors(String(args[0] ?? 'light'), String(args[1] ?? 't3-code'));
  if (source === 'selectThread') return mobileCommand(['select-thread', String(args[0] ?? ''), '', 0], native, storage!).then(change =>
    ({ ...change, environmentId: mobileClient.environmentId, threadId: mobileClient.threadId, requestRoute: String(args[1] ?? '') }));
  if (source === 'threadView') return threadView(args, native);
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
