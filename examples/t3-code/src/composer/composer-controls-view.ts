// What the composer footer and the notices above it show,
// adapted from T3 Code (MIT); see LICENSE-T3. Sources at 9c161462c9: ComposerPrimaryActions.tsx,
// ChatComposer.tsx (footer blocks, provider-unavailable control), composerFooterLayout.ts
// (icon-only before overflow), TraitsPicker.tsx (buildTraitsTriggerDisplay) and ChatView.tsx (the
// Compact chip); the notices follow ChatView.tsx, ChatComposer.tsx (shelfItems) and
// ComposerBannerStack.tsx at 7c3b3b3605ef, and presentPendingBackgroundWork threadExecution.ts.
import { branchMismatchNotice } from './composer-controls-branch';
import { projectCloneBlock, projectCloneNotice } from '../shell/project-clones-live';
import { systemComposerNotices } from '../connections/server-update-notices';
import { autoBalanceState } from './auto-balance';
import { autoBalanceNotices, type MachineRow } from './auto-balance-banner';
import { arr, obj, str, type Obj } from '../client/domain';
import { antigravitySendBlockReason, composerSelection } from './composer-provider-selection'; // one provider rule
import { fanoutSelections } from './composer-controls-fanout';
import type { T3Client } from '../client/client';
import { followUpBehavior, isCompacting, planFollowUp, resolveDispatchMode, resumeState, sendQueuesBehindSetup } from './composer-controls';
import { usageNotices, type UsageAccountView } from './composer-controls-usage';
import { feedbackNotices, feedbackUploading } from './composer-feedback'; // Codex /feedback
import { uploadBlock, waitingSend } from './composer-uploads'; // attachmentUploadBlockReason
import { subagentTitle } from '../timeline/timeline-events';
import { keepsFullHistory, latestTokenUsage, resumeCompactionTokens } from './composer-controls-resume';
import { pendingRequests } from './requests';
import { chordGlyphs } from './composer-controls-model';
import { commandChords } from './composer-presentation';
import { threadWorktreeSetup } from '../timeline/timeline-worktree';
import { machineChanging } from './scratch-machine'; // isEnvironmentChanging (c47f4263f9)
import { labelWidth, type Measure, type ProbeKind } from './composer-measure';
import { resolveRestingComposerControlsLayout } from './composer-resting-layout';
import { terminalOpen } from '../terminal/terminal-drawer-view'; // the real terminalOpen
import { pendingThreadImports } from './composer-thread-import';
import { PLATFORM } from '../settings/desktop-shell-favicon'; // the send tooltip's modifier (isMacPlatform)

// SF Pro advances (AppKit, ASCII 32–126) for the toolbar's two label styles:
// 14pt medium (an upper bound for the "sm" labels, which are drawn regular) and 12pt regular (the resting "xs" ones).
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
 * resolveRestingComposerControlsLayout (composer-resting-layout.ts): trailing
 * blocks lose their labels first (mode, then traits), then move into the "More
 * composer controls" menu (mode, then traits); only then does the model picker
 * shrink, and below its minimum the cluster hides. Labels are measured
 * (composer-measure.ts); the rest is the toolbar's geometry. `previous` holds
 * each size's last step (icon-only blocks plus hidden blocks) and whether the
 * cluster showed, for the promotion slack.
 */
