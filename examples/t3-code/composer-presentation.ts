// The composer's toolbar, menus and placeholder, projected the way T3's
// ChatComposer.tsx, TraitsPicker.tsx and runtimeModeConfig.ts do. Contract
// renders the rows; nothing here mutates the client.
import { arr, obj, str, type Obj } from './domain';
import { requestPresentation } from './requests';
import type { T3Client } from './client';
import { footerLayout, traitsDisplay, primaryAction, composerNotices, syncStatus, tasksProgress, providerControl, contextMeter } from './composer-controls-view';
import { planFollowUp, noteNow } from './composer-controls';
import { queuedView } from './composer-controls-queue';
import { subagentBar } from './composer-controls-subagent';
import { attachOffered } from './composer-controls-attach';
import { fanoutView } from './r3-composer-controls-fanout';
import { chordGlyphs, optionValue, reportedSelection, resolvedCurrent, triggerModelName, type Selection } from './r3-composer-controls-model';
import { sendChords } from './composer-editor-intent';
import { terminalOpen } from './terminal-drawer-view'; // terminal-layout: ChatComposer passes the real terminalOpen
import { measuredLabels } from './r5-composer-measure';
import { composerMenus, effortMenuWidth, measured, probe } from './r5-composer-menus';
import { environmentView } from './r4-git-env';
import { ULTRATHINK_LOCKED_MESSAGE, ultrathinkTraits, withImplicitFastModeDefault } from './composer-provider-state'; // composer-fidelity G9
import { ultrathinkFrame } from './composer-ultrathink';
import { overflowMenu } from './composer-overflow'; // composer-fidelity G11
import type { FooterSteps } from './composer-controls-view';

/** runtimeModeConfig.ts: label, description and lucide icon per mode. */
export const runtimeModes = [
  { mode: 'approval-required', label: 'Supervised', description: 'Ask before commands and file changes.', icon: 'lock' },
  { mode: 'auto-accept-edits', label: 'Auto-accept edits', description: 'Auto-approve edits, ask before other actions.', icon: 'pen-line' },
  { mode: 'auto', label: 'Auto', description: 'Supported providers approve routine actions; others still ask.', icon: 'sparkles' },
  { mode: 'full-access', label: 'Full access', description: 'Allow commands and edits without prompts.', icon: 'lock-open' },
];
const SAVED_OPTION_LABELS: Record<string, string> = { agent: 'Agent', effort: 'Effort', reasoningEffort: 'Reasoning effort', variant: 'Reasoning' };

type ComposerSource = {
  config: Obj; projection: Obj; presentation: Obj; providerId: string; modelId: string; modelOptions: Obj[];
  runtimeMode: string; interactionMode: string; threadId: string; projectId: string; connection: string;
  local: { drafts: Record<string, string>; deviceSettings: { planModeEnabled: boolean } };
  /** The composer's draft (T3Client.draft); the ultrathink flow reads it. */
  draft?: string;
};

/**
 * derivePhase(deriveThreadRuntime(...)): the run that owns live work (preparing,
 * starting, running, waiting), else the newest run not held in the queue; a
 * newer queued run never hides an older executing one (deriveThreadActivityRun).
 * "disconnected" while the thread has no such run and no provider thread.
 */
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

