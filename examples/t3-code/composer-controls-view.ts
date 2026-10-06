// What the composer footer and its docked banners show (lane composer-controls),
// adapted from T3 Code (MIT); see LICENSE-T3. Sources: ComposerPrimaryActions.tsx,
// ChatComposer.tsx (footer blocks, provider-unavailable control, activity row),
// composerFooterLayout.ts (icon-only before overflow), TraitsPicker.tsx
// (buildTraitsTriggerDisplay), ChatView.tsx (woke/parked/background banners,
// tasks progress), threadSync.ts and ComposerTasksBadge.tsx.
import { projectCloneBlock, projectCloneNotice } from './project-clones-live';
import { arr, obj, str, num, type Obj } from './domain';
import { activeRun, providerAvailable } from './protocol';
import type { T3Client } from './client';
import { followUpBehavior, planFollowUp, resolveDispatchMode, resumeState } from './composer-controls';
import { wokeAt, wokeWatermark } from './composer-controls-commands';
import { usageNotices } from './composer-controls-usage';
import { subagentTitle } from './timeline-events';
import { compactBlocked, resumeCompaction } from './r3-composer-controls-resume';
import { chordGlyphs, optionLabel, resolvedCurrent, type Selection } from './r3-composer-controls-model';
import { commandChords } from './composer-presentation';
import { threadWorktreeSetup } from './timeline-worktree';
import { machineChanging } from './r12-threads-scratch'; // r12-threads: isEnvironmentChanging (c47f4263f9)
import { labelWidth, type Measure, type ProbeKind } from './r5-composer-measure';

// SF Pro advances (AppKit, ASCII 32–126) for the toolbar's two label styles:
// 14pt medium ("sm" controls) and 12pt regular (the resting "xs" controls).
const M14 = [3.67,4.44,7.0,8.88,8.88,13.37,9.98,4.27,5.44,5.44,6.52,8.88,4.27,6.52,4.27,4.27,8.95,6.59,8.5,8.84,9.08,8.74,9.0,8.0,9.07,9.0,4.27,4.27,8.88,8.88,8.88,7.26,12.75,9.58,9.24,10.0,10.13,8.32,7.99,10.37,10.45,3.86,7.71,9.33,7.94,12.25,10.37,10.73,8.94,10.73,9.21,8.96,8.86,10.29,9.53,13.61,9.61,9.29,9.18,5.44,4.27,5.44,8.88,8.24,6.85,7.77,8.64,7.83,8.64,8.0,5.15,8.57,8.31,3.51,3.51,7.75,3.61,12.35,8.24,8.27,8.58,8.58,5.46,7.4,5.19,8.24,7.65,11.06,7.48,7.73,7.54,5.44,3.68,5.44,8.88];
const R12 = [3.38,3.73,5.73,7.56,7.56,11.1,8.54,3.56,4.58,4.58,5.66,7.56,3.56,5.66,3.56,3.66,7.56,5.57,7.24,7.52,7.72,7.42,7.64,6.83,7.66,7.64,3.56,3.56,7.56,7.56,7.56,6.15,11.02,8.09,7.89,8.59,8.72,7.15,6.87,8.96,8.91,3.21,6.46,7.9,6.81,10.49,8.91,9.26,7.62,9.26,7.84,7.65,7.61,8.85,8.09,11.61,8.14,7.86,7.94,4.58,3.66,4.58,7.56,7.0,6.0,6.62,7.37,6.71,7.37,6.86,4.34,7.31,7.06,2.96,2.96,6.52,3.04,10.44,7.0,7.09,7.32,7.31,4.57,6.28,4.36,7.0,6.5,9.29,6.29,6.52,6.47,4.58,3.11,4.58,7.56];
/** Label width without kerning (a point or two wide, so labels compact a little early, never clip). */
export function textWidth(text: string, small = false): number {
  const table = small ? R12 : M14, fallback = small ? 7.4 : 8.6;
  let width = 0;
  for (const char of text) { const code = char.codePointAt(0)!; width += code >= 32 && code <= 126 ? table[code - 32]! : char === '·' ? (small ? 3.6 : 4.3) : fallback; }
  return width;
}