export type FooterSteps = { sm: number; xs: number; smHidden?: boolean; xsHidden?: boolean };
export function footerLayout(input: { model: string; traits: string; traitsIcon: boolean; runtime: string; plan: string; host: number; measure?: Measure; previous?: FooterSteps;
  /** An unavailable model's badge, which follows its name inside the picker (0 without one). */
  badge?: number }) {
  // Label widths come from laid-out probes (composer-measure.ts); the estimate stands in until then.
  const width = (text: string, small: boolean, kind: ProbeKind) => labelWidth(text, small, kind, input.measure, textWidth);
  const steps = (small: boolean) => {
    // Block widths from the toolbar's own geometry (composer-controls.contract), matching the
    // reference's measured blocks. Since 3816296021 a block is its trigger alone: no separators, and
    // the controls row has no gap. "sm" triggers pad 9+9 (px-2 and the 1pt border) with 6pt gaps, 16pt
    // icons and a 10pt chevron; "xs" ones pad 7+7 with 4pt gaps, 12pt icons and an 8pt chevron.
    // The picker sits at margin-left -8 (resting -10) and, resting, at most 168pt wide (ProviderModelPicker
    // max-w-42); its minimum is the trigger's min-width (48 / 40) less that margin.
    // The badge sits after the name across the trigger's gap (4pt resting, 6pt otherwise).
    const badge = input.badge ? input.badge + (small ? 4 : 6) : 0;
    const picker = small ? Math.min(158, 34 + width(input.model, true, 'model') + badge) : 48 + width(input.model, false, 'model') + badge;
    const traits = !input.traits && !input.traitsIcon ? 0
      : (small ? 26 : 34) + (input.traitsIcon ? (small ? 16 : 22) : 0) + (input.traits ? width(input.traits, small, 'traits') : 0);
    const traitsIcon = !traits ? 0 : small ? 38 : 50;
    // The plan toggle's glyph: PencilRuler 16pt while planning, Bot 18pt otherwise ("sm"); 12pt resting.
    const planGlyph = small ? 12 : input.plan === 'Plan' ? 16 : 18;
    const plan = input.plan ? (small ? 14 : 18) + planGlyph + (small ? 4 : 6) + width(input.plan, small, 'plan') : 0;
    const mode = (small ? 42 : 56) + width(input.runtime, small, 'runtime') + plan;
    const modeIcon = (small ? 38 : 50) + (input.plan ? (small ? 14 : 18) + planGlyph : 0);
    // The reference's expanded footer has no gap before its actions (gap-2 sm:gap-0), so its controls
    // host is 8pt wider than this footer's controls row (gap=8); the resting row is measured as it is.
    const host = input.host > 0 ? input.host + (small ? 0 : 8) : 0;
    const blocks = traits ? [traits, mode] : [mode], icons = traits ? [traitsIcon, modeIcon] : [modeIcon];
    const previousStep = (small ? input.previous?.xs : input.previous?.sm) ?? 0;
    const previous = { hiddenCount: Math.max(0, previousStep - blocks.length), iconOnlyCount: Math.min(previousStep, blocks.length),
      visible: !(small ? input.previous?.xsHidden : input.previous?.smHidden) };
    // The "…" trigger: an icon-only control (16pt glyph padded 9+9, resting 12pt padded 7+7).
    const layout = host > 0 ? resolveRestingComposerControlsLayout({ gap: 0, naturalFixedWidth: picker, minimumFixedWidth: small ? 30 : 40,
      blockWidths: blocks, iconOnlyBlockWidths: icons, overflowWidth: small ? 26 : 34, hostWidth: host, previous }) : { hiddenCount: 0, iconOnlyCount: 0, visible: true };
    const iconOnly = layout.iconOnlyCount ?? 0, hidden = layout.hiddenCount;
    return { iconMode: iconOnly >= 1, iconTraits: !!traits && iconOnly >= 2, modeHidden: hidden >= 1, traitsHidden: !!traits && hidden >= 2,
      hidden: !layout.visible, step: iconOnly + hidden };
  };
  const sm = steps(false), xs = steps(true);
  return { runtimeIconOnly: sm.iconMode, traitsIconOnly: sm.iconTraits, restingRuntimeIconOnly: xs.iconMode, restingTraitsIconOnly: xs.iconTraits,
    // Blocks in the overflow menu, and the cluster hidden below the picker's minimum.
    modeOverflow: sm.modeHidden, traitsOverflow: sm.traitsHidden, restingModeOverflow: xs.modeHidden, restingTraitsOverflow: xs.traitsHidden,
    controlsHidden: sm.hidden, restingControlsHidden: xs.hidden, steps: { sm: sm.step, xs: xs.step, smHidden: sm.hidden, xsHidden: xs.hidden } as FooterSteps };
}

// traitsDisplay (buildTraitsTriggerDisplay) lives in composer-controls-model.ts, beside the option labels it reads; Settings' TraitsPicker rows share it.
export { traitsDisplay } from './composer-controls-model';

