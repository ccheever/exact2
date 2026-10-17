import { legacySidebarSnapshot } from '../sidebar/legacy-sidebar-view';
import { timelineReadsNeeded } from '../timeline/timeline-prepare';
import { projectCloneBlock } from '../shell/project-clones-live';
import { highlightPending } from '../highlight/shiki-highlight';
import { markdownSkills } from '../timeline/message-chips';
import { workspaceValues } from '../composer/composer-workspace-snapshots';
import { workspaceCwd } from '../composer/composer-editor';
import { snapshotDraftTiles } from '../snapshot/snapshot-settings';
import { arr, obj, str, num, type Message, type Obj } from './domain';
import { activeRun, providerAvailable } from './protocol';
import type { T3Client } from './client';
import { timelineMessages, timelineSnapshot, transcriptRows } from '../timeline/timeline-presentation';
import { sidebarSnapshot } from '../sidebar/sidebar-view';
import { subagentLead } from '../sidebar/sidebar-lineage';
import { requestPresentation } from '../composer/requests';
import { threadErrorView } from '../timeline/timeline-errors';
import { diffNotGit, diffSnapshot, NOT_GIT_REPO } from '../diff/diff';
import { composerSnapshot } from '../composer/composer-presentation';
import { composerSelection } from '../composer/composer-provider-selection';
import { fanoutSelections } from '../composer/composer-controls-fanout';
import { triggerModelName } from '../composer/composer-controls-model';
import { pickerCatalog, withJumpLabels } from '../composer/model-catalog';
import { effectiveShortcut } from '../shell/shortcut-labels'; // the rows' ⌘1–⌘9
import { ariaChord } from '../shell/keyboard-dispatch';
import { shortcutInput } from '../settings/keybinding-settings';
import { shortcutLabel } from '../settings/keybinding-view';
import { terminalOpen } from '../terminal/terminal-drawer-view';
import { settingsPickerCatalog } from '../providers/settings-model-picker'; // General's model rows
import { look } from '../appearance/settings-appearance-look';
import { composerOverlaySnapshot } from '../composer/composer-overlay';
import { composerVideoSnapshot } from '../composer/composer-attachments';
import { composerImagePreview } from '../composer/composer-chip-details';
import { alertClip } from '../shared/laid-out-widths';
import { tableMenuSnapshot } from '../timeline/table-menu';
import { sidebarMinimumWidth, workspaceControlsLeft } from '../sidebar/sidebar-min-width';
import { adoptHostLocale } from '../shared/timestamp-format'; // the Mac's locale, from the status presentation (T3Locale.swift)
import { serverUpdateView } from '../connections/server-update-notices';
import { composerOwner } from '../composer/auto-balance-owner';
import { providerBanner } from '../providers/provider-status-message'; // the banner and its setup button
import { feedbackUploading, withFeedbackRows } from '../composer/composer-feedback'; // Codex /feedback

const modes: Record<string, string> = {
  'approval-required': 'Ask for approval', 'auto-accept-edits': 'Auto-accept edits',
  auto: 'Auto', 'full-access': 'Full access',
};
function section(thread: Obj): string {
  if (thread.pinnedAt) return 'pinned';
  if (thread.snoozedUntil) return 'snoozed';
  if (thread.settledOverride === 'settled' || thread.settledAt && thread.settledOverride !== 'active') return 'settled';
  if (thread.activeRunId || ['preparing', 'starting', 'running', 'waiting'].includes(str(thread.status))) return 'working';
  return 'active';
}

// Live built reference identity (Tailwind v4 600/400 tints); its palette adds indigo to the older source.
const projectPalette = [
  ['#4a5565', '#99a1af'], ['#e7000b', '#ff6467'], ['#f54900', '#ff8904'],
  ['#e17100', '#ffb900'], ['#d08700', '#fdc700'], ['#5ea500', '#9ae600'],
  ['#00a63e', '#05df72'], ['#009966', '#00d492'], ['#009689', '#00d5be'],
  ['#0092b8', '#00d3f2'], ['#0084d1', '#00bcff'], ['#155dfc', '#51a2ff'],
  ['#4f39f6', '#7c86ff'], ['#7f22fe', '#a684ff'], ['#9810fa', '#c27aff'],
  ['#c800de', '#ed6aff'], ['#e60076', '#fb64b6'], ['#ec003f', '#ff637e'],
];
export function projectIdentity(name: string) {
  const normalized = name.normalize('NFKC').trim();
  const words = normalized.match(/[\p{L}\p{N}]+/gu) ?? [];
  const glyphs = Array.from(words[0] ?? 'PR');
  const first = glyphs[0] ?? 'P';
  const second = glyphs.slice(1).find(glyph => /\p{N}/u.test(glyph))
    ?? (words.length > 1 ? Array.from(words[words.length - 1] ?? '')[0] : glyphs[glyphs.length - 1]) ?? first;
  const mark = words.length ? Array.from(`${first}${second}`.toUpperCase()).slice(0, 2).join('') : 'PR';
  let index = 0;
  for (const glyph of normalized.toLocaleLowerCase('en-US') || 'project') {
    index = (index * 31 + (glyph.codePointAt(0) ?? 0)) % projectPalette.length;
  }
  const [light, dark] = projectPalette[index]!;
  return { projectMark: mark, projectInk: `light-dark(${light}, ${dark})`,
    projectSurface: `light-dark(${light}24, ${dark}24)` };
}
// Provider account marks follow the source instance-display helper, including
// disabled sibling instances: their configured identities still disambiguate.
export function providerBadge(provider: Obj | undefined, providers: Obj[]) {
  const accent = str(provider?.accentColor).trim();
  const color = /^#[0-9a-fA-F]{6}$/.test(accent) ? accent : '';
  const driver = str(provider?.driver);
  const siblings = providers.filter(candidate=>candidate.driver === driver
    && (driver !== 'acpRegistry' || candidate.acpRegistryAgentId === provider?.acpRegistryAgentId));
  if (!provider || !color && siblings.length <= 1) return {providerBadge:'',providerBadgeColor:''};
  const words = str(provider.displayName, str(provider.instanceId)).replace(/[_-]+/g,' ').split(/\s+/u).filter(Boolean);
  const initials = words.length === 1 ? Array.from(words[0]!).slice(0,2).join('')
    : words.slice(0,2).map(word=>Array.from(word)[0] ?? '').join('');
  return {providerBadge:initials.toUpperCase(),providerBadgeColor:color};
}

