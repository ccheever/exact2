// A page's Markdown for a reader that asks for it (`Accept: text/markdown`), as an agent fetching
// documentation does: a site that builds `/<page>.md` beside a page's path, and `/404.md` for a
// page it lacks, answers such a request at the page's own URL. Every answer at a path with
// Markdown varies by `Accept` (RFC 9110 §12.5.5), so a cache keeps the two apart.
import { extname } from 'node:path';
import { servedFile } from './generations.mjs';
import { sendStaticBody, webContentType } from './serve.mjs';
import { webRequestURL } from '../../scripts/origin.mjs';

/** Whether `Accept` takes Markdown at a quality no lower than any range that takes HTML
 * (RFC 9110 §12.5.1), so `text/markdown, text/html;q=0.9` does and a browser's does not. */
export function prefersMarkdown(accept = '') {
  let markdown = 0, html = 0;
  for (const range of String(accept ?? '').split(',')) {
    const [type, ...params] = range.split(';').map(part => part.trim().toLowerCase());
    const q = params.find(param => param.startsWith('q='));
    const quality = q === undefined ? 1 : Number(q.slice(2)) || 0;
    if (type === 'text/markdown') markdown = Math.max(markdown, quality);
    else if (type === 'text/html' || type === 'text/*' || type === '*/*') html = Math.max(html, quality);
  }
  return markdown > 0 && markdown >= html;
}

/** The Markdown file for a page's path (`/` is `/index.md`, `/a/b/` is `/a/b.md`); none for a file's path. */
export function markdownRoute(pathname) {
  let path;
  try { path = decodeURIComponent(pathname).replace(/\/+$/, ''); } catch { return null; }
  if (extname(path)) return null;
  return `${path || '/index'}.md`;
}

async function markdownFile(dist, route) {
  const found = route ? await servedFile(dist, route) : null;
  return found?.status === 200 && found.route === route ? found : null;
}

/** Answers `req` from the site's Markdown where it prefers that, else by `serve()`, adding `Accept` to the
 * `Vary` of every answer at a path that has Markdown. */
export async function serveMarkdown(dist, req, res, serve) {
  const target = webRequestURL(req.url), route = target && markdownRoute(target.pathname);
  const page = await markdownFile(dist, route), wanted = ['GET', 'HEAD'].includes(req.method) && prefersMarkdown(req.headers.accept);
  const missing = wanted && !page && route ? await markdownFile(dist, '/404.md') : null;
  const found = wanted ? page ?? missing : null;
  if (found) {
    sendStaticBody(req, res, found.body, { 'content-type': webContentType(found.route), 'cache-control': missing ? 'no-store' : 'no-cache', vary: 'Accept' }, missing ? 404 : 200);
    return;
  }
  if (page) {
    const writeHead = res.writeHead.bind(res);
    res.writeHead = (status, headers = {}) => writeHead(status, { ...headers, vary: [headers.vary, 'Accept'].filter(Boolean).join(', ') });
  }
  return serve();
}