// ── Primary action ─────────────────────────────────────────────────────────

export function primaryAction(client: T3Client, phase: string) {
  const setupQueue = sendQueuesBehindSetup(client), running = phase === 'running' || setupQueue, connecting = phase === 'connecting' && !setupQueue;
  const followUp = followUpBehavior(client);
  const canSteer = !setupQueue && !isCompacting(client);
  // Send follows ⌘ or Control (useShortcutModifierState): the contract shows the held variant while one is held.
  const queueing = resolveDispatchMode(running, followUp, false, canSteer) === 'queue', queueingHeld = resolveDispatchMode(running, followUp, true, canSteer) === 'queue';
  const resume = client.threadId ? resumeState(client.projection) : { runId: '', heldQueue: false };
  const unavailable = client.connection !== 'connected';
  // A press kept until its images land is a send in flight too (useComposerSend awaits them inside onSend).
  const sending = client.busy && !!client.pending && str(client.pending.payload.type) === 'message.dispatch' || !!waitingSend(client);
  const preparing = !!client.threadId && !setupQueue && threadWorktreeSetup(client).preparing;
  // sendDisabledReason: !canOperateThread first, the provider's own block reason after the thread's.
  const chosen = fanoutSelections(client) ? null : composerSelection(client);
  const providerBlock = chosen?.provider ? antigravitySendBlockReason(chosen.provider, chosen.model) : '';
  const status = unavailable ? 'Environment disconnected' : !client.scopes.includes('orchestration:operate') ? 'This connection cannot change threads.'
    : pendingThreadImports(client) ? 'Loading thread context…' : machineChanging(client) ? 'Preparing machine' : feedbackUploading(client) ? 'Sending feedback' : preparing ? 'Preparing worktree'
      : projectCloneBlock(client) || uploadBlock(client) || providerBlock || (connecting ? 'Connecting' : sending ? 'Submitting message' : '');
  const plan = planFollowUp(client);
  const alternate = followUp === 'queue' ? 'steer' : 'queue';
  // alternateShortcutLabel: composer.sendAlternate's effective binding, as formatShortcutLabel prints it (⌘Enter).
  const alternateKey = chordGlyphs(commandChords(client.config, 'composer.sendAlternate', 'Meta+Enter', false, { turnRunning: true, terminalOpen: terminalOpen(client) }).split(' ')[0] ?? '');
  return {
    sendRunning: running,
    sendLabel: queueing ? 'Queue message' : running ? 'Steer message' : 'Submit message',
    sendIcon: queueing ? 'list-plus' : running ? 'corner-up-right' : 'arrow',
    sendHeldLabel: queueingHeld ? 'Queue message' : running ? 'Steer message' : 'Submit message',
    sendHeldIcon: queueingHeld ? 'list-plus' : running ? 'corner-up-right' : 'arrow',
    sendStatus: status, sendSpinner: connecting || sending,
    sendTooltip: status || (running && canSteer ? `Click to ${followUp}, ${/mac/i.test(PLATFORM) ? '⌘' : 'Ctrl'}-click${alternateKey ? ` or ${alternateKey}` : ''} to ${alternate}` : queueing ? 'Queue message' : 'Submit message'),
    canResume: !!resume.runId || resume.heldQueue,
    planReady: !!plan, planTitle: plan?.title ?? '',
  };
}

/**
 * The Compact chip beside Send (ComposerPrimaryActions compactTokens): a stale Claude session's token count, on
 * ("Compact 312k", the send compacts first) or off ("Keep 312k", the next send keeps the history). It hides while a
 * queued message is edited, and the contract hides it where Resume thread replaces Send. `compactWidth` is the chip's
 * width (px-2 gap-1.5, a 16pt glyph, the 14pt medium label and the count), which the resting card reserves.
 */
