import { legacySidebarSnapshot } from './legacy-sidebar-view';
import { timelineReadsNeeded } from './timeline-prepare';
import { markdownSkills } from './r4-timeline-chips';
import { workspaceValues } from './composer-workspace-snapshots';
import { workspaceCwd } from './composer-editor';
import { snapshotDraftTiles } from './snapshot-settings';
import { arr, obj, str, num, type Message, type Obj } from './domain';
import { activeRun, providerAvailable } from './protocol';
import type { T3Client } from './client';
import { timelineMessages, timelineSnapshot, transcriptRows } from './timeline-presentation';
import { sidebarSnapshot } from './sidebar-view';
import { subagentLead } from './sidebar-lineage';
import { requestPresentation } from './requests';
import { threadErrorView } from './timeline-errors';
import { diffSnapshot } from './diff';
import { composerSnapshot } from './composer-presentation';
import { triggerModelName } from './r3-composer-controls-model';
import { pickerCatalog } from './model-catalog';
import { look } from './settings-appearance-look';
import { composerOverlaySnapshot } from './r4-composer-overlay';
import { composerVideoSnapshot } from './r4-composer-attachments';
import { alertClip } from './r6-polish-measure'; // r6-polish
import { tableMenuSnapshot } from './r8-keys-table-menu'; // lane r8-keys
import { sidebarMinimumWidth, workspaceControlsLeft } from './r12-sidebar-width'; // lane r12-sidebar
import { adoptHostLocale } from './timestamp-format'; // desktop-shell-details: the Mac's locale, from the status presentation (T3Locale.swift)

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

/** ServerProvider advisory semantics from T3's ProviderStatusBanner. */
export function providerBanner(provider: Obj | undefined) {
  const empty = { providerBannerKey: '', providerBannerTitle: '', providerBannerMessage: '', providerBannerWarning: false };
  if (!provider || provider.status === 'disabled') return empty;
  const auth = str(obj(provider.auth).status), advisory = obj(provider.compatibilityAdvisory);
  const unauthenticated = provider.status === 'error' && auth === 'unauthenticated';
  const incompatible = !unauthenticated && (advisory.status === 'broken'
    || provider.status === 'ready' && advisory.status === 'unsupported');
  if (!incompatible && (provider.status === 'ready'
    || provider.driver === 'antigravity' && provider.installed && provider.status === 'warning' && auth === 'unknown')) return empty;
  const name = str(provider.displayName, str(provider.driver, 'Provider'));
  const status = incompatible ? str(advisory.status) : str(provider.status);
  const message = incompatible ? str(advisory.message) : str(provider.message,
    unauthenticated ? 'Sign in via the CLI to authenticate again.' : `${name} provider is unavailable.`);
  return {
    providerBannerKey: [str(provider.instanceId), status, incompatible ? str(provider.version) : auth, message].join('\u0000'),
    providerBannerTitle: unauthenticated ? `${name} is unauthenticated` : incompatible
      ? `${name} ${str(provider.version)} is ${advisory.status === 'broken' ? 'known to be broken' : 'unsupported'}` : `${name} provider status`,
    providerBannerMessage: message,
    providerBannerWarning: advisory.status !== 'broken' && (provider.status === 'warning' || incompatible),
  };
}

