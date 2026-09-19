import * as pc from "playcanvas/build/playcanvas.min.mjs";
import { GameAudio } from "./audio.js";

const DT = 1 / 60;
const SAVE_KEY = "lanterns-playcanvas-save-v1";
const $ = (id) => document.getElementById(id);
const canvas = $("game");
const agentMode = new URLSearchParams(location.search).get("agent") === "1";
const level = await fetch("./level.json").then((response) => response.json());
const Ammo = await globalThis.Ammo({ locateFile: () => "./assets/ammo.wasm.wasm" });

const app = new pc.Application(canvas, {
  graphicsDeviceOptions: { antialias: true, alpha: false, powerPreference: "high-performance" }
});
app.setCanvasFillMode(pc.FILLMODE_FILL_WINDOW);
app.setCanvasResolution(pc.RESOLUTION_AUTO, Math.min(devicePixelRatio, 1.75));
app.scene.ambientLight = new pc.Color(.24, .22, .32);
app.scene.exposure = 1.25;
app.scene.gammaCorrection = pc.GAMMA_SRGB;
app.start();
app.autoRender = !agentMode;

const audio = new GameAudio();
const tmpTransform = new Ammo.btTransform();
const tmpOrigin = new Ammo.btVector3(0, 0, 0);
const tmpVelocity = new Ammo.btVector3(0, 0, 0);
let levelData = null;
let fox = null;
let foxAnimation = "";
const foxAnimations = new Set();
let phase = "title";
let ticks = 0;
let accumulator = 0;
let lastTime = performance.now();
let count = 0;
let ready = false;
let events = [];
let commandInput = { moveX: 0, moveZ: 0, jump: false, act: false };
let commandEdges = { jump: false, act: false };
let keyboardEdges = { jump: false, act: false };
const keys = new Set();
const touch = new Set();
const renderBodies = [];
const lanternViews = [];

const collisionConfig = new Ammo.btDefaultCollisionConfiguration();
const dispatcher = new Ammo.btCollisionDispatcher(collisionConfig);
const broadphase = new Ammo.btDbvtBroadphase();
const solver = new Ammo.btSequentialImpulseConstraintSolver();
const world = new Ammo.btDiscreteDynamicsWorld(dispatcher, broadphase, solver, collisionConfig);
world.setGravity(new Ammo.btVector3(0, -level.player.gravity, 0));

const material = (color, metalness = 0, gloss = .45, emissive = null) => {
  const value = new pc.StandardMaterial();
  value.diffuse = new pc.Color(...color);
  value.metalness = metalness;
  value.gloss = gloss;
  if (emissive) { value.emissive = new pc.Color(...emissive); value.emissiveIntensity = 2.5; }
  value.update();
  return value;
};
const mats = {
  grass: material([.20,.34,.24],0,.22), edge: material([.18,.13,.12],0,.15), stone: material([.29,.27,.31],0,.25),
  crate: material([.42,.22,.11],0,.18), wood: material([.25,.12,.06],0,.16), dark: material([.06,.055,.075],.25,.55),
  glass: material([.16,.12,.08],.1,.65), gold: material([.95,.52,.12],.1,.55,[1,.25,.015]), leaf: material([.11,.23,.18],0,.18)
};

function boxEntity(name, position, size, mat, cast = true) {
  const entity = new pc.Entity(name);
  entity.addComponent("render", { type: "box", castShadows: cast, receiveShadows: true });
  entity.render.material = mat;
  entity.setPosition(...position);
  entity.setLocalScale(...size);
  app.root.addChild(entity);
  return entity;
}

function addRigidBox(name, position, size, mass, entity, friction = .7) {
  const shape = new Ammo.btBoxShape(new Ammo.btVector3(size[0]/2, size[1]/2, size[2]/2));
  shape.setMargin(.025);
  return addRigidBody(name, position, shape, mass, entity, friction);
}