export function compactChip(client: T3Client, now: number, editingQueued: boolean) {
  const tokens = editingQueued ? null : resumeCompactionTokens(client, now, contextSnapshot(client.projection));
  if (tokens === null) return { compactTokens: '', compactKeepFull: false, compactTip: '', compactSend: false, compactWidth: 0 };
  const count = formatTokens(tokens), keep = keepsFullHistory(client), label = keep ? 'Keep' : 'Compact';
  const anchor = obj(client.presentation.anchors)[`compact:${label}:${count}`], width = Array.isArray(anchor) ? Number(anchor[1]) : NaN;
  return { compactTokens: count, compactKeepFull: keep, compactSend: !keep, compactWidth: Number.isFinite(width) && width > 0 ? width : Math.ceil(16 + 16 + 6 + textWidth(label) + 6 + textWidth(count)),
    compactTip: keep ? `Next send keeps all ${count} tokens.\nClick to compact first` : `Next send compacts ${count} tokens first.\nClick to keep full history` };
}

// ── Notices above the composer ─────────────────────────────────────────────

/**
 * ComposerBannerStackItem: `group` "environment" is the machine's own notice (offline, a running or failed update,
 * auto-balance), shown first; everything else needs you ("attention").
 */
export type ComposerNotice = { id: string; group: string; variant: string; icon: string; title: string; description: string;
  action: string; actionLabel: string; action2: string; action2Label: string; dismiss: string; dismissLabel: string; priority: number; lines: string[];
  /** Why the action is disabled (its tooltip), '' when it is enabled; the second action shown disabled; the ids the
   *  action and dismiss commands receive. */
  actionReason: string; action2Disabled: boolean; actionId: string; dismissId: string; segments: NoticeSegment[];
  /** The title's and the actions' tooltips, a red icon; `details`: the description is clipped (composer-presentation.ts). */
  tip: string; actionTip: string; action2Tip: string; iconTone: string; details: boolean;
  /** Auto balance: the multi-machine banner's popover rows and its title's accessible name. */
  machines: MachineRow[]; menuLabel: string;
  /** A description that is an account address (RedactedSensitiveText, then `redactAfter`), and the /usage-limits body. */
  redact: string; redactMask: string; redactAfter: string; usage: UsageAccountView[];
  /** The title is cut off (composer-presentation.ts noticeTitleDetails). */
  titleDetails?: boolean;
  /** A title in two parts: `title` a muted lead-in, then this code (the branch mismatch's branch), `tip` on it. */
  code: string };
const notice = (value: Partial<ComposerNotice> & { id: string; title: string }): ComposerNotice => ({ group: 'attention', variant: 'info', icon: '', description: '',
  action: '', actionLabel: '', action2: '', action2Label: '', dismiss: '', dismissLabel: '', priority: 2, lines: [], actionReason: '', action2Disabled: false, actionId: '', dismissId: '', segments: [],
  tip: '', actionTip: '', action2Tip: '', iconTone: '', details: false, machines: [], menuLabel: '', redact: '', redactMask: '', redactAfter: '', usage: [], code: '', ...value });

const BACKGROUND_KINDS: Record<string, { order: number; singular: string; plural: string }> = {
  subagent: { order: 0, singular: 'subagent', plural: 'subagents' }, command: { order: 1, singular: 'command', plural: 'commands' },
  monitor: { order: 2, singular: 'monitor', plural: 'monitors' }, background_task: { order: 3, singular: 'background task', plural: 'background tasks' } };
