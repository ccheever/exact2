import {test} from 'bun:test';
import assert from 'node:assert/strict';
import {fixture} from './surface-record.test.mjs';

test('host-composited children use local homographies, depth order, explicit hiding and retirement', async()=>{
  const f=await fixture(), host=f.create(1), frames=[];
  const child=(x,y,w,h)=>Object.assign(new f.Element('child'),{hasAttribute:()=>false,offsetLeft:x,offsetTop:y,offsetWidth:w,offsetHeight:h,inert:false});
  const hud=child(24,24,100,20), a=child(10,40,100,50), b=child(10,90,100,50);
  host.children=[hud,a,b];
  let hidden=false, none=false;
  Object.assign(f.gpu,{
    gpu_children_mode:()=>3,
    gpu_child:(id,i,...frame)=>frames.push([i,...frame]), gpu_children_count:(id,n)=>frames.push(['count',n]),
    gpu_dirty:()=>true,
    gpu_placement:(id,i,out)=>{ if(i===0 || none)return 0; if(i===1&&hidden)return 2;out.set([2,0,100,0,2,200,0.001,0,1,i===1?-2:-5]);return 1; },
  });
  f.exact.gpu.layout();f.frame();
  assert.deepEqual(frames,[[0,24,24,100,20],[1,10,40,100,50],[2,10,90,100,50],['count',3]]);
  assert.deepEqual(hud.style,{},'None leaves the HUD at its kernel frame');
  const matrix=a.style.transform.slice(9,-1).split(',').map(Number);
  const map=(x,y)=>{const w=matrix[3]*x+matrix[7]*y+matrix[15];return [a.offsetLeft+(matrix[0]*x+matrix[4]*y+matrix[12])/w,a.offsetTop+(matrix[1]*x+matrix[5]*y+matrix[13])/w];};
  assert.deepEqual(map(0,0),[100,200]);
  const p=map(100,50);assert.ok(Math.abs(p[0]-300/1.1)<0.0001&&Math.abs(p[1]-300/1.1)<0.0001);
  assert.equal(a.style.transformOrigin,'0 0');assert.ok(Number(a.style.zIndex)>Number(b.style.zIndex));assert.ok(Number(a.style.zIndex)<0);
  hidden=true;f.frame();assert.equal(a.style.visibility,'hidden');assert.equal(a.inert,true);
  hidden=false;f.frame();assert.equal(a.inert,false);
  none=true;f.frame();assert.equal(a.style.transform,undefined);assert.equal(a.inert,false);
  host.children=[hud];f.exact.gpu.layout();assert.deepEqual(frames.at(-1),['count',1]);
});

for (const fail of [false,true]) test(`placement frames survive module replacement; failed=${fail}`, async()=>{
  const frames=[];
  const f=await fixture({nextGpu:{
    gpu_children_mode:()=>fail ? 0 : 3,
    gpu_child:(id,i,...frame)=>frames.push([i,...frame]), gpu_children_count:()=>{},
    gpu_placement:(id,i,out)=>{out.set([1,0,200,0,1,100,0,0,1,-1]);return 1;},
    gpu_render:()=>fail?2:0,
  }});
  const host=f.create(1), child=Object.assign(new f.Element('child'),{hasAttribute:()=>false,offsetLeft:10,offsetTop:20,offsetWidth:100,offsetHeight:50,inert:false});
  host.children=[child];
  Object.assign(f.gpu,{
    gpu_children_mode:()=>3,gpu_child:()=>{},gpu_children_count:()=>{},gpu_dirty:()=>true,
    gpu_placement:(id,i,out)=>{out.set([1,0,100,0,1,100,0,0,1,-1]);return 1;},
  });
  f.exact.gpu.layout();f.frame();
  const before=child.style.transform;
  if(fail){await assert.rejects(f.exact.gpu.swap(1),/render/);assert.equal(child.style.transform,before);}
  else {await f.exact.gpu.swap(1);assert.deepEqual(frames,[[0,10,20,100,50]]);assert.notEqual(child.style.transform,before);}
});

test('paused placement survives authored style updates and restores latest styles', async()=>{
  const f=await fixture(), host=f.create(1);
  const child=Object.assign(new f.Element('child'),{hasAttribute:()=>false,offsetLeft:10,offsetTop:20,offsetWidth:100,offsetHeight:50,inert:false});
  host.children=[child];let each=true, none=false;const counts=[];
  Object.assign(f.gpu,{
    gpu_children_mode:()=>each ? 3 : 0,gpu_child:()=>{},gpu_children_count:(_,n)=>counts.push(n),gpu_dirty:()=>true,
    gpu_placement:(_,i,out)=>{out.set([1,0,200,0,1,100,0,0,1,-1]);return none?0:1;},
  });
  f.exact.gpu.layout();f.frame();
  f.exact.gpu.beforeStyle(child);
  child.style={position:'absolute',visibility:'visible',zIndex:'7',transform:'rotate(3deg)',transformOrigin:'center'};
  child.offsetLeft=40;child.offsetTop=60;
  f.exact.gpu.afterStyle(child);
  const matrix=child.style.transform.slice(9,-1).split(',').map(Number);
  assert.equal(matrix[12]+child.offsetLeft,200);assert.equal(matrix[13]+child.offsetTop,100);
  assert.ok(child.style.transform.startsWith('matrix3d('),'reapplied without a frame');
  none=true;f.frame();
  assert.deepEqual(child.style,{position:'absolute',visibility:'visible',zIndex:'7',transform:'rotate(3deg)',transformOrigin:'center'});
  each=false;f.exact.gpu.layout();
  assert.equal(counts.at(-1),0);
});

test('draw hook inherits resize generation and advances on equal-paced callbacks',async()=>{
  const f=await fixture(), draws=[];f.create(1);let dirty=true;
  Object.assign(f.gpu,{gpu_dirty:()=>dirty,gpu_render:()=>{dirty=false;return 0;}});
  f.exact.drawCallback=(raw,paced,generation)=>draws.push([raw,paced,generation]);
  f.frame();dirty=true;await f.exact.gpu.settled();dirty=true;f.frame();
  assert.deepEqual(draws,[[0,0,1],[0,0,1],[0,0,2]]);
});

test('replacement without Placed restores the current authored style',async()=>{
  const f=await fixture({nextGpu:{gpu_children_mode:()=>0,gpu_children_count:()=>{}}}),host=f.create(1);
  const child=Object.assign(new f.Element('child'),{hasAttribute:()=>false,offsetLeft:0,offsetTop:0,offsetWidth:100,offsetHeight:50,inert:false});
  child.style.transform='rotate(3deg)';host.children=[child];
  Object.assign(f.gpu,{gpu_children_mode:()=>3,gpu_child:()=>{},gpu_children_count:()=>{},gpu_dirty:()=>true,
    gpu_placement:(_,i,out)=>{out.set([1,0,200,0,1,100,0,0,1,-1]);return 1;}});
  f.exact.gpu.layout();f.frame();assert.ok(child.style.transform.startsWith('matrix3d'));
  await f.exact.gpu.swap(1);
  assert.equal(child.style.transform,'rotate(3deg)');
});