function addRigidBody(name, position, shape, mass, entity, friction) {
  const transform = new Ammo.btTransform(); transform.setIdentity();
  transform.setOrigin(new Ammo.btVector3(...position));
  const motion = new Ammo.btDefaultMotionState(transform);
  const inertia = new Ammo.btVector3(0,0,0);
  if (mass) shape.calculateLocalInertia(mass, inertia);
  const info = new Ammo.btRigidBodyConstructionInfo(mass, motion, shape, inertia);
  const body = new Ammo.btRigidBody(info);
  body.setFriction(friction);
  body.setRestitution(0);
  if (mass) { body.setActivationState(4); body.setDamping(.08,.92); }
  body.name = name;
  world.addRigidBody(body);
  if (entity) renderBodies.push({ body, entity });
  return body;
}

function bodyPosition(body) {
  body.getMotionState().getWorldTransform(tmpTransform);
  const p = tmpTransform.getOrigin();
  return { x:p.x(), y:p.y(), z:p.z() };
}

function bodyVelocity(body) {
  const v = body.getLinearVelocity();
  return { x:v.x(), y:v.y(), z:v.z() };
}

function setBody(body, position, velocity = [0,0,0]) {
  const transform = body.getWorldTransform();
  tmpOrigin.setValue(...position); transform.setOrigin(tmpOrigin);
  body.setWorldTransform(transform); body.getMotionState().setWorldTransform(transform);
  tmpVelocity.setValue(...velocity); body.setLinearVelocity(tmpVelocity);
  tmpVelocity.setValue(0,0,0); body.setAngularVelocity(tmpVelocity); body.activate();
}

const island = boxEntity("island", level.ground.position, level.ground.size, mats.grass);
addRigidBox("ground", level.ground.position, level.ground.size, 0, island, .95);
for (const obstacle of level.obstacles) {
  const entity = boxEntity(obstacle.id, obstacle.position, obstacle.size, obstacle.id === "ledge" ? mats.stone : mats.edge);
  addRigidBox(obstacle.id, obstacle.position, obstacle.size, 0, entity, .85);
}
const crateEntity = boxEntity("pushable crate", level.crate.position, level.crate.size, mats.crate);
const crateBody = addRigidBox("crate", level.crate.position, level.crate.size, level.crate.mass, crateEntity, .62);
crateBody.setAngularFactor(new Ammo.btVector3(.15,.9,.15));

const capsuleHeight = level.player.height - level.player.radius * 2;
const playerShape = new Ammo.btCapsuleShape(level.player.radius, capsuleHeight);
playerShape.setMargin(.02);
const playerBody = addRigidBody("player", [level.spawn[0], level.spawn[1] + level.player.height/2, level.spawn[2]], playerShape, 1.2, null, .18);
playerBody.setAngularFactor(new Ammo.btVector3(0,0,0));
playerBody.setDamping(.03,.98);

const sun = new pc.Entity("sun");
sun.addComponent("light", { type:"directional", color:new pc.Color(1,.56,.34), intensity:1.65, castShadows:true, shadowResolution:2048, shadowDistance:45, shadowBias:.2, normalOffsetBias:.08 });
sun.setEulerAngles(42,-38,20); app.root.addChild(sun);
const sky = new pc.Entity("sky glow"); sky.addComponent("light",{type:"omni",color:new pc.Color(.38,.30,.62),intensity:.32,range:45}); sky.setPosition(0,10,0); app.root.addChild(sky);

const camera = new pc.Entity("camera");
camera.addComponent("camera", { clearColor:new pc.Color(.055,.065,.13), fov:48, farClip:100 });
camera.setPosition(0,12,24); app.root.addChild(camera);

function cylinder(name, position, radius, height, mat) {
  const entity = new pc.Entity(name); entity.addComponent("render",{type:"cylinder",castShadows:true});
  entity.render.material=mat; entity.setPosition(...position); entity.setLocalScale(radius*2,height,radius*2); app.root.addChild(entity); return entity;
}

