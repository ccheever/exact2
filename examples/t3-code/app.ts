import { snapshotSettings } from './src/snapshot/snapshot-settings';
import { keyboardSettings, keybindingSettings } from './src/settings/keybinding-settings';
import { claimBoldChord, richTextComposer } from './src/shell/button-chords'; // ⌘B bolds in the rich-text composer
import { taskFromArguments } from './src/settings/scheduled-settings';
import { scheduledPage } from './src/settings/scheduled-view';
import { sourceControlPage, integrationsPage } from './src/settings/source-control-view';
import { browserProfilesView, emptyBrowserProfilesView } from './src/browser/browser-profiles-settings';
import { viewState } from './src/settings/settings-rest-commands';
import { keyboardDispatchSource } from './src/shell/keyboard-dispatch';
import { projectsView } from './src/settings/projects-view';
import { groupOfMember, PROJECT_SCOPED_PAGES, settingsPageClient } from './src/settings/projects-fleet';
import { diagnosticsPage } from './src/settings/diagnostics-view';
import { settingsNavigation, searchContext } from './src/settings/settings-search';
import { settingsCore } from './src/settings/settings-core-view';
import { archivedSettings, licenseSettings, storageSettings } from './src/settings/settings-data';
import { T3Client } from './src/client/client';
import { composerEditorView, composerWorkspaceView, refreshComposerWorkspace } from './src/composer/composer-editor';
import { chipPopoverView } from './src/composer/composer-chip-popover'; // a skill chip's details popover
import { composerBranches } from './src/composer/composer-controls-branch';
import { providerFieldValues } from './src/providers/providers';
import { providersRoute, scopedAcpRegistry, scopedProviderPage, scopedProviderWizard, scopedSetupOp, scopedUpkeepOp } from './src/providers/providers-scope';
import { connectionsPage } from './src/connections/connections';
import { iconPicker } from './src/settings/settings-b-icons';
import { sshHostsView } from './src/connections/settings-b-ssh';
import { sshPromptSource, sshPromptAnswer } from './src/connections/ssh-auth'; // the SSH password dialog
import { pairingFields } from './src/connections/pairing-fields';
import { pagesHome } from './src/pages/pages-home';
import { usagePage, usageKeys, plotWidth } from './src/usage/pages-usage';
import { pagesSource } from './src/pages/pages-sources';
import { ariaChord } from './src/shell/keyboard-dispatch';
import { snapshot, modelCatalog } from './src/client/presentation';
import { noteComposerTyping } from './src/composer/composer-typing-guard';
import { obj, str, type Obj } from './src/client/domain';
import { nativeFiles, type Native, type Files } from './src/client/protocol';
import { adoptAppFiles } from './src/media/save-file';
import { noteTranscriptFacts } from './src/timeline/transcript-facts';
import { paletteView } from './src/palette/palette-view';
import { attachmentUrls } from './src/timeline/timeline-attachments';
import { noteNow } from './src/composer/composer-controls';
import { prepareTimeline, refreshTimelineReads } from './src/timeline/timeline-prepare';
import { paletteCommand } from './src/palette/palette-commands';
import { shellView } from './src/shell/shell';
import { shellDetails } from './src/details/shell-details';
import { chatCanvasView } from './src/shell/chat-canvas-view';
import { sidebarLaunchWidth } from './src/sidebar/sidebar-width';
import { highlightSlice, startHighlightTurn } from './src/highlight/shiki-highlight';
import { noteServerUpdateClock } from './src/connections/server-update-notices';
import { terminalDrawerView, terminalOpen } from './src/terminal/terminal-drawer-view';
import { terminalFocused } from './src/terminal/terminal-focus';
import { codexContext, codexPrepare, codexTarget } from './src/providers/codex-setup-host'; // ManagedCodexSetup's commands and effects
import { codexSetupOp } from './src/providers/codex-setup-ops';
import { chatGptPlanSnapshot } from './src/providers/chatgpt-plan-view';
import { autoBalancePrepare } from './src/composer/auto-balance'; // Settings › Load balancing, read for the composer
import { letGoAware } from './src/client/let-go';
import { keepAlivePrepare } from './src/client/keep-alive'; // running threads' detail streams
import { migrationToast } from './src/connections/local-lifecycle'; // a let-go answer's native calls reject as 'superseded', never as an error
import { activationPrepare } from './src/shell/desktop-activation';
import { Transport } from './src/transport/transport';
import type { SecretStore } from './src/transport/transport-catalog';

