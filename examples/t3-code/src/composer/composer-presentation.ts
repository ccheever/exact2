// The composer's toolbar, menus and placeholder, projected the way T3's
// ChatComposer.tsx, TraitsPicker.tsx and runtimeModeConfig.ts do. Contract
// renders the rows; nothing here mutates the client.
import { arr, obj, str, type Obj } from '../client/domain';
import { requestPresentation } from './requests';
import type { T3Client } from '../client/client';
import { effortBlockWidth, footerLayout, traitsDisplay, primaryAction, composerNotices, providerControl, contextMeter, compactChip } from './composer-controls-view';
import { resolveComposerControlsOwnership } from './composer-controls-ownership';
import { effortBars, resolveEffortGlyph } from './effort-glyph';
import { planFollowUp, noteNow } from './composer-controls';
import { queuedEditView, settleQueuedEdits } from './composer-controls-queue';
import { endSummary, threadNowProbes } from './thread-end';
import { subagentBar } from './composer-controls-subagent';
import { attachOffered, preparingImages } from './composer-controls-attach';
import { readySend, uploadBlock, uploadTable } from './composer-uploads';
import { fanoutSelections, fanoutView } from './composer-controls-fanout';
import { composerSelection } from './composer-provider-selection';
import { chordGlyphs, displayModelName, optionValue, REASONING_SELECT_DESCRIPTOR_IDS, reportedSelection, resolvedCurrent, triggerModelName, type Selection } from './composer-controls-model';
import { composerRequiresModifier, sendChords, sendCommandChords } from './composer-editor-intent';
import { layoutFrames } from '../shell/layout-frames';
import { draftHoldsCommandEnter } from '../diff/diff';
import { fileDraftHoldsCommandEnter } from '../panel/files-surface'; // the Files preview's draft too
import { activeRef, terminalOpen } from '../terminal/terminal-drawer-view'; // ChatComposer passes the real terminalOpen
import { annotatingInThread, sendChordsWhileAnnotating } from '../browser/browser-annotation'; // ⌘↩ in Annotate
import { measuredLabels } from './composer-measure';
import { atRootFontSize, composerMenus, effortMenuWidth, measured, probe, probeKey, traitsMenuHeight } from './composer-menus';
import { anchorFrame } from '../shared/laid-out-widths';
import { clampInterfaceFontSize } from '../appearance/appearance-fonts';
import { environmentView } from '../details/git-env';
import { ULTRATHINK_LOCKED_MESSAGE, ultrathinkTraits, withImplicitFastModeDefault } from './composer-provider-state';
import { ultrathinkFrame } from './composer-ultrathink';
import { acpInstanceIcon } from './acp-instance-icon'; // the trigger's ACP agent mark
import { overflowMenu } from './composer-overflow';
import { usageConfirm, usageResetClock } from './composer-controls-usage'; // the reset credit confirm and the reset's clock
import { stripLabelsCompact, stripLabelTexts } from './context-strip'; // the context strip's labels or icons
import type { ComposerNotice, FooterSteps } from './composer-controls-view';

/** runtimeModeConfig.ts (v0.1.5): label, description and lucide icon per mode; full access is a crossed shield, since an
 *  open lock reads like a closed one at small sizes. */
export const runtimeModes = [
  { mode: 'approval-required', label: 'Supervised', description: 'Ask before commands and file changes.', icon: 'lock' },
  { mode: 'auto-accept-edits', label: 'Auto-accept edits', description: 'Auto-approve edits, ask before other actions.', icon: 'pen-line' },
  { mode: 'auto', label: 'Auto', description: 'Supported providers approve routine actions; others still ask.', icon: 'sparkles' },
  { mode: 'full-access', label: 'Full access', description: 'Allow commands and edits without prompts.', icon: 'shield-off' },
];
const SAVED_OPTION_LABELS: Record<string, string> = { agent: 'Agent', effort: 'Effort', reasoningEffort: 'Reasoning effort', variant: 'Reasoning' };