/** ChatComposer.tsx placeholder chain (the served 0.0.46 build). */
export function composerPlaceholder(input: { connected: boolean; approval: boolean; question: boolean; choiceOnly: boolean;
  projectRequired: boolean; providerUnavailable: boolean; phase: string; planReady?: boolean }): string {
  if (input.approval) return 'Resolve this approval request to continue';
  if (input.question) return input.choiceOnly ? 'Choose an option above' : 'Type your own answer, or leave this blank to use the selected option';
  if (input.planReady) return 'Add feedback to refine the plan, or leave this blank to implement it';
  if (!input.connected) return 'Connect to your T3 server to send a message';
  if (input.projectRequired) return 'Choose a project above to start a thread';
  if (input.providerUnavailable) return 'Enable a provider in Settings to send a message';
  return input.phase === 'disconnected' ? 'Ask for changes, send follow-ups, or attach images'
    : 'Ask anything, @tag files/folders, $use skills, or / for commands';
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
    // composer-fidelity G9: a prompt that controls the primary effort checks "ultrathink"; "ultrathink" in its body locks the rows.
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

/**
 * Trigger offsets reported by the app module (T3Composer) relative to the
 * toolbar row; menus open above the trigger, start-aligned, and flip to end
 * alignment when the viewport cannot hold them (base-ui collision padding 5).
 */
export function anchorsFrom(presentation: Obj) {
  const anchors = obj(presentation.anchors);
  const box = (name: string) => { const value = Array.isArray(anchors[name]) ? anchors[name] as unknown[] : []; return { x: Number(value[0]) || 0, width: Number(value[1]) || 0 }; };
  return { traits: box('traits'), runtime: box('runtime'), controls: box('controls'), implement: box('implement'), meter: box('meter'), actions: box('actions'), more: box('more') };
}

// r5-composer: the last footer layout per client, for resolveRestingComposerControlsLayout's promotion slack.
const footerSteps = new WeakMap<object, FooterSteps>();
export function composerView(client: ComposerSource, requests: { approval: boolean; question: boolean; choiceOnly: boolean; planReady?: boolean; terminalOpen?: boolean }) {
  const keyContext = { terminalOpen: requests.terminalOpen === true }; // ChatComposer: the real terminalOpen, terminalFocus false
  const providers = arr(client.config.providers);
  const provider = providers.find(entry => entry.instanceId === client.providerId);
  const model = arr(provider?.models).find(entry => entry.slug === client.modelId);
  const descriptors = arr(obj(model?.capabilities).optionDescriptors);
  // The composer's selection and what the active provider thread reports it runs (display only, never dispatched).
  const selection = client.providerId && client.modelId ? { instanceId: client.providerId, model: client.modelId, options: client.modelOptions } : null;
  const reported = client.threadId ? reportedSelection(client.projection) : null;
  // composer-fidelity G9: the traits show the implicit Fast default (Normal) and the prompt's ultrathink (composerProviderState.tsx).
  const shown = withImplicitFastModeDefault(descriptors, client.modelOptions) ?? [], prompt = client.draft ?? '';
  const ultra = ultrathinkTraits(descriptors, prompt);
  const traits = traitsMenu(descriptors, shown, selection, reported, ultra);
  const supported = Array.isArray(provider?.supportedRuntimeModes) ? (provider!.supportedRuntimeModes as unknown[]).map(value => str(value)) : [];
  const runtimes = runtimeModes.filter(option => !supported.length || supported.includes(option.mode));
  const runtime = runtimes.find(option => option.mode === client.runtimeMode) ?? runtimes[0] ?? runtimeModes[0]!;
  const planVisible = client.local.deviceSettings.planModeEnabled && !!provider && provider.showInteractionModeToggle !== false;
  const anchors = anchorsFrom(client.presentation);
  const display = traitsDisplay(str(provider?.driver), descriptors, shown, selection, reported, ultra);
  // getTriggerDisplayModelName; the tooltip adds the picker's shortcut (ProviderModelPicker triggerTooltipContent).
  const modelTitle = model ? triggerModelName(model) : client.modelId || 'Choose model', modelShortcut = chordGlyphs(commandChords(client.config, 'modelPicker.toggle', 'Meta+Shift+M', false, keyContext).split(' ')[0] ?? '');
  const { steps, ...layout } = footerLayout({ model: modelTitle, traits: display.label, traitsIcon: !!display.speed,
    runtime: runtime.label, plan: planVisible ? (client.interactionMode === 'plan' ? 'Plan' : 'Build') : '', host: anchors.controls.width,
    measure: measuredLabels(client.presentation), previous: footerSteps.get(client) });
  footerSteps.set(client, steps);
  // composer-fidelity G11: the "More composer controls" rows for the expanded and the resting footer.
  const runtimeRows = runtimes.map(option => ({ mode: option.mode, label: option.label, selected: option === runtime }));
  const more = overflowMenu({ traits: traits.items, traitsHidden: layout.traitsOverflow, modeHidden: layout.modeOverflow, planVisible, interactionMode: client.interactionMode, runtimes: runtimeRows });
  const restingMore = overflowMenu({ traits: traits.items, traitsHidden: layout.restingTraitsOverflow, modeHidden: layout.restingModeOverflow, planVisible, interactionMode: client.interactionMode, runtimes: runtimeRows });
  return {
    placeholder: composerPlaceholder({ connected: client.connection === 'connected', ...requests, planReady: !!requests.planReady,
      projectRequired: !client.projectId, providerUnavailable: !providers.some(entry => entry.enabled === true && entry.status !== 'disabled'),
      phase: client.threadId ? threadPhase(client.projection) : 'disconnected' }),
    modelTip: modelShortcut ? `${modelTitle}${model?.isUnavailable === true ? ' (Unavailable)' : ''} · ${modelShortcut}` : `${modelTitle}${model?.isUnavailable === true ? ' (Unavailable)' : ''}`,
    traitsLabel: display.label, traitsSpeed: display.speed, traitsAria: display.speed ? `${display.label}, ${display.speed === 'ultrafast' ? 'Ultrafast' : 'Fast'} mode on` : display.label,
    ...layout, traitsCount: traits.count, traitsSelected: traits.selected, traits: traits.items,
    runtimeMode: runtime.mode, runtimeLabel: runtime.label, runtimeIcon: runtime.icon, runtimeDescription: runtime.description,
    runtimeSelected: Math.max(0, runtimes.indexOf(runtime)), runtimeCount: runtimes.length,
    runtimes: runtimes.map((option, index) => ({ ...option, selected: option === runtime, index })),
    planVisible, planActive: planVisible && client.interactionMode === 'plan',
    ultrathink: ultrathinkFrame(client, prompt),
    more: more.items, moreCount: more.count, restingMore: restingMore.items, restingMoreCount: restingMore.count,
    moreMenuWidth: effortMenuWidth(client.presentation, more.items), restingMoreMenuWidth: effortMenuWidth(client.presentation, restingMore.items), // composer-fidelity G9: the spectrum ring and the model icon's chroma
    traitsX: anchors.traits.x, traitsWidth: anchors.traits.width, runtimeX: anchors.runtime.x, runtimeWidth: anchors.runtime.width, moreX: anchors.more.x, moreWidth: anchors.more.width,
    keyEffort: commandChords(client.config, 'composer.effort', 'Meta+Shift+E', false, keyContext), keyMode: commandChords(client.config, 'composer.mode', 'Meta+Shift+A', false, keyContext),
  };
}

/** Window-space tops of the composer's popover anchors (T3ComposerFrames.swift); 0 until measured. */
export function frameTops(presentation: Obj) {
  const frames = obj(presentation.frames);
  const top = (name: string) => { const value = Array.isArray(frames[name]) ? frames[name] as unknown[] : []; return Number(value[1]) || 0; };
  const strip = Array.isArray(frames.strip) ? frames.strip as unknown[] : [];
  const toolbar = Array.isArray(frames.toolbar) ? frames.toolbar as unknown[] : [];
  return { toolbarTop: top('toolbar'), toolbarRight: (Number(toolbar[0]) || 0) + (Number(toolbar[2]) || 0), stripTop: top('strip'), stripLeft: Number(strip[0]) || 0, stripRight: (Number(strip[0]) || 0) + (Number(strip[2]) || 0) };
}

/** The snapshot's `composer` row: request state comes from the requests projection. */
export function composerSnapshot(client: T3Client, now = 0) {
  noteNow(client, now);
  const requests = requestPresentation(client);
  const question = requests.questions[0];
  const phase = client.threadId ? threadPhase(client.projection) : 'disconnected';
  const view = composerView(client, { approval: requests.approvals.length > 0, question: !!question,
    choiceOnly: !!question && !question.customAllowed, planReady: !!planFollowUp(client) && !question, terminalOpen: terminalOpen(client) });
  const tasks = tasksProgress(client), sync = syncStatus(client), anchors = anchorsFrom(client.presentation);
  // The activity row (sync, else the running turn's tasks unless a request blocks the drawer) leads the stack.
  const activity = sync ? 'sync' : tasks.tasksTotal > 0 && !requests.approvals.length && !question ? 'tasks' : '';
  const notices = composerNotices(client, now).map((notice, index) => ({ ...notice, front: !activity && index === 0 }));
  const queue = queuedView(client), action = primaryAction(client, phase);
  if (queue.queueEditing) Object.assign(action, { sendLabel: 'Update queued message', sendIcon: 'check', sendTooltip: 'Update queued message', sendRunning: true });
  const model = arr(arr(client.config.providers).find(entry => entry.instanceId === client.providerId)?.models).find(entry => entry.slug === client.modelId);
  // A provider-native subagent thread mounts no composer: the bar stands alone, without its dock.
  const bar = subagentBar(client, now);
  // Several models for a new thread: the trigger names them (allModelNames), with the picker's shortcut.
  const fan = fanoutView(client), shortcut = chordGlyphs(commandChords(client.config, 'modelPicker.toggle', 'Meta+Shift+M', false, { terminalOpen: terminalOpen(client) }).split(' ')[0] ?? '');
  if (fan.fanout) view.modelTip = shortcut ? `${fan.fanoutAria} · ${shortcut}` : fan.fanoutAria;
  if (bar.subagent) { notices.length = 0; queue.queued = []; }
  return { ...view, ...action, ...fan, attach: !bar.subagent && !requests.approvals.length && attachOffered(client, question), ...providerControl(client), ...tasks, ...queue, ...bar, ...contextMeter(client, str(model?.name, client.modelId)), meterX: anchors.meter.x, meterWidth: anchors.meter.width, actionsX: anchors.actions.x, ...frameTops(client.presentation),
    sendChords: sendChords(client.config, phase === 'running', !client.threadId, terminalOpen(client)), // composer-editor-intent.ts
    // TooltipPopup: 12pt text inset 8pt plus its 1pt border, for the window-edge shift.
    sendTipWidth: Math.ceil(measured(client.presentation, action.sendTooltip, 12, 400) + 18),
    // r5-composer: menu widths from measured texts (r5-composer-menus.ts); Run on's labels join the probes.
    ...composerMenus(client.presentation, view, environmentView(client).envOptions.map(option => option.label), [probe(action.sendTooltip, 12, 400)]),
    syncStatus: bar.subagent ? '' : sync, activity: bar.subagent ? '' : activity, notices, implementX: anchors.implement.x, implementWidth: anchors.implement.width };
}
