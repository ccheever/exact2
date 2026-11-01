import { threadFindTick, threadFindView } from './src/timeline/thread-find-runtime';
import type { StreamPicked } from './src/browser/server-browser-files';
import { serverStreamSource, serverStreamPainted, serverStreamInput, serverStreamUpload, serverStreamFrame, serverStreamDecode } from './src/browser/server-browser-runtime';
import { snapshotSettings } from './src/snapshot/snapshot-settings';
import { promptLinkView, promptLinksReady } from './src/prompt-links/prompt-link-handler';
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
import { composerRichSource } from './src/composer/composer-rich-source';
import { uploadSource } from './src/composer/composer-uploads'; // a draft image's upload, a stream that tells its bytes sent
import { composerFieldOwner } from './src/composer/auto-balance-owner';
import { composerFieldAcknowledged, composerFieldNative, composerFieldObserved, composerFieldResource } from './src/composer/composer-field-bridge';
import { composerIntentAnswer } from './src/composer/composer-intent-source';
import { chipPopoverView } from './src/composer/composer-chip-popover'; // a skill chip's details popover
import { composerChipObserved, composerChipPressed } from './src/composer/composer-chip-events';
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
import { noteInteraction } from './src/pull-requests/pages-pr-refresh';
import { ariaChord } from './src/shell/keyboard-dispatch';
import { snapshot, modelCatalog } from './src/client/presentation';
import { chatLinkMenuView } from './src/timeline/external-link-menu';
import { noteComposerTyping } from './src/composer/composer-typing-guard';
import { arr, obj, str, type Obj } from './src/client/domain';
import { preferenceFiles, type Native, type Files } from './src/client/protocol';
import { adoptAppFiles } from './src/media/save-file';
import { noteTranscriptFacts } from './src/timeline/transcript-facts';
import { paletteView } from './src/palette/palette-view';
import { attachmentUrls } from './src/timeline/timeline-attachments';
import { inlineDocumentsPosted } from './src/timeline/inline-documents';
import { noteNow } from './src/composer/composer-controls';
import { prepareTimeline, refreshTimelineReads } from './src/timeline/timeline-prepare';
import { paletteCommand } from './src/palette/palette-commands';
import { shellView } from './src/shell/shell';
import { shellDetails } from './src/details/shell-details';
import { chatCanvasView } from './src/shell/chat-canvas-view';
import { navigationHistorySource } from './src/sidebar/sidebar-history';
import { sidebarLaunchWidth } from './src/sidebar/sidebar-width';
import { highlightAsk, highlightStore, highlightView } from './src/highlight/highlights';
import { noteServerUpdateClock } from './src/connections/server-update-notices';
import { terminalDrawerView, terminalOpen } from './src/terminal/terminal-drawer-view';
import { terminalFocused } from './src/terminal/terminal-focus';
import { codexContext, codexPrepare, codexReceive, codexReceivingSlots, codexTarget } from './src/providers/codex-setup-host'; // ManagedCodexSetup's commands, effects and loopback callback
import { codexSetupOp } from './src/providers/codex-setup-ops';
import { chatGptPlanSnapshot } from './src/providers/chatgpt-plan-view';
import { autoBalancePrepare } from './src/composer/auto-balance'; // Settings › Load balancing, read for the composer
import { letGoAware } from './src/client/let-go';
import { keepAlivePrepare } from './src/client/keep-alive'; // running threads' detail streams
import { migrationToast } from './src/connections/local-lifecycle'; // a let-go answer's native calls reject as 'superseded', never as an error
import { activationPrepare } from './src/shell/desktop-activation';
import { Transport } from './src/transport/transport';
import { noteLayoutFrames } from './src/shell/layout-frames';
import type { SecretStore } from './src/transport/transport-catalog';
import { externalOpenRequest } from './src/client/external-open';