type ComposerSource = {
  config: Obj; projection: Obj; presentation: Obj; providerId: string; modelId: string; modelOptions: Obj[];
  runtimeMode: string; interactionMode: string; threadId: string; projectId: string; connection: string;
  local: { drafts: Record<string, string>; deviceSettings: { planModeEnabled: boolean } };
  /** The composer's draft (T3Client.draft); the ultrathink flow reads it. */
  draft?: string;
  /** composer-provider-selection.ts: the project defaults and whether the catalog arrived; the field's name: the thread rows' titles. */
  shell?: { projects: Obj[]; threads?: Obj[] }; configLive?: boolean;
};

/**
 * derivePhase(deriveThreadRuntime(...)): the run that owns live work (preparing,
 * starting, running, waiting), else the newest run not held in the queue; a
 * newer queued run never hides an older executing one (deriveThreadActivityRun).
 * "disconnected" while the thread has no such run and no provider thread.
 */
/** Annotate is on in a Browser tab of the open thread (browser-annotation.ts). */
function annotatingOpenThread(client: T3Client): boolean {
  const ref = activeRef(client);
  return !!ref && annotatingInThread(client, ref.environmentId, ref.threadId);
}

export function threadPhase(projection: Obj): string {
  const runs = arr(projection.runs), thread = obj(projection.thread);
  const latest = (list: Obj[]) => list.reduce<Obj | undefined>((best, run) => !best || Number(run.ordinal) > Number(best.ordinal) ? run : best, undefined);
  const unheld = latest(runs.filter(run => !(run.status === 'queued' && run.queueHeld === true)));
  if (!unheld && !str(thread.activeProviderThreadId)) return 'disconnected';
  const active = latest(runs.filter(run => ['preparing', 'starting', 'running', 'waiting'].includes(str(run.status)))) ?? unheld;
  const status = str(active?.status, 'idle');
  if (['preparing', 'starting', 'queued'].includes(status)) return 'connecting';
  if (['running', 'waiting'].includes(status)) return 'running';
  return 'ready';
}

const UNAVAILABLE_BADGE = 'Unavailable';
const stringOrNull = (value: unknown) => typeof value === 'string' ? value : null;

/** resolveProjectSettings(settings, projectId).settings.defaultRuntimeMode: the project's override, else the server's. */
function defaultRuntimeModeOf(client: ComposerSource): string {
  const settings = obj(client.config.settings), overrides = obj(obj(settings.projectSettingsOverrides)[client.projectId]);
  return str(overrides.defaultRuntimeMode || settings.defaultRuntimeMode);
}

/** ChatComposer.tsx placeholder chain (74541bab17; one default line since 3816296021, composerPlaceholder.ts). */
export function composerPlaceholder(input: { connected: boolean; approval: boolean; question: boolean; choiceOnly: boolean;
  projectRequired: boolean; providerUnavailable: boolean; phase: string; planReady?: boolean; feedback?: boolean }): string {
  if (input.approval) return input.feedback ? 'Send feedback to decline this request' : 'Send a message to decline and redirect the agent';
  if (input.question) return input.choiceOnly ? 'Choose an option above' : 'Type your own answer, or leave this blank to use the selected option';
  if (input.planReady) return 'Add feedback to refine the plan, or leave this blank to implement it';
  if (!input.connected) return 'Connect to your T3 server to send a message';
  if (input.projectRequired) return 'Choose a project above to start a thread';
  if (input.providerUnavailable) return 'Enable a provider in Settings to send a message';
  return 'Ask anything. @ files, $ skills, / commands';
}