/**
 * resolveRestingComposerControlsLayout without the overflow menu: trailing
 * blocks lose their labels first (mode, then traits); only then does the model
 * label shrink. Labels are measured (r5-composer-measure.ts); the rest is the toolbar's geometry.
 */
export function footerLayout(input: { model: string; traits: string; traitsIcon: boolean; runtime: string; plan: string; host: number; measure?: Measure; previous?: { sm: number; xs: number } }) {
  // r5-composer: label widths come from laid-out probes (r5-composer-measure.ts); the estimate stands in until then.
  const width = (text: string, small: boolean, kind: ProbeKind) => labelWidth(text, small, kind, input.measure, textWidth);
  const steps = (small: boolean) => {
    // r5-composer: block widths from the toolbar's own geometry (composer-controls.contract), matching the
    // reference's measured blocks: a block is its separator (1pt + 2pt margins, then the row gap: 9 at "sm",
    // 5 in the resting row, which has no gap) and its trigger. "sm" triggers pad 11+11 with 6pt gaps, 16pt
    // icons and a 10pt chevron; "xs" ones pad 7+7 with 4pt gaps, 12pt icons and an 8pt chevron.
    // The picker sits at margin-left -10 and, resting, at most 168pt wide (ProviderModelPicker max-w-42).
    const picker = small ? Math.min(158, 34 + width(input.model, true, 'model')) : 50 + width(input.model, false, 'model');
    const separator = small ? 5 : 9;
    const traits = !input.traits && !input.traitsIcon ? 0
      : separator + (small ? 26 : 38) + (input.traitsIcon ? (small ? 16 : 22) : 0) + (input.traits ? width(input.traits, small, 'traits') : 0);
    const traitsIcon = !traits ? 0 : separator + (small ? 38 : 54);
    // The plan toggle's glyph: PencilRuler 16pt while planning, Bot 18pt otherwise ("sm"); 12pt resting.
    const planGlyph = small ? 12 : input.plan === 'Plan' ? 16 : 18;
    const plan = input.plan ? separator + (small ? 14 : 22) + planGlyph + (small ? 4 : 6) + width(input.plan, small, 'plan') : 0;
    const mode = separator + (small ? 42 : 60) + width(input.runtime, small, 'runtime') + plan;
    const modeIcon = separator + (small ? 38 : 54) + (input.plan ? separator + (small ? 14 : 22) + planGlyph : 0);
    const gap = small ? 0 : 4;
    const total = (iconMode: boolean, iconTraits: boolean) => picker + (traits ? gap + (iconTraits ? traitsIcon : traits) : 0) + gap + (iconMode ? modeIcon : mode);
    // The reference's expanded footer has no gap before its actions (gap-2 sm:gap-0), so its controls
    // host is 8pt wider than this footer's controls row (gap=8); the resting row is measured as it is.
    const host = input.host > 0 ? input.host + (small ? 0 : 8) : 0;
    // resolveRestingComposerControlsLayout: trailing blocks drop their labels one step at a time; a
    // promotion back to labels needs a point of slack (RESTING_CONTROLS_SLACK_PX) over the last layout.
    const previous = (small ? input.previous?.xs : input.previous?.sm) ?? 0;
    const limit = (step: number) => host - (step < previous ? 1 : 0);
    const iconMode = host > 0 && total(false, false) > limit(0);
    const iconTraits = iconMode && !!traits && total(true, false) > limit(1);
    return { iconMode, iconTraits };
  };
  const sm = steps(false), xs = steps(true);
  return { runtimeIconOnly: sm.iconMode, traitsIconOnly: sm.iconTraits, restingRuntimeIconOnly: xs.iconMode, restingTraitsIconOnly: xs.iconTraits };
}

