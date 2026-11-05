import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { search } from '../constants/search.ts';
import { pages as modules } from '../content/index.ts';

type Run = { text: string };
type Block = { kind: string; text: string; href: string; lang: string; runs: Run[]; rows: { cells: { runs: Run[] }[] }[] };
type Page = { title: string; blocks: Block[] };
type Entry = { path: string; route: string; title: string; section: string };
type Record = { objectID: string; url: string; page: string; heading: string; section: string; content: string };

const SITE = resolve(import.meta.dir, '..');
const MAX_CONTENT = 3000;
const entries: Entry[] = JSON.parse(readFileSync(join(SITE, 'content/pages.json'), 'utf8'));

const runsText = (runs: Run[]) => runs.map(r => r.text).join('');
function blockText(b: Block): string {
  if (b.kind === 'table') return b.rows.map(r => r.cells.map(c => runsText(c.runs)).join(' | ')).join('\n');
  if (b.kind === 'code' || b.kind === 'terminal') return b.lang === 'prompt' ? '' : b.text;
  return runsText(b.runs);
}

async function recordsOf(entry: Entry): Promise<Record[]> {
  const load = modules[entry.route];
  if (!load) return [];
  const page = (await load()).default as Page;
  const records: Record[] = [];
  let heading = '', anchor = '', parts: string[] = [];
  const flush = () => {
    const text = parts.join('\n').replace(/\s+\n/g, '\n').trim();
    const chunks: string[] = [];
    for (let rest = text; rest; ) {
      const cut = rest.length <= MAX_CONTENT ? rest.length : Math.max(rest.lastIndexOf('\n', MAX_CONTENT), MAX_CONTENT / 2);
      chunks.push(rest.slice(0, cut).trim());
      rest = rest.slice(cut).trim();
    }
    if (!chunks.length && heading) chunks.push('');
    chunks.forEach((content, i) => records.push({
      objectID: `${entry.route}#${anchor || 'top'}${i ? `~${i}` : ''}`, url: anchor ? `${entry.path}#${anchor}` : entry.path,
      page: entry.title, heading, section: entry.section, content,
    }));
  };
  for (const b of page.blocks) {
    if (b.kind === 'heading') {
      flush();
      heading = runsText(b.runs);
      anchor = b.href;
      parts = [];
    } else {
      const text = blockText(b);
      if (text) parts.push(text);
    }
  }
  flush();
  return records;
}

async function algolia(method: string, path: string, body?: unknown) {
  const key = process.env.ALGOLIA_WRITE_KEY;
  const response = await fetch(`https://${search.appId}.algolia.net${path}`, {
    method,
    headers: { 'X-Algolia-Application-Id': search.appId, 'X-Algolia-API-Key': key ?? '', 'Content-Type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) throw new Error(`Algolia ${method} ${path} answered ${response.status}: ${await response.text()}`);
  return response.json() as Promise<{ taskID?: number; status?: string }>;
}

async function settled(index: string, task: number | undefined) {
  if (task === undefined) return;
  while ((await algolia('GET', `/1/indexes/${index}/task/${task}`)).status !== 'published') await Bun.sleep(500);
}

const records = (await Promise.all(entries.map(recordsOf))).flat();
console.log(`search-index: ${records.length} records from ${entries.length} pages`);
if (process.argv.includes('--dry-run')) {
  console.log(JSON.stringify(records.slice(0, 3), null, 2));
  process.exit(0);
}
if (!process.env.ALGOLIA_WRITE_KEY) {
  console.error('search-index: set ALGOLIA_WRITE_KEY to the Write API key of the Algolia app (dashboard: Settings, API Keys), or pass --dry-run');
  process.exit(1);
}

const staging = `${search.index}_staging`;
await settled(staging, (await algolia('PUT', `/1/indexes/${staging}/settings`, {
  searchableAttributes: ['unordered(heading)', 'unordered(page)', 'content'],
  attributesToSnippet: ['content:24'],
  attributesForFaceting: ['section'],
  attributeForDistinct: 'url',
  distinct: true,
  customRanking: [],
})).taskID);
for (let i = 0; i < records.length; i += 1000) {
  const batch = records.slice(i, i + 1000).map(body => ({ action: 'addObject', body }));
  await settled(staging, (await algolia('POST', `/1/indexes/${staging}/batch`, { requests: batch })).taskID);
}
await settled(search.index, (await algolia('POST', `/1/indexes/${staging}/operation`, { operation: 'move', destination: search.index })).taskID);
console.log(`search-index: ${records.length} records are live in ${search.index}`);