export const appId = 'com.exact.t3code.macos';
// The server connections (src/transport/transport.ts): servers are named at run time (a pairing link, a learned
// route, the embedded server's port), so whole schemes; the tokens and saved environments are one secret. macOS asks for
// Local Network access before the first reach to a server on the local network; the prompt is strings/en.json's text.
// The camera and microphone grants are for the commands terminals and agents run, which inherit the app's TCC identity:
// under the hardened runtime they are refused without a prompt unless the app claims both (build-desktop-artifact.ts).
// Save file and the media menu's Save fetch a server's signed asset and write it under app:/tmp/saves for the window's
// Save panel; a draft's staged copy is read and saved from composer-files (save-file.ts, attachment-files.ts). The preference
// file is t3-code.json (protocol.ts preferenceFiles). `browser.automation` lets data code drive a Browser tab's frame for
// the agent's preview automation and Annotate (browser-automation.ts, browser-annotation.ts); a capture of a tab is written
// under app:/tmp/browser-shots and an agent-driven tab's downloads under app:/tmp/browser-downloads before they move to the
// artifact folder. Copy image writes the media's bytes and their PNG under app:/tmp/copy-image (media-copy-image.ts).
// Thread and review guide notifications post, badge and ask through @exact/notifications (shell-notify.ts); the ChatGPT
// sign-in opens on auth.openai.com and comes back to a loopback port T3's server chose (codex-setup-ops.ts receiveCallback).
export const grants = 'net.websocket ws:\nnet.websocket wss:\nnet.fetch http:\nnet.fetch https:\nsecret.keep t3-catalog\nfs.read app:/data/t3-code.json\nfs.write app:/data/t3-code.json\nfs.read app:/tmp/saves\nfs.write app:/tmp/saves\nfs.read app:/data/composer-files\nfs.write app:/data/composer-files\nfs.read app:/data/snapshots/drafts\nfs.write app:/data/snapshots/drafts\nfs.read doc:/\nfs.read app:/tmp/picked\nfs.read app:/tmp/composer-stage\nfs.write app:/tmp/composer-stage\nfs.read app:/tmp/browser-frames\nfs.write app:/tmp/browser-frames\nfs.read app:/tmp/browser-shots\nfs.write app:/tmp/browser-shots\nfs.read app:/tmp/browser-downloads\nfs.write app:/tmp/browser-downloads\nbrowser.automation\nfs.read app:/cache/media-preview\nfs.write app:/cache/media-preview\nfs.read app:/tmp/copy-image\nfs.write app:/tmp/copy-image\ndevice.local-network purpose.local-network\ndevice.camera purpose.camera\ndevice.microphone purpose.microphone\ndevice.notifications purpose.notifications\nauth.session https://auth.openai.com\nauth.callback http://127.0.0.1/auth/callback';
const client = new T3Client();
const transport = new Transport();

