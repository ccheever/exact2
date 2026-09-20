import { join } from 'node:path';
import { cache, root, run } from './build.mjs';
import { mkdir } from 'node:fs/promises';
const assets = join(process.env.HOME, 'Library/Caches/exact2-cluster-lod/out');
const prepared = new Map();
async function prepare(asset, naive = false) {
  const key = `${asset}:${naive}`;
  if (!prepared.has(key)) prepared.set(key, (async () => {
    const out = join(cache, asset);
    await mkdir(out, { recursive: true });
    // Recompute camera companions per server run: shared scene code may have changed.
    await run([join(root, 'target/debug/examples/web_prepare'), join(assets, `${asset}-5.clod`), out, ...(naive ? ['--naive'] : [])]);
  })());
  return prepared.get(key);
}
export async function startServer(port = 8765) {
  await prepare('gaul');
  const server = Bun.serve({ hostname: '127.0.0.1', port, async fetch(req) {
    try {
      const url = new URL(req.url);
      const asset = url.searchParams.get('asset') === 'washington' ? 'washington' : 'gaul';
      let path;
      let mime;
      if (url.pathname === '/asset.clod') { path = join(assets, `${asset}-5.clod`); mime = 'application/octet-stream'; }
      else if (url.pathname === '/scenes.json') { await prepare(asset); path = join(cache,asset,'scenes.json'); mime = 'application/json'; }
      else if (url.pathname === '/naive.bin') {
        if (asset !== 'gaul') return new Response('Naive mode is available for Gaul only (tab memory budget).', {status: 413});
        await prepare(asset,true); path = join(cache,asset,'naive.bin'); mime = 'application/octet-stream';
      } else if (url.pathname === '/pkg/clod_web.js') { path = join(cache,'pkg/clod_web.js'); mime = 'text/javascript'; }
      else if (url.pathname === '/pkg/clod_web_bg.wasm') { path = join(cache,'pkg/clod_web_bg.wasm'); mime = 'application/wasm'; }
      else if (url.pathname === '/app.mjs') { path = join(import.meta.dir,'app.mjs'); mime = 'text/javascript'; }
      else if (url.pathname === '/') { path = join(import.meta.dir,'index.html'); mime = 'text/html'; }
      else return new Response('Not found', {status:404});
      const file = Bun.file(path);
      if (!await file.exists()) return new Response('Run bun web/build.mjs first', {status:404});
      const headers = { 'Content-Type': mime, 'Accept-Ranges': 'bytes', 'Cache-Control': 'no-store' };
      const range = req.headers.get('Range');
      if (range) {
        const match = /^bytes=(\d*)-(\d*)$/.exec(range);
        let start = match?.[1] ? Number(match[1]) : Math.max(0,file.size-Number(match?.[2]));
        let end = match?.[1] && match[2] ? Math.min(Number(match[2]),file.size-1) : file.size-1;
        if (!match || !Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start > end || start >= file.size) return new Response(null,{status:416,headers:{...headers,'Content-Range':`bytes */${file.size}`}});
        return new Response(req.method==='HEAD' ? null : file.slice(start,end+1), {status:206,headers:{...headers,'Content-Range':`bytes ${start}-${end}/${file.size}`,'Content-Length':String(end-start+1)}});
      }
      return new Response(req.method==='HEAD' ? null : file,{headers:{...headers,'Content-Length':String(file.size)}});
    } catch(e) { console.error(e); return new Response(String(e),{status:500}); }
  }});
  console.log(`Cluster LOD: http://127.0.0.1:${server.port}/ (pid ${process.pid})`);
  return server;
}
if (import.meta.main) await startServer(Number(process.env.PORT || 8765));
