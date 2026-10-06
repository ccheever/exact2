import { snapshotSettings } from './snapshot-settings';
import { keyboardSettings, keybindingSettings } from './keybinding-settings';
import { claimBoldChord, richTextComposer } from './r8-keys-chords'; // lane r8-keys: ⌘B bolds in the rich-text composer
import { taskFromArguments } from './scheduled-settings';
import { scheduledPage } from './scheduled-view';
import { sourceControlPage, integrationsPage } from './source-control-view';
import { viewState } from './settings-rest-commands';
import { keyboardDispatchSource } from './keyboard-dispatch';
import { projectsView } from './projects-view';
import { diagnosticsPage } from './diagnostics-view';
import { settingsNavigation, searchContext } from './settings-search';
import { settingsCore } from './settings-core-view';
import { archivedSettings, licenseSettings, storageSettings } from './settings-data';
import { T3Client } from './client';
import { composerEditorView, composerWorkspaceView, refreshComposerWorkspace } from './composer-editor';
import { composerBranches } from './composer-controls-branch';
import { providerPage, providerWizard, acpRegistry, providerFieldValues } from './providers';
import { connectionsPage } from './connections';
import { iconPicker } from './settings-b-icons';
import { sshHostsView } from './settings-b-ssh';
import { pairingFields } from './r10-connect-pairing'; // lane r10-connect
import { pagesHome } from './pages-home';
import { usagePage, usageKeys, plotWidth } from './pages-usage';
import { pagesSource } from './pages-sources';
import { ariaChord } from './keyboard-dispatch';
import { snapshot, modelCatalog } from './presentation';
import { obj, str, type Obj } from './domain';
import { nativeFiles, type Native, type Files } from './protocol';
import { paletteView } from './palette-view';
import { attachmentUrls } from './timeline-attachments';
import { noteNow } from './composer-controls';
import { prepareTimeline, refreshTimelineReads } from './timeline-prepare';
import { paletteCommand } from './palette-commands';
import { shellView } from './shell';
import { shellDetails } from './shell-details';
import { sidebarLaunchWidth } from './r4-polish-sidebar-width';

export const appId = 'com.exact.t3code.macos';
export const grants = '';
const client = new T3Client();