// The transport's own sources answer first and synchronously where they must: a socket's stream answer is the fetch
// itself, and the tick is a mutation that sends, which answers now (LLP 1016.000 D7).
// Its return is the generated `Answer`'s, per source, which a dispatcher over every source cannot name (`answerClient`'s is `any` too).
export function answer(source: string, args: unknown[], store: unknown, storage: Files, native: Native | null | undefined): any {
  transport.store = store as SecretStore;
  // The window's last interaction, each new second (app.contract `interacted`): the activity reporter's and useLiveRefresh's.
  if (source === 'activityInteraction' || source === 'activityResumed') {
    const at = Number(args[0]) || 0;
    transport.activity.interaction(at); noteInteraction(client, at, source === 'activityResumed');
    return at;
  }
  if (source === 'serverBrowserUpload') { adoptAppFiles(storage); return native?.available ? serverStreamUpload(client, transport.wrap(native, source) as Native, String(args[0] ?? ''), String(args[1] ?? ''), args[2] as StreamPicked[]) : null; }
  if (source === 'serverBrowserDecode') { adoptAppFiles(storage); return serverStreamDecode(client, String(args[0] ?? '')); }
  if (source === 'serverBrowserStatus') { noteNow(client, Number(args[1]) || 0); return serverStreamFrame(client, String(args[0] ?? '')); }
  if (source === 'serverBrowserFrame') return serverStreamSource(client, String(args[0] ?? ''), String(args[1] ?? ''));
  if (source === 'serverBrowserPainted') return serverStreamPainted(client, String(args[0] ?? ''), String(args[1] ?? ''));
  if (source.startsWith('serverBrowser')) {
    noteNow(client, Number(args.at(-1)) || 0);
    const kind = source.slice(13).toLowerCase(), event = typeof args[1] === 'object' && args[1] !== null ? args[1] as Record<string, unknown> : {};
    const value = kind === 'resize' ? { width: args[1], height: args[2] } : kind === 'text' ? { text: args[1] }
      : kind === 'dialog' ? { accept: args[1], promptText: args[2] }
      : { ...event, x: event.offsetX, y: event.offsetY, phase: args[2], width: args[3], height: args[4] };
    return serverStreamInput(client, String(args[0] ?? ''), kind === 'control' ? String(args[1] ?? '') : kind, value);
  }
  if (source === 'composerUpload') return uploadSource(client, Number(args[0]) || 0, String(args[1] ?? ''), String(args[2] ?? ''));
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
  if (native?.available) native = composerFieldNative(client, letGoAware(native), () => composerFieldOwner(client));
  adoptAppFiles(_storage); // the app's own files: Save's bytes, a draft's staged copy (save-file.ts)
  const storage = preferenceFiles(_storage);
  if (source === 'highlightAsk') return highlightAsk(); // the texts waiting for their tokens (app.contract highlightStep)
  if (source === 'highlightStore') return highlightStore(args[0] as { id: number; line: number; records: number[] }[]); // the lexers' answer
  if (source === 'promptLinkView') return promptLinkView(client);
  if (source === 'codexReceive') return codexReceive(client, native, String(args[0] ?? '')); // the sign-in's loopback callback (codex-setup-ops.ts)
  if (source === 'promptLinksReady') return promptLinksReady(client);
  if (source === 'snapshot') {
    noteNow(client, Number(args[0]) || 0);
    transport.observe(Number(args[0]) || 0); // the tick's clock, until its first tick
    noteServerUpdateClock(client, String(args[1] ?? ''), String(args[2] ?? '')); // server-update-notices.ts: the 2 s and 20 s timers
    noteTranscriptFacts(client, args); // the transcript's viewport (transcript-facts.ts)
    noteComposerTyping(client, args[8] === true); // the composer's typing clock (composer-typing-guard.ts)
    noteLayoutFrames(client, args[9]); // the boxes app.contract measured (layout-frames.ts)
    await client.refresh(native, storage);
    await prepareTimeline(client, native); // Mermaid layouts and the worktree setup stream (timeline-prepare.ts).
    await autoBalancePrepare(client, native); // auto-balance.ts
    await keepAlivePrepare(client, native); migrationToast(client); // keep-alive.ts, local-lifecycle.ts: running threads stay live; "Restoring your threads…"
    await codexPrepare(client, native); // codex-setup-host.ts: the managed Codex setups' effects and loopback callbacks
    await activationPrepare(client, native); // desktop-activation.ts: `t3 app` waits until the primary is connected and loaded
    // The views that paint code answer through highlightView (highlights.ts): each reports whether a text it painted
    // waits for its tokens, so the root's highlight pump refreshes it when they land (app.contract highlightStoredThen).
    return highlightView('data', () => ({ ...snapshot(client, Number(args[0]) || 0, String(args[3] ?? ''), String(args[10] ?? '')), chatgptPlan: chatGptPlanSnapshot(client), // chatgpt-plan-view.ts: "Your ChatGPT plan is connected"
      linkMenu: chatLinkMenuView(client, native, storage), // a Markdown web link's menu rows (external-link-menu.ts)
      externalOpen: externalOpenRequest(), // the latest link to open outside the app (external-open.ts)
      ...codexReceivingSlots(client, native) })); // the ChatGPT sign-ins waiting on their loopback callbacks (app.contract codexLoopback)
  }
  if (source === 'threadFind') return native?.available ? threadFindTick(client, native, Number(args[1]) || 0, arr(args[2]).map(skill => ({ name: str(skill.name), displayName: str(skill.displayName) })), String(args[3] || '')) : threadFindView(client);
  if (source === 'composerBranches') return composerBranches(client, native, args[0] === true, String(args[1] || ''));
  if (source === 'refreshTimelineReads') {
    noteNow(client, Number(args[0]) || 0);
    await refreshTimelineReads(client, native);
    return { complete: true };
  }
  if (source === 'composerWorkspace') return composerWorkspaceView(client, args[1] !== false, typeof args[2] === 'number' ? args[2] : undefined); // args[1]: the chat view is on screen (ChatComposer mounted)
  if (source === 'refreshComposerWorkspace') return refreshComposerWorkspace(client, native, String(args[0] || ''));
  if (source === 'composerEditor') { noteLayoutFrames(client, args[2]); return composerEditorView(client, native, Number(args[1]) || 0); }
  if (source === 'composerRich') return composerRichSource(client, { snapshotOwner: String(args[0] ?? ''), owner: String(args[1] ?? ''), value: String(args[2] ?? ''), inputType: String(args[4] ?? ''), surface: args[5] === 'preview' ? 'preview' : 'composer' });
  if (source === 'composerField') return composerFieldResource(client, args, composerFieldOwner(client), native);
  if (source === 'composerFieldObserved') return composerFieldObserved(client, args, composerFieldOwner(client));
  if (source === 'composerFieldAcknowledged') return composerFieldAcknowledged(client, args, composerFieldOwner(client), native);
  if (source === 'composerIntent') return composerIntentAnswer(client, args);
  if (source === 'composerChip') return chipPopoverView(client);
  if (source === 'composerChipObserved') return composerChipObserved(client, args[0]);
  if (source === 'composerChipPressed') return composerChipPressed(client, args[0]);
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
  if (source === 'timelineAttachments') return { ...await attachmentUrls(client, native, Number(args[1]) || 0, String(args[3] || '')), posted: inlineDocumentsPosted(client) }; // timeline-attachments.ts
  if (source === 'prDetail') return highlightView('prDetail', () => pagesSource(client, native, source, args, storage) as Promise<object>); // its Code tab's rows
  if (source === 'prList' || source === 'welcome') return pagesSource(client, native, source, args, storage);
  if (source === 'connectionsPage') return connectionsPage(client, native, args[0] === true, Number(args[5]) || 0); // args[5]: the clock, each second while Authorized clients show
  if (source === 'pairingFields') return pairingFields(String(args[0] ?? '')); // pairing-fields.ts
  if (source === 'sshPrompt') return sshPromptSource(native, Number(args[0]) || 0); // ssh-auth.ts: the password dialog's queue
  if (source === 'sshPromptAnswer') return sshPromptAnswer(native, String(args[0] || ''), String(args[1] || ''));
  if (source === 'settingsBSshHosts') return sshHostsView(client, native, args[0] === true, String(args[1] ?? '')); // settings-b-ssh.ts
  if (source === 'settingsBPicker') return iconPicker(client, native, String(args[0] || ''), String(args[1] ?? ''), String(args[2] ?? ''), String(args[3] ?? ''), String(args[4] || ''), String(args[5] || ''), Number(args[6]) || 0, String(args[7] ?? '')); // settings-b-icons.ts
  // Settings › Providers shows the environment its scope chose (providers-scope.ts): the page's reads and its actions' args[4] name it.
  if (source === 'providerPage') return scopedProviderPage(client, native, args); // args[4..7]: the settings scope
  if (source === 'providerWizard') return scopedProviderWizard(client, native, args); // args[8..9]: the dialog and its target (the ChatGPT dialogs); args[10]: the page's environment; args[11]: the agent epoch
  if (source === 'acpRegistry') return scopedAcpRegistry(client, native, args);
  // Updates, Update all, ACP management, the custom model editor (providers-upkeep.ts).
  if (source === 'providerChange' && String(args[0] || '').startsWith('upkeep:')) { if (native?.available) await scopedUpkeepOp(client, native, args); return { revision: ++client.revision, message: '', relaunch: false, routeReorder: 0 }; }
  if (source === 'providerChange' && /^setup:(codex|chatgpt)-/.test(String(args[0] || ''))) { const target = native?.available ? (String(args[0]).startsWith('setup:chatgpt-') ? codexContext(client, native, '') : codexTarget(client, native, String(args[1] || ''))) : null; if (target) await codexSetupOp(target, String(args[0]), String(args[2] ?? ''), String(args[3] ?? ''), () => client.savePreferences(storage)); return { revision: ++client.revision, message: '', relaunch: false, routeReorder: 0 }; }
  if (source === 'providerChange' && String(args[0] || '').startsWith('setup:')) { if (native?.available) await scopedSetupOp(client, native, args); return { revision: ++client.revision, message: '', relaunch: false, routeReorder: 0 }; }
  if (source === 'providerChange') return client.command(String(args[0] || ''), String(args[1] || ''), args[0] === 'favorite-model' ? String(args[2] || '') : JSON.stringify({ key: String(args[2] ?? ''), value: String(args[3] ?? ''), projectIds: args[5] ? JSON.parse(String(args[5])) : [] }), 0, native, storage, providersRoute(client, String(args[4] ?? '')));
  if (source === 'providerAdd') return client.command('provider-add', String(args[2] || ''), JSON.stringify({ driver: args[0], label: args[1], accentColor: args[3], fields: providerFieldValues(String(args[0] || ''), (Array.isArray(args[4]) ? args[4] : []).map(value => String(value ?? ''))), commandArgs: args[6], environment: args[7] }), 0, native, storage, providersRoute(client, String(args[5] ?? '')));
  if (source === 'keybindingSettings') return keybindingSettings(client, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, String(args[3] || ''), String(args[4] || ''), String(args[5] || ''), String(args[6] || ''),
    Array.isArray(args[9]) ? args[9].map(item => obj(item)).map(item => ({ row: str(item.row), command: str(item.command), key: str(item.key), when: str(item.when) })) : []);
  if (source === 'saveKeybinding') return client.command('keybinding-save', String(args[0]), JSON.stringify({ previous: args[1], command: args[2], key: args[3], when: args[4] }), 0, native, storage);
  if (source === 'scheduledSettings') return scheduledPage(pageClient, native, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), String(args[3] || ''), args[4] === true, Number(args[5]) || 0, String(args[8] || ''), String(args[9] || ''), String(args[10] || ''), String(args[11] || ''), Number(args[6]) || 0); // live-automations: the scope's machine, project and checkout; args[11]: taskBase; args[6]: settingsRefresh
  if (source === 'saveScheduledTask') return client.command('task-save', String(args[0]), JSON.stringify(taskFromArguments(args)), 0, native, storage);
  if (source === 'sourceControlPage') return sourceControlPage(pageClient, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, viewState(client).rescan, String(args[5] || ''), String(args[6] || ''), String(args[7] || ''));
  if (source === 'keyboardDispatch') return keyboardDispatchSource(client, args);
  if (source === 'projectsView') { const legacy = String(args[2] || ''); const group = !args[0] && legacy ? groupOfMember(client, legacy) : undefined; return projectsView(client, group ? group.key : String(args[0] || ''), group ? '' : String(args[1] || ''), args[3] === true, native); } // a bare project id (or a background environment's `fleet:` one) is its project on every environment (settingsScopeOf)
  if (source === 'integrationsPage') return { ...(await integrationsPage(pageClient, native, String(args[0] || ''), String(args[1] || ''), args[2] === true, String(args[5] || ''), String(args[6] || ''), String(args[7] || ''))),
    browserProfiles: args[2] === true ? await browserProfilesView(client, native) : emptyBrowserProfilesView() }; // Browser profiles and the browser defaults
  if (source === 'settingsNavigation') return settingsNavigation(String(args[0] || ''), searchContext(client.config, client.ready, String(args[1] || 'all')), Number(args[2]) || 0);
  if (source === 'settingsCore') {
    const view = () => settingsCore(client, native, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), String(args[3] || ''), String(args[4] || ''), String(args[5] || ''), args[6] === true, String(args[9] || ''), String(args[10] || ''), String(args[11] || 'embedded'), args[12] === true, String(args[13] || 'light'));
    return highlightView('settingsCore', view); // its font preview (settings-font-previews.ts)
  }
  if (source === 'settings') {
    const environmentId = String(args[0] || '');
    const projectId = String(args[1] || '');
    const settings = obj(client.config.settings);
    const override = obj(obj(settings.projectSettingsOverrides)[projectId]);
    const available = client.ready && (!environmentId || environmentId === client.environmentId) && (!projectId || client.shell.projects.some(project => project.id === projectId));
    const model = obj(override.defaultModelSelection || settings.defaultModelSelection);
    return { defaultModelExplicit: available && typeof settings.defaultModelSelection === "object" && settings.defaultModelSelection !== null, defaultModel: available ? [str(model.instanceId), str(model.model)].filter(Boolean).join(" / ") : "", modelOverridden: available && typeof override.defaultModelSelection === "object" && override.defaultModelSelection !== null, environmentId: client.environmentId, available, permission: available ? str(override.defaultRuntimeMode || settings.defaultRuntimeMode, 'approval-required') : '', overridden: available && typeof override.defaultRuntimeMode === 'string', environmentPermission: available ? str(settings.defaultRuntimeMode, 'approval-required') : '', ...client.local.deviceSettings, ...claimBoldChord(keyboardSettings(client.config, args[3] === true, { terminalFocus: terminalFocused(client), terminalOpen: terminalOpen(client), modalOpen: args[4] === true, editableFocus: args[3] === true || args[4] === true, turnRunning: args[5] === true, draftThreadRoute: args[6] !== true, modelPickerOpen: args[7] === true }), args[3] === true, richTextComposer(client), args[8] === true) };
  }
  if (source === 'modelCatalog') return modelCatalog(client, String(args[0] || ''), String(args[1] || ''), String(args[3] || '')); // args[3]: a Settings row's picker target
  if (source === 'createProvider') return client.command('provider-create', String(args[0] || ''), JSON.stringify({ driver: args[1], name: args[2], binaryPath: args[3], homePath: args[4] }), 0, native, storage);
  if (source === 'paletteView') return highlightView('palette', () => paletteView(client, native, args)); // its content search lines
  if (source === 'paletteCommand') return paletteCommand(client, native, storage, String(args[0] || ''), String(args[1] || ''), String(args[2] || ''));
  if (source === 'shellDetails') return shellDetails(client, native, args[0] === true, String(args[1] || ''), args[3] === true, Number(args[4]) || 0, Number(args[5]) || 0, args[6] !== false, Array.isArray(args[7]) ? args[7].map(String) : [], Array.isArray(args[8]) ? args[8].map(String) : [], Array.isArray(args[9]) ? args[9].map(String) : [], args[10] !== false); // args[3]: the card docks inline; args[4]: wall time; args[5]: the window right of the canvas; args[6]: the window has the focus and is visible (exact2 #219); args[10]: the window is visible
  if (source === 'navigationHistory') return navigationHistorySource(client, args); // the window's Back and Forward (sidebar-history.ts)
  if (source === 'sidebarLaunchWidth') return sidebarLaunchWidth(client, Number(args[0]) || 0, !!native?.available); // the width fixed at load
  if (source === 'chatCanvas') noteLayoutFrames(client, args[11]);
  if (source === 'chatCanvas') return chatCanvasView(client, native, { width: Number(args[1]) || 0, viewportHeight: Number(args[2]) || 0, detailsInline: args[3] === true, chatMax: Number(args[4]) || 0, overlaid: args[5] === true, gesture: String(args[6] || ''), wide: args[9] === true, popoverOpen: args[10] === true }); // args[7..8] re-ask when the player changes
  if (source === 'terminalDrawer') return terminalDrawerView(client, native, Number(args[1]) || 0, Number(args[2]) || 0); // args[0] re-asks on each revision
  if (source === 'shellView') { noteNow(client, Number(args[1]) || 0); return highlightView('shell', () => shellView(client, native, storage, Number(args[1]) || 0, String(args[2] || ''), args[3] === true, args[4] === true, { focused: args[5] !== false, visible: args[6] !== 'hidden', notifications: String(args[8] ?? '') }, args[7] !== false)); } // args[5..6]: exactPage().hasFocus and visibilityState (exact2 #219); args[7]: the chat view is shown; args[8]: the notification permission
  // A drop, a paste, Attach files or a sidebar row's drop (app.contract attachFiles): composer-controls-attach.ts.
  if (source === 'composerAttach') return client.command('cclocal:attach-files', String(args[0] || ''), JSON.stringify({ files: (Array.isArray(args[1]) ? args[1] : []).map(String), picked: Array.isArray(args[2]) ? args[2] : [], asText: args[3] === true, target: String(args[4] ?? '') }), 0, native, storage);
  if (source === 'command') return client.command(String(args[0] || ''), String(args[1] || ''), String(args[2] || ''), Number(args[3]) || 0, native, storage);
  throw new Error(`Unknown T3 source: ${source}`);
}