for (const item of level.lanterns) {
  const root = new pc.Entity(item.id); root.setPosition(...item.position); app.root.addChild(root);
  const post = new pc.Entity("post"); post.addComponent("render",{type:"cylinder",castShadows:true}); post.render.material=mats.dark; post.setLocalPosition(0,.55,0); post.setLocalScale(.12,1.05,.12); root.addChild(post);
  const lamp = new pc.Entity("lamp"); lamp.addComponent("render",{type:"sphere",castShadows:true}); lamp.render.material=mats.glass; lamp.setLocalPosition(0,1.12,0); lamp.setLocalScale(.43,.55,.43); root.addChild(lamp);
  const cap = new pc.Entity("cap"); cap.addComponent("render",{type:"cylinder",castShadows:true}); cap.render.material=mats.dark; cap.setLocalPosition(0,1.43,0); cap.setLocalScale(.32,.10,.32); root.addChild(cap);
  const light = new pc.Entity("glow"); light.addComponent("light",{type:"omni",color:new pc.Color(1,.45,.1),intensity:0,range:5,castShadows:false}); light.setLocalPosition(0,1.13,0); root.addChild(light);
  lanternViews.push({ ...item, lit:false, root, lamp, light });
}

function makeTree(x,z,scale) {
  cylinder("tree trunk",[x,.75*scale,z],.18*scale,1.5*scale,mats.wood);
  const crown = new pc.Entity("tree crown"); crown.addComponent("render",{type:"cone",castShadows:true}); crown.render.material=mats.leaf; crown.setPosition(x,2.15*scale,z); crown.setLocalScale(1.25*scale,2.3*scale,1.25*scale); app.root.addChild(crown);
}
function seeded(seed) { let s=seed>>>0; return ()=>((s=Math.imul(1664525,s)+1013904223>>>0)/4294967296); }
const random=seeded(level.seed);
for(let i=0;i<18;i++) { const angle=random()*Math.PI*2, radius=14+random()*2.5; makeTree(Math.cos(angle)*radius,Math.sin(angle)*radius,.65+random()*.55); }
for(let i=0;i<16;i++) { const x=(random()-.5)*31,z=(random()-.5)*31;if(Math.hypot(x,z-level.spawn[2])>3&&Math.hypot(x-10,z-8)>5){const r=.18+random()*.35;const rock=new pc.Entity("rock");rock.addComponent("render",{type:"sphere",castShadows:true});rock.render.material=mats.stone;rock.setPosition(x,r*.55,z);rock.setLocalScale(r*1.4,r,r);app.root.addChild(rock);} }

const signPost = cylinder("sign post",[level.sign.position[0],.7,level.sign.position[2]],.1,1.4,mats.wood);
const signBoard = boxEntity("puzzle sign",[level.sign.position[0],1.45,level.sign.position[2]],[2.5,.8,.16],mats.wood);
signBoard.setEulerAngles(0,-20,0);

async function loadFox() {
  const asset = new pc.Asset("Fox", "container", { url:"./assets/Fox.glb" });
  app.assets.add(asset);
  await new Promise((resolve,reject)=>{asset.once("load",resolve);asset.once("error",reject);app.assets.load(asset);});
  fox = asset.resource.instantiateRenderEntity();
  fox.name = "animated fox"; fox.setLocalScale(.022,.022,.022); app.root.addChild(fox);
  fox.addComponent("anim",{activate:true,rootBone:fox});
  for (const animation of asset.resource.animations) {
    const resource = animation.resource || animation;
    const clipName = String(resource.name || animation.name).split("/").pop();
    fox.anim.assignAnimation(clipName, resource);
    foxAnimations.add(clipName);
  }
  playFox("Survey",0);
}

function playFox(name, blend=.18) {
  if (!fox || foxAnimation===name) return;
  const chosen = [name, name.toLowerCase(), name.toUpperCase()].find((candidate)=>foxAnimations.has(candidate));
  if (!chosen) return;
  fox.anim.baseLayer.transition(chosen,blend); foxAnimation=name;
}