/** ProviderStatusBanner (provider-status-message.ts): the composer instance's banner. */
export { providerBanner };

/** T3's timeline rows (timeline-presentation.ts transcriptRows). */
export function transcriptPresentation(client: T3Client): Message[] { return withFeedbackRows(client, [...subagentLead(client), ...transcriptRows(client)]); }
/** `displayMode`: Contract's `exactViewport().displayMode` (app.contract passes it with the snapshot request). */
export function snapshot(client: T3Client, now = 0, displayMode = '') {
  adoptHostLocale(client.presentation.systemLocale);
  const fullScreen = displayMode === 'fullscreen'; // the viewport's display mode, not a native fact
  const project = client.shell.projects.find(project => project.id === client.projectId);
  const providers = arr(client.config.providers);
  const provider = providers.find(provider => provider.instanceId === client.providerId);
  // The composer's trigger, options and send follow the instance and model the turn runs on (composer-provider-selection.ts).
  const chosen = composerSelection(client, !!fanoutSelections(client));
  const selectedProvider = chosen.provider ?? provider, selectedModel = chosen.entry ? chosen.model : client.modelId;
  const currentModel = arr(selectedProvider?.models).find(model => model.slug === selectedModel);
  const option = arr(obj(currentModel?.capabilities).optionDescriptors)
    .find(option => option.type === 'select' && ['reasoningEffort', 'effort'].includes(str(option.id)));
  const selectedOption = client.modelOptions.find(selection => selection.id === option?.id)?.value
    ?? option?.currentValue ?? arr(option?.options).find(choice => choice.isDefault === true)?.id;
  const modelReady = !!selectedProvider && providerAvailable(selectedProvider) && !!currentModel;
  const run = activeRun(client.projection);
  const pending = client.pending;
  const transcript = transcriptPresentation(client);
  for (const request of arr(client.projection.runtimeRequests)) {
    if (request.status === 'pending' && ['auth_refresh', 'dynamic_tool_call'].includes(str(request.kind))) {
      transcript.push({ id: `unsupported-${str(request.id)}`, kind: 'system', title: 'Continue in T3 Code',
        body: 'This provider is waiting for a request that must be handled in the T3 Code app.' });
    }
  }
  const connectionMessage = !client.available ? 'Open on macOS to connect.'
    : client.connection === 'connected' ? client.ready ? 'Connected' : 'Synchronizing…'
      : client.connection === 'reconnecting' ? 'Reconnecting…' : client.connection === 'connecting' ? 'Connecting…'
        : client.statusMessage || 'Disconnected';
  return {
    revision: client.revision, telemetry: client.telemetryRevision, alertClip: alertClip(client.presentation), ...providerBanner(provider), available: client.available, connected: client.connection === 'connected',
    connecting: ['connecting', 'reconnecting'].includes(client.connection), syncComplete: client.ready,
    status: connectionMessage, serverUrl: client.origin, composerOwner: composerOwner(client), // a moved draft keeps its owner
    uncertain: pending?.uncertain === true,
    uncertainMessage: pending?.uncertain ? `${pending.description} may already have reached T3. Reconnect and check the thread before retrying.` : '',
    sidebarWidth: client.local.sidebarWidth, sidebarMinWidth: sidebarMinimumWidth(client.local.clientSettings?.fontSizeInterface, fullScreen), controlsLeft: workspaceControlsLeft(client.local.clientSettings?.fontSizeInterface, fullScreen), sidebarOpen: client.local.sidebarOpen, query: client.query,
    projectId: client.projectId, projectName: str(project?.title, 'Choose a project'), threadId: client.threadId,
    threadTitle: str(obj(client.projection.thread).title, 'New thread'),
    // The header title keyed by its text: a reused one-line text keeps drawing the previous title clipped to the new width.
    threadHeading: [str(obj(client.projection.thread).title, 'New thread')].map(title => ({ id: title, label: title })), draft: client.draft, snapshotDrafts: snapshotDraftTiles(client), snapshotOwner: client.snapshotOwner,
    settled: section(obj(client.projection.thread)) === 'settled', ...projectIdentity(str(project?.title)),
    running: !!run, subagentsLive: transcript.some(message => (message.activities ?? []).some(activity => (activity.startedMs ?? 0) > 0)), // AgentElapsed ticks
    canSend: client.writable && !pending && !client.busy && modelReady && !!client.projectId && !projectCloneBlock(client) && !feedbackUploading(client), // a cloning project waits (project-clones-live.ts)
    canStop: client.writable && !pending && !client.busy && !!run,
    providerId: client.providerId, modelId: client.modelId, modelLabel: currentModel ? triggerModelName(currentModel) : selectedModel || 'Choose model',
    composerCollapseOnScroll: client.local.deviceSettings.composerCollapseOnScroll,
    planModeEnabled: client.local.deviceSettings.planModeEnabled,
    composerResting: client.presentation.owner === `${client.origin}:${client.projectId}:${client.threadId}` && client.presentation.resting === true,
    transcriptAway: client.presentation.owner === `${client.origin}:${client.projectId}:${client.threadId}` && client.presentation.atEnd === false,
    ...composerOverlaySnapshot(client, transcript), // the composer over the transcript (composer-overlay.ts)
    ...composerVideoSnapshot(client), // the shelf's videos and their preview (composer-attachments.ts)
    composerImagePreview: composerImagePreview(client), // an image chip's expanded preview (composer-chip-details.ts)
    providerDriver: str(selectedProvider?.driver), ...providerBadge(selectedProvider, providers), optionId: str(option?.id),
    optionLabel: str(arr(option?.options).find(choice => choice.id === selectedOption)?.label, 'Default'),
    modelOptions: arr(option?.options).map(choice => ({ id: str(choice.id), value: str(choice.id), label: str(choice.label), selected: choice.id === selectedOption, default: choice.isDefault === true })),
    runtimeMode: client.runtimeMode, modeLabel: modes[client.runtimeMode] || client.runtimeMode, interactionMode: client.interactionMode,
    hasMore: client.thread?.hasMore === true, historyLoading: client.historyLoading,
    threadLoading: !!client.threadId && !client.thread, // the empty timeline's placeholder waits for the detail
    diffOpen: client.diffOpen, diffLoading: client.diffLoading, diffError: diffNotGit(client) || client.diffError === NOT_GIT_REPO ? '' : client.diffError,
    ...diffSnapshot(client, now),
    projects: client.shell.projects.map(project => ({ id: str(project.id), name: str(project.title), path: str(project.workspaceRoot), selected: project.id === client.projectId })),
    ...sidebarSnapshot(client, now, { projectIdentity, providerBadge }),
    legacy: legacySidebarSnapshot(client, now, { projectIdentity }),
    timelineReadsNeeded: timelineReadsNeeded(client),
    markdownSkills: markdownSkills(workspaceValues(provider ?? {}, workspaceCwd(client), 'skills').map(skill => ({ name: str(skill.name), displayName: str(skill.displayName) }))),
    messages: timelineMessages(client, transcript, now), ...timelineSnapshot(client),
    providers: providers.map(provider => ({ id: str(provider.instanceId), name: str(provider.displayName, str(provider.driver)), available: providerAvailable(provider) })),
    models: providers.flatMap(provider => arr(provider.models).map(model => ({ id: str(model.slug), name: str(model.name, str(model.slug)),
      providerId: str(provider.instanceId), selected: provider.instanceId === client.providerId && model.slug === client.modelId }))),
    ...threadErrorView(client, requestPresentation(client)),
    composer: composerSnapshot(client, now),
    serverUpdate: serverUpdateView(client), // server-update-notices.ts: the offline timers' episodes, the details dot, the update confirmation
    look: look(client),
    ...tableMenuSnapshot(client), // a table's Copy popup over every layer
    highlightPending: highlightPending(), // last, after every code text above asked for its tokens
  };
}

/**
 * T3's picker catalog (model-catalog.ts); browsing and searching never change the selected model.
 * A target names the Settings → General row the picker chooses for (settings-model-picker.ts).
 */
export function modelCatalog(client: T3Client, providerId: string, query: string, target = '') {
  const catalog = target ? settingsPickerCatalog(client, target, providerId, query, providerBadge) : pickerCatalog(client, providerId, query, providerBadge);
  // ModelPickerContent modelJumpLabelByKey: each jump command's shortcut in the open picker's context.
  const context = { modelPickerOpen: true, terminalOpen: terminalOpen(client) };
  return withJumpLabels(catalog, command => {
    const shortcut = effectiveShortcut(client.config, command, context);
    return shortcut ? { label: shortcutLabel(shortcutInput(shortcut)), chord: ariaChord(shortcut) } : { label: '', chord: '' };
  });
}
