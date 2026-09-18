// Parent-owned black-box task checks. No candidate implementation is imported.
import {createCommand} from './command.mjs';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const level = JSON.parse(readFileSync(new URL('./level.json',import.meta.url)));
export async function evaluate(task) {
  const args=Object.fromEntries(process.argv.slice(2).map(a=>{const i=a.indexOf('=');return [a.slice(0,i),a.slice(i+1)];}));
  const out=resolve(args.out ?? `game/bench/trials/evidence/task-${task}`);
  mkdirSync(out,{recursive:true});
  const a=await createCommand({root:args.root}), cases=[], trace=[];
  const command=a.command, state=()=>command({op:'state'}), step=ticks=>command({op:'step',ticks});
  const input=(moveX=0,moveZ=0,jump=false,act=false)=>command({op:'input',moveX,moveZ,jump,act});
  const check=(ok,message)=>{if(!ok)throw new Error(message);};
  async function test(name,fn) {
    try {await fn(); cases.push({name,status:'pass',passed:true});}
    catch(e){cases.push({name,status:'fail',passed:false,error:String(e)});}
  }
  async function fresh(){await command({op:'start'});await input();await step(3);}
  async function moveTo(x,z,tolerance=.18) {
    for(let i=0;i<180;i++) {
      const s=await state(),dx=x-s.player.x,dz=z-s.player.z,distance=Math.hypot(dx,dz);
      if(distance<=tolerance){await input();return s;}
      await input(dx/distance,dz/distance);
      await step(Math.max(1,Math.min(12,Math.floor(distance/.075))));
    }
    throw new Error(`Cannot navigate to ${x},${z}: ${JSON.stringify((await state()).player)}`);
  }
  async function act(){await input();await step(1);await input(0,0,false,true);await step(1);await input();return state();}
  async function twelve() {
    await fresh();await moveTo(-8,12);
    for(const lantern of level.lanterns.slice(0,11)) {
      await moveTo(lantern.position[0],lantern.position[2]);
      check((await act()).lanterns.find(l=>l.id===lantern.id)?.lit,`Did not light ${lantern.id}`);
    }
    await moveTo(4.8,8);await input(1,0);await step(65);await input();await step(15);
    let s=await state();await moveTo(s.crate.x-1.5,s.crate.z);
    await input(1,0,true);await step(20);await input();await step(60);
    s=await state();check(s.player.y>.9&&s.player.grounded,'Did not land on crate');
    await input(1,0,true);await step(35);await input();await step(60);
    s=await state();check(s.player.y>2.2&&s.player.grounded,'Did not reach ledge');
    await moveTo(10,8);s=await act();
    check(s.count===12&&s.remaining>0,'Original twelve not collected before night');
    trace.push({stage:'twelve',state:s});return s;
  }
  async function roundtrip(){const before=await state();await command({op:'save'});await a.reload();await command({op:'load'});const after=await state();
    check(before.ticks===after.ticks&&before.count===after.count&&before.phase===after.phase&&before.remaining===after.remaining,'Save lost phase, count or timer');return after;}
  try {
    await fresh();
    await test('original twelve and positions preserved',async()=>{
      const s=await state();for(const l of level.lanterns){const got=s.lanterns.find(x=>x.id===l.id);check(got&&['x','y','z'].every((k,i)=>Math.abs(got[k]-l.position[i])<.035),`Changed ${l.id}`);}
    });
    if(task==='a') {
      await test('thirteenth position and total',async()=>{const s=await state(),l=s.lanterns.find(l=>l.id==='lantern-13');check(s.total===13&&l&&Math.abs(l.x+8)<.035&&Math.abs(l.y-1.2)<.035&&Math.abs(l.z-7)<.035,'Expected lantern-13 at [-8,1.2,7] and total 13');});
      await test('displayed total is 13',async()=>{check((await a.tree()).nodes.some(n=>/\b0\s*\/\s*13\b/.test(n.props?.text??'')),'HUD does not show 0 / 13');});
      await test('cannot light remotely',async()=>{await act();const s=await state();check(s.lanterns.find(l=>l.id==='lantern-13')?.lit===false&&s.count===0,'Thirteenth missing or remotely lit');});
      await test('new raised platform reachable and solid with ordinary jump',async()=>{
        await fresh();await moveTo(-8,12);await moveTo(-8,9.4);
        await input(0,-1,true);await step(27);await input();await step(60);
        let s=await state();check(s.player.y>1.05&&s.player.y<1.4&&s.player.grounded,'Did not land on new 1.2m platform');
        await moveTo(-8,7);s=await act();check(s.lanterns.find(l=>l.id==='lantern-13')?.lit===true,'Could not light thirteenth on platform');
        trace.push({stage:'platform',state:s});
      });
      await test('save/load retains thirteenth',async()=>{const s=await roundtrip();check(s.lanterns.find(l=>l.id==='lantern-13')?.lit===true,'Save missing lit thirteenth');});
      let collected=false;
      await test('original twelve still reachable',async()=>{await twelve();collected=true;});
      await test('twelve no longer wins',async()=>{check(collected&&(await state()).phase==='playing','Original twelve won or route failed');});
      await test('all thirteen wins by ordinary route',async()=>{
        check(collected&&(await state()).phase==='playing','Cannot continue after twelve');
        await act();check((await state()).count===12,'Remote thirteenth collected from old ledge');
        await moveTo(14,8);await moveTo(14,12);await moveTo(-8,12);await moveTo(-8,9.4);
        await input(0,-1,true);await step(27);await input();await step(60);
        await moveTo(-8,7);const s=await act();check(s.count===13&&s.phase==='won','Thirteen did not win');
        await roundtrip();
      });
    } else {
      let collected=false;
      await test('twelve collected without immediate victory',async()=>{const s=await twelve();collected=true;check(s.total===12&&s.phase==='playing','All twelve must await return home');});
      await test('explicit return-home instruction shown',async()=>{check(collected,'Route failed');check((await a.tree()).nodes.some(n=>/return.{0,40}(home|spawn)|go.{0,20}home/i.test(n.props?.text??'')),'No return-home instruction in UI tree');});
      await test('return timer continues',async()=>{const before=await state();await step(60);const after=await state();check(collected&&before.phase==='playing'&&after.phase==='playing'&&after.elapsed>=before.elapsed+.99,'Timer stopped during return');});
      await test('save/load preserves intermediate state and timer',async()=>{const s=await roundtrip();check(collected&&s.phase==='playing'&&s.count===12,'Return-home intermediate state absent after restart/load');});
      await test('timeout during return loses',async()=>{const s=await state();await step(Math.ceil(s.remaining*60)+1);check(collected&&(await state()).phase==='lost','Timeout during return did not lose');});
      await test('return within two horizontal units wins',async()=>{
        await command({op:'load'});check((await state()).phase==='playing','Missing intermediate checkpoint');
        await moveTo(14,8);await moveTo(14,12);await moveTo(1.5,12,.6);
        const s=await state();check(s.phase==='won'&&Math.hypot(s.player.x,s.player.z-12)<=2.05,'Did not win at original spawn');
      });
    }
    const render=await a.screenshot(resolve(out,'final.png'));
    cases.push({name:'actual rendered game output',status:'unsupported',passed:false,reason:render.unsupported});
  } finally {await a.close();}
  const report={task,engine:'exact2-linux',recordedAt:new Date().toISOString(),cases,trace,
    unavailable:[...a.unavailable],limitations:a.limitations,
    functionalPassed:cases.filter(c=>c.status!=='unsupported').every(c=>c.passed),passed:cases.every(c=>c.passed)};
  writeFileSync(resolve(out,`task-${task}.json`),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report));
  if(!report.passed)process.exitCode=1;
}
