#!/usr/bin/env bun
import {spawnSync,spawn} from 'node:child_process';
import {resolve} from 'node:path';
const here = import.meta.dirname;
const exact = resolve(here, '../..');
const env = {...process.env, EXACT_UPDATE_TRUST: 'development', EXACT_APP_DIR: resolve(here,'tally'), CARGO_TARGET_DIR: resolve(here,'target'), EXACT_WEB_DIST: resolve(here,'dist')};
const [cmd,...rest] = process.argv.slice(2);
const commands = {web:['host/web/build.mjs','tally-web'],dev:['host/web/dev.mjs','--app','tally'],macos:['host/apple/build.mjs','tally-apple'],ios:['host/apple/build.mjs','--ios','tally-apple'],agent:['scripts/agent.mjs','--app','tally']};
// The agent CLI has no --web-dist; its existing --url seam serves this build.
if(cmd==='agent'&&rest[0]==='web'&&!rest.includes('--url')) {
  const {createServer}=await import('node:http');
  const {serveStatic}=await import('../../host/web/serve.mjs');
  const {resolveApp}=await import('../../scripts/app.mjs');
  const {assertWebDistApp}=await import('../../scripts/agent.mjs');
  process.env.EXACT_APP_DIR=env.EXACT_APP_DIR;
  assertWebDistApp(env.EXACT_WEB_DIST,resolveApp('tally'));
  const server=createServer((req,res)=>serveStatic(env.EXACT_WEB_DIST,req,res));
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const child=spawn('bun',[resolve(exact,'scripts/agent.mjs'),'--app','tally','--url',`http://127.0.0.1:${server.address().port}/`,...rest],{cwd:here,env,stdio:'inherit'});
  const code=await new Promise(r=>child.once('exit',r));
  await new Promise(r=>server.close(r));
  process.exit(code??1);
}
const r = cmd === 'linux' ? spawnSync('cargo',['build','--locked','--release','-p','tally-linux',...rest],{cwd:here,env,stdio:'inherit'}) : spawnSync('bun',[resolve(exact,commands[cmd]?.[0] ?? 'scripts/agent.mjs'),...(commands[cmd]?.slice(1) ?? []),...rest],{cwd:here,env,stdio:'inherit'});
process.exit(r.status ?? 1);