/** getProviderOptionDescriptors + TraitsMenuContent: one flat menu of headers, dividers and radio rows. */
export function traitsMenu(descriptors: Obj[], selections: Obj[], selection: Selection | null = null, reported: Selection | null = null,
  ultra: { primaryId: string; controlled: boolean; inBody: boolean } = { primaryId: '', controlled: false, inBody: false }) {
  const items: Array<{ key: string; kind: string; descriptor: string; value: string; label: string; description: string; selected: boolean; isDefault: boolean; index: number; disabled: boolean }> = [];
  const labels: string[] = [];
  let index = 0;
  descriptors.filter(descriptor => descriptor.type === 'select').forEach((descriptor, group) => {
    const id = str(descriptor.id), options = arr(descriptor.options);
    // getDescriptorStringValue(descriptor, selection, reported): the checked radio follows the trigger's label.
    // A prompt that controls the primary effort checks "ultrathink"; "ultrathink" in its body locks the rows.
    const primary = id === ultra.primaryId, locked = primary && ultra.inBody;
    const current = primary && ultra.controlled ? 'ultrathink' : optionValue(descriptor, resolvedCurrent(descriptor, selections), selection, reported);
    if (group > 0) items.push({ key: `divider:${id}`, kind: 'divider', descriptor: id, value: '', label: '', description: '', selected: false, isDefault: false, index: -1, disabled: false });
    items.push({ key: `header:${id}`, kind: 'header', descriptor: id, value: '', label: str(descriptor.label, SAVED_OPTION_LABELS[id] ?? id), description: '', selected: false, isDefault: false, index: -1, disabled: false });
    if (locked) items.push({ key: `note:${id}`, kind: 'note', descriptor: id, value: '', label: ULTRATHINK_LOCKED_MESSAGE, description: '', selected: false, isDefault: false, index: -1, disabled: false });
    for (const option of options) {
      items.push({ key: `${id}:${str(option.id)}`, kind: 'option', descriptor: id, value: str(option.id), label: str(option.label, str(option.id)),
        description: str(option.description), selected: option.id === current, isDefault: option.isDefault === true, index: index++, disabled: locked });
    }
    const label = str(options.find(option => option.id === current)?.label);
    if (label) labels.push(label);
  });
  const selects = descriptors.some(descriptor => descriptor.type === 'select');
  descriptors.filter(descriptor => descriptor.type === 'boolean').forEach((descriptor, group) => {
    const id = str(descriptor.id), value = selections.find(selection => selection.id === id)?.value ?? descriptor.currentValue;
    if (group > 0 || selects) items.push({ key: `divider:${id}`, kind: 'divider', descriptor: id, value: '', label: '', description: '', selected: false, isDefault: false, index: -1, disabled: false });
    items.push({ key: `header:${id}`, kind: 'header', descriptor: id, value: '', label: str(descriptor.label, SAVED_OPTION_LABELS[id] ?? id), description: '', selected: false, isDefault: false, index: -1, disabled: false });
    for (const on of [true, false]) items.push({ key: `${id}:${on}`, kind: 'option', descriptor: id, value: String(on), label: on ? 'On' : 'Off', description: '',
      selected: (value === true) === on, isDefault: false, index: index++, disabled: false });
  });
  return { items, label: labels.join(' · '), count: index, selected: items.find(item => item.kind === 'option' && item.selected)?.index ?? 0 };
}

const named: Record<string, string> = { ' ': 'Space', escape: 'Escape', enter: 'Enter', tab: 'Tab', arrowup: 'ArrowUp', arrowdown: 'ArrowDown' };
function whenMatches(ast: unknown, context: Record<string, boolean>, depth = 0): boolean | null {
  const node = obj(ast); if (!node.type) return true; if (depth > 64) return null;
  if (node.type === 'identifier') return Object.prototype.hasOwnProperty.call(context, str(node.name)) ? context[str(node.name)]! : null;
  const left = whenMatches(node.type === 'not' ? node.node : node.left, context, depth + 1);
  if (node.type === 'not') return left === null ? null : !left;
  const right = whenMatches(node.right, context, depth + 1);
  if (left === null || right === null) return null;
  return node.type === 'and' ? left && right : node.type === 'or' ? left || right : null;
}
/** The chords that currently resolve to a command, as `aria-keyshortcuts` (same precedence as keybinding-settings.ts). */
export function commandChords(config: Obj, command: string, fallback: string, modelPickerOpen = false, extra: Record<string, boolean> = {}): string {
  if (!Array.isArray(config.keybindings)) return fallback;
  const context: Record<string, boolean> = { true: true, false: false, isDesktop: true, isWeb: false, terminalFocus: false, terminalOpen: false,
    previewFocus: false, previewOpen: false, usagePageOpen: false, composerFocus: true, editableFocus: true, turnRunning: false,
    modelPickerOpen, draftThreadRoute: false, modalOpen: false, ...extra };
  const winners = new Map<string, string>();
  for (const rule of [...arr(config.keybindings)].reverse()) {
    const shortcut = obj(rule.shortcut), key = str(shortcut.key);
    const chord = [...(shortcut.modKey || shortcut.metaKey ? ['Meta'] : []), ...(shortcut.ctrlKey ? ['Control'] : []),
      ...(shortcut.altKey ? ['Alt'] : []), ...(shortcut.shiftKey ? ['Shift'] : []), named[key] || key].join('+');
    const match = whenMatches(rule.whenAst, context);
    if (match === false || winners.has(chord)) continue;
    winners.set(chord, match === null ? '' : str(rule.command));
  }
  return [...winners].filter(([, winner]) => winner === command).map(([chord]) => chord).join(' ');
}

