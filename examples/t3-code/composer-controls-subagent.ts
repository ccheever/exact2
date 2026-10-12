// The read-only bar that stands in for the composer on a provider-native
// subagent thread (lane composer-controls), adapted from T3 Code (MIT); see
// LICENSE-T3. Sources: chat/ProviderSubagentBar.tsx, ChatView.tsx
// (showProviderSubagentBar, parentThreadLink), packages/client-runtime/src/state/
// threadExecution.ts (deriveProviderSubagentStatus, formatModelSelectionEffort,
// formatProviderSubagentStatus) and packages/shared/src/orchestrationTiming.ts.
import { arr, obj, str, type Obj } from './domain';
import { optionLabel, reportedModelLabel, reportedSelection, resolvedCurrent, type Selection } from './r3-composer-controls-model';

/** isProviderNativeSubagentThread: the provider runs it, so there is nothing to send. */
export function isProviderSubagent(thread: Obj): boolean {
  return obj(thread.lineage).relationshipToParent === 'subagent' && thread.creationSource === 'provider';
}

/** formatDuration (orchestrationTiming.ts). */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '0ms';
  if (ms < 1_000) return `${Math.max(1, Math.round(ms))}ms`;
  if (ms < 10_000) { const tenths = Math.round(ms / 100) / 10; return tenths >= 10 ? '10s' : `${tenths.toFixed(1)}s`; }
  if (ms < 60_000) return `${Math.round(ms / 1_000)}s`;
  const total = Math.round(ms / 1_000), hours = Math.floor(total / 3_600), minutes = Math.floor(total % 3_600 / 60), seconds = total % 60;
  return [hours ? `${hours}h` : '', minutes ? `${minutes}m` : '', seconds ? `${seconds}s` : ''].filter(Boolean).join(' ');
}

const LABELS: Record<string, string> = { idle: 'Idle', pending: 'Working', running: 'Working', waiting: 'Waiting', completed: 'Completed',
  interrupted: 'Interrupted', failed: 'Failed', cancelled: 'Cancelled', rolled_back: 'Cancelled' };
const live = (status: string) => status === 'pending' || status === 'running' || status === 'waiting';

/** deriveProviderSubagentStatus: the runless root turn's status, or null until it arrives. */
export function subagentStatus(projection: Obj): { status: string; startedAt: string; completedAt: string } | null {
  if (!isProviderSubagent(obj(projection.thread))) return null;
  const node = [...arr(projection.nodes)].reverse().find(candidate => candidate.kind === 'root_turn' && (candidate.runId === null || candidate.runId === undefined));
  return node ? { status: str(node.status), startedAt: str(node.startedAt), completedAt: str(node.completedAt) } : null;
}

/** formatProviderSubagentStatus: "Working 12s", "Completed in 34s", or the bare status. */
export function formatSubagentStatus(status: { status: string; startedAt: string; completedAt: string } | null, now: number): string {
  if (!status) return 'Starting';
  const label = LABELS[status.status] ?? status.status;
  const running = live(status.status);
  if (!running && status.status !== 'completed') return label;
  const start = Date.parse(status.startedAt), end = running ? now : Date.parse(status.completedAt);
  if (!Number.isFinite(start) || !Number.isFinite(end)) return label;
  const elapsed = formatDuration(Math.max(1_000, Math.floor((end - start) / 1_000) * 1_000));
  return running ? `${label} ${elapsed}` : `${label} in ${elapsed}`;
}

const EFFORT_IDS = ['reasoningEffort', 'effort', 'reasoning', 'variant'];
/** formatModelSelectionEffort: the effort the composer's picker would name (a provider report fills an unset one), or ''. */
export function selectionEffort(selection: Obj, models: Obj[], reported: Selection | null = null): string {
  const model = models.find(candidate => candidate.slug === selection.model);
  if (!model) return '';
  const descriptors = arr(obj(model.capabilities).optionDescriptors), chosen = arr(selection.options);
  const current: Selection = { instanceId: str(selection.instanceId), model: str(selection.model), options: chosen };
  for (const id of EFFORT_IDS) {
    const descriptor = descriptors.find(candidate => candidate.id === id);
    if (descriptor?.type !== 'select') continue;
    const label = optionLabel(descriptor, resolvedCurrent(descriptor, chosen), current, reported);
    if (label) return label;
  }
  return '';
}

/** The bar's row in the composer view; `subagent` is false for every other thread. */
export function subagentBar(source: { threadId: string; projection: Obj; config: Obj }, now: number) {
  const empty = { subagent: false, subagentModel: '', subagentEffort: '', subagentStatus: '', subagentAnnounce: '', subagentParent: '' };
  const thread = obj(source.projection.thread);
  if (!source.threadId || !isProviderSubagent(thread)) return empty;
  const selection = obj(thread.modelSelection);
  const provider = arr(source.config.providers).find(entry => entry.instanceId === selection.instanceId);
  // Providers can report a dated id or alias (claude-haiku-4-5-20251001): resolve it to the catalog's entry.
  const models = arr(provider?.models);
  const modelLabel = reportedModelLabel(str(provider?.driver), str(selection.model), models);
  const effort = selectionEffort(selection, models, reportedSelection(source.projection));
  const status = subagentStatus(source.projection);
  const parent = str(obj(thread.lineage).parentThreadId);
  const announce = formatSubagentStatus(status && { ...status, startedAt: '' }, 0);
  return { subagent: true, subagentModel: modelLabel, subagentEffort: effort, subagentStatus: formatSubagentStatus(status, now),
    subagentAnnounce: `${effort ? `${modelLabel}, ${effort}` : modelLabel} subagent: ${announce}`,
    subagentParent: parent };
}
