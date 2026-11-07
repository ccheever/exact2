import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { site } from '../constants/site.ts';

// Checks what tools/postbuild.ts wrote into dist against schema.org's types and Google's structured-data
// rules (developers.google.com/search/docs/appearance/structured-data): every page's JSON-LD, its canonical
// link, robots.txt and sitemap.xml. Exits 1 on the first build that breaks one.

const SITE = resolve(import.meta.dir, '..');
const DIST = resolve(SITE, process.argv[2] ?? 'dist');
const ORIGIN = site.origin;
type Page = { path: string; title: string; description: string; updated: string };
const pages: Page[] = JSON.parse(readFileSync(join(SITE, 'content/pages.json'), 'utf8'));

const problems: string[] = [];
const fail = (where: string, what: string) => problems.push(`${where}: ${what}`);
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const absolute = (where: string, field: string, value: unknown) => {
  if (typeof value !== 'string' || !value.startsWith(`${ORIGIN}/`)) fail(where, `${field} is ${JSON.stringify(value)}, not a URL under ${ORIGIN}/`);
};
const text = (where: string, field: string, value: unknown, max = Infinity) => {
  if (typeof value !== 'string' || !value.trim()) fail(where, `${field} is missing or empty`);
  else if (value.length > max) fail(where, `${field} is ${value.length} characters, over ${max}`);
};

type Node = Record<string, unknown>;
const RULES: Record<string, (where: string, node: Node) => void> = {
  WebSite(where, node) {
    text(where, 'WebSite.name', node.name);
    if (ORIGIN) absolute(where, 'WebSite.url', node.url);
  },
  TechArticle(where, node) {
    text(where, 'TechArticle.headline', node.headline, 110);
    text(where, 'TechArticle.description', node.description);
    if (node.dateModified !== undefined && (typeof node.dateModified !== 'string' || !ISO_DATE.test(node.dateModified))) fail(where, `TechArticle.dateModified ${JSON.stringify(node.dateModified)} is not an ISO 8601 date`);
    const part = node.isPartOf as Node | undefined;
    if (!part || part['@type'] !== 'WebSite') fail(where, 'TechArticle.isPartOf is not a WebSite');
    if (ORIGIN) absolute(where, 'TechArticle.url', node.url);
  },
  BreadcrumbList(where, node) {
    const items = node.itemListElement;
    if (!Array.isArray(items) || items.length < 2) return fail(where, 'BreadcrumbList has fewer than 2 items');
    items.forEach((item: Node, i) => {
      if (item['@type'] !== 'ListItem') fail(where, `item ${i + 1} is not a ListItem`);
      if (item.position !== i + 1) fail(where, `item ${i + 1} has position ${item.position}`);
      text(where, `item ${i + 1} name`, item.name);
      if (i < items.length - 1 || item.item !== undefined) absolute(where, `item ${i + 1} item`, item.item);
    });
  },
};

for (const p of pages) {
  const file = join(DIST, p.path === '/' ? 'index.html' : `${p.path.slice(1)}/index.html`);
  if (!existsSync(file)) { fail(p.path, 'no prerendered page'); continue; }
  const html = readFileSync(file, 'utf8');
  const blocks = [...html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)].map(m => m[1]);
  if (blocks.length !== 1) { fail(p.path, `${blocks.length} JSON-LD blocks, not 1`); continue; }
  let data: Node;
  try { data = JSON.parse(blocks[0]); } catch (e) { fail(p.path, `JSON-LD does not parse: ${(e as Error).message}`); continue; }
  if (data['@context'] !== 'https://schema.org') fail(p.path, `@context is ${JSON.stringify(data['@context'])}`);
  const nodes = (Array.isArray(data['@graph']) ? data['@graph'] : [data]) as Node[];
  for (const node of nodes) {
    const type = node['@type'] as string;
    const rule = RULES[type];
    if (!rule) fail(p.path, `@type ${JSON.stringify(type)} is not one this site uses`);
    else rule(p.path, node);
  }
  const main = nodes[0];
  const shown = main.headline ?? main.name;
  if (p.path !== '/' && shown !== p.title) fail(p.path, `headline ${JSON.stringify(shown)} is not the page title`);
  if (!html.includes('<link rel="icon" href="/favicon.svg"')) fail(p.path, 'no favicon link');
  const canonical = html.match(/<link rel="canonical" href="([^"]*)">/)?.[1];
  if (ORIGIN && canonical !== ORIGIN + p.path) fail(p.path, `canonical is ${JSON.stringify(canonical)}, not ${ORIGIN + p.path}`);
  if (!ORIGIN && canonical) fail(p.path, 'a canonical link without an origin');
}

const robots = existsSync(join(DIST, 'robots.txt')) ? readFileSync(join(DIST, 'robots.txt'), 'utf8') : '';
if (!/^User-agent: \*$/m.test(robots)) fail('robots.txt', 'missing, or no "User-agent: *"');
const sitemapPath = join(DIST, 'sitemap.xml');
if (ORIGIN) {
  if (!robots.includes(`Sitemap: ${ORIGIN}/sitemap.xml`)) fail('robots.txt', 'does not name the sitemap');
  if (!existsSync(sitemapPath)) fail('sitemap.xml', 'missing');
  else {
    const xml = readFileSync(sitemapPath, 'utf8');
    if (!xml.includes('xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"')) fail('sitemap.xml', 'no sitemap namespace');
    const locs = [...xml.matchAll(/<loc>([^<]*)<\/loc>/g)].map(m => m[1]);
    const expected = pages.map(p => ORIGIN + p.path);
    for (const url of expected) if (!locs.includes(url)) fail('sitemap.xml', `missing ${url}`);
    for (const url of locs) if (!expected.includes(url)) fail('sitemap.xml', `lists ${url}, which is no page`);
    for (const m of xml.matchAll(/<lastmod>([^<]*)<\/lastmod>/g)) if (!ISO_DATE.test(m[1])) fail('sitemap.xml', `lastmod ${m[1]} is not an ISO 8601 date`);
  }
} else if (existsSync(sitemapPath)) fail('sitemap.xml', 'written without an origin');

if (problems.length) {
  console.error(`seo-check: ${problems.length} problems\n${problems.map(p => `  ${p}`).join('\n')}`);
  process.exit(1);
}
console.log(`seo-check: ${pages.length} pages' JSON-LD, favicon${ORIGIN ? ', canonical links, robots.txt and sitemap.xml' : ' and robots.txt'} are valid${ORIGIN ? '' : ' (no origin yet: URLs, canonical links and sitemap.xml not checked)'}`);