/** The toolbar controls row's width (the app module's `controls` anchor, T3Composer), which footerLayout fits; 0 until laid out. */
export function controlsWidth(presentation: Obj): number {
  const value = obj(presentation.anchors).controls;
  return Array.isArray(value) ? Number(value[1]) || 0 : 0;
}

// The last footer layout per client, for resolveRestingComposerControlsLayout's promotion slack.
const footerSteps = new WeakMap<object, FooterSteps>();
/**
 * ChatComposer's ariaLabel: "Message <title>" by the title the shell carries (ChatView's serverThread; the detail's until
 * the shell lists it), "Message New thread" on a draft (buildLocalDraftThread), "Message" with neither.
 */
function fieldLabel(client: ComposerSource): string {
  const shellTitle = client.threadId ? str(client.shell?.threads?.find(thread => thread.id === client.threadId)?.title) : '';
  const title = client.threadId ? shellTitle || str(obj(client.projection.thread).title) : 'New thread';
  return title ? `Message ${title}` : 'Message';
}
export function composerView(client: ComposerSource, requests: { approval: boolean; feedback?: boolean; question: boolean; choiceOnly: boolean; planReady?: boolean; terminalOpen?: boolean }) {
  const keyContext = { terminalOpen: requests.terminalOpen === true }; // ChatComposer: the real terminalOpen, terminalFocus false
  const providers = arr(client.config.providers);
  // resolveComposerProviderSelection + deriveEffectiveComposerModelState: the instance and model the turn runs on.
  const chosen = composerSelection(client, !!fanoutSelections(client as unknown as T3Client));
  const provider = chosen.provider ?? providers.find(entry => entry.instanceId === client.providerId);
  const modelId = chosen.entry ? chosen.model : client.modelId;
  const model = arr(provider?.models).find(entry => entry.slug === modelId);
  const descriptors = arr(obj(model?.capabilities).optionDescriptors);
  // The composer's selection and what the active provider thread reports it runs (display only, never dispatched).
  const selection = provider && modelId ? { instanceId: str(provider.instanceId), model: modelId, options: client.modelOptions } : null;
  const reported = client.threadId ? reportedSelection(client.projection) : null;
  // The traits show the implicit Fast default (Normal) and the prompt's ultrathink (composerProviderState.tsx).
  const shown = withImplicitFastModeDefault(descriptors, client.modelOptions) ?? [], prompt = client.draft ?? '';
  const ultra = ultrathinkTraits(descriptors, prompt);
  const traits = traitsMenu(descriptors, shown, selection, reported, ultra);
  const supported = Array.isArray(provider?.supportedRuntimeModes) ? (provider!.supportedRuntimeModes as unknown[]).map(value => str(value)) : [];
  const runtimes = runtimeModes.filter(option => !supported.length || supported.includes(option.mode));
  const runtime = runtimes.find(option => option.mode === client.runtimeMode) ?? runtimes[0] ?? runtimeModes[0]!;
  const planVisible = client.local.deviceSettings.planModeEnabled && !!provider && provider.showInteractionModeToggle !== false;
  const display = traitsDisplay(str(provider?.driver), descriptors, shown, selection, reported, ultra);
  // getTriggerDisplayModelName (the spoken name) for the footer's layout; the tooltip is the full name with the picker's
  // shortcut (ProviderModelPicker triggerTooltipContent). The trigger's text and accessible name are the snapshot's.
  const modelTitle = model ? triggerModelName(model, str(provider?.driver)) : modelId || 'Choose model', modelShortcut = chordGlyphs(commandChords(client.config, 'modelPicker.toggle', 'Meta+Shift+M', false, keyContext).split(' ')[0] ?? '');
  const modelLabel = model ? `${displayModelName(model)}${model.isUnavailable === true ? ' (Unavailable)' : ''}` : modelTitle;
  // ProviderModelPicker's Unavailable Badge (outline, sm): its text with 3pt padding and the 1pt border each side.
  const badge = model?.isUnavailable === true && !fanoutSelections(client as unknown as T3Client) ? measured(client.presentation, UNAVAILABLE_BADGE, 10, 500) + 8 : 0;
  // TraitsPicker in the composer (v0.1.5): the primary reasoning select as a word ("High", Settings › Appearance ›
  // Effort display "label", the default) or rising bars ("bars"); Max, Ultracode and Ultrathink keep their names, and
  // traits the bars cannot show (an agent mode) read as their label. The gauge shows only in an icon-only footer.
  const primary = descriptors.find(descriptor => descriptor.type === 'select' && descriptor.id === ultra.primaryId);
  const glyph = primary && REASONING_SELECT_DESCRIPTOR_IDS.has(str(primary.id))
    ? resolveEffortGlyph(primary, ultra.controlled ? 'ultrathink' : stringOrNull(optionValue(primary, resolvedCurrent(primary, shown), selection, reported))) : null;
  const effortDisplay = (client.local as { clientSettings?: { effortDisplay?: unknown } }).clientSettings?.effortDisplay === 'bars' ? 'bars' : 'label';
  const effortWord = glyph ? (glyph.special ?? (effortDisplay === 'label' ? glyph.label : '')) : display.label;
  const effort = { shown: display.label !== '', word: effortWord, bars: glyph && !effortWord ? glyph.count : 0,
    speed: glyph ? display.speedLabel ?? '' : '', other: glyph ? display.otherLabels.join(' · ') : '' };
  // ComposerControlsBar: Permissions hide while the thread's mode is the project's default, or while the new thread's
  // strip names them (permissionsShownInStrip: a draft of a project, whose strip always shows).
  const defaultRuntimeMode = defaultRuntimeModeOf(client);
  const permissionsInStrip = !client.threadId && !!client.projectId;
  const accessMatchesDefault = runtime.mode === defaultRuntimeMode;
  const planUi = planVisible;
  const firstOwnership = resolveComposerControlsOwnership({ matchesDefault: accessMatchesDefault || permissionsInStrip, hasTraits: effort.shown,
    planModeUiEnabled: planUi, collapsed: false, measuredHiddenBlockCount: 0 });
  const { steps, measuredHidden, ...layout } = footerLayout({ model: modelTitle, traits: effortBlockWidth(effort, measuredLabels(client.presentation)),
    runtime: firstOwnership.accessDisplay === 'label' ? runtime.label : '', runtimeShown: firstOwnership.accessDisplay !== 'hidden', mode: firstOwnership.showModeBlock,
    plan: planVisible ? (client.interactionMode === 'plan' ? 'Plan' : 'Build') : '', host: controlsWidth(client.presentation),
    measure: measuredLabels(client.presentation), previous: footerSteps.get(client), badge });
  footerSteps.set(client, steps);
  const ownership = resolveComposerControlsOwnership({ matchesDefault: accessMatchesDefault || permissionsInStrip, hasTraits: effort.shown,
    planModeUiEnabled: planUi, collapsed: false, measuredHiddenBlockCount: measuredHidden });
  const keyEffort = commandChords(client.config, 'composer.effort', 'Meta+Shift+E', false, keyContext), keyMode = commandChords(client.config, 'composer.mode', 'Meta+Shift+A', false, keyContext);
  // The effort chip's name and tooltip: "Effort: High · Fast", and while it holds Permissions, "Permissions: Supervised · ⇧⌘A".
  const permissionsHint = ownership.accessDisplay === 'hidden' || accessMatchesDefault || permissionsInStrip
    ? [`Permissions: ${runtime.label}`, chordGlyphs(keyMode.split(' ')[0] ?? '')].filter(Boolean).join(' · ') : '';
  const traitsTip = [`Effort: ${display.label}`, effort.speed, permissionsHint].filter(Boolean).join(' · ');
  // The "More composer controls" rows; Permissions join them only while their home is in the menu.
  const more = overflowMenu({ traits: traits.items, traitsHidden: ownership.traitsHidden, modeHidden: ownership.modeHidden, planVisible, interactionMode: client.interactionMode,
    showPermissions: ownership.overflowShowsPermissions });
  return {
    placeholder: composerPlaceholder({ connected: client.connection === 'connected', ...requests, planReady: !!requests.planReady,
      projectRequired: !client.projectId, providerUnavailable: chosen.showProviderUnavailable,
      phase: client.threadId ? threadPhase(client.projection) : 'disconnected' }),
    modelTip: modelShortcut ? `${modelLabel} · ${modelShortcut}` : modelLabel, fieldLabel: fieldLabel(client),
    traitsLabel: display.label, traitsTip,
    effortShown: effort.shown, effortWord: effort.word, effortBars: glyph && !effort.word ? effortBars(glyph) : [], effortBarsWidth: glyph ? 4 * glyph.count - 2 : 0,
    effortSpeed: effort.speed, effortOther: effort.other,
    providerIcon: acpInstanceIcon(client.config, provider),
    ...layout, traitsOverflow: ownership.traitsHidden, modeOverflow: ownership.modeHidden, traitsCount: traits.count, traitsSelected: traits.selected, traits: traits.items,
    // Where Permissions live (resolveComposerControlsOwnership): the chip shows its label, its icon or nothing; the
    // effort chip or the overflow menu answers composer.mode while it does not.
    accessDisplay: ownership.accessDisplay, accessEmphasized: !accessMatchesDefault, showModeBlock: ownership.showModeBlock,
    effortOwnsMode: ownership.effortOwnsModeShortcut, overflowPermissions: ownership.overflowShowsPermissions, permissionsInStrip,
    permissionsTip: `Permissions: ${runtime.label}. ${runtime.description}`,
    stripPermissionsTip: ['Permissions', chordGlyphs(keyMode.split(' ')[0] ?? '')].filter(Boolean).join(' · '),
    runtimeMode: runtime.mode, runtimeLabel: runtime.label, runtimeIcon: runtime.icon, runtimeDescription: runtime.description,
    runtimeSelected: Math.max(0, runtimes.indexOf(runtime)), runtimeCount: runtimes.length,
    runtimes: runtimes.map((option, index) => ({ ...option, selected: option === runtime, index })),
    planVisible, planActive: planVisible && client.interactionMode === 'plan',
    ultrathink: ultrathinkFrame(client, prompt),
    more: more.items, moreCount: more.count,
    moreMenuWidth: effortMenuWidth(client.presentation, more.items), moreMenuHeight: traitsMenuHeight(more.items),
    keyEffort, keyMode,
  };
}

