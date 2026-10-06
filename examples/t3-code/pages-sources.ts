// The data sources of lane "pages" that app.ts delegates by name: the Pull
// Requests list, its detail panel and the first-run setup wizard.
import type { T3Client } from './client';
import type { Native } from './protocol';
import { pullRequestsPage } from './pages-prs';
import { pullRequestDetail } from './pages-pr-detail';
import { welcomeView } from './pages-welcome';

export function pagesSource(client: T3Client, native: Native | null | undefined, source: string, args: unknown[]) {
  if (source === 'prList') return pullRequestsPage(client, native, { open: args[0] === true, refresh: Number(args[1]) || 0, now: Number(args[2]) || 0, selected: String(args[3] || ''), query: String(args[4] ?? ''), typed: args[5] === true });
  if (source === 'prDetail') return pullRequestDetail(client, native, { selected: String(args[0] || ''), refresh: Number(args[1]) || 0, now: Number(args[2]) || 0 });
  if (source === 'welcome') return welcomeView(client, native, { step: String(args[0] || ''), now: Number(args[1]) || 0 });
  throw new Error(`Unknown T3 source: ${source}`);
}
