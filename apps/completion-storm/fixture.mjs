#!/usr/bin/env bun
// Two loopback listeners: held data cannot starve the control connection pool.
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { readStaticFileAsync, serveStatic } from '../../host/web/serve.mjs';

export async function createFixture({port=4319, controlPort=4320, maxWaves=8, maxHeld=512, holdMs=30_000,
  dist=resolve(import.meta.dir, 'target/dist')} = {}) {
  const waves = new Map();
  let nextWave = 1, received = 0, issued = 0, rejected = 0, abandoned = 0;
  const headers = {'access-control-allow-origin':'*', 'access-control-allow-methods':'GET, POST, OPTIONS', 'access-control-allow-headers':'cache-control, content-type', 'cache-control':'no-store'};
  const heldCount = () => [...waves.values()].reduce((n, w) => n + w.held.size, 0);
  const snapshot = (message='Fixture snapshot') => ({message, received, issued, held:heldCount(), rejected, abandoned, waves:waves.size});
  const json = (res, status, body) => { res.writeHead(status, {...headers,'content-type':'application/json'}); res.end(JSON.stringify(body)); };
  const reject = (res, status, message) => { rejected++; json(res,status,{message}); };
  const integer = (url, name, min, max) => {
    const raw=url.searchParams.get(name);
    if (raw===null || !/^\d+$/.test(raw)) return null;
    const value=Number(raw);
    return Number.isSafeInteger(value) && value>=min && value<=max ? value : null;
  };
  const cleanup = (wave) => {
    if (wave.finished>=wave.count) { clearTimeout(wave.timer); waves.delete(wave.id); }
  };
  const finish = (wave, lane, res, timeout=false) => {
    wave.held.delete(lane);
    wave.finished++;
    issued++;
    if (timeout) json(res,408,{message:'Held wave expired after 30 seconds'});
    else if ((lane*37)%100 < wave.errors) {
      switch (lane%4) {
        case 0: json(res,503,{message:'Injected HTTP failure'}); break;
        case 1: res.writeHead(200,{...headers,'content-type':'application/json'}); res.end('{broken'); break;
        case 2: json(res,200,{wave:wave.id+1,lane,value:`wave ${wave.id} lane ${lane}`}); break;
        // A partial body avoids transparent GET retry after a bare socket reset.
        case 3:
          res.writeHead(200,{...headers,'content-type':'application/json','content-length':'1024'});
          res.write('cut');
          setTimeout(()=>res.destroy(),5);
          break;
      }
    } else json(res,200,{wave:wave.id,lane,value:`wave ${wave.id} lane ${lane}`});
    cleanup(wave);
  };
  const expire = (wave) => {
    for (const [lane,res] of [...wave.held]) finish(wave,lane,res,true);
    clearTimeout(wave.timer);
    waves.delete(wave.id);
  };
  const dataServer = createServer((req,res) => {
    if (req.method==='OPTIONS') { res.writeHead(204,headers); res.end(); return; }
    const url = new URL(req.url, 'http://fixture');
    if (req.method!=='GET' || url.pathname!=='/api/hold') return reject(res,404,'No such data endpoint');
    const id=integer(url,'wave',1,Number.MAX_SAFE_INTEGER), lane=integer(url,'lane',0,127);
    const wave=waves.get(id);
    if (!wave) return reject(res,410,'Unknown or expired wave');
    if (lane===null || lane>=wave.count) return reject(res,400,'Lane outside admitted count');
    if (wave.seen.has(lane)) return reject(res,409,'Lane already received');
    if (heldCount()>=maxHeld && !wave.released) return reject(res,429,'Fixture held-response limit reached');
    wave.seen.add(lane);
    received++;
    if (wave.released) return finish(wave,lane,res);
    wave.held.set(lane,res);
    const disconnected = () => {
      if (wave.held.delete(lane)) { abandoned++; wave.finished++; cleanup(wave); }
    };
    req.once('aborted',disconnected);
    res.once('close',disconnected);
  });
  const controlServer = createServer((req,res) => {
    if (req.method==='OPTIONS') { res.writeHead(204,headers); res.end(); return; }
    const url = new URL(req.url, 'http://fixture');
    if (url.pathname==='/api/open' && req.method==='POST') {
      const count=integer(url,'count',1,128), errors=integer(url,'errors',0,100);
      if (count===null || errors===null) return reject(res,400,'count: 1–128; errors: 0–100; integers only');
      if (waves.size>=maxWaves) return reject(res,429,'Fixture wave limit reached; release or wait for expiry');
      const wave={id:nextWave++,count,errors,released:false,seen:new Set(),held:new Map(),finished:0};
      wave.timer=setTimeout(()=>expire(wave),holdMs);
      waves.set(wave.id,wave);
      return json(res,200,{id:wave.id,count});
    }
    if (url.pathname==='/api/release' && req.method==='POST') {
      const id=integer(url,'wave',0,Number.MAX_SAFE_INTEGER);
      if (id===null) return reject(res,400,'wave must be an integer; 0 releases all');
      const selected=id===0 ? [...waves.values()] : [waves.get(id)].filter(Boolean);
      let released=0;
      for (const wave of selected) {
        wave.released=true;
        for (const [lane,reply] of [...wave.held]) { released++; finish(wave,lane,reply); }
      }
      return json(res,200,{...snapshot(`Released ${released} held responses; ${selected.length} wave gates opened`),released});
    }
    if (url.pathname==='/api/stats' && req.method==='GET') return json(res,200,snapshot());
    if (url.pathname.startsWith('/api/')) return reject(res,404,'Unknown control endpoint or method');
    if (dist===null) { res.writeHead(404,headers); res.end('API-only fixture'); return; }
    serveStatic(dist,req,res);
  });
  const listen = server => new Promise((ok,fail) => { server.once('error',fail); server.listen(server===dataServer?port:controlPort,'127.0.0.1',ok); });
  const close = async () => {
    for (const wave of [...waves.values()]) expire(wave);
    await Promise.all([dataServer,controlServer].map(server=>new Promise(ok=> { server.close(ok); server.closeAllConnections(); })));
  };
  try { await listen(dataServer); await listen(controlServer); }
  catch (error) { await close(); throw error; }
  return {
    data:`http://127.0.0.1:${dataServer.address().port}`,
    control:`http://127.0.0.1:${controlServer.address().port}`,
    close,
  };
}

if (import.meta.main) {
  const apiOnly=process.argv.includes('--api-only');
  if (!apiOnly && !Bun.which('cargo')) throw new Error('Serving built dist uses the shared filesystem helper; put ~/.cargo/bin on PATH first.');
  if (!apiOnly && !await readStaticFileAsync(resolve(import.meta.dir,'target/dist'), '/index.html')) {
    throw new Error('No readable built dist. Run the README build command first; cargo and its filesystem helper must be available.');
  }
  const fixture=await createFixture(apiOnly?{dist:null}:{});
  console.log(`Completion Storm${apiOnly?' (API-only)':''}: ${fixture.control}\nHeld requests: ${fixture.data}\nRelease all: curl -X POST '${fixture.control}/api/release?wave=0'\nLimits: 128 lanes/wave, 8 live waves, 512 held responses, 30s wave lifetime.`);
  for (const signal of ['SIGINT','SIGTERM']) process.once(signal,async()=>{await fixture.close();process.exit(0);});
}
