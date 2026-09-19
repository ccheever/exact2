// Opt-in parent packaging; not part of the default checks or candidate context.
import {existsSync,readFileSync,readdirSync,rmSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
export async function prepareContext(work, {target} = {}) {
  const {gameDefaults,prepareGame}=await import(pathToFileURL(resolve(work,'game/app/shells.mjs')));
  const {resolveApp}=await import(pathToFileURL(resolve(work,'scripts/app.mjs')));
  const selected=resolve(work,'game/games/lanterns'), consumers=[];
  for(const group of ['games','bench','verification']) {
    const base=resolve(work,'game',group);
    if(!existsSync(base))continue;
    for(const name of readdirSync(base)) {
      const dir=resolve(base,name);
      if(!existsSync(resolve(dir,'logic/src/lib.rs')))continue;
      const metadata=gameDefaults(dir,resolve(work,'game'));
      if(!metadata?.game)throw new Error(`consumer has no explicit metadata: ${dir}`);
      prepareGame(dir,metadata.game,resolve(work,'game'));
      consumers.push(dir);
    }
  }
  const previousTarget=process.env.CARGO_TARGET_DIR;
  try {
    if(target)process.env.CARGO_TARGET_DIR=target;else delete process.env.CARGO_TARGET_DIR;
    var app=resolveApp(selected);
  } finally {if(previousTarget===undefined)delete process.env.CARGO_TARGET_DIR;else process.env.CARGO_TARGET_DIR=previousTarget;}
  for(const dir of consumers)if(dir!==selected) {
    const manifest=readFileSync(resolve(dir,'logic/Cargo.toml'));
    for(const name of readdirSync(dir))if(!['.shells','Cargo.lock','app.json'].includes(name))rmSync(resolve(dir,name),{recursive:true,force:true});
    mkdirSync(resolve(dir,'logic/src'),{recursive:true});
    writeFileSync(resolve(dir,'logic/Cargo.toml'),manifest);
    writeFileSync(resolve(dir,'logic/src/lib.rs'),'// Unavailable consumer fixture; captured Cargo metadata only.\n');
  }
  for(const group of ['bench','verification']) {
    const base=resolve(work,'game',group);
    if(existsSync(base))for(const name of readdirSync(base))if(!consumers.includes(resolve(base,name)))rmSync(resolve(base,name),{recursive:true,force:true});
  }
  for(const name of ['llp','rules','experiments','game/diaries','game/twins','game/new','QUEUE.md','.codex','.claude','.github'])rmSync(resolve(work,name),{recursive:true,force:true});
  function strip(dir) {
    for(const entry of readdirSync(dir,{withFileTypes:true})) {
      const path=resolve(dir,entry.name);
      if(entry.isSymbolicLink())continue;
      if(entry.isDirectory())strip(path);
      else if(['AGENTS.md','CLAUDE.md'].includes(entry.name))rmSync(path);
    }
  }
  strip(work);
  for(const dir of consumers)if(dir!==selected) {
    if(readFileSync(resolve(dir,'logic/src/lib.rs'),'utf8')!=='// Unavailable consumer fixture; captured Cargo metadata only.\n')throw new Error('consumer source escaped stripping');
    if(!existsSync(resolve(dir,'.shells/Cargo.toml'))||!existsSync(resolve(dir,'Cargo.lock')))throw new Error('consumer lost workspace or captured lock');
  }
  if(existsSync(resolve(work,'game/tests/trials')))throw new Error('evaluator escaped context stripping');
  return app;
}
