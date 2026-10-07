import { mobileComposerAttachmentAction, mobileComposerAttachments, mobileComposerAttachmentPreviews } from './composer-attachments';
// @ref llp/1106.003-pairing-and-transport.decision.md#decision
import { mobileClient, mobileSnapshot, mobileCommand, mobilePairingFields } from './client';
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
import { mobileLayoutFacts, homeChromeEvent, homeChromeView, settingsRoot, settingsScopeEvent } from './root-presentation';
import { connectionView } from './presentation';
import { bridgeReply, type Files, type Native } from './shared/protocol';

export const appId = 'com.exact.t3code.ios';
export const grants = 'device.camera purpose.camera';

export function answer(source: string, args: unknown[], _store?: unknown, storage?: Files, native?: Native | null) {
  if (source === 'attachmentAction') return mobileComposerAttachmentAction(String(args[0] ?? 'menu'), String(args[1] ?? ''), native, storage!);
  if (source === 'composerAttachments') return mobileComposerAttachmentPreviews(native).then(() => mobileComposerAttachments());
  if (source === 'preferences') return mobilePreferencesResource(native);
  if (source === 'preferenceChange') return mobileSavePreference(String(args[0] ?? ''), String(args[1] ?? ''), native);
  if (source === 'applyAppearance') return mobileApplyAppearance(args[0], native);
  if (source === 'appearance') {
    const preferences = normalizeMobilePreferences(args[0]);
    const resolved = resolveMobileAppearance(preferences, String(args[1] ?? 'light'));
    return { scheme: resolved.scheme, themeId: resolved.themeId, baseFontSize: resolved.baseFontSize,
      themeMode: preferences.themeMode, groupingMode: preferences.projectGroupingMode };
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
