import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { site } from '../constants/site.ts';

// Checks the Markdown that tools/postbuild.ts wrote into dist, as an agent reads it: one .md file for each
// page and nothing else, each with its frontmatter, the llms.txt pointer, an H1 with the feedback block
// below it, closed code fences, no HTML or CSS leaked into the prose, and links and images that reach a
// file in dist. llms.txt and sitemap.md list every page. Exits 1 on the first build that breaks one. docs.expo.dev's
// scripts/check-markdown-pages.ts is the model.

const SITE = resolve(import.meta.dir, '..');
const DIST = resolve(SITE, process.argv[2] ?? 'dist');
const ORIGIN = site.origin;
type Page = { path: string; title: string; description: string };
const pages: Page[] = JSON.parse(readFileSync(join(SITE, 'content/pages.json'), 'utf8'));

const problems: string[] = [];
const fail = (where: string, what: string) => problems.push(`${where}: ${what}`);
const markdownPath = (path: string) => `${path === '/' ? '/index' : path}.md`;

const FENCE = /^ {0,3}(`{3,}|~{3,})(.*)$/;
const LEAKED_HTML = /<\/?(div|span|p|a|img|br|section|details|summary|table|tr|td|pre|code|button)\b[^>]*>/i;
const LEAKED_CSS = /\b(class|className|style)="[^"]*"/;
// A code span may wrap onto the next line of its paragraph, but not past a blank line.
const CODE_SPAN = /(`+)(?:(?!\n\s*\n)[\s\S])*?\1/g;
const LINK = /!?\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;

function prose(where: string, body: string): string[] {
  const lines: string[] = [];
  let open: { char: string; length: number; line: number } | null = null;
  body.split('\n').forEach((line, i) => {
    const fence = line.match(FENCE);
    if (open) {
      if (fence && fence[1][0] === open.char && fence[1].length >= open.length && !fence[2].trim()) open = null;
      return;
    }
    if (fence && !(fence[1][0] === '`' && fence[2].includes('`'))) {
      open = { char: fence[1][0], length: fence[1].length, line: i + 1 };
      return;
    }
    lines.push(line);
  });
  if (open) fail(where, `the code fence opened on body line ${(open as { line: number }).line} is never closed`);
  return lines.join('\n').replace(CODE_SPAN, 'code').split('\n');
}

function reaches(href: string): boolean {
  if (!href.startsWith(`${ORIGIN}/`)) return true;
  const file = decodeURI(href.slice(ORIGIN.length).split('#')[0].split('?')[0]);
  return file === '/llms.txt' || existsSync(join(DIST, file.slice(1))) || (file === '/' && existsSync(join(DIST, 'index.html')));
}

for (const p of pages) {
  const where = markdownPath(p.path);
  const file = join(DIST, where.slice(1));
  if (!existsSync(file)) { fail(where, 'missing'); continue; }
  const text = readFileSync(file, 'utf8');
  const front = text.match(/^---\n([\s\S]*?)\n---\n/);
  if (!front) { fail(where, 'no frontmatter'); continue; }
  const fields = Object.fromEntries(front[1].split('\n').map(line => {
    const at = line.indexOf(': ');
    try { return [line.slice(0, at), JSON.parse(line.slice(at + 2))]; } catch { return [line, undefined]; }
  }));
  if (fields.title !== p.title) fail(where, `frontmatter title ${JSON.stringify(fields.title)} is not the page title`);
  if (typeof fields.description !== 'string' || !fields.description.trim()) fail(where, 'frontmatter description is missing or empty');
  const body = text.slice(front[0].length);
  if (!body.includes(`> Fetch the complete documentation index at: ${ORIGIN}/llms.txt`)) fail(where, 'no llms.txt pointer');
  const lines = prose(where, body);
  if (!lines.some(line => /^# \S/.test(line))) fail(where, 'no H1');
  if (!new RegExp(`^# .*\\n\\n<AgentInstructions>\\n[\\s\\S]*--subject "Exact docs: ${(ORIGIN + p.path).replace(/[.*+?^${}()|[\]\\/]/g, '\\$&')}"[\\s\\S]*?\\n</AgentInstructions>\\n`, 'm').test(body)) fail(where, 'no feedback block naming this page below the H1');
  if (lines.filter(line => line.trim()).length < 3) fail(where, 'nothing below the pointer and the H1');
  lines.forEach(line => {
    const html = line.match(LEAKED_HTML);
    if (html) fail(where, `raw HTML in the prose: ${html[0]}`);
    const css = line.match(LEAKED_CSS);
    if (css) fail(where, `a CSS class or style in the prose: ${css[0]}`);
    if (/\S\. \.(\s|$)/.test(line)) fail(where, `a doubled period: ${line.trim().slice(0, 80)}`);
    for (const [, href] of line.matchAll(LINK)) if (!reaches(href)) fail(where, `${href} is no file in dist`);
  });
}

const expected = new Set(pages.map(p => markdownPath(p.path)));
const written = readdirSync(DIST, { recursive: true, encoding: 'utf8' })
  .filter(f => f.endsWith('.md') && !f.startsWith('assets/'))
  .map(f => `/${f.split('\\').join('/')}`);
for (const f of written) if (!expected.has(f) && f !== '/404.md' && f !== '/sitemap.md') fail(f, 'is no page in content/pages.json');
if (!written.includes('/404.md')) fail('/404.md', 'missing');

const llms = existsSync(join(DIST, 'llms.txt')) ? readFileSync(join(DIST, 'llms.txt'), 'utf8') : '';
if (!llms) fail('/llms.txt', 'missing');
else for (const p of pages) if (!llms.includes(`](${ORIGIN}${markdownPath(p.path)})`)) fail('/llms.txt', `does not list ${markdownPath(p.path)}`);

const sitemap = existsSync(join(DIST, 'sitemap.md')) ? readFileSync(join(DIST, 'sitemap.md'), 'utf8') : '';
if (!sitemap) fail('/sitemap.md', 'missing');
else for (const p of pages) if (!sitemap.includes(`](${ORIGIN}${markdownPath(p.path)}) | Type: `)) fail('/sitemap.md', `does not list ${markdownPath(p.path)}`);

if (problems.length) {
  console.error(`markdown-check: ${problems.length} problems\n${problems.map(p => `  ${p}`).join('\n')}`);
  process.exit(1);
}
console.log(`markdown-check: ${pages.length} .md pages, 404.md, llms.txt and sitemap.md are valid`);