export const appId = 'com.exact.t3code.macos';
// The server connections (src/transport/transport.ts): servers are named at run time (a pairing link, a learned
// route, the embedded server's port), so whole schemes; the tokens and saved environments are one secret.
// Save file and the media menu's Save fetch a server's signed asset and write it under app:/tmp/saves for the window's
// Save panel; a draft's staged copy is read and saved from composer-files (save-file.ts, attachment-files.ts).
export const grants = 'net.websocket ws:\nnet.websocket wss:\nnet.fetch http:\nnet.fetch https:\nsecret.keep t3-catalog\nfs.read app:/tmp/saves\nfs.write app:/tmp/saves\nfs.read app:/data/composer-files';
const client = new T3Client();
const transport = new Transport();

// The transport's own sources answer first and synchronously where they must: a socket's stream answer is the fetch
// itself, and the tick is a mutation that sends, which answers now (LLP 1016.000 D7).
// Its return is the generated `Answer`'s, per source, which a dispatcher over every source cannot name (`answerClient`'s is `any` too).
export function answer(source: string, args: unknown[], store: unknown, storage: Files, native: Native | null | undefined): any {
  transport.store = store as SecretStore;
  if (source === 'transportSocket') return transport.socket(Number(args[0]) || 0, String(args[1] ?? ''));
  if (source === 'transportConnect') return transport.connect(Number(args[0]) || 0, String(args[1] ?? ''), Number(args[2]) || 0, Number(args[3]) || 0, native);
  if (source === 'transportUpdateStep') return transport.updateStep(Number(args[0]) || 0, String(args[1] ?? ''), Number(args[2]) || 0);
  if (source === 'transportUpdateSocket') return transport.updateSocket(Number(args[0]) || 0, String(args[1] ?? ''));
  if (source === 'transportDeliver') return transport.deliver();
  if (source === 'transportPump') return transport.pump(Number(args[0]) || 0, String(args[1] ?? ''));
  return answerClient(source, args, store, storage, transport.wrap(native, source) as Native | null | undefined);
}