const joinWithAnd = (parts: string[]) => parts.length <= 1 ? parts.join('') : `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
/**
 * presentPendingBackgroundWork: what a thread with no active turn still runs.
 * Subagent descriptions read as display names; only work the agent waits on is
 * "waiting" (backgroundWorkHoldsCompletion): a subagent or a background task, or
 * a command or monitor the agent marked `awaited`, since either is as often a
 * dev server or its watchdog as a wait.
 */
export type NoticeSegment = { key: string; label: string; threadId: string; aria: string; last: boolean };
export function presentBackgroundWork(tasks: Obj[]): { title: string; description: string; waiting: boolean; segments: NoticeSegment[] } | null {
  if (!tasks.length) return null;
  const waiting = tasks.some(task => task.awaited === true || (task.kind !== 'command' && task.kind !== 'monitor'));
  const kind = (task: Obj) => BACKGROUND_KINDS[str(task.kind)] ?? BACKGROUND_KINDS.background_task!;
  const items = tasks.map(task => {
    const description = str(task.description).trim(), label = task.kind === 'subagent' ? subagentTitle(description).trim() : description;
    return { kind: str(task.kind), meta: kind(task), label: label || kind(task).singular, child: task.kind === 'subagent' ? str(task.childThreadId) : '' };
  }).sort((a, b) => a.meta.order - b.meta.order);
  // The description lists every item; a subagent with its own thread is an inline "Open subagent …" link.
  const segments = (list: typeof items) => list.map((item, index) => ({ key: `${index}:${item.label}`, label: item.label, threadId: item.child,
    aria: `Open subagent ${item.label}`, last: index === list.length - 1 }));
  if (items.length === 1) {
    const only = items[0]!, noun = only.meta.singular, named = only.label !== noun;
    const title = waiting ? (named ? `${only.kind === 'monitor' ? '' : `Waiting on ${noun} `}${only.label}` : `Waiting on a ${noun}`) : named ? `Running: ${only.label}` : `Running a ${noun}`;
    // A single named item is already in the title, so a lone subagent's link reads "Show".
    return { title, description: only.child ? 'Show' : '', waiting,
      segments: only.child ? [{ key: `0:${only.label}`, label: 'Show', threadId: only.child, aria: `Show subagent ${only.label}`, last: true }] : [] };
  }
  const counts = new Map<string, number>();
  for (const item of items) counts.set(item.kind, (counts.get(item.kind) ?? 0) + 1);
  const groups = [...counts].map(([key, count]) => { const meta = BACKGROUND_KINDS[key] ?? BACKGROUND_KINDS.background_task!; return `${count} ${count === 1 ? meta.singular : meta.plural}`; });
  return { title: `${waiting ? 'Waiting on' : 'Running'} ${joinWithAnd(groups)}`, description: items.map(item => item.label).join(', '), waiting,
    segments: items.some(item => item.child) ? segments(items) : [] };
}

/**
 * ChatView composerBannerItems, as ChatComposer and ComposerBannerStack show them: notices about sending from this
 * composer, each its own attached card, the machine's notices first and then feedback, limit recovery, usage limits,
 * the project clone and the branch mismatch, in that fixed order so cards never trade places. An open approval or
 * question owns the space above the composer: only a machine error or warning, which is why an answer could not go
 * through, stays beside it. What the thread is doing lives at the end of the timeline (thread-end.ts).
 */
export function composerNotices(client: T3Client, now: number): ComposerNotice[] {
  const mismatch = branchMismatchNotice(client); // returning to Home resets revelation, but keeps session dismissals
  // ChatView projectCloneBannerItem: a project added by cloning shows its clone where its draft is (project-clones-live.ts).
  const clone = projectCloneNotice(client);
  const cloneItem = clone ? [notice({ ...clone, icon: 'download' })] : [];
  // systemComposerBannerItems (server-update-notices.ts): the environment's offline and server-update notices.
  // An Auto draft shows one banner for every machine's update instead of the single-machine notice.
  const system = [...systemComposerNotices(client, { automaticEnvironment: autoBalanceState(client).automatic }), ...autoBalanceNotices(client)]
    .map(item => notice({ ...item, group: 'environment' }));
  const attention = client.threadId
    ? [...feedbackNotices(client).map(item => notice(item)), ...usageNotices(client, now).map(item => notice(item)), ...cloneItem, ...(mismatch ? [notice(mismatch)] : [])]
    : [...usageNotices(client, now).map(item => notice(item)), ...cloneItem];
  const requests = client.threadId ? pendingRequests(client.projection) : { approvals: [], inputs: [] };
  if (requests.approvals.length > 0 || requests.inputs.length > 0) return system.filter(item => item.variant === 'error' || item.variant === 'warning');
  return [...system, ...attention];
}

// ── Provider-unavailable control ───────────────────────────────────────────

/**
 * showProviderUnavailable: no instance can run the turn (composer-provider-selection.ts, the rule
 * the placeholder also reads). Its setup target is the instance the draft or thread asked for
 * (unavailableProviderInstanceId), else, unlocked, the first that can be set up; with neither the
 * control reads "No provider available" and is disabled.
 */
export function providerControl(client: T3Client) {
  const selected = composerSelection(client, !!fanoutSelections(client));
  if (!selected.showProviderUnavailable) return { noProvider: '', providerSetupId: '' };
  return { noProvider: selected.providerSetupInstanceId ? 'Open provider settings' : 'No provider available', providerSetupId: selected.providerSetupInstanceId };
}

// ── Context window meter ───────────────────────────────────────────────────

/** formatContextWindowTokens. */
export function formatTokens(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '0';
  if (value < 1_000) return `${Math.round(value)}`;
  if (value < 10_000) return `${(value / 1_000).toFixed(1).replace(/\.0$/, '')}k`;
  // Anything from 999,500 rounds to 1000k, so it belongs to the next unit.
  if (value < 999_500) return `${Math.round(value / 1_000)}k`;
  return `${(value / 1_000_000).toFixed(1).replace(/\.0$/, '')}m`;
}
const finite = (value: unknown): number | null => typeof value === 'number' && Number.isFinite(value) ? value : null;
/**
 * contextWindow.ts latestProviderTurnTokenUsage (74541bab17): the newest live turn usage. A run fork with none of its own, and no
 * provider-thread report, keeps the usage its fork transfer carried from the source thread, while the transfer is
 * pending or resolved as a native fork on the same provider instance.
 */
export function liveTokenUsage(projection: Obj): Obj | null {
  const latest = latestTokenUsage(arr(projection.providerTurns));
  const thread = obj(projection.thread), providerThread = arr(projection.providerThreads).find(entry => entry.id === thread.activeProviderThreadId);
  if (latest !== null || providerThread?.contextUsage != null) return latest;
  const forkedFrom = obj(thread.forkedFrom);
  if (forkedFrom.type !== 'run') return null;
  const transfer = arr(projection.contextTransfers).find(transfer => transfer.type === 'fork' && transfer.targetThreadId === thread.id
    && transfer.sourceThreadId === forkedFrom.threadId && obj(transfer.sourcePoint).runId === forkedFrom.runId
    && transfer.sourceProviderInstanceId === obj(thread.modelSelection).instanceId
    && (transfer.status === 'pending' || obj(transfer.resolution).strategy === 'native_fork'));
  return transfer?.sourceTokenUsage == null ? null : obj(transfer.sourceTokenUsage);
}
/** deriveLatestContextWindowSnapshot: live turn usage, else the provider thread's report, else the last compaction. */
export type ContextUsage = { used: number; max: number | null; total: number | null; cost: Obj | null; auto: boolean; threshold: number | null; updatedAt: string;
  /** The provider's prompt cache lifetime, reported with live usage only. */
  promptCacheTtlMs?: number };
export function contextSnapshot(projection: Obj): ContextUsage | null {
  const usage = liveTokenUsage(projection);
  if (usage) {
    const used = Math.max(0, finite(usage.usedTokens) ?? 0), max = finite(usage.maxTokens), ttl = finite(usage.promptCacheTtlMs);
    return { used, max, total: null as number | null, cost: null as Obj | null, auto: true, threshold: finite(usage.autoCompactThreshold), updatedAt: str(usage.updatedAt),
      ...(ttl !== null && ttl > 0 ? { promptCacheTtlMs: ttl } : {}) };
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
    contextTotal: '', contextCost: '', contextNote: '', contextCompact: false, contextValueNow: 0 };
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
    contextNote: !usage.auto ? '' : usage.threshold !== null && usage.threshold > 0 ? `Compacts automatically at about ${usage.threshold.toLocaleString('en-US')} tokens.`
      : modelName ? `Context for ${modelName} compacts automatically when needed.` : 'Context compacts automatically when needed.',
    contextCompact: arr(provider?.slashCommands).some(command => command.name === 'compact'),
    // The bar's `aria-valuenow`, as ContextWindowMeter's: the used share, held to 0–100 and rounded.
    contextValueNow: Math.round(normalized) };
}
