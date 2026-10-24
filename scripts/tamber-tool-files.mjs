// The design tool's built files, as the Tamber wire serves them under
// `/__tamber/tool/` (scripts/tamber-server.mjs; stage 10 B's remote doors,
// scripts/tamber-remote.mjs, serve them too).
import { existsSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { relative, resolve, sep } from 'node:path';
// Where `bun exact.mjs tamber tool` builds the design tool (its port's build), and the wire serves it from.
import { TOOL_DIST } from './tamber-tool-port.mjs';
import { generationFile, generationOf, generationPath, withGenerationBase } from '../host/web/generations.mjs';
export { TOOL_DIST };
const TOOL_TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.png': 'image/png', '.plan': 'application/octet-stream' };

const json = (res, status, body) => {
  res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
  res.end(JSON.stringify(body));
};
const notFound = res => json(res, 404, { message: 'Not found.' });

/** The design tool's files, to a local request only. */
export function serveTool(req, res, url, access, toolDir = TOOL_DIST) {
  if (!['GET', 'HEAD'].includes(req.method) || !access.local) return notFound(res);
  // The tool's data module calls 127.0.0.1 (its grant's origin): a page
  // opened as localhost would be another origin, every call refused.
  const host = /^localhost(:\d+)?$/i.exec(String(req.headers.host ?? ''));
  if (host) { res.writeHead(308, { location: `http://127.0.0.1${host[1] ?? ''}${url.pathname}`, 'cache-control': 'no-store' }); return res.end(); }
  serveToolFiles(req, res, url, toolDir);
}

const TOOL_BASE = '/__tamber/tool/';
/** The tool's built files under `/__tamber/tool/`, static GETs only: no
 * secret is in them (the remote listener serves them too, stage 10 B). A
 * page's files come from the build it booted (host/web/generations.mjs): its
 * document's base names that build's generation, `/__tamber/tool/.exact/gen/
 * <id>/`, whose files are read from that build, the current one or the one
 * kept after another replaced it; 410 for one kept no more. */
export async function serveToolFiles(req, res, url, toolDir = TOOL_DIST, headers = {}) {
  if (!['GET', 'HEAD'].includes(req.method)) return notFound(res);
  if (url.pathname === '/__tamber/tool') { res.writeHead(308, { location: '/__tamber/tool/', 'cache-control': 'no-store' }); return res.end(); }
  const under = generationPath(url.pathname, TOOL_BASE);
  if (under === 'bad') return notFound(res);
  if (under) {
    // The build's own rules for its files (host/web/serve.mjs `buildTreeFile`: no dot segment, no link out).
    const found = await generationFile(toolDir, under.id, under.path);
    if (found === 'gone') { res.writeHead(410, { 'content-type': 'text/plain; charset=utf-8', 'cache-control': 'no-store' }); return res.end('this design tool build is no longer kept here: load the tool again\n'); }
    if (!found || found.route.endsWith('.html')) return notFound(res);
    const type = TOOL_TYPES[found.route.slice(found.route.lastIndexOf('.'))] ?? 'application/octet-stream';
    res.writeHead(200, { 'content-type': type, 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' });
    return res.end(req.method === 'HEAD' ? undefined : found.body);
  }
  let rel;
  try { rel = decodeURIComponent(url.pathname.slice('/__tamber/tool/'.length)); } catch { return notFound(res); }
  // No dot segment (the build's own `.gen`, `.exact`), no way out.
  if (rel.split('/').some(s => s.startsWith('.')) || /[\\\0]/.test(rel)) return notFound(res);
  let file = resolve(toolDir, rel || 'index.html');
  if (file !== toolDir && !file.startsWith(toolDir + sep)) return notFound(res);
  try { if (statSync(file).isDirectory()) file = resolve(file, 'index.html'); } catch { /* below */ }
  let bytes, generation;
  try {
    // Through no link out of the build, nor to a dot file in it; read whole from one build (its generation the same
    // before and after the read, else read again).
    for (let attempt = 0; attempt < 4; attempt++) {
      generation = generationOf(toolDir);
      const root = realpathSync(toolDir), real = realpathSync(file);
      if (!real.startsWith(root + sep) || relative(root, real).split(sep).some(s => s.startsWith('.'))) return notFound(res);
      bytes = readFileSync(real);
      if (generationOf(toolDir) === generation) break;
    }
  } catch {
    if (!existsSync(resolve(toolDir, 'index.html'))) return json(res, 404, { message: 'the design tool is not built: run `bun exact.mjs tamber tool`' });
    return notFound(res);
  }
  const type = TOOL_TYPES[file.slice(file.lastIndexOf('.'))] ?? 'application/octet-stream';
  // Its pages anchor their files at the root; here that is this path.
  if (type.startsWith('text/html')) bytes = Buffer.from(toolPage(bytes, generation));
  res.writeHead(200, { 'content-type': type, 'cache-control': 'no-store', 'x-content-type-options': 'nosniff', ...(type.startsWith('text/html') ? headers : {}) });
  res.end(req.method === 'HEAD' ? undefined : bytes);
}

/** A built tool page as the wire serves it: its files anchored at `/__tamber/tool/`, under its build's generation
 * there when the build has one (so a check of what a tool serves compares the same text). */
export function toolPage(bytes, generation) {
  const page = bytes.toString('utf8').replace(/<base\s+href=["']\/["']\s*\/?\s*>/i, `<base href="${TOOL_BASE}">`);
  return generation ? withGenerationBase(page, generation, TOOL_BASE) : page;
}