/** buildTraitsTriggerDisplay: speed traits become a bolt (two for Ultrafast); booleans read "<label> On|Off". */
export function traitsDisplay(driver: string, descriptors: Obj[], selections: Obj[], selection: Selection | null = null, reported: Selection | null = null) {
  let speed = '', fallback = '';
  const labels: string[] = [];
  const current = (descriptor: Obj) => resolvedCurrent(descriptor, selections) ?? arr(descriptor.options).find(option => option.isDefault === true)?.id;
  for (const descriptor of descriptors) {
    if (descriptor.id === 'fastMode' && descriptor.type === 'boolean') {
      speed = current(descriptor) === true ? 'fast' : ''; fallback = speed ? 'Fast' : 'Normal'; continue;
    }
    if (driver === 'codex' && descriptor.id === 'serviceTier' && descriptor.type === 'select') {
      const value = current(descriptor), options = arr(descriptor.options);
      const fast = options.find(option => option.label === 'Fast'), ultra = options.find(option => option.label === 'Ultrafast');
      if (((fast || ultra) && value === 'default') || (fast && value === fast.id) || (ultra && value === ultra.id)) {
        speed = ultra && value === ultra.id ? 'ultrafast' : fast && value === fast.id ? 'fast' : '';
        fallback = str(options.find(option => option.id === value)?.label, 'Normal'); continue;
      }
    }
    if (descriptor.type === 'boolean') { labels.push(`${str(descriptor.label, str(descriptor.id))} ${current(descriptor) === true ? 'On' : 'Off'}`); continue; }
    if (descriptor.type !== 'select') continue;
    // getProviderOptionCurrentLabel: a provider-reported value labels an option the user left unset.
    const label = optionLabel(descriptor, resolvedCurrent(descriptor, selections), selection, reported);
    if (label) labels.push(label);
  }
  if (!labels.length && fallback) return { label: fallback, speed: '' };
  return { label: labels.join(' · '), speed };
}

// ── Primary action ─────────────────────────────────────────────────────────

export function primaryAction(client: T3Client, phase: string) {
  const running = phase === 'running', connecting = phase === 'connecting';
  const followUp = followUpBehavior(client);
  const modifiers = str(obj(client.presentation).modifiers);
  const held = /\b(meta|control)\b/.test(modifiers);
  const queueing = resolveDispatchMode(running, followUp, held) === 'queue';
  const resume = client.threadId ? resumeState(client.projection) : { runId: '', heldQueue: false };
  const unavailable = client.connection !== 'connected';
  const sending = client.busy && !!client.pending && str(client.pending.payload.type) === 'message.dispatch';
  // worktreeSetupBlocksSend (ChatView): a new worktree thread holds sends until its agent starts (lane r4-git).
  const preparing = !!client.threadId && threadWorktreeSetup(client).preparing;
  const status = unavailable ? 'Environment disconnected' : machineChanging(client) ? 'Preparing machine' : preparing ? 'Preparing worktree' : projectCloneBlock(client) || (connecting ? 'Connecting' : sending ? 'Submitting message' : '');
  const plan = planFollowUp(client);
  const alternate = followUp === 'queue' ? 'steer' : 'queue';
  // alternateShortcutLabel: composer.sendAlternate's effective binding, as formatShortcutLabel prints it (⌘Enter).
  const alternateKey = chordGlyphs(commandChords(client.config, 'composer.sendAlternate', 'Meta+Enter', false, { turnRunning: true }).split(' ')[0] ?? '');
  return {
    sendRunning: running,
    sendLabel: queueing ? 'Queue message' : running ? 'Steer message' : 'Submit message',
    sendIcon: queueing ? 'list-plus' : running ? 'corner-up-right' : 'arrow',
    sendStatus: status, sendSpinner: connecting || sending,
    sendTooltip: status || (running ? `Click to ${followUp}, Ctrl/⌘-click${alternateKey ? ` or ${alternateKey}` : ''} to ${alternate}` : 'Submit message'),
    canResume: !!resume.runId || resume.heldQueue,
    planReady: !!plan, planTitle: plan?.title ?? '',
  };
}

// ── Docked banners ─────────────────────────────────────────────────────────

export type ComposerNotice = { id: string; variant: string; icon: string; title: string; description: string;
  action: string; actionLabel: string; action2: string; action2Label: string; dismiss: string; dismissLabel: string; priority: number; lines: string[];
  /** Why the action is disabled (its tooltip), '' when it is enabled; the id the dismiss command receives. */
  actionReason: string; dismissId: string; segments: NoticeSegment[] };