function playerFeet() { const p=bodyPosition(playerBody); return {x:p.x,y:p.y-level.player.height/2,z:p.z}; }
function grounded() {
  const p=bodyPosition(playerBody);
  const from=new Ammo.btVector3(p.x,p.y,p.z), to=new Ammo.btVector3(p.x,p.y-level.player.height/2-.11,p.z);
  const callback=new Ammo.ClosestRayResultCallback(from,to); world.rayTest(from,to,callback);
  const result=callback.hasHit(); Ammo.destroy(callback);Ammo.destroy(from);Ammo.destroy(to); return result;
}

function activeInput() {
  let x=commandInput.moveX,z=commandInput.moveZ;
  if(keys.has("KeyA")||keys.has("ArrowLeft")||touch.has("left"))x-=1;
  if(keys.has("KeyD")||keys.has("ArrowRight")||touch.has("right"))x+=1;
  if(keys.has("KeyW")||keys.has("ArrowUp")||touch.has("up"))z-=1;
  if(keys.has("KeyS")||keys.has("ArrowDown")||touch.has("down"))z+=1;
  const length=Math.hypot(x,z); if(length>1){x/=length;z/=length;} return {x,z,magnitude:Math.min(1,length)};
}

function emit(type, detail={}) { events.push({type,tick:ticks,...detail}); if(events.length>64) events=events.slice(-64); }
function nearestLantern() { const p=playerFeet(); let best=null,distance=Infinity;for(const lamp of lanternViews){if(lamp.lit)continue;const d=Math.hypot(p.x-lamp.position[0],p.y-lamp.position[1],p.z-lamp.position[2]);if(d<distance){distance=d;best=lamp;}}return distance<=1.5?best:null; }
function lightNearest() { const lamp=nearestLantern();if(!lamp)return;lamp.lit=true;lamp.lamp.render.material=mats.gold;lamp.light.light.intensity=1.45;count++;emit("lantern-lit",{id:lamp.id,count});audio.lantern();if(count===lanternViews.length){phase="won";emit("win");audio.win();updateScreens();} }

function simulateTick() {
  if(phase!=="playing")return;
  ticks++;
  const move=activeInput(), velocity=bodyVelocity(playerBody), onGround=grounded();
  const speed=level.player.speed;
  tmpVelocity.setValue(move.x*speed,velocity.y,move.z*speed); playerBody.setLinearVelocity(tmpVelocity);
  playerBody.activate();
  const jump=commandEdges.jump||keyboardEdges.jump||touch.has("jump");
  const act=commandEdges.act||keyboardEdges.act||touch.has("act");
  commandEdges.jump=false;commandEdges.act=false;keyboardEdges.jump=false;keyboardEdges.act=false;touch.delete("jump");touch.delete("act");
  if(jump&&onGround){const v=bodyVelocity(playerBody);tmpVelocity.setValue(v.x,level.player.jump,v.z);playerBody.setLinearVelocity(tmpVelocity);emit("jump");audio.jump();}
  if(act)lightNearest();
  world.stepSimulation(DT,1,DT);
  const feet=playerFeet();
  if(feet.y < -4)setBody(playerBody,[level.spawn[0],level.spawn[1]+level.player.height/2,level.spawn[2]]);
  if(ticks>=level.duration*60&&phase==="playing"){phase="lost";emit("lose");audio.lose();updateScreens();}
  const name=move.magnitude===0?"Survey":move.magnitude<.65?"Walk":"Run";playFox(name);
}