// The disconnected answer is baked. Native work starts only when Exact reports
// that the module is available, after its ordinary first-frame adoption.
export async function answer(source: string, args: unknown[], _store: unknown, _storage: Files, native: Native | null | undefined) {
  const storage = native?.available ? nativeFiles(native) : _storage;
  if (source === 'snapshot') {
    noteNow(client, Number(args[0]) || 0);
    await client.refresh(native, storage);
    await prepareTimeline(client, native); // Mermaid layouts and the worktree setup stream (timeline-prepare.ts).
    return snapshot(client, Number(args[0]) || 0);
  }
  if (source === 'composerBranches') return composerBranches(client, native, args[0] === true, String(args[1] || ''));
  if (source === 'refreshTimelineReads') {
    noteNow(client, Number(args[0]) || 0);
    await refreshTimelineReads(client, native);
    return { complete: true };
  }
  if (source === 'composerWorkspace') return composerWorkspaceView(client);
  if (source === 'refreshComposerWorkspace') return refreshComposerWorkspace(client, native, String(args[0] || ''));
  if (source === 'composerEditor') return composerEditorView(client, native, Number(args[1]) || 0);
  if (source === 'snapshotSettings') return snapshotSettings(client, native, args[0] === true);
  if (source === 'archivedSettings') return archivedSettings(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, Number(args[3]) || 0);
  if (source === 'licenseSettings') return licenseSettings(client, native, String(args[0] || ''), String(args[1] || ''), Number(args[2]) || 0, args[3] === true);
  if (source === 'diagnosticsSettings') return diagnosticsPage(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, Number(args[3]) || 0, Number(args[4]) || 0);
  if (source === 'storageSettings') return storageSettings(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true);
  if (source === 'pagesHome') return pagesHome(client, native);
  if (source === 'usagePage') return usagePage(client, native, storage, { open: args[0] === true, metric: String(args[1] || ''), windowDays: Number(args[2]) || 0, breakdown: String(args[3] || 'model'), refresh: Number(args[4]) || 0, width: plotWidth(Number(args[5]) || 1280, Number(args[6]) || 0), now: Number(args[7]) || 0, environmentOff: args[8] === true, viewport: Number(args[5]) || 1280 });
  if (source === 'usageKeys') return usageKeys(client.config, ariaChord);
  if (source === 'timelineAttachments') return attachmentUrls(client, native, Number(args[1]) || 0); // timeline-attachments.ts
  if (source === 'prList' || source === 'prDetail' || source === 'welcome') return pagesSource(client, native, source, args);
  if (source === 'connectionsPage') return connectionsPage(client, native, args[0] === true);
  if (source === 'pairingFields') return pairingFields(String(args[0] ?? '')); // lane r10-connect (r10-connect-pairing.ts)
  if (source === 'settingsBSshHosts') return sshHostsView(client, native, args[0] === true, String(args[1] ?? '')); // settings-b-ssh.ts
  if (source === 'settingsBPicker') return iconPicker(client, native, String(args[0] || ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), String(args[4] || ''), String(args[5] || ''), Number(args[6]) || 0); // settings-b-icons.ts
  if (source === 'providerPage') return providerPage(client, String(args[0] || ''), Number(args[3]) || 0);
  if (source === 'providerWizard') return providerWizard(client, args[0] === true, Number(args[1]) || 0, String(args[2] || 'codex'), args[3] === true, String(args[4] || ''), args[5] === true, String(args[6] || ''));
  if (source === 'acpRegistry') return acpRegistry(client, native, String(args[0] || ''), args[1] === true, Object.values(obj(obj(client.config.settings).providerInstances)).filter(entry => obj(entry).driver === 'acpRegistry').map(entry => str(obj(obj(entry).config).agentId)));
  if (source === 'providerChange') return client.command(String(args[0] || ''), String(args[1] || ''), args[0] === 'favorite-model' ? String(args[2] || '') : JSON.stringify({ key: String(args[2] ?? ''), value: String(args[3] ?? '') }), 0, native, storage);
  if (source === 'providerAdd') return client.command('provider-add', String(args[2] || ''), JSON.stringify({ driver: args[0], label: args[1], accentColor: args[3], fields: providerFieldValues(String(args[0] || ''), args.slice(4, 9).map(value => String(value ?? ''))) }), 0, native, storage);
  if (source === 'keybindingSettings') return keybindingSettings(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, String(args[3] || ''), String(args[4] || ''), String(args[5] || ''), String(args[6] || ''));
  if (source === 'saveKeybinding') return client.command('keybinding-save', String(args[0]), JSON.stringify({ previous: args[1], command: args[2], key: args[3], when: args[4] }), 0, native, storage);
  if (source === 'scheduledSettings') return scheduledPage(client, native, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), String(args[3] || ''), args[4] === true, Number(args[5]) || 0);
  if (source === 'saveScheduledTask') return client.command('task-save', String(args[0]), JSON.stringify(taskFromArguments(args)), 0, native, storage);
  if (source === 'sourceControlPage') return sourceControlPage(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, viewState(client).rescan, viewState(client).reveal);
  if (source === 'keyboardDispatch') return keyboardDispatchSource(client, args);
  if (source === 'projectsView') { const legacy = String(args[2] || ''); const group = !args[0] && legacy ? client.projectGroups().find(candidate => candidate.members.some(member => member.id === legacy)) : undefined; return projectsView(client, group ? group.key : String(args[0] || ''), group ? legacy : String(args[1] || ''), args[3] === true, native); }
  if (source === 'integrationsPage') return integrationsPage(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true);
  if (source === 'settingsNavigation') return settingsNavigation(String(args[0] || ''), searchContext(client.config, client.ready, String(args[1] || 'all')), Number(args[2]) || 0);
  if (source === 'settingsCore') return settingsCore(client, native, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), String(args[3] || ''), String(args[4] || ''), String(args[5] || ''), args[6] === true, String(args[9] || ''), String(args[10] || ''), String(args[11] || 'embedded'), args[12] === true);
  if (source === 'settings') {
    const environmentId = String(args[0] || '');
    const projectId = String(args[1] || '');
    const settings = obj(client.config.settings);
    const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
    const available = client.ready && (!environmentId || environmentId === client.environmentId) && (!projectId || client.shell.projects.some(project => project.id === projectId));
    const model = obj(override.defaultModelSelection || settings.defaultModelSelection);
    return { defaultModelExplicit: available && typeof settings.defaultModelSelection === "object" && settings.defaultModelSelection !== null, defaultModel: available ? [str(model.instanceId), str(model.model)].filter(Boolean).join(" / ") : "", modelOverridden: available && typeof override.defaultModelSelection === "object" && override.defaultModelSelection !== null, environmentId: client.environmentId, available, permission: available ? str(override.defaultRuntimeMode || settings.defaultRuntimeMode, 'approval-required') : '', overridden: available && typeof override.defaultRuntimeMode === 'string', environmentPermission: available ? str(settings.defaultRuntimeMode, 'approval-required') : '', ...client.local.deviceSettings, ...claimBoldChord(keyboardSettings(client.config, args[3] === true, { modalOpen: args[4] === true, editableFocus: args[3] === true || args[4] === true, turnRunning: args[5] === true, draftThreadRoute: args[6] !== true, modelPickerOpen: args[7] === true }), args[3] === true, richTextComposer(client)) };
  }
  if (source === 'modelCatalog') return modelCatalog(client, String(args[0] || ''), String(args[1] || ''));
  if (source === 'createProvider') return client.command('provider-create', String(args[0] || ''), JSON.stringify({ driver: args[1], name: args[2], binaryPath: args[3], homePath: args[4] }), 0, native, storage);
  if (source === 'paletteView') return paletteView(client, native, args);
  if (source === 'paletteCommand') return paletteCommand(client, native, storage, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''));
  if (source === 'shellDetails') return shellDetails(client, native, args[0] === true, String(args[1] || ''), args[3] === true, Number(args[4]) || 0, Number(args[5]) || 0); // args[3]: the card docks inline; args[4]: wall time; args[5]: the window right of the canvas (lane r6-pr)
  if (source === 'sidebarLaunchWidth') return sidebarLaunchWidth(client, Number(args[0]) || 0, !!native?.available); // r4-polish: the width fixed at load
  if (source === 'shellView') return shellView(client, native, storage, Number(args[1]) || 0, String(args[2] || ''), args[3] === true, args[4] === true);
  if (source === 'command') return client.command(String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), Number(args[3]) || 0, native, storage);
  throw new Error(`Unknown T3 source: ${source}`);
}
