import { resolve, sep } from 'node:path';
const root = resolve(process.argv[2] || '.');
const types = { '.html':'text/html; charset=utf-8', '.js':'text/javascript', '.mjs':'text/javascript', '.wasm':'application/wasm', '.json':'application/json', '.css':'text/css', '.glb':'model/gltf-binary', '.png':'image/png', '.svg':'image/svg+xml' };
const server = Bun.serve({ hostname:'127.0.0.1', port:Number(process.env.PORT || 0), async fetch(request) {
  const pathname = decodeURIComponent(new URL(request.url).pathname);
  const path = resolve(root, '.' + (pathname.endsWith('/') ? pathname + 'index.html' : pathname));
  if (path !== root && !path.startsWith(root + sep)) return new Response('Forbidden', {status:403});
  const file = Bun.file(path);
  if (!(await file.exists())) return new Response('Not found', {status:404});
  const extension = path.slice(path.lastIndexOf('.'));
  return new Response(file, { headers:{
    'Content-Type':types[extension] || 'application/octet-stream',
    'Cross-Origin-Opener-Policy':'same-origin', 'Cross-Origin-Embedder-Policy':'require-corp',
    'Cache-Control':'no-cache',
  }});
}});
console.log(JSON.stringify({pid:process.pid,port:server.port,root}));
for (const signal of ['SIGINT','SIGTERM']) process.on(signal, () => { server.stop(true); process.exit(0); });