/** T3's timeline rows (timeline-presentation.ts transcriptRows). */
export function transcriptPresentation(client: T3Client): Message[] { return [...subagentLead(client), ...transcriptRows(client)]; }
export function snapshot(client: T3Client, now = 0) {
  adoptHostLocale(client.presentation.systemLocale);
  const fullScreen = client.presentation.fullScreen === true; // T3FullScreen.swift
  const project = client.shell.projects.find(project => project.id === client.projectId);
  const providers = arr(client.config.providers);
  const provider = providers.find(provider => provider.instanceId === client.providerId);
  const currentModel = arr(provider?.models).find(model => model.slug === client.modelId);
  const option = arr(obj(currentModel?.capabilities).optionDescriptors)
    .find(option => option.type === 'select' && ['reasoningEffort', 'effort'].includes(str(option.id)));
  const selectedOption = client.modelOptions.find(selection => selection.id === option?.id)?.value
    ?? option?.currentValue ?? arr(option?.options).find(choice => choice.isDefault === true)?.id;
  const modelReady = !!provider && providerAvailable(provider) && !!currentModel;
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
    revision: client.revision, alertClip: alertClip(client.presentation), ...providerBanner(provider), available: client.available, connected: client.connection === 'connected',
    connecting: ['connecting', 'reconnecting'].includes(client.connection), syncComplete: client.ready,
    status: connectionMessage, serverUrl: client.origin,
    uncertain: pending?.uncertain === true,
    uncertainMessage: pending?.uncertain ? `${pending.description} may already have reached T3. Reconnect and check the thread before retrying.` : '',
    sidebarWidth: client.local.sidebarWidth, sidebarMinWidth: sidebarMinimumWidth(client.local.clientSettings?.fontSizeInterface, fullScreen), controlsLeft: workspaceControlsLeft(client.local.clientSettings?.fontSizeInterface, fullScreen), sidebarOpen: client.local.sidebarOpen, query: client.query,
    projectId: client.projectId, projectName: str(project?.title, 'Choose a project'), threadId: client.threadId,
    threadTitle: str(obj(client.projection.thread).title, 'New thread'),
    // The header title keyed by its text: a reused one-line text keeps drawing the previous title clipped to the new width.
    threadHeading: [str(obj(client.projection.thread).title, 'New thread')].map(title => ({ id: title, label: title })), draft: client.draft, snapshotDrafts: snapshotDraftTiles(client), snapshotOwner: client.snapshotOwner,
    settled: section(obj(client.projection.thread)) === 'settled', ...projectIdentity(str(project?.title)),
    running: !!run, canSend: client.writable && !pending && !client.busy && modelReady && !!client.projectId,
    canStop: client.writable && !pending && !client.busy && !!run,
    providerId: client.providerId, modelId: client.modelId, modelLabel: currentModel ? triggerModelName(currentModel) : client.modelId || 'Choose model',
    composerCollapseOnScroll: client.local.deviceSettings.composerCollapseOnScroll,
    planModeEnabled: client.local.deviceSettings.planModeEnabled,
    composerResting: client.presentation.owner === `${client.origin}:${client.projectId}:${client.threadId}` && client.presentation.resting === true,
    transcriptAway: client.presentation.owner === `${client.origin}:${client.projectId}:${client.threadId}` && client.presentation.atEnd === false,
    ...composerOverlaySnapshot(client, transcript), // the composer over the transcript (r4-composer-overlay.ts)
    ...composerVideoSnapshot(client), // the shelf's videos and their preview (r4-composer-attachments.ts)
    providerDriver: str(provider?.driver), ...providerBadge(provider,providers), optionId: str(option?.id),
    optionLabel: str(arr(option?.options).find(choice => choice.id === selectedOption)?.label, 'Default'),
    modelOptions: arr(option?.options).map(choice => ({ id: str(choice.id), value: str(choice.id), label: str(choice.label), selected: choice.id === selectedOption, default: choice.isDefault === true })),
    runtimeMode: client.runtimeMode, modeLabel: modes[client.runtimeMode] || client.runtimeMode, interactionMode: client.interactionMode,
    hasMore: client.thread?.hasMore === true, historyLoading: client.historyLoading,
    diffOpen: client.diffOpen, diffLoading: client.diffLoading, diffError: client.diffError,
    ...diffSnapshot(client, now),
    projects: client.shell.projects.map(project => ({ id: str(project.id), name: str(project.title), path: str(project.workspaceRoot), selected: project.id === client.projectId })),
    ...sidebarSnapshot(client, now, { projectIdentity, providerBadge }),
    legacy: legacySidebarSnapshot(client, now, { projectIdentity }), // legacy-sidebar
    timelineReadsNeeded: timelineReadsNeeded(client),
    markdownSkills: markdownSkills(workspaceValues(provider ?? {}, workspaceCwd(client), 'skills').map(skill => ({ name: str(skill.name), displayName: str(skill.displayName) }))),
    messages: timelineMessages(client, transcript, now), ...timelineSnapshot(client),
    providers: providers.map(provider => ({ id: str(provider.instanceId), name: str(provider.displayName, str(provider.driver)), available: providerAvailable(provider) })),
    models: providers.flatMap(provider => arr(provider.models).map(model => ({ id: str(model.slug), name: str(model.name, str(model.slug)),
      providerId: str(provider.instanceId), selected: provider.instanceId === client.providerId && model.slug === client.modelId }))),
    ...threadErrorView(client, requestPresentation(client)),
    composer: composerSnapshot(client, now),
    look: look(client),
    ...tableMenuSnapshot(client), // lane r8-keys: a table's Copy popup over every layer
  };
}

/** T3's picker catalog (model-catalog.ts); browsing and searching never change the selected model. */
export function modelCatalog(client: T3Client, providerId: string, query: string) {
  return pickerCatalog(client, providerId, query, providerBadge);
}
