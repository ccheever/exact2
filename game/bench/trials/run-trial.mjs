#!/usr/bin/env bun
// Ref-addressed, sequential, parent-owned measurement. No candidate can read this file.
import {spawn,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync,mkdirSync,readFileSync,writeFileSync,appendFileSync,rmSync,cpSync,realpathSync,readdirSync,symlinkSync} from 'node:fs';
import {resolve,dirname} from 'node:path';
const here=import.meta.dirname, root=resolve(here,'../../..'), home=process.env.HOME;
const argv=process.argv.slice(2), refIndex=argv.indexOf('--ref');
if(refIndex<0)throw new Error('Usage: run-trial.mjs --ref <git ref> <a|b> <attempt>');
const ref=argv.splice(refIndex,2)[1], [task,attempt]=argv;
if(!['a','b'].includes(task)||!/^\d+$/.test(attempt??'')||!ref||ref.startsWith('-'))throw new Error('Invalid task, attempt or ref');
const scratch=resolve(home,'lanes/gamenext/scratch/trials'), refName=ref.replace(/[^a-zA-Z0-9._/-]/g,'_');
if(refName.split('/').some(p=>p==='..'||!p))throw new Error('Unsafe ref path');
const out=resolve(scratch,refName,`${task}-${attempt}`), work=resolve(out,'workspace');
if(existsSync(out))throw new Error(`Attempt already exists: ${out}`);
mkdirSync(out,{recursive:true});
const env={...process.env,PATH:`${home}/.cargo/bin:${home}/.local/bin:${process.env.PATH}`,EXACT_UPDATE_TRUST:'development',CARGO_PROFILE_DEV_DEBUG:'0',CARGO_PROFILE_TEST_DEBUG:'0',CARGO_INCREMENTAL:'0'};
const sync=(cmd,args,cwd=root)=>{const r=spawnSync(cmd,args,{cwd,env,encoding:'utf8',maxBuffer:64*1024*1024});if(r.status!==0)throw new Error(`${cmd} failed: ${r.stderr||r.error}`);return r.stdout.trim();};
const sha=sync('git',['rev-parse',`${ref}^{commit}`]);
const disk=()=>{const lines=sync('df',['-Pk',home]).split('\n');const kb=Number(lines.at(-1).trim().split(/\s+/)[3]);if(kb<25*1024*1024)throw new Error(`Less than 25 GiB free: ${kb} KiB`);return kb;};
const now=()=>new Date().toISOString();
const report={engine:'exact2-linux',ref,sha,task,attempt,model:'gpt-5.6-sol',effort:'high',wallCeilingSeconds:900,startedAt:now(),humanInterventions:0,agentSeconds:null,buildSeconds:null,proofSeconds:null,usage:null,passed:false};
report.evaluatorHashes=Object.fromEntries(['command.mjs','acceptance.mjs','task-evaluator.mjs','task-a.mjs','task-b.mjs','level.json'].map(name=>[name,createHash('sha256').update(readFileSync(resolve(here,name))).digest('hex')]));
report.externalQueueDelaySeconds=null;
writeFileSync(resolve(out,'trial.json'),JSON.stringify(report,null,2)+'\n');
const save=()=>writeFileSync(resolve(out,'trial.json'),JSON.stringify(report,null,2)+'\n');
async function run(cmd,args,{cwd=work,childEnv=env,log,ceiling}={}) {
  const start=performance.now(), child=spawn(cmd,args,{cwd,env:childEnv,stdio:['ignore','pipe','pipe']});
  const streams=[]; const recorded=new Map();
  writeFileSync(resolve(out,`${log}.log`),'');writeFileSync(resolve(out,`${log}.stderr`),'');
  function inventory() {
    const ps=spawnSync('ps',['-axo','pid=,ppid=,lstart='],{encoding:'utf8'});
    return (ps.stdout??'').split('\n').flatMap(l=>{const m=l.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);return m?[{pid:+m[1],ppid:+m[2],stamp:m[3]}]:[];});
  }
  function sample() {const rows=inventory(), owned=new Set([child.pid]);let changed=true;while(changed){changed=false;for(const r of rows)if(owned.has(r.ppid)&&!owned.has(r.pid)){owned.add(r.pid);changed=true;}}for(const r of rows)if(owned.has(r.pid))recorded.set(r.pid,r.stamp);}
  let timedOut=false;
  const killOwned=()=>{sample();for(const r of inventory())if(recorded.get(r.pid)===r.stamp){try{process.kill(r.pid,'SIGKILL');}catch{}}};
  const monitor=setInterval(sample,500);sample();
  child.stdout.on('data',d=>{streams.push(String(d));appendFileSync(resolve(out,`${log}.log`),d);});
  const stderr=[];child.stderr.on('data',d=>{stderr.push(String(d));appendFileSync(resolve(out,`${log}.stderr`),d);});
  const timer=ceiling?setTimeout(()=>{timedOut=true;killOwned();},ceiling*1000):null;
  let code;
  try{code=await new Promise((ok,no)=>{child.on('exit',ok);child.on('error',no);});}
  finally {clearTimeout(timer);clearInterval(monitor);killOwned();}
  const stdout=streams.join('');writeFileSync(resolve(out,`${log}.log`),stdout);writeFileSync(resolve(out,`${log}.stderr`),stderr.join(''));
  writeFileSync(resolve(out,`${log}.pids.json`),JSON.stringify([...recorded]));
  return {code,seconds:(performance.now()-start)/1000,timedOut,stdout};
}
try {
  report.freeKiBBefore=disk();mkdirSync(work,{recursive:true});
  for(const dep of ['ibex','snapback-sb4'])symlinkSync(realpathSync(resolve(root,'..',dep)),resolve(out,dep),'dir');
  const archive=resolve(out,'source.tar');sync('git',['archive','--format=tar',`--output=${archive}`,sha]);sync('tar',['-xf',archive,'-C',work]);rmSync(archive);
  // Dependencies stay available as an installed SDK would. Remove unrelated
  // prose, benchmark instruments, old trial data and all inherited agent prompts.
  for(const name of ['llp','rules','experiments','game/bench/trials','game/diaries','game/twins','QUEUE.md'])rmSync(resolve(work,name),{recursive:true,force:true});
  function strip(dir){for(const e of readdirSync(dir,{withFileTypes:true})){const p=resolve(dir,e.name);if(e.isSymbolicLink())continue;if(e.isDirectory())strip(p);else if(['AGENTS.md','CLAUDE.md'].includes(e.name))rmSync(p);}}
  strip(work);
  // Only Lanterns and its shared authored fragments are supplied as games.
  for(const name of readdirSync(resolve(work,'game/games')))if(!['lanterns','shared'].includes(name))rmSync(resolve(work,'game/games',name),{recursive:true,force:true});
  const target=resolve(work,'game/target');mkdirSync(target,{recursive:true});
  // Copy-on-write where supported; never a hardlink or a shared writable target.
  if(existsSync(resolve(root,'game/target')))sync('cp',['-a','--reflink=auto',`${root}/game/target/.`,target]);
  // Cargo dep-info embeds absolute generated-source paths. Invalidate copied
  // fingerprints before warming at this location; source paths must name this ref.
  function invalidate(dir){for(const e of readdirSync(dir,{withFileTypes:true})){if(!e.isDirectory())continue;const p=resolve(dir,e.name);if(e.name==='.fingerprint')rmSync(p,{recursive:true,force:true});else invalidate(p);}}
  invalidate(target);
  const tmp=resolve(work,'.trial-tmp'), codexHome=resolve(work,'.trial-codex');mkdirSync(tmp,{recursive:true});mkdirSync(codexHome,{recursive:true});
  // Only auth is copied: no history, memories, config, MCPs or skills.
  cpSync(resolve(home,'.codex/auth.json'),resolve(codexHome,'auth.json'));
  const warm=await run('bun',[resolve(here,'build.mjs'),work],{log:'setup-build'});
  report.setupSeconds=warm.seconds;if(warm.code!==0)throw new Error('Baseline warm build failed');
  // Record same-ref controls outside the candidate. Never shown to the model.
  for(const name of ['acceptance','task-a','task-b'])await run('bun',[resolve(here,`${name}.mjs`),`root=${work}`,`out=${out}/controls/${name}`],{log:`control-${name}`});
  sync('git',['init','-q'],work);sync('git',['config','user.name','Trial baseline'],work);sync('git',['config','user.email','trial@localhost'],work);
  const ignore=resolve(work,'.git/info/exclude');writeFileSync(ignore,'/.trial-tmp/\n/.trial-codex/\n');
  sync('git',['add','-A'],work);sync('git',['commit','-qm',`Immutable input ${sha}`],work);
  report.inputCommit=sync('git',['rev-parse','HEAD'],work);
  const brief=readFileSync(resolve(home,'lanes/gamenext/comparison-full/BRIEF.md'),'utf8');
  const trials=readFileSync(resolve(home,'lanes/gamenext/comparison-full/TRIALS.md'),'utf8');
  const taskText=trials.split(`## Task ${task.toUpperCase()}:`)[1].split('\n## ')[0];
  const prompt=`Implement this change in game/games/lanterns only. This is a fresh 900-second change trial. Model gpt-5.6-sol, reasoning high. At most three fixes per failure loop; stop and report after three. No sub-agents. Do not read anything outside this workspace or seek external evaluators/other attempts. Do not change the engine, host, shared scripts, or baseline physics/movement. Ordinary input only; no teleport or test mutation. Read game/README.md and your game. Dependencies and existing build/proof scripts are present for compilation and verification. A warmed Linux build is ready. This machine has no GPU, Chrome, or Apple SDK. Use bun game/games/lanterns/proof.mjs linux, and cargo test --manifest-path game/Cargo.toml -p lanterns-logic. Preserve the environment's Cargo debug/incremental settings. Write your final report before the deadline.\n\n${brief.split('## Game\n')[1].split('## Common browser')[0]}\n\nChange request:${taskText}`;
  writeFileSync(resolve(out,'prompt.txt'),prompt);
  const codex=realpathSync(sync('which',['codex'])), bun=realpathSync(sync('which',['bun']));
  const policy={read:['/usr','/bin','/sbin','/lib','/lib64','/etc',realpathSync('/etc/resolv.conf'),'/proc','/sys',dirname(codex),dirname(bun),resolve(home,'.rustup'),realpathSync(resolve(root,'../ibex')),realpathSync(resolve(root,'../snapback-sb4'))],write:[work,resolve(home,'.cargo'),'/dev/null','/dev/urandom','/dev/random']};
  const policyPath=resolve(out,'isolation.json');writeFileSync(policyPath,JSON.stringify(policy));
  // Run a real read-denial probe, not stat (Landlock intentionally permits stat).
  const probe=await run('python3',[resolve(here,'isolate.py'),policyPath,'python3','-c',`from pathlib import Path\ntry: Path(${JSON.stringify(resolve(here,'task-a.mjs'))}).read_text(); raise RuntimeError('evaluator readable')\nexcept PermissionError: print('parent evaluator read denied')\nPath('/etc/resolv.conf').read_text()\nprint('resolver readable')`],{log:'isolation-probe'});
  if(probe.code!==0)throw new Error('Isolation probe failed');
  const childEnv={...env,CODEX_HOME:codexHome,TMPDIR:tmp,CARGO_TARGET_DIR:target,EXACT_APP_DIR:resolve(work,'game/games/lanterns')};
  delete childEnv.EXACT_LINUX_BIN;
  report.dispatchedAt=now();save();
  const agent=await run('python3',[resolve(here,'isolate.py'),policyPath,codex,'exec','--ignore-user-config','--ignore-rules','--ephemeral','--json','--color','never','--skip-git-repo-check','--dangerously-bypass-approvals-and-sandbox','-m','gpt-5.6-sol','-c','model_reasoning_effort="high"','-c','project_doc_max_bytes=0','-C',work,prompt],{log:'agent',ceiling:900,childEnv});
  report.agentSeconds=agent.seconds;report.agentTimedOut=agent.timedOut;report.agentExit=agent.code;report.modelCompletedAt=now();save();
  const events=agent.stdout.split('\n').flatMap(l=>{try{return [JSON.parse(l)];}catch{return [];}});
  report.usage=events.filter(e=>e.type==='turn.completed').at(-1)?.usage??null;
  const commands=events.filter(e=>e.type==='item.completed'&&e.item?.type==='command_execution').map(e=>e.item.command);
  report.invocations={build:commands.filter(c=>/cargo (?:build|rustc)|build\.mjs|run\.sh build/.test(c)).length,proof:commands.filter(c=>/proof\.mjs|run\.sh proof/.test(c)).length,tests:commands.filter(c=>/cargo test|bun test|run\.sh test/.test(c)).length,commands:commands.length,method:'completed shell command records matching build/proof/test; combined shell commands count once; proof-internal builds excluded'};
  writeFileSync(resolve(out,'commands.json'),JSON.stringify(commands,null,2));
  sync('git',['add','-A'],work);const diff=sync('git',['diff','--cached','--binary',report.inputCommit],work);writeFileSync(resolve(out,'changes.diff'),diff+'\n');
  const stats=sync('git',['diff','--cached','--numstat',report.inputCommit],work).split('\n').filter(Boolean).map(l=>{const [add,del,...path]=l.split('\t');return {path:path.join('\t'),added:Number(add)||0,deleted:Number(del)||0};});
  report.files=stats;report.filesChanged=stats.length;report.linesAdded=stats.reduce((n,f)=>n+f.added,0);report.linesDeleted=stats.reduce((n,f)=>n+f.deleted,0);
  report.scopeViolations=stats.filter(f=>!f.path.startsWith('game/games/lanterns/')).map(f=>f.path);
  report.sourceHashes=Object.fromEntries(stats.filter(f=>existsSync(resolve(work,f.path))).map(f=>[f.path,createHash('sha256').update(readFileSync(resolve(work,f.path))).digest('hex')]));
  report.freeKiBBeforeBuild=disk();
  const build=await run('bun',[resolve(here,'build.mjs'),work],{log:'build'});report.buildSeconds=build.seconds;report.buildExit=build.code;report.buildCompletedAt=now();
  report.evaluatorStartedAt=now();const begin=performance.now();
  for(const name of ['acceptance',`task-${task}`])await run('bun',[resolve(here,`${name}.mjs`),`root=${work}`,`out=${out}/evaluation` ,...(name==='acceptance'?[`task=${task==='a'?'extra':'home'}`]:[])],{log:`evaluate-${name}`});
  report.proofSeconds=(performance.now()-begin)/1000;
  const acceptance=JSON.parse(readFileSync(resolve(out,'evaluation/acceptance.json'))),taskReport=JSON.parse(readFileSync(resolve(out,`evaluation/task-${task}.json`)));
  report.taskFunctionalPassed=taskReport.functionalPassed;
  report.baselineMechanicsPassed=acceptance.cases.every(c=>c.passed||c.mechanicsPassed);
  report.passed=agent.code===0&&!agent.timedOut&&build.code===0&&acceptance.passed&&taskReport.passed&&!report.scopeViolations.length;
  report.selfRepairRounds=null;report.screenshotInspections=null;report.protocolCompliant=null; // Retained-log audit, not invented counts.
  report.finishedAt=now();
} catch(error) {report.error=String(error);report.finishedAt=now();}
finally {
  // Logs, diff and JSON survive. Auth, sources and all compilation outputs do not.
  rmSync(work,{recursive:true,force:true});for(const dep of ['ibex','snapback-sb4'])rmSync(resolve(out,dep),{force:true});report.buildOutputDeleted=!existsSync(work);save();
}
console.log(JSON.stringify(report));
if(!report.passed)process.exitCode=1;
