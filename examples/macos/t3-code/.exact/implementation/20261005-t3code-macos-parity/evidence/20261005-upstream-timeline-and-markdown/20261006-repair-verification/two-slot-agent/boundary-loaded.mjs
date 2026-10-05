import{readFileSync,writeFileSync,mkdirSync}from'node:fs';import{spawnSync}from'node:child_process';import{resolve}from'node:path';
const out=resolve('target/t3-repair/timeline-second');mkdirSync(out,{recursive:true});const pause=ms=>new Promise(r=>setTimeout(r,ms));
function ax(name,args){const pid=String(Number(readFileSync('target/t3-repair/final-agent3/app.pid')));const at=new Date().toISOString();const r=spawnSync('orca',['computer',...args,'--app','pid:'+pid,'--no-screenshot','--json'],{encoding:'utf8'});const result=JSON.parse(r.stdout);writeFileSync(out+'/'+name+'.json',JSON.stringify({at,...result}));if(!result.ok)throw Error(JSON.stringify(result.error));return result;}
function click(name,label){const t=ax(name+'-before',['get-app-state']);const line=t.result.snapshot.treeText.split('\n').find(s=>s.includes(' button '+label));if(!line)throw Error('MissingAX '+label);return ax(name,['click','--element-index',line.trim().match(/^\d+/)[0]]);}
export async function concurrency(s){
 const initial=ax('initial',['get-app-state']);if(!initial.result.snapshot.treeText.includes('button printf')){click('group-open','Ran 2 commands');await pause(500);}
 click('a-open','printf "verified output"');await pause(1000);click('b-open','Read project/fixture.txt');await pause(3200);const first=ax('b-first',['get-app-state']);await pause(12500);const final=ax('both-complete',['get-app-state']);
 writeFileSync(out+'/complete-tree.json',JSON.stringify(await s.tree()));return {out,first:first.result.snapshot.treeText,final:final.result.snapshot.treeText};
}
export async function boundary(s){
 await s.clock('settle');
 const id='["verify-timeline","fixture-dynamic"]', target='work-output-'+id, content='work-output-content-'+id;
 let t=await s.tree();let contentNode=t.nodes.find(n=>n.props?.testId===content);if(!contentNode?.props?.text?.includes('Long output line'))throw Error('Long output not loaded: '+contentNode?.props?.text);let node=t.nodes.find(n=>n.props?.testId===target);if(!node)throw Error('dynamic output absent');let parent=t.nodes.find(n=>n.id===node.parent);while(parent&&parent.type!=='ScrollView')parent=t.nodes.find(n=>n.id===parent.parent);await s.tap(parent.id,{wheel:[0,10000]});
 const capture=async(name)=>{const value=await s.layout(content);writeFileSync(out+'/'+name+'.json',JSON.stringify(value));return value.node.scroll.find(x=>x.id===node.id)?.sy;};
 await s.tap(target,{wheel:[0,10000]});await pause(100);const bottom=await capture('bottom');
 for(let i=0;i<3;i++)await s.type(target,{key:'ArrowDown'});const clamped=await capture('down3');await s.type(target,{key:'ArrowUp'});const up=await capture('up');
 const result={bottom,clamped,up,pass:Number.isFinite(bottom)&&Math.abs(bottom-clamped)<0.1&&Math.abs((clamped-up)-40)<0.1};writeFileSync(out+'/boundary-result.json',JSON.stringify(result));return result;
}