/** Window-space tops of the composer's toolbar and context strip, which its menus open from (layout-frames.ts); 0 until measured. */
export function frameTops(frames: Obj) {
  const top = (name: string) => { const value = Array.isArray(frames[name]) ? frames[name] as unknown[] : []; return Number(value[1]) || 0; };
  const strip = Array.isArray(frames.strip) ? frames.strip as unknown[] : [];
  const toolbar = Array.isArray(frames.toolbar) ? frames.toolbar as unknown[] : [];
  return { toolbarTop: top('toolbar'), toolbarRight: (Number(toolbar[0]) || 0) + (Number(toolbar[2]) || 0), stripTop: top('strip'), stripLeft: Number(strip[0]) || 0, stripRight: (Number(strip[0]) || 0) + (Number(strip[2]) || 0) };
}

/**
 * NoticeDescription (ComposerBannerStack.tsx): a description, plain or a roster of segments, offers its details while
 * its natural width (its probes) exceeds the width it was laid out at (its `nd:` anchor). While the button shows, its
 * 24pt and the 4pt gap before it count as room, so it cannot keep itself shown once the text would fit.
 */
const detailsShown = new WeakMap<T3Client, Set<string>>();
const measurable = (notice: ComposerNotice) => (!!notice.description || notice.segments.length > 0) && !notice.redact && !notice.machines.length;
/** The texts a description lays out, each at its size and weight: a roster's links are medium, its commas regular. */
function descriptionTexts(notice: ComposerNotice): Array<[string, number]> {
  if (!notice.segments.length) return [[notice.description, 400]];
  return notice.segments.flatMap(segment => [[segment.label, segment.threadId ? 500 : 400] as [string, number], ...(segment.last ? [] : [[', ', 400] as [string, number]])]);
}
export function withNoticeDetails<T extends ComposerNotice>(client: T3Client, notices: T[], rootFontSize: number): T[] {
  const anchors = obj(client.presentation.anchors), shown = detailsShown.get(client), next = new Set<string>();
  const width = (name: string) => { const value = anchors[name]; return Array.isArray(value) ? Number(value[1]) : NaN; };
  const out = notices.map(notice => {
    if (!measurable(notice)) return notice;
    const laidOut = width(`nd:${notice.id}`), parts = descriptionTexts(notice).map(([text, weight]) => width(probeKey(text, 12, weight)));
    if (!(laidOut >= 0) || !parts.every(part => part > 0)) return notice;
    const natural = parts.reduce((sum, part) => sum + part, 0) * rootFontSize / 16;
    const details = natural > laidOut + (shown?.has(notice.id) ? 28 * rootFontSize / 16 : 0) + 0.5;
    if (details) next.add(notice.id);
    return { ...notice, details };
  });
  detailsShown.set(client, next);
  return out;
}

