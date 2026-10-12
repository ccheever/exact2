// A subagent's hover card: provider glyph, model (with the account when several
// accounts share the provider), status, elapsed time, the workspace rows that
// differ from the parent, and a one-line preview (task composer-fidelity, A15).
// Adapted from T3 Code 1e2ecbd975 (MIT); see LICENSE-T3. Sources:
// apps/web/src/components/chat/SubagentTooltipContent.tsx (daa1d0ed94),
// packages/client-runtime/src/state/subagentDisplay.ts (resolveSubagentMetadata,
// subagentDetailPreview), subagentRuntime.ts (isTerminalSubagentStatus) and
// providerInstanceDisplay.ts (shouldShowInstanceBadge,
// resolveProviderInstanceDisplayName, normalizeProviderAccentColor).
// Changes: one plain view record for a Contract card; the brand-label table of
// resolveProviderInstanceDisplayName is the snapshot's displayName, else the
// humanized instance id; elapsed time is the caller's string.
import { arr, obj, str, type Obj } from './domain';
import { formatModelSlugName, resolveSelectableModel } from './r3-composer-controls-model';

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

/** SubagentTooltipContent as one record: the card's lines in order. */
export function subagentCard(input: { title: string; model: string | null; provider?: Obj; providers?: Obj[]; driver?: string; elapsed?: string; status: string;
  result?: string | null; progress?: string | null; parentThread?: Obj; childThread?: Obj; parentProject?: Obj; childProject?: Obj }) {
  const { modelLabel, workspace } = subagentMetadata(input);
  const badge = !!input.provider && showsInstanceBadge(input.provider, input.providers ?? []);
  const working = WORKING.includes(input.status), failed = input.status === 'failed' || input.status === 'error';
  return {
    title: input.title,
    driver: str(input.provider?.driver) || (input.driver ?? ''),
    // The account follows the model only when several accounts share the provider (or the instance has an accent).
    modelLine: badge ? `${modelLabel} · ${instanceDisplayName(input.provider!)}` : modelLabel,
    accent: badge ? accent(input.provider!.accentColor) : '',
    status: input.status.replace(/_/g, ' '),
    statusIcon: working ? 'circle-dashed' : failed ? 'circle-x' : input.status === 'completed' ? 'check' : 'circle-dashed',
    tone: working ? 'info' : failed ? 'error' : input.status === 'completed' ? 'success' : 'muted',
    elapsed: input.elapsed ?? '',
    rows: workspace.map(row => ({ ...row, icon: row.label === 'Branch' ? 'git-branch' : 'folder' })),
    preview: subagentPreview(input),
  };
}
