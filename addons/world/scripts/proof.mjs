import {killChildren} from './processes.mjs';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';
process.env.EXACT_APP_DIR=resolve(import.meta.dirname,'../tally');
process.env.CARGO_TARGET_DIR=resolve(import.meta.dirname,'../target');
process.env.EXACT_UPDATE_TRUST='development';
const {open}=await import('../../../scripts/agent.mjs');
const host=process.argv[2]??'web';
const evidence={host,checks:[],observations:{}};
function check(name,ok,actual) {evidence.checks.push({name,ok,actual}); if(!ok)console.error('FAIL',name,actual);}
let s;
async function start() {const session=await open({host,app:'tally',webDist:resolve(import.meta.dirname,'../dist'),size:[820,900]});if(host==='macos')await new Promise(r=>setTimeout(r,250));return session;}
async function close() {await killChildren();if(s)await s.close();s=null;}
async function inspect(){await s.tap('inspect');return (await s.state()).slots.inspection;}
async function play(){await s.tap('draw');await s.clock('+100');await s.tap('hold');await s.clock('+100');}
try {
 s=await start();
 evidence.boot=s.boot;
 const initial=await inspect();evidence.observations.initial=initial;
 check('tick 1 branch pin',initial.tick===1&&initial.hash==='0x32a9f7776ceb5052',initial.hash);
 const pre=await s.state();await s.clock('+1000');const after=await s.state();
 check('+1000 is exactly 60 ticks',after.resources.world.tick-pre.resources.world.tick===60,after.resources.world);
 evidence.observations.clockCommits=after.epoch-pre.epoch;
 await play();
 const held=await s.state();
 check('draw then hold banks hand',held.resources.world.published.hand.length===0&&held.resources.world.published.score>0,held.resources.world.published);
 check('Contract tree shows score',(await s.tree()).nodes.some(n=>n.props?.testId==='score'&&n.props.text===`Score ${held.resources.world.published.score}`));
 await s.tap('save');const checkpoint=(await s.state()).slots.exported;
 writeFileSync(resolve(import.meta.dirname,`../evidence/${host}.checkpoint`),checkpoint);
 await play();const expected=await inspect();evidence.observations.continued=expected;
 check('tick 85 branch pin',expected.tick===85&&expected.hash==='0x3d943e409a0493be',expected.hash);
 await s.tap('save');const final=(await s.state()).slots.exported;
 evidence.observations.tree=await s.tree();evidence.observations.logs=await s.logs();
 await s.screenshot(resolve(import.meta.dirname,`../evidence/${host}.png`));await close();
 s=await start();await s.type('checkpoint',checkpoint);await s.tap('restore');await play();
 const restored=await inspect();evidence.observations.restored=restored;
 check('relaunch continuation hash and entities',JSON.stringify([expected.tick,expected.hash,expected.entities])===JSON.stringify([restored.tick,restored.hash,restored.entities]),restored.hash);
 await s.tap('save');check('relaunch continuation checkpoint bytes',final===(await s.state()).slots.exported);
 await s.tap('reset');await s.tap('hold');await s.tap('draw');await s.tap('draw');await s.clock('+100');
 const multi=(await s.state()).resources.world;
 check('three same-time sends preserved in order',multi.published.score===0&&multi.published.hand.length===2&&multi.published.pile_count===10,multi);
 await s.tap('fail');await s.clock('+100');const failure=await s.state();
 check('tick error is visible and app alive',failure.resources.world.error.includes('injected Tally tick failure'),failure.resources.world);
 const failedInspect=await inspect();evidence.observations.failure=failedInspect;
 check('bounded kernel failure log visible',failedInspect.logs.includes('injected Tally tick failure'));
 check('person sees error',(await s.tree()).nodes.some(n=>n.props?.testId==='error'&&n.props.text.includes('injected')));
 await s.tap('reset');await s.clock('+100');const recovered=(await s.state()).resources.world;
 check('reset recovers',recovered.error===''&&recovered.tick===7&&recovered.published.pile_count===12,recovered);
 evidence.observations.agentStore=(await s.state()).store;
 await s.tap('hold');await s.tap('draw');await s.tap('draw');await s.tap('save');
 const pending=(await s.state()).slots.exported;await s.clock('+100');const pendingExpected=await inspect();
 await close();s=await start();await s.type('checkpoint',pending);await s.tap('restore');await s.clock('+100');
 const pendingRestored=await inspect();
 check('pending input queue survives relaunch',pendingExpected.hash===pendingRestored.hash&&pendingExpected.tick===pendingRestored.tick,pendingRestored.hash);
} catch(e) {evidence.error=String(e.stack??e);console.error(e);}
finally {await close();writeFileSync(resolve(import.meta.dirname,`../evidence/${host}-proof.json`),JSON.stringify(evidence,null,2));}
console.log(JSON.stringify({host,passed:evidence.checks.filter(x=>x.ok).length,total:evidence.checks.length,error:evidence.error}));
process.exitCode=evidence.error||evidence.checks.some(x=>!x.ok)?1:0;