// The client's answers, given the bridge it talks to: `answer` puts the transport in front of the app's module; a test
// hands its own scripted server. The disconnected answer is baked. Native work starts only when Exact reports
// that the module is available, after its ordinary first-frame adoption.
export async function answerClient(source: string, args: unknown[], _store: unknown, _storage: Files, native: Native | null | undefined) {
  if (native?.available) native = letGoAware(native);
  adoptAppFiles(_storage); // the app's own files: Save's bytes, a draft's staged copy (save-file.ts)
  const storage = native?.available ? nativeFiles(native) : _storage;
  if (source === 'highlightSlice') return highlightSlice(); // a background highlight turn
  startHighlightTurn(); // any other answer tokenizes code within one turn's budget
  if (source === 'snapshot') {
    noteNow(client, Number(args[0]) || 0);
    transport.observe(Number(args[0]) || 0); // the tick's clock, until its first tick
    noteServerUpdateClock(client, String(args[1] ?? ''), String(args[2] ?? '')); // server-update-notices.ts: the 2 s and 20 s timers
    noteTranscriptFacts(client, args); // the transcript's viewport (transcript-facts.ts)
    noteComposerTyping(client, args[9] === true); // the composer's typing clock (composer-typing-guard.ts)
    await client.refresh(native, storage);
    await prepareTimeline(client, native); // Mermaid layouts and the worktree setup stream (timeline-prepare.ts).
    await autoBalancePrepare(client, native); // auto-balance.ts
    await keepAlivePrepare(client, native); migrationToast(client); // keep-alive.ts, local-lifecycle.ts: running threads stay live; "Restoring your threads…"
    await codexPrepare(client, native); // codex-setup-host.ts: the managed Codex setups' effects and loopback callbacks
    await activationPrepare(client, native); // desktop-activation.ts: `t3 app` waits until the primary is connected and loaded
    return { ...snapshot(client, Number(args[0]) || 0, String(args[3] ?? '')), chatgptPlan: chatGptPlanSnapshot(client) }; // chatgpt-plan-view.ts: "Your ChatGPT plan is connected"
  }
  if (source === 'composerBranches') return composerBranches(client, native, args[0] === true, String(args[1] || ''));
  if (source === 'refreshTimelineReads') {
    noteNow(client, Number(args[0]) || 0);
    await refreshTimelineReads(client, native);
    return { complete: true };
  }
  if (source === 'composerWorkspace') return composerWorkspaceView(client, args[1] !== false); // args[1]: the chat view is on screen (ChatComposer mounted)
  if (source === 'refreshComposerWorkspace') return refreshComposerWorkspace(client, native, String(args[0] || ''));
  if (source === 'composerEditor') return composerEditorView(client, native, Number(args[1]) || 0);
  if (source === 'composerChip') return chipPopoverView(client, native); // composer-chip-popover.ts: args[0] re-asks on each revision
  if (source === 'snapshotSettings') return snapshotSettings(client, native, args[0] === true);
  const scoped = PROJECT_SCOPED_PAGES.has(source) ? settingsPageClient(client, native, args) : { client, args }; // a background environment's project (projects-fleet.ts)
  const pageClient = scoped.client; args = scoped.args;
  if (source === 'archivedSettings') return archivedSettings(pageClient, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, Number(args[3]) || 0, Number(args[4]) || 0, String(args[6] || ''), String(args[7] || ''), String(args[8] || '')); // args[4]: settingsRefresh
  if (source === 'licenseSettings') return licenseSettings(client, native, String(args[0] || ''), String(args[1] || ''), Number(args[2]) || 0, args[3] === true);
  if (source === 'diagnosticsSettings') return diagnosticsPage(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, Number(args[3]) || 0, Number(args[4]) || 0);
  if (source === 'storageSettings') return storageSettings(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, String(args[5] || ''), String(args[6] || ''), String(args[7] || ''), Number(args[8]) || 0);
  if (source === 'pagesHome') return pagesHome(client, native);
  if (source === 'usagePage') return usagePage(client, native, storage, { open: args[0] === true, metric: String(args[1] || ''), windowDays: Number(args[2]) || 0, breakdown: String(args[3] || 'model'), refresh: Number(args[4]) || 0, width: plotWidth(Number(args[5]) || 1280, Number(args[6]) || 0), now: Number(args[7]) || 0, viewport: Number(args[5]) || 1280, sidebar: Number(args[6]) || 0 });
  if (source === 'usageKeys') return usageKeys(client.config, ariaChord);
  if (source === 'timelineAttachments') return attachmentUrls(client, native, Number(args[1]) || 0); // timeline-attachments.ts
  if (source === 'prList' || source === 'prDetail' || source === 'welcome') return pagesSource(client, native, source, args, storage);
  if (source === 'connectionsPage') return connectionsPage(client, native, args[0] === true, Number(args[5]) || 0); // args[5]: the clock, each second while Authorized clients show
  if (source === 'pairingFields') return pairingFields(String(args[0] ?? '')); // pairing-fields.ts
  if (source === 'sshPrompt') return sshPromptSource(native, Number(args[0]) || 0); // ssh-auth.ts: the password dialog's queue
  if (source === 'sshPromptAnswer') return sshPromptAnswer(native, String(args[0] || ''), String(args[1] || ''));
  if (source === 'settingsBSshHosts') return sshHostsView(client, native, args[0] === true, String(args[1] ?? '')); // settings-b-ssh.ts
  if (source === 'settingsBPicker') return iconPicker(client, native, String(args[0] || ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), String(args[4] || ''), String(args[5] || ''), Number(args[6]) || 0); // settings-b-icons.ts
  // Settings › Providers shows the environment its scope chose (providers-scope.ts): the page's reads and its actions' args[4] name it.
  if (source === 'providerPage') return scopedProviderPage(client, native, args); // args[4..7]: the settings scope
  if (source === 'providerWizard') return scopedProviderWizard(client, native, args); // args[8..9]: the dialog and its target (the ChatGPT dialogs); args[10]: the page's environment
  if (source === 'acpRegistry') return scopedAcpRegistry(client, native, args);
  // Updates, Update all, ACP management, the custom model editor (providers-upkeep.ts).
  if (source === 'providerChange' && String(args[0] || '').startsWith('upkeep:')) { if (native?.available) await scopedUpkeepOp(client, native, args); return { revision: ++client.revision, message: '', relaunch: false, routeReorder: 0 }; }
  if (source === 'providerChange' && /^setup:(codex|chatgpt)-/.test(String(args[0] || ''))) { const target = native?.available ? (String(args[0]).startsWith('setup:chatgpt-') ? codexContext(client, native, '') : codexTarget(client, native, String(args[1] || ''))) : null; if (target) await codexSetupOp(target, String(args[0]), String(args[2] ?? ''), String(args[3] ?? ''), () => client.savePreferences(storage)); return { revision: ++client.revision, message: '', relaunch: false, routeReorder: 0 }; }
  if (source === 'providerChange' && String(args[0] || '').startsWith('setup:')) { if (native?.available) await scopedSetupOp(client, native, args); return { revision: ++client.revision, message: '', relaunch: false, routeReorder: 0 }; }
  if (source === 'providerChange') return client.command(String(args[0] || ''), String(args[1] || ''), args[0] === 'favorite-model' ? String(args[2] || '') : JSON.stringify({ key: String(args[2] ?? ''), value: String(args[3] ?? '') }), 0, native, storage, providersRoute(client, String(args[4] ?? '')));
  if (source === 'providerAdd') return client.command('provider-add', String(args[2] || ''), JSON.stringify({ driver: args[0], label: args[1], accentColor: args[3], fields: providerFieldValues(String(args[0] || ''), args.slice(4, 9).map(value => String(value ?? ''))) }), 0, native, storage, providersRoute(client, String(args[9] ?? '')));
  if (source === 'keybindingSettings') return keybindingSettings(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, String(args[3] || ''), String(args[4] || ''), String(args[5] || ''), String(args[6] || ''));
  if (source === 'saveKeybinding') return client.command('keybinding-save', String(args[0]), JSON.stringify({ previous: args[1], command: args[2], key: args[3], when: args[4] }), 0, native, storage);
  if (source === 'scheduledSettings') return scheduledPage(pageClient, native, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), String(args[3] || ''), args[4] === true, Number(args[5]) || 0, String(args[8] || ''), String(args[9] || ''), String(args[10] || ''), String(args[11] || ''), Number(args[6]) || 0); // live-automations: the scope's machine, project and checkout; args[11]: taskBase; args[6]: settingsRefresh
  if (source === 'saveScheduledTask') return client.command('task-save', String(args[0]), JSON.stringify(taskFromArguments(args)), 0, native, storage);
  if (source === 'sourceControlPage') return sourceControlPage(pageClient, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, viewState(client).rescan, String(args[5] || ''), String(args[6] || ''), String(args[7] || ''));
  if (source === 'keyboardDispatch') return keyboardDispatchSource(client, args);
  if (source === 'projectsView') { const legacy = String(args[2] || ''); const group = !args[0] && legacy ? groupOfMember(client, legacy) : undefined; return projectsView(client, group ? group.key : String(args[0] || ''), group ? '' : String(args[1] || ''), args[3] === true, native); } // a bare project id (or a background environment's `fleet:` one) is its project on every environment (settingsScopeOf)
  if (source === 'integrationsPage') return { ...(await integrationsPage(pageClient, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, String(args[5] || ''), String(args[6] || ''), String(args[7] || ''))),
    browserProfiles: args[2] === true ? await browserProfilesView(client, native) : emptyBrowserProfilesView() }; // Browser profiles and the import wizard
  if (source === 'settingsNavigation') return settingsNavigation(String(args[0] || ''), searchContext(client.config, client.ready, String(args[1] || 'all')), Number(args[2]) || 0);
  if (source === 'settingsCore') return settingsCore(client, native, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), String(args[3] || ''), String(args[4] || ''), String(args[5] || ''), args[6] === true, String(args[9] || ''), String(args[10] || ''), String(args[11] || 'embedded'), args[12] === true, String(args[13] || 'light'));
  if (source === 'settings') {
    const environmentId = String(args[0] || '');
    const projectId = String(args[1] || '');
    const settings = obj(client.config.settings);
    const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
    const available = client.ready && (!environmentId || environmentId === client.environmentId) && (!projectId || client.shell.projects.some(project => project.id === projectId));
    const model = obj(override.defaultModelSelection || settings.defaultModelSelection);
    return { defaultModelExplicit: available && typeof settings.defaultModelSelection === "object" && settings.defaultModelSelection !== null, defaultModel: available ? [str(model.instanceId), str(model.model)].filter(Boolean).join(" / ") : "", modelOverridden: available && typeof override.defaultModelSelection === "object" && override.defaultModelSelection !== null, environmentId: client.environmentId, available, permission: available ? str(override.defaultRuntimeMode || settings.defaultRuntimeMode, 'approval-required') : '', overridden: available && typeof override.defaultRuntimeMode === 'string', environmentPermission: available ? str(settings.defaultRuntimeMode, 'approval-required') : '', ...client.local.deviceSettings, ...claimBoldChord(keyboardSettings(client.config, args[3] === true, { terminalFocus: terminalFocused(client), terminalOpen: terminalOpen(client), modalOpen: args[4] === true, editableFocus: args[3] === true || args[4] === true, turnRunning: args[5] === true, draftThreadRoute: args[6] !== true, modelPickerOpen: args[7] === true }), args[3] === true, richTextComposer(client)) };
  }
  if (source === 'modelCatalog') return modelCatalog(client, String(args[0] || ''), String(args[1] || ''), String(args[3] || '')); // args[3]: a Settings row's picker target
  if (source === 'createProvider') return client.command('provider-create', String(args[0] || ''), JSON.stringify({ driver: args[1], name: args[2], binaryPath: args[3], homePath: args[4] }), 0, native, storage);
  if (source === 'paletteView') return paletteView(client, native, args);
  if (source === 'paletteCommand') return paletteCommand(client, native, storage, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''));
  if (source === 'shellDetails') return shellDetails(client, native, args[0] === true, String(args[1] || ''), args[3] === true, Number(args[4]) || 0, Number(args[5]) || 0, args[6] !== false); // args[3]: the card docks inline; args[4]: wall time; args[5]: the window right of the canvas; args[6]: the window has the focus and is visible (exact2 #219)
  if (source === 'sidebarLaunchWidth') return sidebarLaunchWidth(client, Number(args[0]) || 0, !!native?.available); // the width fixed at load
  if (source === 'chatCanvas') return chatCanvasView(client, native, { width: Number(args[1]) || 0, viewportHeight: Number(args[2]) || 0, detailsInline: args[3] === true, chatMax: Number(args[4]) || 0, overlaid: args[5] === true, gesture: String(args[6] || ''), wide: args[9] === true, popoverOpen: args[10] === true }); // args[7..8] re-ask when the player changes
  if (source === 'terminalDrawer') return terminalDrawerView(client, native, Number(args[1]) || 0, Number(args[2]) || 0); // args[0] re-asks on each revision
  if (source === 'shellView') return shellView(client, native, storage, Number(args[1]) || 0, String(args[2] || ''), args[3] === true, args[4] === true, { focused: args[5] !== false, visible: args[6] !== 'hidden' }); // args[5..6]: exactPage().hasFocus and visibilityState (exact2 #219)
  if (source === 'command') return client.command(String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), Number(args[3]) || 0, native, storage);
  throw new Error(`Unknown T3 source: ${source}`);
}