const notice = (value: Partial<ComposerNotice> & { id: string; title: string }): ComposerNotice => ({ variant: 'info', icon: '', description: '',
  action: '', actionLabel: '', action2: '', action2Label: '', dismiss: '', dismissLabel: '', priority: 2, lines: [], actionReason: '', dismissId: '', segments: [], ...value });

const BACKGROUND_KINDS: Record<string, { order: number; singular: string; plural: string }> = {
  subagent: { order: 0, singular: 'subagent', plural: 'subagents' }, command: { order: 1, singular: 'command', plural: 'commands' },
  monitor: { order: 2, singular: 'monitor', plural: 'monitors' }, background_task: { order: 3, singular: 'background task', plural: 'background tasks' } };
const joinWithAnd = (parts: string[]) => parts.length <= 1 ? parts.join('') : `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
/**
 * presentPendingBackgroundWork: what a thread with no active turn still runs.
 * Subagent descriptions read as display names; only work that wakes the agent
 * (anything but a command, backgroundWorkHoldsCompletion) is "waiting".
 */
export type NoticeSegment = { key: string; label: string; threadId: string; last: boolean };
export function presentBackgroundWork(tasks: Obj[]): { title: string; description: string; waiting: boolean; segments: NoticeSegment[] } | null {
  if (!tasks.length) return null;
  const waiting = tasks.some(task => task.kind !== 'command');
  const kind = (task: Obj) => BACKGROUND_KINDS[str(task.kind)] ?? BACKGROUND_KINDS.background_task!;
  const items = tasks.map(task => {
    const description = str(task.description).trim(), label = task.kind === 'subagent' ? subagentTitle(description).trim() : description;
    return { kind: str(task.kind), meta: kind(task), label: label || kind(task).singular, child: task.kind === 'subagent' ? str(task.childThreadId) : '' };
  }).sort((a, b) => a.meta.order - b.meta.order);
  // The description lists every item; a subagent with its own thread is an inline "Open subagent …" link.
  const segments = (list: typeof items) => list.map((item, index) => ({ key: `${index}:${item.label}`, label: item.label, threadId: item.child, last: index === list.length - 1 }));
  if (items.length === 1) {
    const only = items[0]!, noun = only.meta.singular, named = only.label !== noun;
    const title = waiting ? (named ? `Waiting on ${noun} ${only.label}` : `Waiting on a ${noun}`) : named ? `Running: ${only.label}` : `Running a ${noun}`;
    // A single named item is already in the title.
    return { title, description: only.child ? only.label : '', waiting, segments: only.child ? segments(items) : [] };
  }
  const counts = new Map<string, number>();
  for (const item of items) counts.set(item.kind, (counts.get(item.kind) ?? 0) + 1);
  const groups = [...counts].map(([key, count]) => { const meta = BACKGROUND_KINDS[key] ?? BACKGROUND_KINDS.background_task!; return `${count} ${count === 1 ? meta.singular : meta.plural}`; });
  return { title: `${waiting ? 'Waiting on' : 'Running'} ${joinWithAnd(groups)}`, description: items.map(item => item.label).join(', '), waiting,
    segments: items.some(item => item.child) ? segments(items) : [] };
}
function backgroundWork(client: T3Client): ComposerNotice | null {
  if (activeRun(client.projection)) return null;
  const shell = client.shell.threads.find(thread => thread.id === client.threadId);
  const presented = presentBackgroundWork(arr(shell?.pendingBackgroundTasks));
  // A dev server can run for hours after the agent is done, so only work that will wake the agent pulses.
  return presented ? notice({ id: `background-work:${client.threadId}`, variant: 'default', icon: presented.waiting ? 'dot' : 'dot-static', priority: 0, title: presented.title,
    description: presented.description, segments: presented.segments, action: 'cc:stop-background', actionLabel: 'Stop' }) : null;
}

export function composerNotices(client: T3Client, now: number): ComposerNotice[] {
  // ChatView projectCloneBannerItem: a project added by cloning shows its clone where its draft is (project-clones-live.ts).
  const clone = projectCloneNotice(client);
  const cloneItem = clone ? [notice({ ...clone, icon: 'download', priority: clone.variant === 'info' ? 0 : 2 })] : [];
  if (!client.threadId) return [...usageNotices(client, now), ...cloneItem];
  const shell = client.shell.threads.find(thread => thread.id === client.threadId) ?? {};
  const capabilities = obj(obj(client.config.environment).capabilities);
  // ChatView composerBannerItems order: limit recovery, usage limits, background work, woke, parked.
  const items: ComposerNotice[] = [...usageNotices(client, now), ...cloneItem];
  const background = backgroundWork(client);
  if (background) items.push(background);
  // resumeCompactionBannerItem: an idle Claude session offers to compact before resuming.
  const resume = resumeCompaction(client, now, contextSnapshot(client.projection));
  if (resume) {
    const reason = compactBlocked(client);
    items.push(notice({ id: `resume-compaction:${resume.key}`, icon: 'minimize', title: 'Resume with less context', description: `${formatTokens(resume.used)} tokens from earlier`,
      action: 'cc:compact', actionLabel: 'Compact', actionReason: reason, dismiss: `cclocal:resume-compaction-dismiss`, dismissId: resume.key, dismissLabel: 'Keep full history' }));
  }
  // Snooze is derived: a passed wake time stops classifying as snoozed and feeds the woke notice.
  const snoozeSupported = capabilities.threadSnooze !== false;
  const woke = snoozeSupported ? wokeAt(shell, now) : '';
  const until = Date.parse(str(shell.snoozedUntil));
  const snoozed = snoozeSupported && Number.isFinite(until) && until > now && !woke;
  const settled = capabilities.threadSettlement !== false && shell.settledOverride === 'settled';
  if (woke && !settled) {
    // The sidebar's watermark (server-owned when tracked), floored at the latest run's completion.
    const seen = Date.parse(wokeWatermark(client, shell)), completed = Date.parse(str(obj(shell.latestRun).completedAt));
    const visited = Math.max(Number.isFinite(seen) ? seen : -Infinity, Number.isFinite(completed) ? completed : -Infinity);
    if (visited < Date.parse(woke)) items.push(notice({ id: `thread-woke:${client.threadId}`, icon: 'alarm', title: 'Thread woke from snooze',
      description: 'Send a message to continue', dismiss: `cclocal:dismiss-woke`, dismissLabel: 'Dismiss Woke notification' }));
  }
  if (snoozed || settled) items.push(notice({ id: `thread-${snoozed ? 'snoozed' : 'settled'}:${client.threadId}`, icon: snoozed ? 'alarm' : 'circle-check',
    title: `This thread is ${snoozed ? 'snoozed' : 'settled'}`, description: `Send a message to ${snoozed ? 'wake' : 'unsettle'}`,
    action: snoozed ? 'cc:unsnooze' : 'unsettle', actionLabel: snoozed ? 'Wake now' : 'Un-settle' }));
  // Activity stays attached; warnings and errors order the notices behind it.
  const rank = (item: ComposerNotice) => item.priority === 0 ? 0 : item.variant === 'error' || item.variant === 'warning' ? 1 : 2;
  return items.map((item, order) => ({ item, order })).sort((a, b) => rank(a.item) - rank(b.item) || a.order - b.order).map(entry => entry.item);
}

// ── Activity row: thread sync and tasks ────────────────────────────────────

const syncSeen = new Map<T3Client, string>();
/**
 * resolveThreadSyncPhase. useDelayedStatus holds a status back 400ms so a short
 * load never flashes; data sources have no clock here, so a phase shows once a
 * second snapshot still finds it (the next event batch after it began).
 */
export function syncStatus(client: T3Client): string {
  const phase = !client.threadId || client.connection !== 'connected' || !client.shell.threads.some(thread => thread.id === client.threadId) ? ''
    : !client.thread ? 'loading' : client.ready ? '' : 'syncing';
  const key = phase ? `${client.environmentId}:${client.threadId}:${phase}` : '';
  const shown = !!key && syncSeen.get(client) === key;
  syncSeen.set(client, key);
  return shown ? (phase === 'loading' ? 'Loading messages...' : 'Syncing messages...') : '';
}

export type TaskStep = { key: string; text: string; status: string; duration: string };
function durationLabel(milliseconds: number): string {
  if (milliseconds < 1000) return `${Math.max(1, Math.round(milliseconds))}ms`;
  if (milliseconds < 60000) return `${Math.round(milliseconds / 100) / 10}s`.replace(/\.0s$/, 's');
  const seconds = Math.round(milliseconds / 1000), minutes = Math.floor(seconds / 60);
  return minutes < 60 ? `${minutes}m ${seconds % 60}s`.replace(/ 0s$/, '') : `${Math.floor(minutes / 60)}h ${minutes % 60}m`.replace(/ 0m$/, '');
}
/** deriveActivePlanState for the running turn's own todo list (activeComposerTasksProgress). */
export function tasksProgress(client: T3Client) {
  const empty = { tasksTotal: 0, tasksDone: 0, tasksCurrent: '', tasks: [] as TaskStep[] };
  const run = activeRun(client.projection);
  if (!client.threadId || !run) return empty;
  const plans = arr(client.projection.plans).filter(plan => plan.kind === 'todo_list');
  const plan = [...plans].reverse().find(candidate => candidate.runId === run.id);
  const steps = arr(plan?.steps);
  if (!plan || !steps.length) return empty;
  const status = (value: unknown) => value === 'running' || value === 'inProgress' ? 'inProgress' : value === 'completed' ? 'completed' : 'pending';
  const seen = new Map<string, number>();
  const tasks = steps.map(step => {
    const text = str(step.text), occurrence = seen.get(text) ?? 0; seen.set(text, occurrence + 1);
    const state = status(step.status);
    return { key: `${text}:${occurrence}`, text, status: state,
      duration: typeof step.durationMs === 'number' ? durationLabel(num(step.durationMs)) : state === 'inProgress' ? 'now' : '' };
  });
  const current = tasks.find(task => task.status === 'inProgress') ?? tasks.find(task => task.status === 'pending') ?? tasks[tasks.length - 1]!;
  return { tasksTotal: tasks.length, tasksDone: tasks.filter(task => task.status === 'completed').length, tasksCurrent: current.text, tasks };
}

// ── Provider-unavailable control ───────────────────────────────────────────

/** hasProviderSetup (ProviderStatusBanner.tsx): the instance can be signed in or installed from Settings. */
const hasProviderSetup = (provider: Obj) => provider.driver === 'antigravity' || obj(provider.setup).canAuthenticate === true || obj(provider.setup).canInstall === true;
/**
 * showProviderUnavailable: no selectable provider. Its setup target is the
 * instance the draft or thread asked for (resolveComposerProviderSelection's
 * unavailableProviderInstanceId), else the first that can be set up; with
 * neither the control reads "No provider available" and is disabled.
 */
export function providerControl(client: T3Client) {
  const providers = arr(client.config.providers);
  if (!client.configLive && !providers.length) return { noProvider: '', providerSetupId: '' };
  if (providers.some(provider => providerAvailable(provider))) return { noProvider: '', providerSetupId: '' };
  const settings = obj(client.config.settings), project = client.shell.projects.find(entry => entry.id === client.projectId);
  const requested = [client.providerId, str(obj(obj(client.projection.thread).modelSelection).instanceId),
    str(obj(obj(obj(settings.projectSettingsOverrides)[client.projectId]).defaultModelSelection).instanceId),
    str(obj(project?.defaultModelSelection).instanceId), str(obj(settings.defaultModelSelection).instanceId)].find(Boolean) ?? '';
  const setup = requested || str(providers.find(hasProviderSetup)?.instanceId);
  return { noProvider: setup ? 'Open provider settings' : 'No provider available', providerSetupId: setup };
}

// ── Context window meter ───────────────────────────────────────────────────

/** formatContextWindowTokens. */
export function formatTokens(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '0';
  if (value < 1_000) return `${Math.round(value)}`;
  if (value < 10_000) return `${(value / 1_000).toFixed(1).replace(/\.0$/, '')}k`;
  if (value < 1_000_000) return `${Math.round(value / 1_000)}k`;
  return `${(value / 1_000_000).toFixed(1).replace(/\.0$/, '')}m`;
}
const finite = (value: unknown): number | null => typeof value === 'number' && Number.isFinite(value) ? value : null;
/** deriveLatestContextWindowSnapshot: live turn usage, else the provider thread's report, else the last compaction. */
export function contextSnapshot(projection: Obj) {
  const turn = [...arr(projection.providerTurns)].reverse().find(entry => entry.tokenUsage !== undefined && entry.tokenUsage !== null);
  if (turn) {
    const usage = obj(turn.tokenUsage), used = Math.max(0, finite(usage.usedTokens) ?? 0), max = finite(usage.maxTokens);
    return { used, max, total: null as number | null, cost: null as Obj | null, auto: true, threshold: null as number | null, updatedAt: str(usage.updatedAt, str(turn.updatedAt)) };
  }
  const thread = arr(projection.providerThreads).find(entry => entry.id === obj(projection.thread).activeProviderThreadId);
  const report = obj(thread?.contextUsage);
  if (thread && thread.contextUsage) return { used: finite(report.usedTokens) ?? 0, max: finite(report.maxTokens), total: finite(report.totalProcessedTokens),
    cost: report.cost ? obj(report.cost) : null, auto: report.compactsAutomatically === true, threshold: finite(report.autoCompactThreshold), updatedAt: str(thread.updatedAt) };
  const compaction = [...arr(projection.visibleTurnItems)].reverse().map(row => obj(row.item)).find(item => item.type === 'compaction' && (finite(item.afterTokenCount) ?? -1) >= 0);
  return compaction ? { used: finite(compaction.afterTokenCount)!, max: null, total: finite(compaction.beforeTokenCount), cost: null, auto: true, threshold: null,
    updatedAt: str(compaction.startedAt, str(compaction.updatedAt)) } : null;
}
const percentLabel = (value: number) => value < 10 ? `${value.toFixed(1).replace(/\.0$/, '')}%` : `${Math.round(value)}%`;
/** ContextWindowMeter, shown when Settings › Context window indicator is on and usage exists. */
export function contextMeter(client: T3Client, modelName: string) {
  const empty = { contextShow: false, contextAria: '', contextPercent: '', contextOffset: 0, contextOver: false, contextUsed: '', contextMax: '',
    contextTotal: '', contextCost: '', contextNote: '', contextCompact: false };
  const settings = (client.local as { clientSettings?: Obj }).clientSettings;
  if (!client.threadId || settings?.contextWindowMeterEnabled !== true) return empty;
  const usage = contextSnapshot(client.projection);
  if (!usage) return empty;
  const fraction = usage.max !== null && usage.max > 0 ? Math.min(100, usage.used / usage.max * 100) : null;
  const normalized = Math.max(0, Math.min(100, fraction ?? 0));
  const circumference = 2 * Math.PI * 9.75;
  const provider = arr(client.config.providers).find(entry => entry.instanceId === client.providerId);
  const cost = usage.cost ? `${str(usage.cost.currency)} ${Number(usage.cost.amount).toFixed(Math.abs(Number(usage.cost.amount)) > 0 && Math.abs(Number(usage.cost.amount)) < 0.01 ? 4 : 2)}` : '';
  return { contextShow: true,
    contextAria: usage.max !== null && fraction !== null ? `Context window ${percentLabel(fraction)} used` : `Context window ${formatTokens(usage.used)} tokens used`,
    contextPercent: usage.max !== null && fraction !== null ? percentLabel(fraction) : '', contextOffset: Math.round(circumference * (1 - normalized / 100) * 100) / 100,
    contextOver: normalized > 90, contextUsed: formatTokens(usage.used), contextMax: usage.max !== null ? formatTokens(usage.max) : '',
    contextTotal: usage.total !== null && usage.total > 0 ? formatTokens(usage.total) : '', contextCost: cost,
    contextNote: !usage.auto ? '' : usage.threshold !== null && usage.threshold > 0 ? `Compacts automatically at ${usage.threshold.toLocaleString('en-US')} tokens.`
      : modelName ? `Context for ${modelName} compacts automatically when needed.` : 'Context compacts automatically when needed.',
    contextCompact: arr(provider?.slashCommands).some(command => command.name === 'compact') };
}
