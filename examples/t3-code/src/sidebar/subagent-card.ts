// A subagent's hover card: provider glyph, model (with the account when several
// accounts share the provider, then the effort and speed it runs with), status,
// elapsed time, the workspace rows that differ from the parent, and a one-line preview.
// Adapted from T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Sources:
// apps/web/src/components/chat/SubagentTooltipContent.tsx (at 74541bab17),
// packages/client-runtime/src/state/subagentDisplay.ts (resolveSubagentMetadata,
// subagentDetailPreview), subagentRuntime.ts (isTerminalSubagentStatus) and
// providerInstanceDisplay.ts (shouldShowInstanceBadge,
// resolveProviderInstanceDisplayName, normalizeProviderAccentColor).
// Changes: one plain view record for a Contract card; the brand-label table of
// resolveProviderInstanceDisplayName is the snapshot's displayName, else the
// humanized instance id; elapsed time is the caller's string.
import { arr, obj, str, type Obj } from '../client/domain';
import { formatModelSlugName, REASONING_EFFORT_OPTION_IDS, resolveSelectableModel, traitsSpeed } from '../composer/composer-controls-model';

const TERMINAL = new Set(['completed', 'failed', 'cancelled', 'interrupted', 'rolled_back', 'idle']);
const WORKING = ['running', 'in_progress', 'pending', 'waiting'];