/** The snapshot's `composer` row: request state comes from the requests projection. */
export function composerSnapshot(client: T3Client, now = 0) {
  noteNow(client, now);
  settleQueuedEdits(client);
  const requests = requestPresentation(client);
  const question = requests.questions[0];
  const phase = client.threadId ? threadPhase(client.projection) : 'disconnected';
  const view = composerView(client, { approval: requests.approvals.length > 0, feedback: requests.approvals[0]?.feedback === true, question: !!question,
    choiceOnly: !!question && !question.customAllowed, planReady: !!planFollowUp(client) && !question, terminalOpen: terminalOpen(client) });
  // ComposerBannerStack: each notice an attached card; consecutive cards join edge to edge (`stacked`).
  const rootFontSize = clampInterfaceFontSize((client.local as { clientSettings?: { fontSizeInterface?: unknown } }).clientSettings?.fontSizeInterface);
  const notices = withNoticeDetails(client, composerNotices(client, now), rootFontSize).map((notice, index) => ({ ...notice, stacked: index > 0,
    titleDetails: noticeTitleDetails(client.presentation, notice, rootFontSize) }));
  const queue = queuedEditView(client), action = primaryAction(client, phase);
  if (queue.queueEditing) Object.assign(action, { sendLabel: 'Update queued message', sendIcon: 'check', sendTooltip: 'Update queued message', sendRunning: true });
  // A send that compacts first says so (ComposerPrimaryActions submitLabel, submitTooltip).
  const compact = compactChip(client, now, !!queue.queueEditing);
  if (compact.compactSend) Object.assign(action, { sendLabel: 'Compact and send', sendTooltip: action.sendStatus || `Summarize ${compact.compactTokens} tokens of history, then send` });
  const chosen = composerSelection(client, !!fanoutSelections(client));
  const modelId = chosen.entry ? chosen.model : client.modelId;
  const model = arr((chosen.provider ?? arr(client.config.providers).find(entry => entry.instanceId === client.providerId))?.models).find(entry => entry.slug === modelId);
  // A provider-native subagent thread mounts no composer: the bar stands alone, without notices.
  const subagentFrame = layoutFrames(client).subagent, bar = subagentBar(client, now, Array.isArray(subagentFrame) ? Number(subagentFrame[2]) || 0 : 0);
  // Several models for a new thread: the trigger names them (allModelNames), with the picker's shortcut.
  const fan = fanoutView(client), shortcut = chordGlyphs(commandChords(client.config, 'modelPicker.toggle', 'Meta+Shift+M', false, { terminalOpen: terminalOpen(client) }).split(' ')[0] ?? '');
  if (fan.fanout) view.modelTip = shortcut ? `${fan.fanoutAria} · ${shortcut}` : fan.fanoutAria;
  if (bar.subagent) notices.length = 0;
  return atRootFontSize({ ...view, ...action, ...compact, ...fan, attach: !bar.subagent && attachOffered(client, question), ...providerControl(client), ...queue, ...bar, ...contextMeter(client, str(model?.name, modelId)), ...frameTops(layoutFrames(client)),
    // composer-editor-intent.ts; the focused composer's key action takes these (app.contract composerKeyEvent). A focused
    // Diff (or Files preview) comment draft keeps ⌘↩ (DiffCommentAnnotation's isCommentSubmitShortcut), and Annotate's
    // editor too (browser-annotation.ts): the set is empty, or without ⌘↩, then.
    sendChords: draftHoldsCommandEnter(client) || fileDraftHoldsCommandEnter(client) ? '' : sendChordsWhileAnnotating(sendChords(client.config, phase === 'running', !client.threadId, terminalOpen(client), composerRequiresModifier(client.local.deviceSettings.sendShortcut, client.draft)), annotatingOpenThread(client)),
    sendCommandChords: sendCommandChords(client.config, phase === 'running', !client.threadId, terminalOpen(client), composerRequiresModifier(client.local.deviceSettings.sendShortcut, client.draft)),
    // TooltipPopup: 12pt text inset 8pt plus its 1pt border, for the window-edge shift.
    sendTipWidth: Math.ceil(measured(client.presentation, action.sendTooltip, 12, 400) + 18),
    // Menu widths from measured texts (composer-menus.ts); Run on's labels join the probes, and the texts the notices
    // and the timeline's end lines measure by.
    ...composerMenus(client.presentation, view, runOnLabels(environmentView(client)),
      [probe(action.sendTooltip, 12, 400), probe(UNAVAILABLE_BADGE, 10, 500), ...notices.filter(measurable).flatMap(notice => descriptionTexts(notice).map(([text, weight]) => probe(text, 12, weight))), ...stripLabelTexts(client).map(text => probe(text, 12, 400)), ...notices.map(notice => probe(notice.title, 12, 500)), ...threadNowProbes(client)]),
    stripCompact: stripLabelsCompact(client, rootFontSize),
    // ScrollToEndButton's summary: what waits at the end of the timeline while it is out of view.
    endSummary: endSummary(client),
    // Images taken into the draft and still being prepared: their tiles show at once, disabled (preparingComposerImages).
    preparingImages: preparingImages(client),
    // The draft images' uploads, which start as each joins the draft (composer-uploads.ts). Send and Return wait for them
    // (attachmentUploadBlockReason); a send that goes anyway, an annotation's, is kept until they land, and the root sends
    // the first kept one that can go (`sendResume`).
    uploads: uploadTable(client), uploadBlock: uploadBlock(client), sendResume: readySend(client),
    // The shown approval's reason and why full access still asked (ComposerPendingApprovalPanel).
    approvalReason: requests.approvals[0]?.reason ?? '', approvalWhy: requests.approvals[0]?.whyAsked ?? '',
    notices, resetConfirm: bar.subagent ? '' : usageConfirm(client), ...usageResetClock(client, now) }, rootFontSize);
}

