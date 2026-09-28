// Springs, holds and drags on the web (LLP 1002, LLP 1041 §8): the motion and
// arrange controllers, an after-paint piece fetched when a batch first needs
// one (LLP 1047 D5); navigation.js stands in until then.
// Presentation ownership is a host projection. The shared Engine owns hold
// validity and authored targets; this controller owns actual browser sampling.
export function motionBytes(facts) {
  const {op,view=0,property='translate',token=0,x=0,y=0,now=0}=facts;
  const reorder=['reorder-begin','reorder-preview','reorder-terminal','reorder-cancel','reorder-rebase','reorder-finish'].indexOf(op);
  if(reorder>=0) {
    const rows=facts.rows??[];if(rows.length>4096)throw Error('too many reorder samples');
    const bytes=new Uint8Array(176+32*rows.length),d=new DataView(bytes.buffer);
    const u64=(at,value=0)=>{if(typeof value==='number'&&!Number.isSafeInteger(value))throw Error('unsafe reorder identity');
      const n=BigInt(value);if(n<0n||n>0xffffffffffffffffn)throw Error('invalid reorder identity');d.setBigUint64(at,n,true);};
    d.setUint32(0,3,true);d.setUint32(4,15+reorder,true);
    ['runtime','handleKey','listKey','wrapperKey','rootKey','rowEpoch','token','revision','scrollSequence'].forEach((k,i)=>u64(8+i*8,facts[k]));
    d.setUint32(80,rows.length,true);
    ['scrollTop','portWidth','portHeight','rowWidth','totalExtent','contentY','x','y','vx','vy','now'].forEach((k,i)=>d.setFloat64(88+i*8,facts[k]??0,true));
    rows.forEach((r,i)=>{u64(176+i*32,r.key);u64(184+i*32,r.hold);d.setFloat64(192+i*32,r.value[0],true);d.setFloat64(200+i*32,r.value[1],true);});
    return bytes;
  }
  const transform=['transform-geometry','transform-begin','transform-move','transform-action','transform-invalidate'].indexOf(op);
  if(transform>=0) {
    // Frozen Rust counterpart: v2, 120 LE bytes; serials never pass through Number.
    const bytes=new Uint8Array(120),d=new DataView(bytes.buffer);
    d.setUint32(0,2,true);d.setUint32(4,10+transform,true);
    for(const [i,name] of ['runtime','handleKey','targetKey','clipKey','geometrySequence','translateToken','scaleToken'].entries()) {
      const value=facts[name]??0;
      if(typeof value==='number'&&!Number.isSafeInteger(value))throw Error('unsafe transform identity');
      const n=BigInt(value);if(n<0n||n>0xffffffffffffffffn)throw Error('invalid transform identity');
      d.setBigUint64(8+i*8,n,true);
    }
    if(!Array.isArray(facts.values)||facts.values.length!==6)throw Error('invalid transform tuple');
    for(let i=0;i<6;i++)d.setFloat64(64+i*8,facts.values[i],true);
    d.setFloat64(112,now,true);return bytes;
  }
  const operations=['begin','move','release','cancel','live','action','height-owner','clear-height-owner','height-begin','height-action','release-measured','gesture','track'];
  const properties=['translate','scale','rotate','opacity','height'];
  const code=operations.indexOf(op), prop=properties.indexOf(property);
  if(code<0||prop<0) throw Error('invalid motion operation');
  const bytes=new Uint8Array(48), d=new DataView(bytes.buffer), serial=BigInt(token);
  if(serial<0n||serial>0xffffffffffffffffn || typeof token==='number'&&!Number.isSafeInteger(token)) throw Error('invalid hold serial');
  d.setUint32(0,1,true); d.setUint32(4,code,true); d.setUint32(8,view,true); d.setUint32(12,prop,true);
  d.setBigUint64(16,serial,true); d.setFloat64(24,x,true); d.setFloat64(32,y,true); d.setFloat64(40,now,true);
  return bytes;
}
// @ref LLP 1057.003 D3 — a consumer's animation while its timeline's source
// plays a release spring. `p` is the timeline's progress at each of the
// spring's evenly spaced frames; the answer is a `linear()` easing of the
// animation's directed progress (Web Animations 1 §4.8) over the spring's
// duration, for a copy of its keyframes. The frames interpolate linearly, so
// with a point wherever the local time crosses the delay, an iteration or
// the end it is exact between frames too. `undefined` for an endless
// animation, which holds its start; `null` where no easing can say it: a
// fill that leaves the animation out of effect where the spring goes.
export function timelineEasing(timing,p) {
  const {delay=0,duration,iterations=1,iterationStart=0,direction='normal',fill='none',endTime}=timing;
  if(!Number.isFinite(endTime))return undefined;
  if(!(duration>0)||!Number.isFinite(iterations)||iterations>1000||p.length<2)return null;
  const active=duration*iterations,before=Math.max(Math.min(delay,endTime),0),after=Math.max(Math.min(delay+active,endTime),0);
  const L=p.map(v=>v*endTime),lo=Math.min(...L),hi=Math.max(...L);
  if(!L.every(Number.isFinite)||lo<before&&fill!=='backwards'&&fill!=='both'||hi>=after&&fill!=='forwards'&&fill!=='both')return null;
  const q=t=>{
    const phase=t<before?-1:t>=after?1:0;
    const at=phase<0?Math.max(t-delay,0):phase>0?Math.max(Math.min(t-delay,active),0):t-delay;
    const overall=at/duration+iterationStart;
    let simple=overall%1;
    if(simple===0&&phase>=0&&at===active&&iterations!==0)simple=1;
    const odd=(simple===1?Math.floor(overall)-1:Math.floor(overall))%2!==0;
    return direction==='reverse'||direction==='alternate'&&odd||direction==='alternate-reverse'&&!odd?1-simple:simple;
  };
  const breaks=[before,after];
  for(let k=Math.floor(iterationStart)+1;k<iterationStart+iterations;k++)breaks.push(delay+(k-iterationStart)*duration);
  // Each segment's ends are limits from inside it, so a jump that falls on
  // a frame (an iteration's wrap) is two points at one input.
  const n=L.length-1,points=[],e=1e-7;
  const push=(x,y)=>{const last=points.at(-1);if(!last||last[0]!==x||Math.abs(last[1]-y)>1e-9)points.push([x,y]);};
  for(let i=0;i<n;i++) {
    const a=L[i],b=L[i+1],s=Math.sign(b-a),x0=i/n,x1=(i+1)/n;
    push(x0,q(a+e*s));
    for(const t of breaks.filter(t=>(t-a)*(t-b)<0).sort((u,v)=>(u-v)*s)) {
      const x=x0+(t-a)/(b-a)*(x1-x0);push(x,q(t-e*s));push(x,q(t+e*s));
    }
    push(x1,q(b-e*s));
  }
  return `linear(${points.map(([x,y])=>`${+y.toPrecision(12)} ${+(x*100).toPrecision(12)}%`).join(', ')})`;
}
export function motionController({views,now,generation,request,applyBatch,inert,releaseInteraction=()=>{},ready=()=>true}) {
  const properties=['translate','scale','rotate','opacity','height'];
  const animations=new Map(), held=new Map(), authored=new Map(), drags=new Map();
  const raised=new Set();
  const active=new Map(), heightBindings=new Map(); let reconciling=false;
  const transformBindings=new Map(), geometryDirty=new Set();
  let geometryFrame=null, geometryDelivering=false, geometrySerial=0n;
  const key=(id,property)=>`${id}/${property}`;
  const cssProperty=(el,property)=>property==='scale'&&el?.style.getPropertyValue('--exact-press')?'--exact-scale':property;
  // CSS height clamps negative interpolated lengths. Keep every spring sample
  // and its timing; only its displayed length changes, not the engine curve.
  const css=(property,value)=>property==='translate'?`${value[0]}px ${value[1]}px`:property==='rotate'?`${value[0]}deg`:property==='height'?`${Math.max(0,value[0])}px`:String(value[0]);
  const call=(op,h,value=[0,0],t=now())=>request({op,view:h.view,property:h.property,token:h.token??0,x:value[0],y:value[1],now:t});
  const local=h=>h && h.generation===generation() && views.get(h.view)===h.el && h.el.isConnected && held.get(key(h.view,h.property))===h;
  const eligible=el=>el?.isConnected&&!el.closest('[disabled]')&&!el.matches(':disabled')&&!inert(el)&&el.getClientRects().length>0&&getComputedStyle(el).visibility==='visible';
  const live=h=>local(h)&&call('live',h).accepted===true;
  // The thresholds exact2 defines itself, from exact_motion::gesture (LLP 1057.001 §3).
  let constants=null;
  const gesture=()=>constants??=request({op:'gesture'});
  // Precedence rule 3 (LLP 1057.001 §1): a recognizer that does not capture at
  // down (swipe) marks its pointer 'pending' until it claims or refuses; an
  // ancestor's recognizer that would capture waits for that verdict.
  const contacts=()=>(globalThis.exact??={}).contacts??=new Map();
  const deferred=d=>{
    if(!d.deferred)return false;
    const state=contacts().get(d.pointer??[...d.pointers.keys()][0]);
    if(state==='pending')return true;
    if(state==='claimed'){drags.get(d.id)?.();return true;}
    d.deferred=false;
    for(const pointer of d.pointers?.keys()??[d.pointer])d.el.setPointerCapture(pointer);
    return false;
  };
  // A press handler or control between the contact and an ancestor's
  // recognizer keeps the contact (rule 3's boundary, as AppKit and Linux do).
  const pressable=(e,el)=>{const inner=e.target.closest('button,a[href],select,[data-exact-on~="press"]');return !!inner&&inner!==el&&el.contains(inner);};
  // @ref LLP 1057.003 D2 — drag timelines. A node's presented
  // translate drives the `animation`s of nodes bound to its named timeline:
  // their CSS animations are paused (css.rs) and seeked here. A held value is
  // followed where the drag presents it (transform drag's `present`), in the
  // pointer event. A translate spring hands each consumer its frames
  // (`follow`, D3), so the browser plays both and nothing here runs per
  // frame; only a consumer that easing can't express is sought in each
  // animation frame while the spring runs (`kickTimelines`).
  const timelineSources='[style*="--exact-drag-timeline"]';
  const timelineName=el=>el.style.getPropertyValue('--exact-drag-timeline').trim().split(/\s+/);
  // @ref LLP 1057.003 D4 — the source a consumer's name resolves to, as CSS
  // scopes names and as the kernel's `lookup` does: walking up from the
  // consumer, the first element that declares the name or scopes it
  // (`--exact-timeline-scope`) decides. A scope's source is the one
  // declaring element whose nearest scope of the name, itself included, is
  // that scope; none or several are an inactive timeline (null), but `all`
  // declares only the names below it. No timeline in scope is undefined.
  const scopes=(el,name)=>{const s=el.style.getPropertyValue('--exact-timeline-scope').trim();return s==='all'?2:s.split(/\s*,\s*/).includes(name)?1:0;};
  const timelineSource=(consumer,name)=>{
    for(let el=consumer;el?.style;el=el.parentElement) {
      if(timelineName(el)[0]===name)return el;
      const scope=scopes(el,name);
      if(!scope)continue;
      const found=[...el.querySelectorAll(timelineSources)].filter(s=>{if(timelineName(s)[0]!==name)return false;while(!scopes(s,name))s=s.parentElement;return s===el;});
      if(found.length===1)return found[0];
      if(found.length||scope===1)return null;
    }
  };
  const inactive=new WeakMap();
  let timelineFrame=0;
  const kickTimelines=()=>{if(!timelineFrame&&typeof requestAnimationFrame==='function'&&typeof document!=='undefined'&&document.querySelector(timelineSources))
    timelineFrame=requestAnimationFrame(()=>{timelineFrame=0;if(followTimelines())kickTimelines();});};
  // Seek every bound consumer; whether a source is still moving on its own.
  function followTimelines() {
    const sources=new Map();let moving=false;
    for(const el of document.querySelectorAll(timelineSources)) {
      const [name,axis='y']=el.style.getPropertyValue('--exact-drag-timeline').trim().split(/\s+/);
      let v=[...held.values()].find(h=>h.el===el&&h.property==='translate'&&local(h))?.value;
      if(!v) {
        const t=getComputedStyle(el).translate.trim().split(/\s+/);
        v=t[0]==='none'?[0,0]:[parseFloat(t[0]),parseFloat(t[1]??'0')];
        moving||=el.getAnimations().some(a=>a.playState==='running');
      }
      sources.set(el,axis==='x'?v[0]:v[1]);
    }
    for(const el of document.querySelectorAll('[style*="--exact-animation-timeline"]')) {
      const name=el.style.getPropertyValue('--exact-animation-timeline').trim();
      const range=el.style.getPropertyValue('--exact-animation-range').trim().split(/\s+/).map(parseFloat);
      if(range.length!==2||range[1]===range[0])continue;
      // No timeline in scope: the animations keep the time they have (0 if
      // new), as Chrome's and the engine's do.
      const source=timelineSource(el,name);
      if(source===undefined)continue;
      // Unclamped, and over the delay and active interval together, as a CSS
      // scroll timeline maps them; an endless animation holds its start
      // (motion's `seek_timeline`).
      const p=(sources.get(source)-range[0])/(range[1]-range[0]);
      // An inactive timeline: not in effect, whatever the fill (Chrome's
      // unresolved time). Cancelled, and revived when the name resolves
      // again, unless its CSS has since dropped or replaced it.
      const parked=inactive.get(el)??[],live=el.getAnimations().filter(a=>a.animationName!==undefined);
      if(!source){for(const a of live)a.cancel();inactive.set(el,[...parked,...live]);continue;}
      inactive.delete(el);
      const names=parked.length?getComputedStyle(el).animationName.split(/,\s*/):[];
      for(const a of [...live,...parked.filter(a=>names.includes(a.animationName)&&!live.some(b=>b.animationName===a.animationName))]) {
        const t=a.effect?.getComputedTiming();
        if(!t)continue;
        if(a.playState!=='paused')a.pause();
        a.currentTime=Number.isFinite(t.endTime)?p*t.endTime:Math.max(0,t.delay);
      }
    }
    return moving;
  }
  // @ref LLP 1057.003 D3 — a source's release spring, `op`, for its
  // consumers: each CSS animation's keyframes, copied and played over the
  // spring's delay and duration with the easing its progress gives
  // (`timelineEasing`). They start with the spring, so the agent's clock
  // seeks them with it; a grab or a new spring cancels them
  // (`cancelProperty`), and when they end the paused animations take over
  // where the source rests.
  const followers=new Map();
  // A source a CSS transition moves (an eased release, not a spring) is
  // sought in each frame while it runs: its easing is the browser's, not
  // frames this glue holds.
  if(typeof document?.addEventListener==='function')document.addEventListener('transitionrun',e=>{if(e.propertyName==='translate'&&e.target.style?.getPropertyValue('--exact-drag-timeline'))kickTimelines();});
  function follow(id,el,op,source) {
    const [name,axis='y']=timelineName(el);
    if(!name||op.values.length<2)return;
    const at=op.values.map(v=>axis==='x'?v[0]:v[1]),made=[];let seek=false;
    for(const c of document.querySelectorAll('[style*="--exact-animation-timeline"]')) {
      if(c.style.getPropertyValue('--exact-animation-timeline').trim()!==name||timelineSource(c,name)!==el)continue;
      const [a,b]=c.style.getPropertyValue('--exact-animation-range').trim().split(/\s+/).map(parseFloat);
      if(!Number.isFinite(a)||!Number.isFinite(b)||a===b)continue;
      for(const animation of c.getAnimations()) {
        if(animation.animationName===undefined||animation.effect?.target!==c)continue;
        const easing=timelineEasing(animation.effect.getComputedTiming(),at.map(v=>(v-a)/(b-a)));
        if(easing===undefined)continue;
        const keyframes=animation.effect.getKeyframes().map(({computedOffset,...k})=>k);
        // Added to the paused animation it stands in for, it would count twice.
        if(easing===null||animation.effect.composite!=='replace'||keyframes.some(k=>k.composite==='add'||k.composite==='accumulate')){seek=true;continue;}
        const f=c.animate(keyframes,{delay:op.delay,duration:op.duration,easing,fill:'both'});
        if(source.startTime!==null)f.startTime=source.startTime;
        made.push(f);
      }
    }
    if(made.length) {
      followers.set(id,made);
      made[0].finished.then(()=>{if(followers.get(id)!==made)return;followers.delete(id);followTimelines();for(const f of made)f.cancel();},()=>{});
    }
    if(seek)kickTimelines();
  }
  function cancelProperty(id,property,el=views.get(id)) {
    const k=key(id,property); animations.get(k)?.cancel(); animations.delete(k);
    if(property==='translate'&&followers.has(id)) {
      for(const f of followers.get(id))f.cancel();
      followers.delete(id);
      // A grab holds the source where it was caught: its consumers too.
      if(el)followTimelines();
    }
    // Browser easing is a CSSTransition; other properties continue undisturbed.
    for(const animation of el?.getAnimations()??[]) {
      if(animation.effect?.target===el && animation.transitionProperty===cssProperty(el,property)) animation.cancel();
    }
  }
  function overlay(id) {
    const el=views.get(id); if(!el) return;
    const active=properties.map(p=>held.get(key(id,p))).filter(local);
    if(raised.has(id)){el.style.zIndex='2147483647';if(getComputedStyle(el).position==='static')el.style.position='relative';}
    if(!active.length) return;
    const transition=el.style.transition;
    el.style.transition=[...(transition && transition!=='none'?[transition]:[]), ...active.map(h=>`${cssProperty(el,h.property)} 0s linear 0s`)].join(',');
    for(const h of active) { el.style.setProperty(cssProperty(el,h.property),css(h.property,h.value)); cancelProperty(id,h.property,el); }
  }
  function restore(id) {
    const el=views.get(id); if(!el) return;
    if(authored.has(id)) el.style.cssText=authored.get(id);
    overlay(id);
    if(!raised.has(id)&&!properties.some(p=>held.has(key(id,p)))) authored.delete(id);
  }
  function sample(el,property) {
    const text=getComputedStyle(el).getPropertyValue(cssProperty(el,property)), numbers=text.trim().split(/\s+/);
    if(property==='translate') {
      if(text==='none') return [0,0];
      const box=el.getBoundingClientRect();
      return [0,1].map(i=>numbers[i]?.endsWith('%')?parseFloat(numbers[i])*[box.width,box.height][i]/100:parseFloat(numbers[i]??'0'));
    }
    if(property==='scale' && numbers.length>1 && Number(numbers[0])!==Number(numbers[1])) return null;
    return [text==='none'?(property==='scale'?1:0):parseFloat(text),0];
  }
  function adopt(view,property,reply,adopted) {
    const el=views.get(view);
    if(!el||!reply.token) return null;
    if(!authored.has(view)) authored.set(view,el.style.cssText);
    const h={view,property,el,token:reply.token,generation:generation(),value:reply.value};
    held.set(key(view,property),h);
    cancelProperty(view,property,el); restore(view);
    // A begin batch can retire its binding synchronously. The recognizer must
    // already own this token when that cancellation is delivered.
    adopted?.(h);
    if(reply.batch) applyBatch(reply.batch);
    return h;
  }
  function adoptPair(b,reply,adopted) {
    if(!reply.translateToken||!reply.scaleToken||reply.runtime!==b.runtime||reply.geometrySequence!==b.sequence)return null;
    const el=b.targetEl,view=b.target;
    if(!authored.has(view))authored.set(view,el.style.cssText);
    const pair=['translate','scale'].map((property,i)=>({view,property,el,token:i?reply.scaleToken:reply.translateToken,
      generation:generation(),runtime:b.runtime,value:i?[reply.value[2],0]:reply.value.slice(0,2)}));
    // BOTH records and recognizer ownership precede any synchronous begin batch.
    for(const h of pair)held.set(key(view,h.property),h);
    for(const h of pair)cancelProperty(view,h.property,el);
    // The admitted target may have a browser-owned pair curve, not only a
    // registered spring. Snapshot admission excludes curves coupled to another
    // property, so this cannot cancel an unrelated opacity/geometry animation.
    for(const a of el.getAnimations())if(a.effect?.target===el&&pairCurve(a).pair)a.cancel();
    restore(view);adopted(pair);
    if(reply.batch)applyBatch(reply.batch);
    return pair;
  }
  const bindingLive=b=>b&&heightBindings.get(b.id)===b&&views.get(b.id)===b.el&&views.get(b.target)===b.targetEl&&eligible(b.el)&&eligible(b.targetEl);
  const maxPixel=3.4028234663852886e38;
  const pixel=v=>Number.isFinite(v)&&Math.abs(v)<=maxPixel;
  const positiveScale=v=>pixel(v)&&v>0&&Math.fround(v)>0;
  const transformLocal=b=>b&&b.generation===generation()&&transformBindings.get(b.id)===b
    &&views.get(b.id)===b.el&&views.get(b.target)===b.targetEl&&views.get(b.clip)===b.clipEl
    &&b.el.isConnected&&b.targetEl.isConnected&&b.clipEl.isConnected;
  const px=text=>/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?px$/i.test(text)?Number(text.slice(0,-2)):NaN;
  function transformSample(cs) {
    const t=cs.translate==='none'?['0px','0px']:cs.translate.trim().split(/\s+/);
    const s=cs.scale==='none'?['1']:cs.scale.trim().split(/\s+/);
    if(t.length>2||s.length>2||!s.length||s.length===2&&Number(s[0])!==Number(s[1]))return null;
    const value=[px(t[0]),px(t[1]??'0px'),Number(s[0])];
    return pixel(value[0])&&pixel(value[1])&&positiveScale(value[2])?value:null;
  }
  function pairCurve(animation) {
    const props=new Set(animation.effect.getKeyframes().flatMap(frame=>Object.keys(frame))
      .filter(p=>!['offset','computedOffset','easing','composite'].includes(p)).map(p=>p==='--exact-scale'?'scale':p));
    return {pair:props.has('translate')||props.has('scale'),coupled:[...props].some(p=>p!=='translate'&&p!=='scale')};
  }
  function transformSnapshot(b) {
    if(!transformLocal(b)||!eligible(b.el)||!eligible(b.targetEl)||!eligible(b.clipEl))return null;
    const target=getComputedStyle(b.targetEl),clip=getComputedStyle(b.clipEl),value=transformSample(target);
    const zeroInsets=cs=>['paddingTop','paddingRight','paddingBottom','paddingLeft','borderTopWidth','borderRightWidth','borderBottomWidth','borderLeftWidth'].every(p=>px(cs[p])===0);
    if(!value||target.boxSizing!=='border-box'||!zeroInsets(target)||!zeroInsets(clip)
      ||clip.overflowX!=='hidden'||clip.overflowY!=='hidden'||b.targetEl.parentElement!==b.clipEl)return null;
    const dimensions=[px(target.width),px(target.height),px(clip.width),px(clip.height)];
    if(!dimensions.every(v=>pixel(v)&&v>=0)||!['marginTop','marginRight','marginBottom','marginLeft'].every(p=>px(target[p])===0))return null;
    const close=(x,y)=>Math.abs(x-y)<=Math.max(.02,Math.max(Math.abs(x),Math.abs(y))*Number.EPSILON*8);
    if(!close(dimensions[0],dimensions[2])||!close(dimensions[1],dimensions[3]))return null;
    const origin=target.transformOrigin.split(/\s+/).map(px);
    if(origin.length<2||origin.length>3||!close(origin[0],dimensions[0]/2)||!close(origin[1],dimensions[1]/2)||origin.length===3&&origin[2]!==0)return null;
    const path=[];
    for(let el=b.el;el;el=el.parentElement) {
      path.push(el);const cs=el===b.targetEl?target:el===b.clipEl?clip:getComputedStyle(el),sample=transformSample(cs);
      if(!sample||!['none','0deg'].includes(cs.rotate)||cs.transform!=='none'||cs.perspective!=='none'
        ||cs.transformStyle==='preserve-3d'||!['1','normal',''].includes(cs.zoom))return null;
      if(el!==b.targetEl&&(sample[0]!==0||sample[1]!==0||sample[2]!==1))return null;
      // A current identity crossing/delay is not proof that the ancestor curve
      // stays identity. Refuse unsupported active curves, not decompose them.
      for(const a of el.getAnimations()) {
        if(a.effect?.target!==el||['idle','finished'].includes(a.playState))continue;
        const forbidden=['rotate','transform','transformOrigin','transform-origin','perspective','zoom',...(el===b.targetEl?[]:['translate','scale','--exact-scale','--exact-press-factor'])];
        if(forbidden.includes(a.transitionProperty)||a.effect.getKeyframes().some(frame=>forbidden.some(p=>p in frame)))return null;
        const curve=pairCurve(a);if(el===b.targetEl&&curve.pair&&curve.coupled)return null;
      }
    }
    if(!path.includes(b.targetEl)||!path.includes(b.clipEl))return null;
    const tr=b.targetEl.getBoundingClientRect(),cr=b.clipEl.getBoundingClientRect();
    if(![tr.x,tr.y,tr.width,tr.height,cr.x,cr.y,cr.width,cr.height].every(Number.isFinite)
      ||!close(tr.x+tr.width/2-value[0],cr.x+cr.width/2)||!close(tr.y+tr.height/2-value[1],cr.y+cr.height/2))return null;
    return {dimensions,origin:[cr.x,cr.y,b.clipEl.scrollLeft,b.clipEl.scrollTop],path,value};
  }
  const sameGeometry=(a,b)=>a===b||!!(a&&b&&a.dimensions.every((v,i)=>v===b.dimensions[i])
    &&a.origin.every((v,i)=>v===b.origin[i])&&a.path.length===b.path.length&&a.path.every((v,i)=>v===b.path[i]));
  function transformFacts(b,op,values,pair=null,time=now()) {
    return {op,runtime:b.runtime,handleKey:b.handleKey,targetKey:b.targetKey,clipKey:b.clipKey,geometrySequence:b.sequence,
      translateToken:pair?.[0].token??0,scaleToken:pair?.[1].token??0,values,now:time};
  }
  function watchTransformPath(b,path) {
    const next=new Set([...path,window]);
    for(const el of b.observedPath)if(!next.has(el))el.removeEventListener('scroll',b.scrolled);
    for(const el of next)if(!b.observedPath.has(el))el.addEventListener('scroll',b.scrolled,{passive:true});
    b.observedPath=next;
  }
  function enqueueGeometry(b) {
    if(!transformLocal(b)||b.exhausted)return;
    geometryDirty.add(b);
    if(geometryFrame===null)geometryFrame=requestAnimationFrame(()=>{
      geometryFrame=null;if(!ready())return;
      for(const b of [...geometryDirty])flushGeometry(b);
    });
  }
  function checkGeometry(b) {
    if(!transformLocal(b))return null;
    const next=transformSnapshot(b);
    if(!sameGeometry(b.facts,next)) {
      // Retire contact BEFORE any geometry callback; the original Engine pair
      // remains held until the accepted geometry receipt has its latest targets.
      drags.get(b.id)?.suspend?.();
      enqueueGeometry(b);
    }
    return next;
  }
  function flushGeometry(b) {
    if(!transformLocal(b)||!ready()||geometryDelivering||b.exhausted)return false;
    geometryDirty.delete(b);
    const next=transformSnapshot(b);
    if(sameGeometry(b.facts,next)) {
      // Mapping can move away and back before this coalesced callback. Contact
      // already ended on the first change; its original pair still needs cleanup.
      const ends=b.pendingEnds;b.pendingEnds=null;
      for(const h of ends??[])api.end(h,[0,0],true);
      return b.admitted;
    }
    drags.get(b.id)?.suspend?.();
    const ends=b.pendingEnds;b.pendingEnds=null;
    const previous=b.facts;b.facts=next;b.admitted=false;
    if(next)watchTransformPath(b,next.path);
    if(!next&&previous===undefined)return false;
    if(geometrySerial===0xffffffffffffffffn){b.exhausted=true;for(const h of ends??[])api.end(h,[0,0],true);return false;}
    b.sequence=String(++geometrySerial);
    geometryDelivering=true;
    try {
      const reply=request(transformFacts(b,next?'transform-geometry':'transform-invalidate',next?[...next.dimensions,0,0]:[0,0,0,0,0,0]));
      if(reply.batch)applyBatch(reply.batch);
      if(transformLocal(b))b.admitted=reply.accepted===true&&!reply.batch?.error&&!!next&&next.dimensions.every(v=>v>0);
    } finally {
      geometryDelivering=false;
      for(const h of ends??[])api.end(h,[0,0],true);
    }
    if(transformLocal(b)&&!sameGeometry(b.facts,transformSnapshot(b))){b.admitted=false;enqueueGeometry(b);}
    return b.admitted;
  }
  function detachTransform(b) {
    // Remove ownership before cancellation can synchronously publish a binding.
    geometryDirty.delete(b);transformBindings.delete(b.id);
    b.observer.disconnect();for(const el of b.observedPath)el.removeEventListener('scroll',b.scrolled);
    const ends=b.pendingEnds;b.pendingEnds=null;
    drags.get(b.id)?.();
    for(const h of ends??[])api.end(h,[0,0],true);
  }
  const api={
    // The agent's seek presents sources without a frame; follow at once.
    followTimelines() { if(typeof document!=='undefined'&&document.querySelector(`${timelineSources},[style*="--exact-animation-timeline"]`))followTimelines(); },
    presentReorder(view,token,value) {
      const h=held.get(key(view,'translate'));if(!local(h)||h.token!==token)return false;
      h.value=value;h.el.style.translate=css('translate',value);return true;
    },
    raiseReorder(id,enabled) {
      const el=views.get(id);if(!el)return;
      if(enabled){if(!authored.has(id))authored.set(id,el.style.cssText);raised.add(id);}else raised.delete(id);
      restore(id);
    },
    captureReorder(frame) {
      return frame.map(row=>{const el=views.get(row.view),v=el&&sample(el,'translate'),r=el?.getBoundingClientRect();
        return v?.every(Number.isFinite)&&r?{...row,el,value:v,visual:[r.x,r.y]}:null;}).filter(Boolean);
    },
    adoptReorder(frame,samples,runtime) {
      const captured=new Map(samples.map(s=>[s.key,s]));
      for(const row of frame){const s=captured.get(row.key);if(!s||!row.hold||row.hold==='0'||views.get(row.view)!==s.el)continue;
        const old=held.get(key(row.view,'translate'));if(old?.token===row.hold)continue;
        const h=adopt(row.view,'translate',{token:row.hold,value:s.value});if(h)h.runtime=runtime;}
    },
    rebaseReorder(samples) {
      const rows=[];
      for(const s of samples){const h=held.get(key(s.view,'translate'));if(!local(h)||h.token!==(s.hold??s.token))continue;
        const r=h.el.getBoundingClientRect();h.value=[h.value[0]+s.visual[0]-r.x,h.value[1]+s.visual[1]-r.y];
        h.el.style.translate=css('translate',h.value);rows.push({...s,value:h.value});}
      return rows;
    },
    releaseReorder(samples) {
      for(const s of samples){const h=held.get(key(s.view,'translate'));if(!local(h)||h.token!==s.hold)continue;
        h.el.getBoundingClientRect();held.delete(key(s.view,'translate'));restore(s.view);}
    },
    reorderSettled(view) {
      return !(views.get(view)?.getAnimations()??[]).some(a=>a.effect?.target===views.get(view)
        &&(a===animations.get(key(view,'translate'))||a.transitionProperty==='translate')
        &&!['idle','finished'].includes(a.playState)&&Number(a.currentTime)<a.effect.getComputedTiming().endTime);
    },
    settleReorder(view,finish,valid=()=>true) {
      const check=()=>{if(!valid())return;if(api.reorderSettled(view)){finish();return;}
        const el=views.get(view),pending=el.getAnimations().filter(a=>a.effect?.target===el
          &&(a===animations.get(key(view,'translate'))||a.transitionProperty==='translate')&&!['idle','finished'].includes(a.playState));
        Promise.allSettled(pending.map(a=>a.finished)).then(check);};check();
    },
    // The Kernel resolves authored IDREFs; DOM code only consumes these exact
    // generational bindings, emitted after the tree's create/attach operations.
    heightBinding(op) {
      const old=heightBindings.get(op.id);
      if(old&&old.target===op.target&&old.handleKey===op.handleKey&&old.targetKey===op.targetKey&&views.get(op.id)===old.el&&views.get(op.target)===old.targetEl) return;
      drags.get(op.id)?.(); heightBindings.delete(op.id);
      if(op.target!==null&&views.has(op.id)&&views.has(op.target)) heightBindings.set(op.id,{...op,el:views.get(op.id),targetEl:views.get(op.target)});
    },
    transformBinding(op) {
      const old=transformBindings.get(op.id);
      if(old&&['runtime','handleKey','target','targetKey','clip','clipKey'].every(k=>old[k]===op[k])&&transformLocal(old))return;
      if(old)detachTransform(old);
      if(transformBindings.has(op.id))return;
      if(op.target===null||op.clip===null||!views.has(op.id)||!views.has(op.target)||!views.has(op.clip))return;
      const b={...op,el:views.get(op.id),targetEl:views.get(op.target),clipEl:views.get(op.clip),generation:generation(),
        sequence:'0',facts:undefined,admitted:false,observedPath:new Set(),pendingEnds:null};
      b.scrolled=()=>checkGeometry(b);
      b.observer=new ResizeObserver(()=>checkGeometry(b));
      transformBindings.set(op.id,b);b.observer.observe(b.targetEl);b.observer.observe(b.clipEl);enqueueGeometry(b);
    },
    setHeightOwner(view=null) {
      const reply=request({op:view===null?'clear-height-owner':'height-owner',view:view??0,property:'height',now:now()});
      if(reply.accepted!==true) return false;
      if(reply.batch) applyBatch(reply.batch);
      return true;
    },
    begin(view,property) {
      const el=views.get(view); if(!eligible(el)) return null;
      const value=sample(el,property); if(!value?.every(Number.isFinite)) return null;
      return adopt(view,property,call('begin',{view,property},value));
    },
    move(h,value) {
      if(!local(h)) return false;
      const t=now(),reply=call('move',h,value,t); if(reply.accepted!==true) return false;
      h.t=t;
      h.value=value; h.el.style.setProperty(cssProperty(h.el,h.property),css(h.property,value));
      if(reply.batch) applyBatch(reply.batch);
      return true;
    },
    // velocity 'measured': the engine's own estimate over the held values,
    // where the platform gives none (LLP 1057.001 §3).
    end(h,velocity=[0,0],cancel=false) {
      if(!local(h)) return false;
      const reply=call(cancel?'cancel':velocity==='measured'?'release-measured':'release',h,velocity==='measured'?[0,0]:velocity);
      if(!local(h)) return false;
      if(reply.accepted!==true) {
        // A receipt may have cancelled Rust's token before its binding removal
        // reaches the DOM. An explicit stale reply retires this exact overlay;
        // a validation error must leave a still-live hold intact.
        if(reply.accepted===false) { held.delete(key(h.view,h.property)); restore(h.view); }
        return false;
      }
      // Flush held presentation before restoring the latest easing/target.
      // A spring batch below installs its first frame before this turn paints.
      h.el.getBoundingClientRect(); held.delete(key(h.view,h.property)); restore(h.view);
      if(reply.batch) applyBatch(reply.batch);
      return true;
    },
    finish(h,value,velocity,commit=false,cancel=false) {
      if(local(h)&&!eligible(h.el)) { api.end(h,[0,0],true); return false; }
      if(!live(h)||!api.move(h,value)) return false;
      if(commit) { const reply=call('action',h); if(reply.batch) applyBatch(reply.batch); }
      // The action may delete the held node. That makes this a harmless no-op.
      api.end(h,velocity,cancel); return true;
    },
    // Only live gestures/holds are visited, never all mounted swipe handlers.
    // Cancel may synchronously apply another batch; retire once before reentry.
    commit() {
      if(reconciling) return;
      reconciling=true;
      try {
        for(const b of transformBindings.values())checkGeometry(b);
        for(const [id,stop] of [...active]) if(!eligible(views.get(id))||(stop.valid&&!stop.valid())) stop();
        for(const h of [...held.values()]) if(local(h)&&!eligible(h.el)) api.end(h,[0,0],true);
      } finally { reconciling=false; }
    },
    style(id,text) {
      const el=views.get(id); if(!el) return;
      if(authored.has(id)) authored.set(id,text);
      el.style.cssText=text; overlay(id);
    },
    animate(op) {
      const {id,property}=op, k=key(id,property);
      cancelProperty(id,property);
      if(!op.values.length || held.has(k)) return;
      const el=views.get(id); if(!el) return;
      const animation=el.animate(op.values.map(value=>({[cssProperty(el,property)]:css(property,property==='translate'?value:[value,0])})),
        {delay:op.delay,duration:op.duration,easing:'linear',fill:'backwards'});
      // Started where the engine lowered it (`at`, the host's clock), not
      // when the browser next commits a pending animation, two frames later
      // after a release: then the page shows the engine's value, and a catch
      // mid-spring adopts what is on screen. The agent's clock seeks it
      // instead.
      if(Number.isFinite(op.at))animation.startTime=performance.now()-now()+op.at;
      animations.set(k,animation);
      animation.finished.then(()=>{if(animations.get(k)===animation) animations.delete(k);},()=>{});
      if(property==='translate'&&el.style.getPropertyValue('--exact-drag-timeline'))follow(id,el,op,animation);
    },
    // Authored eligibility can disappear without a dirty Engine frame. Retire
    // only this property, restoring current authoring and other held overlays.
    retire(id,property,token=null,runtime=null) {
      const h=held.get(key(id,property));
      if(token!==null&&(!local(h)||h.token!==token||h.runtime!==runtime))return;
      cancelProperty(id,property);
      held.delete(key(id,property));
      restore(id);
    },
    destroy(id) {
      drags.get(id)?.(); drags.delete(id);
      for(const [handle,b] of [...heightBindings]) if(handle===id||b.target===id) { drags.get(handle)?.(); heightBindings.delete(handle); }
      for(const b of [...transformBindings.values()])if(b.id===id||b.target===id||b.clip===id)detachTransform(b);
      // Detaching cancels CSS transitions. Only our retained WAAPI animations
      // need explicit cancellation; querying getAnimations here flushes styles
      // once per retired list row while the DOM batch is still being applied.
      for(const property of properties) { cancelProperty(id,property,null); held.delete(key(id,property)); }
      raised.delete(id);authored.delete(id);
    },
    reset() {
      for(const stop of drags.values()) stop(); drags.clear();
      heightBindings.clear();
      for(const b of [...transformBindings.values()])detachTransform(b);
      geometryDirty.clear();if(geometryFrame!==null)cancelAnimationFrame(geometryFrame);geometryFrame=null;
      for(const animation of animations.values()) animation.cancel(); animations.clear();
      for(const made of followers.values())for(const f of made)f.cancel(); followers.clear();
      const ids=[...authored.keys()]; held.clear();raised.clear(); for(const id of ids) restore(id); authored.clear();
    },
    // Pan and pinch on the photo pair (LLP 1057.001 §4): up to two pointers,
    // a trackpad's ctrl+wheel, Safari's gesture events. Every sample is the
    // pair anchored at its focal point, re-anchored whenever the contact set
    // changes; one release while both tokens are live.
    attachTransformDrag(el,id,on) {
      let drag=null,suppressClick=false;
      const interactive=e=>e.target.closest('button,a,input,textarea,select,[contenteditable]');
      const pairLocal=d=>d.pair?.every(local)&&transformLocal(d.binding)&&d.binding.admitted&&d.sequence===d.binding.sequence;
      const finiteTerminal=v=>pixel(v[0])&&pixel(v[1])&&positiveScale(v[2])&&v.slice(3).every(Number.isFinite);
      function clearContact(d) {
        for(const pointer of d.pointers.keys()){if(el.hasPointerCapture(pointer))el.releasePointerCapture(pointer);releaseInteraction(pointer);}
        clearTimeout(d.idle);
      }
      function stop(defer=false) {
        const d=drag;drag=null;active.delete(id);
        if(!d)return;
        if(d.pair) {
          suppressClick=true;
          if(defer)d.binding.pendingEnds=d.pair;
          else for(const h of d.pair)api.end(h,[0,0],true);
        }
        clearContact(d);
      }
      stop.suspend=()=>stop(true);
      stop.valid=()=>!drag||transformLocal(drag.binding)&&drag.binding.admitted&&(!drag.pair||pairLocal(drag));
      drags.set(id,stop);
      // The contact's focal point (client px) and, with two pointers, its spread.
      const focal=d=>{
        if(d.gesture)return d.gesture;
        const points=[...d.pointers.values()],n=points.length;
        return {x:points.reduce((a,p)=>a+p[0],0)/n,y:points.reduce((a,p)=>a+p[1],0)/n,
          spread:n===2?Math.hypot(points[0][0]-points[1][0],points[0][1]-points[1][1]):0};
      };
      // Anchor at the pair's current value and the contact as it is now.
      function anchor(d) {
        const f=focal(d),r=d.binding.clipEl.getBoundingClientRect();
        d.anchor={value:d.value,x:f.x,y:f.y,spread:f.spread,factor:d.factor??1,cx:r.x+r.width/2,cy:r.y+r.height/2};
      }
      // Content under the anchored focus stays under the current one; scale
      // multiplies by the pinch, so translate follows the centroid.
      function position(d) {
        const a=d.anchor,f=focal(d);
        const k=d.gesture||d.wheel?(d.factor??1)/a.factor:f.spread>0&&a.spread>0?f.spread/a.spread:1;
        const from=[a.x-a.cx,a.y-a.cy],to=[f.x-a.cx,f.y-a.cy];
        return [to[0]-k*(from[0]-a.value[0]),to[1]-k*(from[1]-a.value[1]),a.value[2]*k];
      }
      function present(d,v) {
        d.value=v;d.pair[0].value=v.slice(0,2);d.pair[1].value=[v[2],0];
        for(const h of d.pair)if(local(h))h.el.style.setProperty(cssProperty(h.el,h.property),css(h.property,h.value));
        if(d.binding.targetEl.style.getPropertyValue('--exact-drag-timeline'))followTimelines();
      }
      function arm(e,extra) {
        const b=transformBindings.get(id);if(!transformLocal(b))return null;
        stop();flushGeometry(b);
        if(!transformLocal(b)||!b.admitted||!sameGeometry(b.facts,transformSnapshot(b)))return null;
        drag={pointers:new Map(),binding:b,...extra};active.set(id,stop);
        return drag;
      }
      function begin(d) {
        flushGeometry(d.binding);
        if(drag!==d||!d.binding.admitted)return false;
        const snap=transformSnapshot(d.binding),t=now();
        if(!snap||!sameGeometry(d.binding.facts,snap)||!Number.isFinite(t)){stop();return false;}
        const reply=request(transformFacts(d.binding,'transform-begin',[...snap.value,0,0,0],null,t));
        if(reply.accepted!==true){if(reply.batch)applyBatch(reply.batch);stop();return false;}
        adoptPair(d.binding,reply,pair=>{d.pair=pair;d.sequence=d.binding.sequence;});
        if(drag!==d)return false;
        if(!pairLocal(d)){stop();return false;}
        d.value=[...d.pair[0].value,d.pair[1].value[0]];d.t=t;anchor(d);
        return true;
      }
      function follow(d) {
        checkGeometry(d.binding);
        if(drag!==d)return false;
        if(!stop.valid()){stop();return false;}
        const value=position(d),t=now();
        if(!finiteTerminal([...value,0,0,0])||!Number.isFinite(t)||t<d.t){stop();return false;}
        const reply=request(transformFacts(d.binding,'transform-move',[...value,0,0,0],d.pair,t));
        if(reply.accepted===true)present(d,value);
        if(reply.batch)applyBatch(reply.batch);
        if(drag!==d)return false;
        if(reply.accepted!==true||!pairLocal(d)||!transformSample(getComputedStyle(d.binding.targetEl))){stop();return false;}
        d.t=t;return true;
      }
      // Terminal preflight is whole: the final sample and time are checked
      // before the action. The engine measures the release velocity.
      function release(d) {
        checkGeometry(d.binding);if(drag!==d)return;
        if(!pairLocal(d)){stop();return;}
        const v=position(d),t=now(),values=[...v,0,0,0];
        if(!finiteTerminal(values)||!Number.isFinite(t)||t<d.t){stop();return;}
        drag=null;active.delete(id);suppressClick=true;
        const reply=request(transformFacts(d.binding,'transform-action',values,d.pair,t));
        if(reply.accepted===true)present(d,v);
        if(reply.batch)applyBatch(reply.batch);
        const cancel=reply.accepted!==true||reply.committed!==true;
        const velocity=reply.velocity?.length===3&&reply.velocity.every(Number.isFinite)?reply.velocity:[0,0,0];
        // An action/receipt may replace exactly one property. Independently end
        // each original; local/token checks never retire its replacement.
        api.end(d.pair[0],velocity.slice(0,2),cancel);
        api.end(d.pair[1],[velocity[2],0],cancel);
        clearContact(d);
      }
      on('pointerdown',e=>{
        if(e.button!==0||interactive(e))return;
        const second=drag&&!drag.wheel&&!drag.gesture&&drag.pointers.size===1&&!drag.pointers.has(e.pointerId);
        if(!second&&!e.isPrimary)return;
        if(!second&&pressable(e,el))return;
        const d=second?drag:arm(e,{x:e.clientX,y:e.clientY,id,el,deferred:contacts().get(e.pointerId)==='pending'});if(!d)return;
        d.pointers.set(e.pointerId,[e.clientX,e.clientY]);
        if(second&&d.deferred){d.deferred=false;for(const pointer of d.pointers.keys())el.setPointerCapture(pointer);}
        if(!d.deferred)el.setPointerCapture(e.pointerId);e.preventDefault();
        // A second finger is a pinch: recognized at once, then re-anchored.
        if(second){if(!d.pair&&!begin(d))return;if(drag===d)anchor(d);}
      });
      const move=e=>{
        const d=drag;if(!d||!d.pointers.has(e.pointerId))return false;
        d.pointers.set(e.pointerId,[e.clientX,e.clientY]);
        if(deferred(d))return false;
        checkGeometry(d.binding);
        if(drag!==d)return false;
        if(!stop.valid()){stop();return false;}
        if(!d.pair) {
          if(Math.hypot(e.clientX-d.x,e.clientY-d.y)<gesture().slop)return false;
          if(!begin(d))return false;
          e.preventDefault();e.stopPropagation();return true;
        }
        if(!follow(d))return false;
        e.preventDefault();e.stopPropagation();return true;
      };
      on('pointermove',move);
      const finish=e=>{
        const d=drag;if(!d||!d.pointers.has(e.pointerId))return;
        if(e.type!=='pointerup'||!d.pair){stop();return;}
        d.pointers.set(e.pointerId,[e.clientX,e.clientY]);
        if(d.pointers.size>1) {
          // One finger lifts: its partner pans on from where the pinch left it.
          if(!follow(d))return;
          d.pointers.delete(e.pointerId);anchor(d);
          if(el.hasPointerCapture(e.pointerId))el.releasePointerCapture(e.pointerId);
          releaseInteraction(e.pointerId);
        } else release(d);
        e.preventDefault();e.stopPropagation();
      };
      for(const event of ['pointerup','pointercancel','lostpointercapture'])on(event,finish);
      // A trackpad pinch: Chromium and Firefox send ctrl+wheel (the listener is
      // not passive, so the page does not zoom too); it ends when it goes quiet.
      // Without ctrl, a wheel over a zoomed photo pans it (two-finger scroll,
      // as macOS and Preview); unzoomed it stays the page's scroll.
      on('wheel',e=>{
        if(drag&&!drag.wheel)return;
        const b=transformBindings.get(id);if(!transformLocal(b)||!b.admitted)return;
        const unit=e.deltaMode===1?16:e.deltaMode===2?innerHeight:1,dx=e.deltaX*unit,dy=e.deltaY*unit;
        let d=drag;
        if(!d&&!e.ctrlKey&&!(transformSample(getComputedStyle(b.targetEl))?.[2]>1.001))return;
        e.preventDefault();
        if(!d){d=arm(e,{wheel:true,factor:1,pan:[0,0]});if(!d)return;d.gesture={x:e.clientX,y:e.clientY,spread:0};if(!begin(d))return;}
        // A switch between zooming and panning re-anchors where the pair is.
        if(d.zooming!==e.ctrlKey){d.zooming=e.ctrlKey;anchor(d);d.pan=[0,0];}
        if(e.ctrlKey){d.factor*=Math.exp(-dy/100);d.gesture={x:e.clientX,y:e.clientY,spread:0};}
        else{d.pan[0]-=dx;d.pan[1]-=dy;d.gesture={x:d.anchor.x+d.pan[0],y:d.anchor.y+d.pan[1],spread:0};}
        if(!follow(d))return;
        clearTimeout(d.idle);d.idle=setTimeout(()=>{if(drag===d)release(d);},150);
      });
      // Safari's trackpad and touch pinch: cumulative scale about the gesture.
      on('gesturestart',e=>{
        if(drag)return;e.preventDefault();
        const d=arm(e,{factor:1});if(!d)return;
        d.gesture={x:e.clientX,y:e.clientY,spread:0};begin(d);
      });
      on('gesturechange',e=>{
        const d=drag;if(!d?.gesture)return;e.preventDefault();
        if(!(e.scale>0))return;
        d.factor=e.scale;d.gesture={x:e.clientX,y:e.clientY,spread:0};follow(d);
      });
      on('gestureend',e=>{const d=drag;if(!d?.gesture)return;e.preventDefault();release(d);});
      on('dragstart',e=>{if(transformLocal(transformBindings.get(id))&&!interactive(e))e.preventDefault();});
      on('click',e=>{if(suppressClick){suppressClick=false;e.preventDefault();e.stopPropagation();}});
    },
    attachHeightDrag(el,id,on) {
      let drag=null,suppressClick=false;
      const position=d=>sample(d.binding.targetEl,'height')?.[0];
      function stop() {
        const d=drag; drag=null; active.delete(id);
        if(!d) return;
        if(d.h) { suppressClick=true; api.end(d.h,[0,0],true); }
        if(el.hasPointerCapture(d.pointer)) el.releasePointerCapture(d.pointer);
        releaseInteraction(d.pointer);
      }
      stop.valid=()=>!drag||bindingLive(drag.binding)&&(!drag.h||local(drag.h));
      drags.set(id,stop);
      on('pointerdown',e=>{
        const binding=heightBindings.get(id);
        if(!e.isPrimary||e.button!==0||!bindingLive(binding)||e.target.closest('button,a,input,textarea,select,[contenteditable]')) return;
        stop(); drag={pointer:e.pointerId,x:e.clientX,y:e.clientY,binding,id,el,deferred:contacts().get(e.pointerId)==='pending'}; active.set(id,stop);
        // A fast first move can already leave a narrow header. Retain delivery
        // while intent is pending; motion takeover still waits for recognition.
        if(!drag.deferred)el.setPointerCapture(e.pointerId); e.preventDefault();
      });
      const move=e=>{
        if(!drag||drag.pointer!==e.pointerId) return false;
        if(deferred(drag)) return false;
        if(!stop.valid()) { stop(); return false; }
        const current=drag;
        const dx=e.clientX-drag.x,dy=e.clientY-drag.y,slop=gesture().slop;
        if(!drag.h) {
          if(Math.abs(dx)>slop&&Math.abs(dx)>=Math.abs(dy)) { stop(); return false; }
          if(Math.abs(dy)<=slop||Math.abs(dy)<=Math.abs(dx)) return false;
          const value=position(drag);
          if(!Number.isFinite(value)||value<0) { stop(); return false; }
          const reply=request({op:'height-begin',view:id,property:'height',token:drag.binding.handleKey,x:value,y:0,now:now()});
          if(!reply.token||reply.target!==drag.binding.target) { stop(); return false; }
          adopt(reply.target,'height',reply,h=>{current.h=h;});
          if(drag!==current) return false;
          if(!drag.h) { stop(); return false; }
          drag.origin=e.clientY; drag.base=drag.h.value[0];
          el.setPointerCapture(e.pointerId);
        }
        const value=Math.max(0,Math.min(3.4028234663852886e38,drag.base-(e.clientY-drag.origin)));
        if(!api.move(drag.h,[value,0])) { stop(); return false; }
        if(drag!==current) return false;
        // CSS min/max may clamp a held sample. Release velocity follows actual
        // displayed height, never the finger's speed beyond that constraint:
        // the shown height goes into the engine's hold at the move's instant.
        const shown=position(drag);
        if(!Number.isFinite(shown)) { stop(); return false; }
        if(shown!==value) call('track',drag.h,[shown,0],drag.h.t);
        e.preventDefault(); if(e.type==='pointermove') e.stopPropagation(); return true;
      };
      on('pointermove',move);
      const finish=e=>{
        if(!drag||drag.pointer!==e.pointerId) return;
        if(e.type!=='pointerup'||!drag.h) { stop(); return; }
        if(!move(e)) return;
        const d=drag; drag=null; active.delete(id); suppressClick=true;
        const shown=position(d);
        // Final constrained sample precedes the synchronous authored snap. The
        // host checks this handle/target/token again before dispatching it,
        // at the engine's velocity over the heights shown (LLP 1057.001 §3).
        if(bindingLive(d.binding)&&live(d.h)&&api.move(d.h,[shown,0])) {
          const reply=request({op:'height-action',view:id,property:'height',token:d.h.token,x:shown,y:0,now:now()});
          if(reply.batch) applyBatch(reply.batch);
          api.end(d.h,'measured',reply.accepted!==true);
        } else api.end(d.h,[0,0],true);
        if(el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
        releaseInteraction(e.pointerId);
      };
      on('pointerup',finish); on('pointercancel',finish); on('lostpointercapture',finish);
      on('click',e=>{if(suppressClick){suppressClick=false;e.preventDefault();e.stopPropagation();}});
    },
    attachSwipe(el,id,on) {
      let drag=null,suppressClick=false;
      const {knee,resistance}=gesture();
      const rubber=x=>Math.abs(x)<=knee?x:Math.sign(x)*(knee+(Math.abs(x)-knee)*resistance);
      const inverse=x=>Math.abs(x)<=knee?x:Math.sign(x)*(knee+(Math.abs(x)-knee)/resistance);
      const progressOf=x=>Math.max(0,Math.min(1,x/knee));
      // A swiped card may drive a drag timeline (LLP 1057.003 D4's list):
      // its consumers follow in this event, as the transform drag's `present`.
      const track=(h,value)=>{
        if(!api.move(h,value))return false;
        if(h.property==='translate'&&h.el.style.getPropertyValue('--exact-drag-timeline'))followTimelines();
        return true;
      };
      function stop() {
        const ended=drag; drag=null; active.delete(id);
        if(ended) {
          contacts().delete(ended.pointer);
          if(ended.holds) suppressClick=true;
          for(const h of ended.holds??[]) api.end(h,[0,0],true);
          if(el.hasPointerCapture(ended.pointer)) el.releasePointerCapture(ended.pointer);
          releaseInteraction(ended.pointer);
        }
      }
      drags.set(id,stop);
      on('pointerdown',e=>{
        if(!e.isPrimary||e.button!==0||!eligible(el)||e.target.closest('input,textarea,[contenteditable]')||pressable(e,el))return;
        stop(); drag={pointer:e.pointerId,x:e.clientX,y:e.clientY}; active.set(id,stop);
        contacts().set(e.pointerId,'pending');
      });
      const move=e=>{
        if(!drag||e.pointerId!==drag.pointer)return false;
        if(!eligible(el)){stop();return false;}
        const dx=e.clientX-drag.x,dy=e.clientY-drag.y;
        if(!drag.holds) {
          const slop=gesture().slop;
          if(Math.abs(dy)>slop&&Math.abs(dy)>=Math.abs(dx)){stop();return false;}
          if(Math.abs(dx)<=slop||Math.abs(dx)<=Math.abs(dy))return false;
          if(dx<0 && !(sample(el,'translate')?.[0]>0)){stop();return false;}
          const h=api.begin(id,'translate'); if(!h){stop();return false;}
          contacts().set(e.pointerId,'claimed');
          drag.holds=[h]; drag.origin=e.clientX; drag.base=[...h.value]; drag.raw=inverse(h.value[0]);
          // One authored companion, at most two additional property holds.
          const indicator=[...el.children].find(n=>n.getAttribute('swipeIndicator')==='true');
          if(indicator) for(const property of ['opacity','scale']) {
            const h=api.begin(Number(indicator.dataset.view),property); if(h)drag.holds.push(h);
          }
          for(const h of drag.holds) h.base=[...h.value];
          el.setPointerCapture(e.pointerId);
        }
        const h=drag.holds[0], displacement=e.clientX-drag.origin;
        const x=displacement===0?drag.base[0]:rubber(drag.raw+displacement);
        if(!track(h,[x,drag.base[1]])){stop();return false;}
        const progress=progressOf(x), caught=progressOf(drag.base[0]);
        for(const companion of drag.holds.slice(1)) {
          const base=companion.base[0];
          const value=progress===caught?base:progress<caught?base*progress/caught:base+(1-base)*(progress-caught)/(1-caught);
          track(companion,[value,0]);
        }
        e.preventDefault(); if(e.type==='pointermove') e.stopPropagation(); return true;
      };
      on('pointermove',move);
      const finish=e=>{
        if(!drag||drag.pointer!==e.pointerId)return;
        // Taking capture from a text child bubbles that child's capture loss here.
        if(e.type==='lostpointercapture'&&e.target!==el)return;
        if(!drag.holds){stop();return;}
        if(e.type==='pointerup'&&!move(e))return;
        const ended=drag; drag=null; active.delete(id); suppressClick=true;
        contacts().delete(ended.pointer);
        const [h,...companions]=ended.holds, cancel=e.type!=='pointerup';
        api.finish(h,h.value,'measured',!cancel&&h.value[0]>=knee,cancel);
        for(const companion of companions) api.end(companion,'measured',cancel);
        if(el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      };
      on('pointerup',finish); on('pointercancel',finish); on('lostpointercapture',finish);
      on('click',e=>{if(suppressClick){suppressClick=false;e.preventDefault();e.stopPropagation();}});
    },
  };
  return api;
}

// One physical Arrange contact and its settling source; Common owns logical keys.
export function arrangeController({views,collections,motion,request,applyBatch,now,generation,inert,ready=()=>true}) {
  const bindings=new Map();let current=null,edge=null,edgeTime=null,busy=false,pending=null;
  const live=b=>b&&b.generation===generation()&&views.get(b.id)===b.el&&views.get(b.wrapper)===b.row&&b.el.isConnected;
  function mapping(b,y) {
    if(!live(b)||b.el.closest('[disabled]')||inert(b.el)||!b.el.getClientRects().length)return null;
    for(let el=b.el;el;el=el.parentElement) {
      const cs=getComputedStyle(el),translate=cs.translate==='none'?[0,0]:cs.translate.split(/\s+/).map(parseFloat);
      if(cs.visibility!=='visible'||cs.transform!=='none'||cs.perspective!=='none'||!['none','0deg'].includes(cs.rotate)
        ||!['none','1'].includes(cs.scale)||!['1','normal',''].includes(cs.zoom)||el!==b.row&&translate.some(v=>v!==0))return null;
      for(const a of el.getAnimations())if(a.effect?.target===el&&!['idle','finished'].includes(a.playState)) {
        const forbidden=['scale','--exact-scale','--exact-press-factor','rotate','transform','perspective','zoom',...(el===b.row?[]:['translate'])];
        if(forbidden.includes(a.transitionProperty)||a.effect.getKeyframes().some(f=>forbidden.some(p=>p in f)))return null;
      }
    }
    return collections.reorderMapping(b.list,y);
  }
  const facts=(d,op,extra={})=>({...d.binding,...d.map,op,token:d.token??0,x:d.value?.[0]??0,y:d.value?.[1]??0,now:now(),...extra});
  const call=(d,op,extra)=>request(facts(d,op,extra));
  function adoptReply(d,r,captured=[]) {
    if(r.accepted!==true)return false;
    d.token=r.token??d.token;d.frame=r.frame??d.frame;d.terminal=r.terminal??d.terminal;
    motion.adoptReorder(d.frame??[],captured,d.binding.runtime);
    if(r.batch)applyBatch(r.batch);return current===d;
  }
  function stopEdge(){if(edge!==null)cancelAnimationFrame(edge);edge=null;edgeTime=null;}
  function clearContact(d) {
    stopEdge();if(d.binding.el.hasPointerCapture(d.pointer))d.binding.el.releasePointerCapture(d.pointer);
    for(const [el,name,fn] of d.listeners??[])el.removeEventListener(name,fn);
    d.listeners=[];
  }
  function finish(d) {
    if(current!==d)return;
    const r=call(d,'reorder-finish');if(r.accepted!==true)return;
    // Remove identity before applying a receipt or promise can reenter.
    current=null;clearContact(d);motion.raiseReorder(d.binding.wrapper,false);
    collections.releaseRetainedInteraction(d.lease);if(r.batch)applyBatch(r.batch);
  }
  function terminal(d,cancel=false) {
    if(current!==d||d.phase!=='active')return;
    d.phase='terminalizing';clearContact(d);busy=true;
    try {
      const captured=motion.captureReorder(d.frame??[]);
      // The release velocity is the engine's (LLP 1057.001 §3).
      let r=call(d,cancel?'reorder-cancel':'reorder-terminal',{rows:captured});
      if(r.accepted!==true&&!cancel)r=call(d,'reorder-cancel',{rows:captured});
      if(!adoptReply(d,r,captured)){d.phase='active';return;}
      const old=new Map(captured.map(s=>[s.key,s]));
      const survivors=(d.frame??[]).filter(row=>row.hold!=='0'&&old.has(row.key)).map(row=>({...old.get(row.key),...row}));
      const rebased=motion.rebaseReorder(survivors);
      r=call(d,'reorder-rebase',{rows:rebased});
      if(r.accepted!==true){d.phase='terminalizing';return;}
      motion.releaseReorder(rebased);d.phase='settling';
      if(r.batch)applyBatch(r.batch);
      motion.settleReorder(d.binding.wrapper,()=>finish(d),()=>current===d&&d.phase==='settling');
    } finally {busy=false;}
  }
  function sampleMove(d,x,y) {
    if(current!==d||d.phase!=='active')return false;
    d.x=x;d.y=y;const m=mapping(d.binding,y);if(current!==d||d.phase!=='active'||m===undefined)return false;if(!m){terminal(d,true);return false;}
    d.map=m;d.x=x;d.y=y;d.value=[d.base[0],d.base[1]+y-d.originY+m.raw-d.originRaw];
    const source=d.frame?.find(r=>r.key===d.binding.wrapperKey);
    const captured=source?[{...source,value:d.value,el:d.binding.row}]:[];
    // The existing hold is already adopted: use the fixed packet to update Rust,
    // then update the same DOM overlay without an extra clock/lowering call.
    const r=call(d,'reorder-preview');if(!adoptReply(d,r,captured)){terminal(d,true);return false;}
    motion.presentReorder(d.binding.wrapper,source?.hold,d.value);return true;
  }
  function edgeRange(d) {
    const m=d.map,offset=d.y-m.portTop,direction=offset<32?-1:offset>m.portHeight-32?1:0;
    // Translate contributes to browser scroll overflow. Never chase the held
    // source beyond the collection's certified untransformed content extent.
    return {m,direction,available:Math.max(0,direction<0?m.raw:m.totalExtent-m.portHeight-m.raw)};
  }
  function edges(d) {
    if(current!==d||d.phase!=='active')return;
    const range=edgeRange(d);if(!range.direction||!range.available){stopEdge();return;}
    if(edge!==null)return;
    edge=requestAnimationFrame(at=>{edge=null;
      if(current!==d||d.phase!=='active'||!ready()||!Number.isFinite(at)){stopEdge();return;}
      // 720 CSS px/s, with at most 32ms of catch-up after a delayed frame.
      // The first frame after idle establishes a fresh rAF clock origin.
      const dt=edgeTime===null?0:Math.min(32,Math.max(0,at-edgeTime));edgeTime=at;
      const {m,direction,available}=edgeRange(d);
      if(!direction||!available){stopEdge();return;}
      if(!dt){edges(d);return;}
      const before=m.port.scrollTop;m.port.scrollTop+=direction*Math.min(available,720*dt/1000);
      if(m.port.scrollTop!==before&&sampleMove(d,d.x,d.y))edges(d);else stopEdge();
    });
  }
  function down(b,e) {
    if(!ready()||!e.isPrimary||e.button!==0||e.target.closest('input,textarea,select,[contenteditable]'))return;
    let reservation=null,returning=current?.phase==='settling'&&current.binding.wrapper===b.wrapper?current:null;
    // A tap or horizontal refusal is not a takeover. Keep the old Terminal
    // and its return/pin owner until replacement recognition actually succeeds.
    if(current&&!returning){
      if(current.phase==='active')terminal(current,true);
      if(current?.phase==='settling'){
        if(current.binding.wrapper===b.wrapper){
          reservation=collections.transferRetainedInteraction(current.lease,b.row,e.pointerId);
          if(!reservation)return; // Keep the still-visible old source on refusal.
        }
        finish(current);
      }
      if(current)return;
    }
    pending?.();b=bindings.get(b.id);if(!b){collections.releaseRetainedInteraction(reservation);return;}
    if(!reservation&&!returning)collections.reorderContact(b.el,e.pointerId);
    let contact={x:e.clientX,y:e.clientY},drag=null;
    const events=[],on=(el,name,fn)=>{el.addEventListener(name,fn);events.push([el,name,fn]);};
    const cleanup=()=>{for(const [el,name,fn]of events)el.removeEventListener(name,fn);if(pending===cleanup)pending=null;if(!drag){if(reservation)collections.releaseRetainedInteraction(reservation);else collections.releaseInteraction(e.pointerId);}};
    cleanup.handle=b.id;
    pending=cleanup;
    on(b.el.ownerDocument,'pointermove',v=>{
      if(v.pointerId!==e.pointerId)return;
      if(!drag){
        const dx=v.clientX-contact.x,dy=v.clientY-contact.y;
        if(Math.abs(dx)>8&&Math.abs(dx)>Math.abs(dy)){cleanup();return;}
        if(Math.abs(dy)<8)return;
        if(returning){
          if(current!==returning||!mapping(b,v.clientY)){cleanup();return;}
          reservation=collections.transferRetainedInteraction(returning.lease,b.row,e.pointerId);
          if(!reservation)return;
          finish(returning);returning=null;
          if(current){cleanup();return;}
          b=bindings.get(b.id);if(!b){cleanup();return;}
        }
        const m=mapping(b,v.clientY);
        const lease=m&&(reservation?collections.transferRetainedInteraction(reservation,b.el,e.pointerId):collections.retainInteraction(b.el,e.pointerId));
        if(reservation&&m!==null&&!lease)return; // Await another sample/feedback; pointerup releases this one reservation.
        if(!m||!lease){cleanup();return;}
        if(reservation)reservation=lease;
        const captured=motion.captureReorder([{view:b.wrapper,key:b.wrapperKey,hold:'0'}]);
        if(captured.length!==1){collections.releaseRetainedInteraction(lease);cleanup();return;}
        const d={binding:b,map:m,lease,pointer:e.pointerId,phase:'active',value:captured[0].value,base:captured[0].value,
          originY:v.clientY,originRaw:m.raw,x:v.clientX,y:v.clientY,listeners:events};
        current=d;busy=true;let ok;
        try{ok=adoptReply(d,call(d,'reorder-begin'),captured);}finally{busy=false;}
        if(!ok){current=null;collections.releaseRetainedInteraction(lease);cleanup();return;}
        drag=d;pending=null;motion.raiseReorder(b.wrapper,true);b.el.setPointerCapture(v.pointerId);
        on(m.port,'scroll',()=>{if(!busy&&sampleMove(d,d.x,d.y))edges(d);});
      }
      if(sampleMove(drag,v.clientX,v.clientY)){v.preventDefault();v.stopPropagation();edges(drag);}
    });
    const up=v=>{if(v.pointerId!==e.pointerId)return;cleanup();if(!drag)return;
      const cancel=v.type!=='pointerup';if(!cancel&&!sampleMove(drag,v.clientX,v.clientY)){terminal(drag,true);return;}
      terminal(drag,cancel);};
    for(const name of ['pointerup','pointercancel','lostpointercapture'])on(b.el.ownerDocument,name,up);
  }
  function destroy(id) {
    const b=bindings.get(id);if(!b)return;
    b.el.removeEventListener('pointerdown',b.down);b.el.style.touchAction=b.touch;bindings.delete(id);
    if(pending?.handle===id)pending();
    // The completed batch supplies the terminal frame and surviving source
    // wrapper. Preserve that hold/pin until commit can capture and rebase it.
  }
  return {
    binding(op) {
      const old=bindings.get(op.id);
      if(old&&['runtime','handleKey','listKey','wrapperKey','rootKey','rowEpoch'].every(k=>old[k]===op[k])&&live(old))return;
      destroy(op.id);
      if(op.list===null||!views.has(op.id)||!views.has(op.wrapper))return;
      const b={...op,el:views.get(op.id),row:views.get(op.wrapper),generation:generation()};
      b.touch=b.el.style.touchAction;b.el.style.touchAction='none';b.down=e=>down(b,e);
      bindings.set(op.id,b);b.el.addEventListener('pointerdown',b.down);
    },
    destroy,
    state(op) {if(current&&op.runtime===current.binding.runtime&&op.token===current.token){current.frame=op.frame;current.terminal=op.terminal;}},
    commit() {
      if(busy||collections.reporting()||!current)return;const d=current;busy=true;let invalid=false;
      try{if(d.phase==='active'){
        const m=d.terminal?null:mapping(d.binding,d.y);invalid=m===null;
        if(m&&['raw','portWidth','portHeight','rowWidth','scrollSequence'].some(k=>m[k]!==d.map[k]))sampleMove(d,d.x,d.y);
      }}finally{busy=false;}
      if(invalid)terminal(d,true);if(current!==d)return;
      if(d.phase==='active'){motion.raiseReorder(d.binding.wrapper,true);edges(d);}
      if(d.phase==='settling'&&motion.reorderSettled(d.binding.wrapper))finish(d);
    },
    reset() {
      if(current){if(current.phase==='active')terminal(current,true);if(current?.phase==='settling')finish(current);}
      if(current){clearContact(current);collections.releaseRetainedInteraction(current.lease);motion.raiseReorder(current.binding.wrapper,false);current=null;}
      for(const b of bindings.values()){b.el.removeEventListener('pointerdown',b.down);b.el.style.touchAction=b.touch;}
      pending?.();pending=null;bindings.clear();stopEdge();
    },
  };
}

if (globalThis.exact) globalThis.exact.motionGlue = { motionBytes, motionController, arrangeController };