const basename = (path: string) => path.replace(/\/+$/u, '').split('/').pop() ?? path;
const humanize = (slug: string) => slug.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/[_-]+/g, ' ').trim().replace(/\b\w/g, char => char.toUpperCase());
const accent = (value: unknown) => { const trimmed = str(value).trim(); return /^#[0-9a-fA-F]{6}$/u.test(trimmed) ? trimmed : ''; };

/** shouldShowInstanceBadge: an accent colour, or another instance of the same provider (ACP agents by their agent id). */
export function showsInstanceBadge(provider: Obj, providers: Obj[]): boolean {
  if (accent(provider.accentColor)) return true;
  const driver = str(provider.driver);
  return providers.filter(candidate => candidate.driver === driver && (driver !== 'acpRegistry' || candidate.acpRegistryAgentId === provider.acpRegistryAgentId)).length > 1;
}
/** resolveProviderInstanceDisplayName, without the driver brand table. */
export function instanceDisplayName(provider: Obj): string {
  return str(provider.displayName).trim() || humanize(str(provider.instanceId)) || str(provider.driver);
}

/** resolveSubagentMetadata: the catalog's name for a reported model, and only the workspace facts that differ from the parent. */
export function subagentMetadata(input: { model: string | null; provider?: Obj; parentThread?: Obj; childThread?: Obj; parentProject?: Obj; childProject?: Obj }) {
  const model = (input.model ?? '').trim();
  const models = arr(input.provider?.models);
  const slug = input.provider ? resolveSelectableModel(str(input.provider.driver), model, models) : model;
  const catalog = input.provider ? models.find(candidate => candidate.slug === slug) : undefined;
  const reported = catalog ? str(catalog.shortName) || str(catalog.name) : model ? formatModelSlugName(model) : 'Not reported';
  const qualifier = str(catalog?.subProvider).trim();
  const modelLabel = qualifier ? reported.replace(new RegExp(`^${qualifier.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?:\\s*[.:/-]\\s*|\\s+)`, 'iu'), '').trim() || reported : reported;
  const parentWorkspace = str(input.parentThread?.worktreePath) || str(input.parentProject?.workspaceRoot);
  const childWorkspace = str(input.childThread?.worktreePath) || str(input.childProject?.workspaceRoot);
  const workspace: Array<{ label: string; value: string }> = [];
  if (input.parentThread && input.childProject && input.childProject.id !== input.parentThread.projectId) workspace.push({ label: 'Project', value: str(input.childProject.title) });
  if (parentWorkspace && childWorkspace && parentWorkspace !== childWorkspace) {
    const branch = str(input.childThread?.branch);
    workspace.push({ label: branch ? 'Branch' : str(input.childThread?.worktreePath) ? 'Worktree' : 'Workspace', value: branch || basename(childWorkspace) });
  }
  return { modelLabel, workspace };
}

/** subagentDetailPreview: live work leads with progress, settled work with its result; one line of at most 280 characters. */
export function subagentPreview(input: { status: string; result?: string | null; progress?: string | null }): string {
  const result = (input.result ?? '').trim(), progress = (input.progress ?? '').trim();
  const detail = (TERMINAL.has(input.status) ? result || progress : progress || result) || '';
  const compact = detail.replace(/\s+/gu, ' ');
  return compact.length > 280 ? `${compact.slice(0, 280).trimEnd()}…` : compact;
}

/**
 * SubagentTooltipContent's effort and speed: the subagent's own model selection,
 * else an app-owned child thread's, counts only for the model and instance the
 * card shows. Effort is the selection's raw value; speed is the fast-mode or
 * Codex service tier bolt ('' for none).
 */
export function subagentTraits(input: { model: string | null; provider?: Obj; providerInstanceId?: string; origin?: string; modelSelection?: Obj; childThread?: Obj }) {
  const model = (input.model ?? '').trim(), provider = input.provider;
  const models = arr(provider?.models), driver = str(provider?.driver);
  const slug = provider ? resolveSelectableModel(driver, model, models) || model : model;
  const selection = input.modelSelection ?? (input.origin === 'app_owned' ? obj(input.childThread?.modelSelection) : undefined);
  const childModel = str(selection?.model).trim(), childSlug = provider ? resolveSelectableModel(driver, childModel, models) || childModel : childModel;
  const matching = selection && childSlug === slug && str(selection.instanceId) === str(input.providerInstanceId) ? arr(selection.options) : [];
  const value = (id: string) => { const entry = matching.find(option => option.id === id)?.value; return typeof entry === 'string' ? entry : ''; };
  const effort = REASONING_EFFORT_OPTION_IDS.map(value).find(Boolean) ?? '';
  let speed = '';
  for (const descriptor of provider ? arr(obj(models.find(candidate => candidate.slug === slug)?.capabilities).optionDescriptors) : []) {
    const saved = matching.find(option => option.id === descriptor.id)?.value;
    const applies = (descriptor.id === 'fastMode' && descriptor.type === 'boolean' && typeof saved === 'boolean')
      || (driver === 'codex' && descriptor.id === 'serviceTier' && descriptor.type === 'select' && typeof saved === 'string' && arr(descriptor.options).some(option => option.id === saved));
    const display = applies ? traitsSpeed(driver, { ...descriptor, currentValue: saved as Obj[string] }) : null;
    if (display) { speed = display.speed; break; }
  }
  return { effort, speed };
}

/** SubagentTooltipContent as one record: the card's lines in order. */
export function subagentCard(input: { title: string; model: string | null; providerInstanceId?: string; origin?: string; modelSelection?: Obj; provider?: Obj; providers?: Obj[];
  driver?: string; elapsed?: string; status: string; result?: string | null; progress?: string | null; parentThread?: Obj; childThread?: Obj; parentProject?: Obj; childProject?: Obj }) {
  const { modelLabel, workspace } = subagentMetadata(input);
  const { effort, speed } = subagentTraits(input);
  const badge = !!input.provider && showsInstanceBadge(input.provider, input.providers ?? []);
  const working = WORKING.includes(input.status), failed = input.status === 'failed' || input.status === 'error';
  return {
    title: input.title,
    driver: str(input.provider?.driver) || (input.driver ?? ''),
    // The account follows the model only when several accounts share the provider (or the instance has an accent).
    modelLine: badge ? `${modelLabel} · ${instanceDisplayName(input.provider!)}` : modelLabel,
    accent: badge ? accent(input.provider!.accentColor) : '',
    // " · " then the bolt and the effort, after the model line; the speed's screen reader name.
    effort, speed: input.provider ? speed : '', speedLabel: input.provider && speed ? (speed === 'ultrafast' ? 'Ultrafast mode on' : 'Fast mode on') : '',
    status: input.status.replace(/_/g, ' '),
    statusIcon: working ? 'circle-dashed' : failed ? 'circle-x' : input.status === 'completed' ? 'check' : 'circle-dashed',
    tone: working ? 'info' : failed ? 'error' : input.status === 'completed' ? 'success' : 'muted',
    elapsed: input.elapsed ?? '',
    rows: workspace.map(row => ({ ...row, icon: row.label === 'Branch' ? 'git-branch' : 'folder' })),
    preview: subagentPreview(input),
  };
}
