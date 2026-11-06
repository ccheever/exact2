// The Usage page's providers, T3 Code 7c3b3b3605ef (MIT, see LICENSE-T3):
// apps/web/src/components/usage/usageProviders.ts (PROVIDER_PRESENTATION, PROVIDER_ORDER). Changes: each
// series colour is a light and a dark value, since Contract has no CSS variables (`--contrast-foreground`
// for Codex, Grok's color-mix over the background).
export type ProviderKind = 'codex' | 'claude' | 'grok' | 'cursor' | 'opencode' | 'antigravity' | 'pi' | 'muse';
export const PROVIDER_ORDER: ProviderKind[] = ['codex', 'claude', 'grok', 'cursor', 'opencode', 'antigravity', 'pi', 'muse'];
/** Label, series colour (light, dark) and the driver whose mark it shows. */
export const PROVIDERS: Record<ProviderKind, { label: string; color: [string, string]; driver: string }> = {
  codex: { label: 'Codex', color: ['#18181b', '#f5f5f5'], driver: 'codex' },
  claude: { label: 'Claude Code', color: ['#d97757', '#d97757'], driver: 'claudeAgent' },
  grok: { label: 'Grok Build', color: ['#636365', '#b3b3b3'], driver: 'grok' },
  cursor: { label: 'Cursor', color: ['#8b8b8b', '#8b8b8b'], driver: 'cursor' },
  opencode: { label: 'OpenCode', color: ['#5b9bbd', '#5b9bbd'], driver: 'opencode' },
  pi: { label: 'Pi', color: ['#4d9f8a', '#4d9f8a'], driver: 'pi' },
  antigravity: { label: 'Antigravity', color: ['#8c7bd1', '#8c7bd1'], driver: 'antigravity' },
  muse: { label: 'Muse Code', color: ['#0866ff', '#0866ff'], driver: 'muse' },
};