function syncRender() {
  for(const {body,entity} of renderBodies){body.getMotionState().getWorldTransform(tmpTransform);const p=tmpTransform.getOrigin(),q=tmpTransform.getRotation();entity.setPosition(p.x(),p.y(),p.z());entity.setRotation(q.x(),q.y(),q.z(),q.w());}
  const p=playerFeet();
  if(fox){fox.setPosition(p.x,p.y,p.z);const v=bodyVelocity(playerBody);if(Math.hypot(v.x,v.z)>.05)fox.setEulerAngles(0,Math.atan2(v.x,v.z)*180/Math.PI,0);}
  const target=new pc.Vec3(p.x,p.y+1.2,p.z);
  const desired=new pc.Vec3(p.x+7,p.y+9,p.z+13); camera.setPosition(camera.getPosition().lerp(camera.getPosition(),desired,.08));camera.lookAt(target);
  $("count").textContent=`${count} / ${lanternViews.length}`;
  const remaining=Math.max(0,level.duration-ticks/60);$("time").textContent=formatTime(Math.ceil(remaining));
  $("prompt").classList.toggle("hidden",phase!=="playing"||!nearestLantern());
  $("sign-copy").classList.toggle("hidden",phase!=="playing"||Math.hypot(p.x-level.sign.position[0],p.z-level.sign.position[2])>3.2);
  if(agentMode)app.renderNextFrame=true;
}

function frame(now) {
  const delta=Math.min(.1,(now-lastTime)/1000);lastTime=now;
  if(!agentMode&&phase==="playing"){accumulator+=delta;while(accumulator>=DT){simulateTick();accumulator-=DT;}}
  if(!agentMode)syncRender();requestAnimationFrame(frame);
}

function fresh(next="title") {
  ticks=0;count=0;phase=next;events=[];accumulator=0;commandInput={moveX:0,moveZ:0,jump:false,act:false};keys.clear();touch.clear();
  setBody(playerBody,[level.spawn[0],level.spawn[1]+level.player.height/2,level.spawn[2]]);
  setBody(crateBody,level.crate.position);
  for(const lamp of lanternViews){lamp.lit=false;lamp.lamp.render.material=mats.glass;lamp.light.light.intensity=0;}
  playFox("Survey");updateScreens();syncRender();
}

function state() {
  const p=playerFeet(),pv=bodyVelocity(playerBody),c=bodyPosition(crateBody),cv=bodyVelocity(crateBody);
  return {phase,ticks,elapsed:ticks/60,remaining:Math.max(0,level.duration-ticks/60),player:{x:p.x,y:p.y,z:p.z,vx:pv.x,vy:pv.y,vz:pv.z,grounded:grounded(),animation:foxAnimation},crate:{x:c.x,y:c.y-level.crate.size[1]/2,z:c.z,vx:cv.x,vy:cv.y,vz:cv.z},lanterns:lanternViews.map((l)=>({id:l.id,x:l.position[0],y:l.position[1],z:l.position[2],lit:l.lit})),count,total:lanternViews.length,events:[...events],assetReady:ready};
}

function updateScreens() {
  $("title").classList.toggle("hidden",phase!=="title");$("title").classList.toggle("show",phase==="title");
  $("pause-screen").classList.toggle("hidden",phase!=="paused");$("pause-screen").classList.toggle("show",phase==="paused");
  const terminal=phase==="won"||phase==="lost";$("terminal").classList.toggle("hidden",!terminal);$("terminal").classList.toggle("show",terminal);
  $("hud").classList.toggle("hidden",phase==="title");$("touch").classList.toggle("active",phase==="playing");
  if(terminal){const won=phase==="won";$("terminal-kicker").textContent=won?"THE ISLAND SHINES":"NIGHT HAS FALLEN";$("terminal-title").textContent=won?"All twelve glow":"Last light lost";$("terminal-copy").textContent=won?`Every lantern burns with ${formatTime(ticks/60)} elapsed.`:`You lit ${count} of ${lanternViews.length}. The island will wait for another dusk.`;}
  $("resume-save").classList.toggle("hidden",!localStorage.getItem(SAVE_KEY));
}

