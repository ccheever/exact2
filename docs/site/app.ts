import type { Answer, Result, Sources } from './app.contract.d.ts';
import { pages } from './content/index.ts';
import nav from './content/nav.ts';
import moved from './content/redirects.ts';

export const appId = 'dev.exact.docs';
export const grants = '';

type Page = Result<'loadPage'>;

const none = { title: '', href: '' };
const missing: Page = {
  slug: 'notfound',
  section: '',
  title: 'Page Not Found',
  description: 'No page of the Exact docs is at this address.',
  subtitle: '',
  updated: '',
  prev: none,
  next: none,
  editUrl: '',
  feedbackUrl: '',
  markdownPath: '',
  claudeUrl: '',
  claudeCodeUrl: '',
  chatgptUrl: '',
  codexUrl: '',
  cursorUrl: '',
  blocks: [],
  toc: [],
};

const words = (s: string) => s.toLowerCase().split(/[^a-z0-9]+/).filter(w => w.length > 2);

// The not-found page's URL, as expo/docs' getRedirectPath reads it: no query or fragment, no index.html, .html
// or .md, no trailing slash. A listed old path gives its page; a page's own path (reached with a trailing slash
// or an extension) gives itself. Else the pages sharing the most words with the URL are suggested, as
// expo/docs' worker/url-recovery.ts suggests them.
function resolve(url: string): Result<'resolveMoved'> {
  if (!url) return { to: '', go: '', suggestions: [] };
  const path = url.split(/[?#]/)[0].replace(/^\/+/, '/').replace(/\/index\.html$/, '/').replace(/\.(html|md)$/, '');
  const clean = path.length > 1 ? path.replace(/\/+$/, '') : path;
  const go = (to: string) => ({ to, go: `${to}?from=${encodeURIComponent(path)}`, suggestions: [] });
  if (moved.redirects[clean]) return go(moved.redirects[clean]);
  if (clean !== url && moved.pages.some(p => p.path === clean)) return { to: clean, go: clean, suggestions: [] };
  const asked = new Set(words(clean));
  const suggestions = moved.pages
    .map(p => ({ p, score: words(`${p.path} ${p.title}`).filter(w => asked.has(w)).length }))
    .filter(x => x.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, 3)
    .map(({ p }) => ({ title: p.title, href: p.path }));
  return { to: '', go: '', suggestions };
}

const sources: Sources = {
  loadPage: async ([route]): Promise<Page> => {
    const load = pages[route];
    const module = await load?.().catch(() => null);
    return module ? (module.default as Page) : missing;
  },
  loadNav: async (): Promise<Result<'loadNav'>> => nav,
  resolveMoved: async ([url]): Promise<Result<'resolveMoved'>> => resolve(url),
  fetchMarkdown: async ([route]): Promise<Result<'fetchMarkdown'>> => {
    const response = await fetch(`/assets/markdown/${route}.md`);
    if (!response.ok) throw new Error(`The page's Markdown answered ${response.status}.`);
    return { text: await response.text() };
  },
};

export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
