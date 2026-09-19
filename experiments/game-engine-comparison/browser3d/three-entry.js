const start = performance.now();
const name = "three";
const canvas=document.querySelector('canvas');
let scene, player, lanterns, setX, getX, setVisible, getVisible, render, names;

 const T=await import('three');
 scene=new T.Scene(); scene.background=new T.Color('#182536');
 const camera=new T.PerspectiveCamera(50,640/360,.1,100); camera.position.set(5,8,12); camera.lookAt(2,0,0);
 const renderer=new T.WebGLRenderer({canvas,antialias:false}); renderer.setSize(640,360);
 const mesh=(n,x,color,ground=false)=>{const m=new T.Mesh(ground?new T.PlaneGeometry(12,8):new T.BoxGeometry(.5,1,.5),new T.MeshBasicMaterial({color}));m.name=n;m.position.set(x,ground?0:.5,0);if(ground)m.rotation.x=-Math.PI/2;scene.add(m);return m;};
 mesh('plane',2,'#334b45',true);player=mesh('player',0,'#4caaff');lanterns=[1,2,3].map(x=>mesh(`lantern-${x}`,x,'#ffc24b'));
 setX=x=>player.position.x=x;getX=()=>player.position.x;setVisible=(m,v)=>m.visible=v;getVisible=m=>m.visible;render=()=>renderer.render(scene,camera);names=()=>scene.children.map(x=>x.name);
let ticks=0,score=0;
const state=()=>({ticks,score,playerX:Number(getX().toFixed(6)),lanterns:lanterns.map(x=>({name:x.name,visible:getVisible(x)})),names:names()});
function reset(){ticks=0;score=0;setX(0);lanterns.forEach(x=>setVisible(x,true));render();return state();}
function tick(){ticks++;setX(ticks*.05);lanterns.forEach((x,i)=>{if(getVisible(x)&&Math.abs(getX()-(i+1))<.026){setVisible(x,false);score++;}});render();return state();}
render();if(name==='babylon'){await scene.whenReadyAsync();render();}window.fixture={state,reset,tick,readyMs:performance.now()-start,engine:name};