/**
 * ComposerBannerStack NoticeText (T3 Code 74541bab17): whether a notice's plain title is cut off, from its natural width
 * (its probe, scaled as withNoticeDetails scales the description's) against the content row it shares with the
 * description, which shrinks to nothing first, and the description's own details button while that shows. The title's
 * details button never counts against the title, so showing it cannot keep itself shown.
 */
export function noticeTitleDetails(presentation: Obj, notice: { id: string; title: string; tip: string; description: string; redact: string; segments: unknown[]; machines: unknown[]; details?: boolean }, rootFontSize = 16): boolean {
  if (notice.tip || notice.machines.length > 0 || !notice.title) return false;
  const content = anchorFrame(presentation, `notice-content:${notice.id}`);
  const probed = anchorFrame(presentation, probe(notice.title, 12, 500).key);
  if (!content || !probed) return false;
  const scale = rootFontSize / 16, described = !!(notice.description || notice.redact || notice.segments.length);
  const after = (described ? 4 : 0) + (notice.details ? 28 : 0);
  return probed[1] * scale > content[1] - after * scale + 0.5;
}

/** The Run on rows the menus measure, the machines "Add to another machine" lists among them. */
function runOnLabels(view: { envOptions: { label: string }[]; envAdd: { label: string }[] }): string[] {
  return [...view.envOptions, ...view.envAdd].map(option => option.label);
}
