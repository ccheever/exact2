// The same utility corpus runs in Hermes and Chrome. Returning JSON keeps the
// Contract signature small; the separate transfer corpus also uses Bun's JSON.
// What an object says of itself never steers a copy (review r4a 5, 9, 10):
// its own `constructor`, getters and tags, a key named `__proto__`, holes.
function steered() {
  const proto=structuredClone(JSON.parse('{"__proto__":{"admin":true},"ok":1}'));
  const bytes=new Uint8Array([1,2]);
  Object.defineProperty(bytes,'constructor',{value:function(){return new Uint8Array([99,99]);}});
  Object.defineProperty(bytes,'buffer',{get(){return new ArrayBuffer(8);}});
  const bytesCopy=structuredClone(bytes), sortedBytes=bytes.toSorted(), withBytes=bytes.with(0,7);
  const date=new Date(5); Object.defineProperty(date,'getTime',{value:()=>9});
  const fake={[Symbol.toStringTag]:'Date',n:1};
  const map=new Map([[1,2]]); Object.defineProperty(map,'forEach',{value:()=>{}});
  const re=/a/g; Object.defineProperty(re,'flags',{value:'i'}); Object.defineProperty(re,'global',{value:false});
  let visits=0;
  const holes=[2,,1].toSorted(); holes.map(()=>visits++);
  return {proto:[Object.keys(proto),Object.getPrototypeOf(proto)===Object.prototype,'admin' in proto,JSON.stringify(proto)],
    bytes:[Array.from(bytesCopy),Object.getPrototypeOf(bytesCopy)===Uint8Array.prototype,bytesCopy.buffer.byteLength,Array.from(sortedBytes),Object.getPrototypeOf(sortedBytes)===Uint8Array.prototype,Array.from(withBytes)],
    date:structuredClone(date).getTime(), fake:[Object.keys(structuredClone(fake)),typeof (structuredClone(fake) as any).getTime],
    map:[...structuredClone(map)], re:[structuredClone(re).flags,structuredClone(re).source],
    holes:[holes.length,0 in holes,1 in holes,2 in holes,visits]};
}
export function exercise(source: string): string {
  const encoder = new TextEncoder();
  if(source==='text') {
    const labels=['UTF-8',' utf8 ','unicode-1-1-utf-8','unicode11utf8','unicode20utf8','x-unicode20utf8'].map(s=>new TextDecoder(s).encoding);
    const bytes = new Uint8Array(8).fill(17);
    const into = encoder.encodeInto('😀\ud800x', bytes.subarray(1,7));
    const decoder = new TextDecoder(' UTF8 ');
    const chunks = [decoder.decode(new Uint8Array([239]), {stream:true}), decoder.decode(new Uint8Array([187,191,240,159]), {stream:true}), decoder.decode(new Uint8Array([152,128,239,187,191]), {stream:true}), decoder.decode()];
    const fatal = new TextDecoder('utf-8',{fatal:true});
    let rejected=false;
    try{fatal.decode(new Uint8Array([0xe2]),{stream:true});fatal.decode();}catch(e){rejected=e instanceof TypeError;}
    const splitCases:string[][]=[];
    for(const bytes of [[239,187,191,65,239,187,191],[240,159,152,128],[226,130],[237,160,128],[226,40,161],[244,144,128,128],[128,65]]) {
      for(let split=0;split<=bytes.length;split++) {
        const d=new TextDecoder();
        splitCases.push([d.decode(new Uint8Array(bytes.slice(0,split)),{stream:true}), d.decode(new Uint8Array(bytes.slice(split)))]);
      }
    }
    const view = new DataView(new Uint8Array([88,0,65,89]).buffer,1,2);
    return JSON.stringify({labels,splitCases,encoded:Array.from(encoder.encode('a\0é😀\ud800z\udfff')),into,bytes:Array.from(bytes),chunks,rejected,reset:fatal.decode(new Uint8Array([65])),bom:new TextDecoder('utf8',{ignoreBOM:true}).decode(new Uint8Array([239,187,191])),view:decoder.decode(view),invalid:decoder.decode(new Uint8Array([0xed,0xa0,0x80,0xe2,0x28,0xa1]))});
  }
  if(source==='url') {
    const url = new URL('../c?q=a%20b&x=1&x=2#old','https://例え.テスト/a/b/');
    const params=url.searchParams;
    params.set('q','😀\ud800');params.delete('x','1');params.append('a','\0');params.sort();
    url.port='8080abc';url.hash='new';const first=url.href;
    url.search='?b=2&a=1&a=3';params.sort();const linked=String(params);
    url.pathname='a b';url.protocol='mailto';
    const live=new URLSearchParams('a=1'), iterator=live.entries();
    const seen=[iterator.next().value];live.append('b','2');seen.push(iterator.next().value);
    const visits:string[]=[];live.forEach((v,k)=>{visits.push(k+v);if(k==='a')live.append('c','3');});
    return JSON.stringify({seen,visits,first,linked,href:url.href,same:params===url.searchParams,entries:Array.from(params),bad:URL.canParse('/x'),parse:URL.parse('not a url'),nul:new URLSearchParams([['\0','\0']]).get('\0')});
  }
  if(source==='standard') {
    // What a web developer expects of a data module (docs/reference.md):
    // the globals Hermes is given (js/src/standard.js, Ibex's abort.js).
    const shared={n:1}, cyclic:any={shared,list:[shared,shared],when:new Date(86400000),re:/a+/gi,
      map:new Map<unknown,unknown>([[shared,'k'],['v',shared]]),set:new Set([1,shared]),bytes:new Uint8Array([1,2,3]).subarray(1),
      boxed:Object('s'),big:12n,error:new RangeError('far',{cause:shared}),sparse:[1,,3],get read(){return 'got';}};
    cyclic.self=cyclic;
    const copy=structuredClone(cyclic);
    const refused=[()=>{},Symbol('s'),Promise.resolve(),new WeakMap()].map(v=>{try{structuredClone(v);return 'cloned';}catch(e){return (e as DOMException).name;}});
    const controller=new AbortController(), heard:string[]=[];
    controller.signal.addEventListener('abort',()=>heard.push('listener'));
    controller.signal.onabort=()=>heard.push('onabort');
    controller.abort();
    let thrown='';
    try{controller.signal.throwIfAborted();}catch(e){thrown=(e as DOMException).name;}
    const any=AbortSignal.any([new AbortController().signal,AbortSignal.abort('why')]);
    return JSON.stringify({
      distinct:copy!==cyclic&&copy.shared!==shared, shared:copy.list[0]===copy.list[1]&&copy.list[0]===copy.shared&&copy.map.get('v')===copy.shared,
      cyclic:copy.self===copy, when:copy.when.getTime(), re:[copy.re.source,copy.re.flags], map:[...copy.map.values()].length, set:copy.set.has(copy.shared),
      bytes:[Array.from(copy.bytes),copy.bytes.byteOffset,copy.bytes.buffer.byteLength], boxed:typeof copy.boxed, big:String(copy.big),
      error:[copy.error instanceof RangeError,copy.error.message,copy.error.cause===copy.shared], sparse:[copy.sparse.length,1 in copy.sparse], read:copy.read,
      refused, aborted:[controller.signal.aborted,(controller.signal.reason as DOMException).name,thrown,heard], any:[any.aborted,any.reason],
      sorted:[[3,1,2].toSorted(),[3,1,2].toSorted((a,b)=>b-a),Array.prototype.toSorted.call({length:2,0:'b',1:'a'})],
      typed:[Array.from(new Int8Array([1,-2,3]).toReversed()),Array.from(new Int8Array([3,-2,1]).toSorted()),Array.from(new Int8Array([1,2,3]).with(-1,9))],
      newer:[[1,2,3].at(-1),[1,2,3].findLast(n=>n<3),Object.groupBy([1,2,3],n=>n%2?'odd':'even'),'a.b'.replaceAll('.','/'),typeof Promise.withResolvers],
      ...steered(),
    });
  }
  if(source==='microtask') {
    const order:string[]=[];
    queueMicrotask(()=>order.push('first'));
    Promise.resolve().then(()=>order.push('promise'));
    queueMicrotask(()=>{order.push('throws');throw new Error('reported, not rejected');});
    queueMicrotask(()=>order.push('after'));
    order.push('sync');
    return new Promise<string>(done=>queueMicrotask(()=>done(JSON.stringify(order)))) as unknown as string;
  }
  if(source==='abort') {
    // A fetch's `signal`, before and after the request starts; the reply of
    // the aborted one is never awaited.
    const early=fetch('https://example.invalid/early',{signal:AbortSignal.abort()}).then(()=>'fetched',e=>(e as Error).name);
    const controller=new AbortController();
    const late=fetch('https://example.invalid/late',{signal:controller.signal}).then(()=>'fetched',e=>(e as Error).name);
    controller.abort(new Error('mine'));
    // An abort listener the app added first cannot stop the fetch's own
    // (review r4a 8); a signal that is no AbortSignal is a TypeError.
    const quiet=new AbortController();
    quiet.signal.addEventListener('abort',e=>e.stopImmediatePropagation());
    const suppressed=fetch('https://example.invalid/quiet',{signal:quiet.signal}).then(()=>'fetched',e=>(e as Error).name);
    quiet.abort();
    const bad=fetch('https://example.invalid/bad',{signal:{aborted:false} as unknown as AbortSignal}).then(()=>'fetched',e=>(e as Error).name);
    return Promise.all([early,late,late.then(()=>controller.signal.reason.message),suppressed,bad]).then(JSON.stringify) as unknown as string;
  }
  if(source==='base64') {
    const inputs=['','Zg','Zh','Zg==','Zm8','Zm9','Zm9v',' /w==\n','AA=='];
    const rejected=['a','a===','Zg=','%%%%','-w=='].map(s=>{try{atob(s);return false;}catch(e){return (e as Error).name==='InvalidCharacterError';}});
    let unicode=false;try{btoa('\ud800');}catch(e){unicode=(e as Error).name==='InvalidCharacterError';}
    return JSON.stringify({values:inputs.map(s=>atob(s)),encoded:btoa('\0ÿabc'),rejected,unicode});
  }
  throw new Error(source);
}
function transfer(mode:string):unknown {
  const big='a\\\0é😀\u2028\u2029'.repeat(8192);
  if(mode==='plain')return big;
  if(mode==='boxed')return new String(big);
  if(mode==='lone')return 'x'.repeat(65536)+'\ud800';
  if(mode==='arrayMethodsHook'){
    for(const name of ['concat','push','reverse'])
      Object.defineProperty(Array.prototype,name,{value(){throw new Error('application array method hook');},configurable:true});
    return {first:big,second:big};
  }
  if(mode==='arrayHook'){
    Object.defineProperty(Array.prototype,'toJSON',{value(){return 'custom array';},configurable:true});
    return big;
  }
  if(mode==='private')return typeof (globalThis as any).__exact_capture_string;
  if(mode==='small')return 'a later call';
  if(mode==='nullable')return [undefined,NaN,Infinity,null];
  if(mode==='extra')return {first:big,second:'present',extra:'not declared'};
  if(mode==='missing')return {first:big,second:undefined};
  if(mode==='throw')return {first:big,get extra(){throw new Error('getter failed');}};
  if(mode==='cycle'){const value:any={first:big};value.cycle=value;return value;}
  if(mode==='bigint')return {first:big,extra:1n};
  if(mode==='branches') {
    let reads=0;
    const shared={text:big};
    return {first:{left:shared,right:{toJSON(){return shared;}}},
      get second(){reads++;shared.text=big+' changed';return shared;},
      last:[{left:{text:'small'},right:shared},{left:shared,right:{text:big}}],
      get reads(){return reads;}};
  }
  if(mode==='object') {
    let reads=0;
    const shared={text:big};
    return {get first(){reads++;return big;},second:{toJSON(){return big;}},
      aliases:[shared,shared],get reads(){return reads;},omitted:undefined};
  }
  throw new Error(mode);
}
// Exercise the actual native envelope parser, including replies the ordinary
// serializer cannot emit. Restore stringify before it visits capture paths.
function wire(text:string,mode:string):unknown {
  const stringify=JSON.stringify;
  JSON.stringify=((value:unknown,replacer:never,space:never)=>{
    if (!mode.startsWith('async') && (value as any)?.tag === 3) return stringify(value,replacer,space);
    JSON.stringify=stringify;
    stringify(value,replacer,space);
    return text;
  }) as typeof JSON.stringify;
  const value=mode.includes('capture')?'a\\\0é😀\u2028\u2029'.repeat(8192):'settled';
  return mode.startsWith('async')?Promise.resolve(value):value;
}
(globalThis as any).exact={abi:1,appId:'test.pure',grants:'',answer:(source:string,args:unknown[]=[])=>source==='wire'?wire(String(args[0]),String(args[1])):source==='transfer'?transfer(String(args[0])):exercise(source)};
