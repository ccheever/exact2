const start = performance.now();
const name = new URLSearchParams(location.search).get('engine');
const canvas=document.querySelector('canvas');
let scene, player, lanterns, setX, getX, setVisible, getVisible, render, names;
if(name==='three') {
 const T=await import('/node_modules/three/build/three.module.js');
 scene=new T.Scene(); scene.background=new T.Color('#182536');
 const camera=new T.PerspectiveCamera(50,640/360,.1,100); camera.position.set(5,8,12); camera.lookAt(2,0,0);
 const renderer=new T.WebGLRenderer({canvas,antialias:false}); renderer.setSize(640,360);
 const mesh=(n,x,color,ground=false)=>{const m=new T.Mesh(ground?new T.PlaneGeometry(12,8):new T.BoxGeometry(.5,1,.5),new T.MeshBasicMaterial({color}));m.name=n;m.position.set(x,ground?0:.5,0);if(ground)m.rotation.x=-Math.PI/2;scene.add(m);return m;};
 mesh('plane',2,'#334b45',true);player=mesh('player',0,'#4caaff');lanterns=[1,2,3,4].map(x=>mesh(`lantern-${x}`,x,'#ffc24b'));
 setX=x=>player.position.x=x;getX=()=>player.position.x;setVisible=(m,v)=>m.visible=v;getVisible=m=>m.visible;render=()=>renderer.render(scene,camera);names=()=>scene.children.map(x=>x.name);
} else if(name==='babylon') {
 const B=await import('/node_modules/@babylonjs/core/index.js');
 const engine=new B.Engine(canvas,false,{preserveDrawingBuffer:true});scene=new B.Scene(engine);scene.clearColor=new B.Color4(.094,.145,.21,1);
 const camera=new B.FreeCamera('camera',new B.Vector3(5,8,12),scene);camera.setTarget(new B.Vector3(2,0,0));
 const mesh=(n,x,color,ground=false)=>{const m=ground?B.MeshBuilder.CreateGround(n,{width:12,height:8},scene):B.MeshBuilder.CreateBox(n,{width:.5,height:1,depth:.5},scene);m.position.set(x,ground?0:.5,0);const mat=new B.StandardMaterial(n+'-mat',scene);mat.emissiveColor=B.Color3.FromHexString(color);mat.disableLighting=true;m.material=mat;return m;};
 mesh('plane',2,'#334b45',true);player=mesh('player',0,'#4caaff');lanterns=[1,2,3,4].map(x=>mesh(`lantern-${x}`,x,'#ffc24b'));
 setX=x=>player.position.x=x;getX=()=>player.position.x;setVisible=(m,v)=>m.setEnabled(v);getVisible=m=>m.isEnabled();render=()=>scene.render();names=()=>scene.meshes.map(x=>x.name);
} else if(name==='playcanvas') {
 const P=await import('/node_modules/playcanvas/build/playcanvas.mjs');
 const app=new P.Application(canvas,{graphicsDeviceOptions:{antialias:false}});scene=app.root;app.setCanvasResolution(P.RESOLUTION_FIXED,640,360);
 const camera=new P.Entity('camera');camera.addComponent('camera',{clearColor:new P.Color(.094,.145,.21)});camera.setPosition(5,8,12);camera.lookAt(2,0,0);scene.addChild(camera);
 const mesh=(n,x,color,ground=false)=>{const m=new P.Entity(n);m.addComponent('render',{type:ground?'plane':'box'});m.setLocalScale(ground?12:.5,ground?1:1,ground?8:.5);m.setPosition(x,ground?0:.5,0);const mat=new P.StandardMaterial();mat.useLighting=false;mat.emissive=new P.Color().fromString(color);mat.update();m.render.material=mat;scene.addChild(m);return m;};
 mesh('plane',2,'#334b45',true);player=mesh('player',0,'#4caaff');lanterns=[1,2,3,4].map(x=>mesh(`lantern-${x}`,x,'#ffc24b'));
 setX=x=>player.setPosition(x,.5,0);getX=()=>player.getPosition().x;setVisible=(m,v)=>m.enabled=v;getVisible=m=>m.enabled;render=()=>{app.update(1/60);app.render();};names=()=>scene.children.map(x=>x.name);
}
let ticks=0,score=0,actionHeld=false;
const state=()=>({ticks,score,actionHeld,playerX:Number(getX().toFixed(6)),lanterns:lanterns.map(x=>({name:x.name,visible:getVisible(x)})),names:names()});
function reset(){ticks=0;score=0;actionHeld=false;setX(0);lanterns.forEach(x=>setVisible(x,true));render();return state();}
function tick(){ticks++;setX(ticks*.05);lanterns.forEach((x,i)=>{if(i<4&&getVisible(x)&&Math.abs(getX()-(i+1))<.026){setVisible(x,false);score++;}});render();return state();}
function action(down){const edge=down&&!actionHeld;actionHeld=down;if(edge&&getVisible(lanterns[3])&&Math.abs(getX()-4)<.026){setVisible(lanterns[3],false);score++;}render();return state();}
function save(){return JSON.parse(JSON.stringify(state()));}
function restore(s){ticks=s.ticks;score=s.score;actionHeld=s.actionHeld;setX(s.playerX);lanterns.forEach((l,i)=>setVisible(l,s.lanterns[i].visible));render();return state();}
render();if(name==='babylon'){await scene.whenReadyAsync();render();}window.fixture={state,reset,tick,action,save,restore,readyMs:performance.now()-start,engine:name};
