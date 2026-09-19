import { extname, join, normalize } from 'node:path';

const root = normalize(join(import.meta.dir, process.env.SERVE_DIST === '0' ? '.' : 'dist'));
const port = Number(process.env.PORT || 4173);
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.glb': 'model/gltf-binary', '.map': 'application/json', '.md': 'text/markdown' };

const server = Bun.serve({
  port,
  async fetch(request) {
    const url = new URL(request.url);
    const relative = decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname);
    const path = normalize(join(root, relative));
    if (!path.startsWith(root)) return new Response('Forbidden', { status: 403 });
    const file = Bun.file(path);
    if (!(await file.exists())) return new Response('Not found', { status: 404 });
    return new Response(file, { headers: { 'content-type': types[extname(path)] || 'application/octet-stream', 'cache-control': 'no-store' } });
  },
});

console.log(`Lanterns server http://127.0.0.1:${server.port} (${root})`);
