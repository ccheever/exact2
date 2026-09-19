const start = performance.now();
const name = "babylon";
const canvas=document.querySelector('canvas');
let scene, player, lanterns, setX, getX, setVisible, getVisible, render, names;

 const B=await import('@babylonjs/core');
 const engine=new B.Engine(canvas,false,{preserveDrawingBuffer:true});scene=new B.Scene(engine);scene.clearColor=new B.Color4(.094,.145,.21,1);
 const camera=new B.FreeCamera('camera',new B.Vector3(5,8,12),scene);camera.setTarget(new B.Vector3(2,0,0));
 const mesh=(n,x,color,ground=false)=>{const m=ground?B.MeshBuilder.CreateGround(n,{width:12,height:8},scene):B.MeshBuilder.CreateBox(n,{width:.5,height:1,depth:.5},scene);m.position.set(x,ground?0:.5,0);const mat=new B.StandardMaterial(n+'-mat',scene);mat.emissiveColor=B.Color3.FromHexString(color);mat.disableLighting=true;m.material=mat;return m;};
 mesh('plane',2,'#334b45',true);player=mesh('player',0,'#4caaff');lanterns=[1,2,3].map(x=>mesh(`lantern-${x}`,x,'#ffc24b'));
 setX=x=>player.position.x=x;getX=()=>player.position.x;setVisible=(m,v)=>m.setEnabled(v);getVisible=m=>m.isEnabled();render=()=>scene.render();names=()=>scene.meshes.map(x=>x.name);
let ticks=0,score=0;
const state=()=>({ticks,score,playerX:Number(getX().toFixed(6)),lanterns:lanterns.map(x=>({name:x.name,visible:getVisible(x)})),names:names()});
function reset(){ticks=0;score=0;setX(0);lanterns.forEach(x=>setVisible(x,true));render();return state();}
function tick(){ticks++;setX(ticks*.05);lanterns.forEach((x,i)=>{if(getVisible(x)&&Math.abs(getX()-(i+1))<.026){setVisible(x,false);score++;}});render();return state();}
render();if(name==='babylon'){await scene.whenReadyAsync();render();}window.fixture={state,reset,tick,readyMs:performance.now()-start,engine:name};