function formatTime(seconds){return `${Math.floor(seconds/60)}:${String(Math.floor(seconds%60)).padStart(2,"0")}`;}
function saveGame(){emit("save");const snapshot=state();localStorage.setItem(SAVE_KEY,JSON.stringify(snapshot));return {saved:true};}
function loadGame(){const raw=localStorage.getItem(SAVE_KEY);if(!raw)return state();const saved=JSON.parse(raw);ticks=saved.ticks;phase=saved.phase;events=saved.events||[];count=0;for(const lamp of lanternViews){const source=saved.lanterns.find((l)=>l.id===lamp.id);lamp.lit=!!source?.lit;lamp.lamp.render.material=lamp.lit?mats.gold:mats.glass;lamp.light.light.intensity=lamp.lit?1.45:0;if(lamp.lit)count++;}setBody(playerBody,[saved.player.x,saved.player.y+level.player.height/2,saved.player.z],[saved.player.vx,saved.player.vy,saved.player.vz]);setBody(crateBody,[saved.crate.x,saved.crate.y+level.crate.size[1]/2,saved.crate.z],[saved.crate.vx,saved.crate.vy,saved.crate.vz]);commandInput={moveX:0,moveZ:0,jump:false,act:false};commandEdges={jump:false,act:false};keys.clear();touch.clear();emit("load");updateScreens();syncRender();return state();}

function togglePause(){if(phase==="playing")phase="paused";else if(phase==="paused")phase="playing";accumulator=0;lastTime=performance.now();updateScreens();return state();}

window.lanterns={command:async(request={})=>{
  switch(request.op){
    case "ready":return {ready,engine:"PlayCanvas",version:"2.22.2"};
    case "start":fresh("playing");return state();
    case "reset":fresh("title");return state();
    case "state":return state();
    case "input":{const previous=commandInput;const next={moveX:Number(request.moveX)||0,moveZ:Number(request.moveZ)||0,jump:request.jump===true,act:request.act===true};commandEdges.jump ||= next.jump&&!previous.jump;commandEdges.act ||= next.act&&!previous.act;commandInput=next;return state();}
    case "step":{const amount=Math.max(0,Math.min(36000,Math.floor(request.ticks||0)));for(let i=0;i<amount;i++)simulateTick();syncRender();return state();}
    case "pause":return togglePause();
    case "save":return saveGame();
    case "load":return loadGame();
    default:throw new Error(`Unknown lanterns command: ${request.op}`);
  }
}};

addEventListener("keydown",(event)=>{if(["ArrowUp","ArrowDown","ArrowLeft","ArrowRight","Space"].includes(event.code))event.preventDefault();if(!event.repeat){if(event.code==="Space")keyboardEdges.jump=true;if(event.code==="KeyE")keyboardEdges.act=true;if(event.code==="Escape"&&(phase==="playing"||phase==="paused"))togglePause();}keys.add(event.code);});
addEventListener("keyup",(event)=>keys.delete(event.code));addEventListener("blur",()=>{keys.clear();touch.clear();keyboardEdges={jump:false,act:false};});
for(const button of document.querySelectorAll("[data-key]")){const key=button.dataset.key;button.addEventListener("pointerdown",(e)=>{e.preventDefault();button.setPointerCapture(e.pointerId);touch.add(key);});for(const type of ["pointerup","pointercancel","lostpointercapture"])button.addEventListener(type,()=>touch.delete(key));}
$("start").onclick=()=>fresh("playing");$("resume-save").onclick=()=>loadGame();$("pause").onclick=()=>togglePause();$("continue").onclick=()=>togglePause();$("save").onclick=()=>{$("save-note").textContent="Run saved on this browser.";saveGame();};$("load").onclick=()=>loadGame();$("quit").onclick=()=>fresh("title");$("restart").onclick=()=>fresh("playing");$("terminal-title-button").onclick=()=>fresh("title");$("sound").onclick=()=>{$("sound").textContent=audio.toggle()?"♪":"×";};

await loadFox();
ready=true;fresh("title");$("loading").classList.remove("show");setTimeout(()=>$("loading").classList.add("hidden"),260);requestAnimationFrame(frame);
