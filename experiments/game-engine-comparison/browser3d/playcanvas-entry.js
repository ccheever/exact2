const start = performance.now();
const name = "playcanvas";
const canvas=document.querySelector('canvas');
let scene, player, lanterns, setX, getX, setVisible, getVisible, render, names;

 const P=await import('playcanvas');
 const app=new P.Application(canvas,{graphicsDeviceOptions:{antialias:false}});scene=app.root;app.setCanvasResolution(P.RESOLUTION_FIXED,640,360);
 const camera=new P.Entity('camera');camera.addComponent('camera',{clearColor:new P.Color(.094,.145,.21)});camera.setPosition(5,8,12);camera.lookAt(2,0,0);scene.addChild(camera);
 const mesh=(n,x,color,ground=false)=>{const m=new P.Entity(n);m.addComponent('render',{type:ground?'plane':'box'});m.setLocalScale(ground?12:.5,ground?1:1,ground?8:.5);m.setPosition(x,ground?0:.5,0);const mat=new P.StandardMaterial();mat.useLighting=false;mat.emissive=new P.Color().fromString(color);mat.update();m.render.material=mat;scene.addChild(m);return m;};
 mesh('plane',2,'#334b45',true);player=mesh('player',0,'#4caaff');lanterns=[1,2,3].map(x=>mesh(`lantern-${x}`,x,'#ffc24b'));
 setX=x=>player.setPosition(x,.5,0);getX=()=>player.getPosition().x;setVisible=(m,v)=>m.enabled=v;getVisible=m=>m.enabled;render=()=>{app.update(1/60);app.render();};names=()=>scene.children.map(x=>x.name);let ticks=0,score=0;
const state=()=>({ticks,score,playerX:Number(getX().toFixed(6)),lanterns:lanterns.map(x=>({name:x.name,visible:getVisible(x)})),names:names()});
function reset(){ticks=0;score=0;setX(0);lanterns.forEach(x=>setVisible(x,true));render();return state();}
function tick(){ticks++;setX(ticks*.05);lanterns.forEach((x,i)=>{if(getVisible(x)&&Math.abs(getX()-(i+1))<.026){setVisible(x,false);score++;}});render();return state();}
render();if(name==='babylon'){await scene.whenReadyAsync();render();}window.fixture={state,reset,tick,readyMs:performance.now()-start,engine:name};
