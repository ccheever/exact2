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
    const utf16 = new Uint8Array([255,254,65,0,61,216,0,222]);
    const utf16le = new TextDecoder('utf-16le').decode(utf16);
    let utf16Fatal=false;
    try{new TextDecoder('utf-16le',{fatal:true}).decode(new Uint8Array([65]));}catch(e){utf16Fatal=e instanceof TypeError;}
    const utf16Bom=[new TextDecoder('utf-16le').decode(new Uint8Array([255,254,65,0])),new TextDecoder('utf-16le',{ignoreBOM:true}).decode(new Uint8Array([255,254,65,0]))];
    return JSON.stringify({labels,splitCases,encoded:Array.from(encoder.encode('a\0é😀\ud800z\udfff')),into,bytes:Array.from(bytes),chunks,rejected,reset:fatal.decode(new Uint8Array([65])),bom:new TextDecoder('utf8',{ignoreBOM:true}).decode(new Uint8Array([239,187,191])),utf16le,utf16Fatal,utf16Bom,view:decoder.decode(view),invalid:decoder.decode(new Uint8Array([0xed,0xa0,0x80,0xe2,0x28,0xa1]))});
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
  if(source==='abortAny') {
    // `AbortSignal.any`'s argument as Web IDL converts a sequence: its iterator method, `next` and each `value` read
    // once, each value converted as it is yielded; a step result that is not an object refused
    const signal=new AbortController().signal, said=(f:()=>unknown)=>{try{f();return 'made';}catch(e){return (e as Error).name+(e instanceof TypeError?'':': '+(e as Error).message);}};
    let reads=0;
    const once={get [Symbol.iterator](){reads++;return reads===1?function*(){yield signal;}:undefined;}};
    function* lateThrow(){yield {} as AbortSignal;throw new Error('after the bad value');}
    const badStep={[Symbol.iterator]:()=>({next:()=>1})};
    let nexts=0,n=0;
    const nextOnce={[Symbol.iterator]:()=>({get next(){nexts++;return ()=>({done:n++>0,value:signal});}})};
    let gets=0;
    const valueOnce={[Symbol.iterator]:()=>{let k=0;return {next:()=>k++?{done:true}:{done:false,get value(){gets++;return signal;}}};}};
    // An iterator method and a `next` with a `call` of their own, which a conversion must not use (defined, as data
    // code's prototypes are frozen)
    const ownCall={[Symbol.iterator]:()=>({next:Object.defineProperty(()=>({done:true}),'call',{value:()=>({done:false,value:{}})})})};
    const methodCall={[Symbol.iterator]:Object.defineProperty(function*(){yield signal;},'call',{value:()=>1})};
    return JSON.stringify([said(()=>AbortSignal.any(once as unknown as AbortSignal[])),reads,said(()=>AbortSignal.any(lateThrow())),said(()=>AbortSignal.any(badStep as unknown as AbortSignal[])),
      said(()=>AbortSignal.any(nextOnce as unknown as AbortSignal[])),nexts,said(()=>AbortSignal.any(valueOnce as unknown as AbortSignal[])),gets,said(()=>AbortSignal.any(ownCall as unknown as AbortSignal[])),said(()=>AbortSignal.any(methodCall as unknown as AbortSignal[]))]);
  }
  if(source==='response') {
    // `new Response(body, init)`, as an app's own server makes one (Fetch, "Response class")
    const bytes=new Uint8Array([0,104,195,169,0]);
    const made=[new Response('{"a":1}',{status:201,statusText:'Created',headers:{'Content-Type':'application/json','X-A':'b','Set-Cookie':'a=1'}}),
      new Response(null,{status:302,headers:{Location:'app:/back?code=1'}}), new Response('hi'), new Response(new URLSearchParams({a:'1 2'})), new Response(),
      new Response(new Uint8Array(bytes.buffer,1,3)), new Response(new DataView(bytes.buffer,1,3)), new Response({} as unknown as string),
      new Response('\uFEFF{"b":2}'), new Response('a\uD800b'), new Response('',{statusText:'OK\tfine \u00e9'}),
      Response.json({a:[1]},{status:202}), Response.redirect('https://example.test/x?y=1',303)];
    const shapes=made.map(r=>[r.status,r.ok,r.statusText,r.headers.get('content-type'),r.headers.get('location'),r.headers.get('x-a'),r.headers.get('set-cookie')]);
    const said=(f:()=>unknown)=>{try{return f();}catch(e){return (e as Error).name;}};
    const statuses=[99,600,200.5,65736,NaN].map(status=>said(()=>new Response('x',{status}).status));
    const refused=[said(()=>new Response('x',{status:204})), said(()=>new Response('x',{status:205})), said(()=>new Response('x',{status:304})), said(()=>new Response(null,{status:204}).status),
      said(()=>new Response('x',{statusText:'Done \u2713'})), said(()=>new Response('x',{statusText:'a\nb'})),
      said(()=>Response.json(undefined)), said(()=>Response.redirect('https://example.test/x',200)), said(()=>Response.redirect('http://[',200)), said(()=>Response.redirect('/relative')),
      Response.redirect('https://example.test/x',301.9).status];
    const original=new Response('copied',{headers:{'X-A':'b'}}), copy=original.clone();
    copy.headers.set('X-A','c');
    // Headers: a cookie a server sets is refused, a redirect's take no change, and a clone keeps its original's guard
    const guarded=new Response('g'), redirect=Response.redirect('https://example.test/',301);
    guarded.headers.set('Set-Cookie','a=1'); guarded.headers.append('set-cookie2','b'); guarded.headers.set('X-B','c');
    const guardedCopy=guarded.clone();
    guardedCopy.headers.set('set-cookie','z');
    const guards=[guarded.headers.has('set-cookie'),guarded.headers.has('set-cookie2'),guarded.headers.get('x-b'),guardedCopy.headers.has('set-cookie'),
      said(()=>redirect.headers.set('X','y')),said(()=>redirect.clone().headers.delete('location')),redirect.headers.get('location')];
    // A clone copies the response, not what an app gave it
    const odd=new Response('e',{status:203,statusText:'Odd'}) as Response&{extra?:number};
    odd.extra=1;
    Object.defineProperty(odd,'trap',{get(){throw new Error('read');},enumerable:true});
    const oddCopy=odd.clone() as Response&{extra?:number};
    const clones=[oddCopy.status,oddCopy.statusText,oddCopy.ok,oddCopy.extra===undefined,'trap' in oddCopy,Object.keys(new Response('k')).length];
    // Web IDL's conversions: a BigInt status, a symbol body and an init that is no dictionary refused; each member read
    // once, in the dictionary's order
    let reads=0;
    const order:string[]=[];
    const init={get statusText(){order.push('statusText');return 'Made';},get status(){order.push('status');return 201;},get headers(){order.push('headers');return {'X-A':'b'};}};
    // A status converted once: one whose second conversion throws is still the range's error
    const once600=()=>{let n=0;return {valueOf(){if(n++)throw new Error('converted twice');return 600;}} as unknown as number;};
    let redirectStatus=0;
    const converted=[said(()=>new Response(null,{status:BigInt(200) as unknown as number})),said(()=>new Response(Symbol('s') as unknown as string)),
      said(()=>new Response(null,7 as unknown as ResponseInit)),said(()=>new Response('x',init).status),order.join(),
      said(()=>new Response(null,{status:once600()})),said(()=>Response.redirect('https://example.test/',once600())),
      // A redirect's status converted before its URL is parsed
      said(()=>Response.redirect('http://[',{valueOf(){redirectStatus++;return 301;}} as unknown as number)),redirectStatus,
      // Each member converted as it is read: the BigInt status refused before the next member's getter runs
      said(()=>new Response(null,{status:BigInt(200) as unknown as number,get statusText():string{throw new Error('read');}})),
      // Headers converted as strings, checked as headers only after the status: the status's RangeError first, and a
      // later member's getter still run
      said(()=>new Response(null,{headers:{'bad name':'x'},status:600})),said(()=>new Response(null,{headers:{'bad name':'x'},get status(){reads++;return 200;}})),reads,
      // HeadersInit as Web IDL converts it: an iterator method read once and one that is no function refused, a pair
      // that is no sequence refused as it is yielded (before the status), and a record's keys read one at a time, a
      // symbol refused and one a getter deletes skipped
      said(()=>{let n=0;return new Response(null,{headers:{get [Symbol.iterator](){return n++?undefined:function*(){yield ['X-A','1'];};}} as unknown as HeadersInit}).headers.get('x-a');}),
      said(()=>new Response(null,{headers:{[Symbol.iterator]:1} as unknown as HeadersInit})),said(()=>new Response(null,{headers:[42] as unknown as HeadersInit,status:600})),
      said(()=>new Response(null,{headers:{[Symbol('s')]:'x'} as unknown as HeadersInit})),
      said(()=>{const h:Record<string,string>={get 'x-a'(){delete h['x-b'];return '1';},'x-b':'2'};return [...new Response(null,{headers:h}).headers].join();}),
      // and its `next` read once, the iterator method called without its own `call`
      said(()=>{let nexts=0,k=0;const h={[Symbol.iterator]:()=>({get next(){nexts++;return ()=>k++?{done:true}:{done:false,value:['X-N','1']};}})};
        return new Response(null,{headers:h as unknown as HeadersInit}).headers.get('x-n')+nexts;}),
      said(()=>new Response(null,{headers:{[Symbol.iterator]:Object.defineProperty(function*(){yield ['X-C','1'];},'call',{value:()=>({next:()=>({done:true})})})} as unknown as HeadersInit}).headers.get('x-c'))];
    // The body extracted after the init: a form an init getter changes is sent as changed; `Response.json` converts its
    // init before it serializes the data
    const params=new URLSearchParams({a:'1'}), changed=new Response(params,{get status(){params.set('b','2');return 200;}});
    const data={x:1}, factored=Response.json(data,{get status(){data.x=2;return 200;}});
    // and a buffer's bytes taken after the init, an init getter's write among them
    const changing=new Uint8Array([1,2]), extracted=new Response(changing,{get status(){changing[0]=9;return 200;}});
    // What an app gives the objects changes nothing: a buffer's own `slice`, a view's own `buffer`, a form's own
    // `toString`, and a response's headers' own iterator and `forEach`
    const kept=new Uint8Array([1,2]).buffer;
    Object.defineProperty(kept,'slice',{value:()=>kept});
    const sliced=new Response(kept);
    new Uint8Array(kept)[0]=9;
    const lying=new Uint8Array([3,4]);
    Object.defineProperty(lying,'buffer',{value:new Uint8Array([7]).buffer});
    const viewed=new Response(lying), form=new URLSearchParams({a:'1'});
    Object.defineProperty(form,'toString',{value:()=>'z=9'});
    const formed=new Response(form), iterated=new Response('i',{headers:{'X-A':'1'}});
    Object.defineProperty(iterated.headers,Symbol.iterator,{value:function*(){yield ['x-b','2'];}});
    Object.defineProperty(iterated.headers,'forEach',{value:()=>{throw new Error('forEach');}});
    const iteratedCopy=said(()=>iterated.clone()) as Response;
    const ownHeaders=typeof iteratedCopy==='string'?iteratedCopy:[iteratedCopy.headers.get('x-a'),iteratedCopy.headers.get('x-b')];
    // `json()` reads the response's own body, not a `text` an app gives it, and only a Response's
    const overridden=new Response('{"a":1}');
    Object.defineProperty(overridden,'text',{value:()=>Promise.resolve('2')});
    const ownJson=Promise.all([overridden.json(),Response.prototype.json.call({text:()=>Promise.resolve('1')}).then(()=>'read',e=>(e as Error).name)]);
    // The constructor copies a BufferSource's bytes: a later write changes nothing
    const written=new Uint8Array([1,2,3]), snapshot=new Response(written);
    written[0]=9;
    // A body reads once, a clone of a read one refused; no body reads empty as often as asked
    const once=new Response('once'), empty=new Response();
    const bytesOf=(r:Response)=>r.arrayBuffer().then(b=>Array.from(new Uint8Array(b)));
    const readOnce=Promise.all([once.text(),once.text().then(()=>'read again',e=>(e as Error).name),empty.text(),empty.text()])
      .then(read=>[...read,once.bodyUsed,empty.bodyUsed,said(()=>once.clone())]);
    // A body is used as soon as a read starts
    const started=new Response('s');
    started.text();
    const startedNow=[started.bodyUsed,said(()=>started.clone())];
    const nameOf=(e:unknown)=>(e as Error).name, lengthOr=(v:unknown)=>typeof v==='string'?v:(v as ArrayBuffer).byteLength;
    // The BOM's JSON, the view's and the form's bytes from clones made before the bodies below are read
    return Promise.all([made[0].json(),made[8].clone().json(),bytesOf(made[5].clone()),bytesOf(made[6].clone()),bytesOf(made[3].clone()),...made.slice(1).map(r=>r.text()),
      bytesOf(new Response('hé')),bytesOf(new Response(new Uint8Array([1,2,255]).buffer)),copy.text(),bytesOf(snapshot),readOnce,changed.text(),ownJson,
      factored.text(),bytesOf(sliced),bytesOf(viewed),formed.text(),
      // A body is read in a later turn, as Fetch reads one, so a value already there wins a race with it; no body is
      // read at once, so it wins
      Promise.race([new Response('x').text(),Promise.resolve('fallback')]),Promise.race([new Response().text(),Promise.resolve('fallback')]),
      Promise.race([new Response().arrayBuffer(),Promise.resolve('fallback')]).then(lengthOr),
      Promise.race([new Response(new Uint8Array([1])).arrayBuffer(),Promise.resolve('fallback')]).then(lengthOr),
      // JSON is parsed as the body is read: a body's loses the race, and no body's rejects at once, as do a used
      // body's and a read of what is no Response
      Promise.race([new Response('{"a":1}').json(),Promise.resolve('fallback')]),Promise.race([new Response().json(),Promise.resolve('fallback')]).then(v=>v,nameOf),
      Promise.race([(()=>{const used=new Response('u');used.text();return used.json();})(),Promise.resolve('fallback')]).then(v=>v,nameOf),
      Promise.race([Response.prototype.json.call({}),Promise.resolve('fallback')]).then(v=>v,nameOf),bytesOf(extracted)])
      .then(bodies=>JSON.stringify({shapes,bodies,statuses,refused,guards,clones,converted,ownHeaders,startedNow,headers:[original.headers.get('x-a'),copy.headers.get('x-a')]})) as unknown as string;
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
  if(source==='hooks') {
    const hook=(globalThis as any).__exact_ibex2_abort_hooks;
    return JSON.stringify({present:'__exact_ibex2_abort_hooks' in globalThis,own:typeof hook?.own,subscribe:typeof hook?.subscribe,
      headersHooks:['__exact_ibex2_headers_hooks','__ibex2_headers_hooks'].some(name=>name in globalThis)});
  }
  // Intl.NumberFormat as the browser formats it (x2apps stocks #3: Hermes
  // on macOS printed `9,274,743` for compact): compact in its displays,
  // currency and percent, each in a few locales.
  if(source==='intl') {
    const values=[0,-0,0.5,0.0123,1,12,999,999.9,1000,1234,1250,9999,12345,99999,123456,999999,1e6,1234567,9274743,99999999,543578062292.7,1e12,1.5e13,-2500,-9274743,1e15];
    const locales=['en-US','de-DE','fr-FR','ja-JP','es-ES','en-GB','pt-BR','it-IT','zh-CN','ko-KR','en-IN','hi-IN'];
    const out:Record<string,string[]>={};
    for(const locale of locales) for(const [name,options] of [
      ['compact',{notation:'compact'}],
      ['compact2',{notation:'compact',maximumFractionDigits:2}],
      ['compact-sd',{notation:'compact',maximumSignificantDigits:3}],
    ] as [string,Intl.NumberFormatOptions][]) out[locale+' '+name]=values.map(v=>new Intl.NumberFormat(locale,options).format(v));
    out['en-US toLocaleString']=values.map(v=>v.toLocaleString('en-US',{notation:'compact'}));
    // toLocaleString keeps a formatter per locales and options it reads the
    // same: an options object changed between calls formats by its new value.
    const changing:Intl.NumberFormatOptions={maximumFractionDigits:1};
    const kept=[1.25,1.25,12345.678].map(v=>v.toLocaleString('en-US',changing));
    changing.maximumFractionDigits=3;
    kept.push((1.23456).toLocaleString('en-US',changing),(1.23456).toLocaleString(['de-DE'],changing),(1.23456).toLocaleString('de-DE',changing),(1234.5).toLocaleString('en-US'));
    out['en-US toLocaleString kept']=kept;
    // A DateTimeFormat made with the same locales and options shares one
    // engine formatter, each `new` an object of its own; a Date's
    // toLocale…String adds its default fields when the options name none.
    const days=[Date.UTC(2026,0,5,9,30,15),Date.UTC(2025,11,31,23,5,0),Date.UTC(2024,1,29,12,0,0)].map(t=>new Date(t));
    const utc={timeZone:'UTC'} as const, short:Intl.DateTimeFormatOptions={month:'short',day:'numeric',year:'numeric',timeZone:'UTC'};
    const dtf=(locale:string,options:Intl.DateTimeFormatOptions)=>days.map(d=>new Intl.DateTimeFormat(locale,options).format(d));
    out['dtf short']=dtf('en-US',short).concat(dtf('en-US',short),dtf('de-DE',short));
    out['dtf long']=dtf('en-US',{dateStyle:'long',timeZone:'UTC'}).concat(dtf('fr-FR',{weekday:'long',month:'long',day:'numeric',timeZone:'UTC'}));
    const a=new Intl.DateTimeFormat('en-US',short), b=new Intl.DateTimeFormat('en-US',short);
    out['dtf objects']=[String(a===b),String(a instanceof Intl.DateTimeFormat),String(a.constructor===Intl.DateTimeFormat),Object.prototype.toString.call(a),
      String(Intl.DateTimeFormat.length),String(Date.prototype.toLocaleDateString.length),a.resolvedOptions().month as string,a.resolvedOptions().timeZone as string,
      a.formatToParts(days[0]).map(p=>p.value).join(''),String(a.format===b.format),String(a.format===a.format)];
    const changingDate:Intl.DateTimeFormatOptions={month:'long',timeZone:'UTC'};
    const keptDates=days.map(d=>d.toLocaleDateString('en-US',changingDate));
    changingDate.month='numeric';
    keptDates.push(days[0].toLocaleDateString('en-US',changingDate));
    out['date kept']=keptDates;
    const call=(f:()=>string)=>{try{return f();}catch(e){return (e as Error).name;}};
    out['toLocaleDateString']=days.map(d=>d.toLocaleDateString('en-US',utc)).concat(days.map(d=>d.toLocaleDateString('en-GB',utc)),
      days.map(d=>d.toLocaleDateString('en-US',{weekday:'short',month:'short',day:'numeric',year:'numeric',timeZone:'UTC'})),
      days.map(d=>d.toLocaleDateString('en-US',{hour:'numeric',hourCycle:'h23',timeZone:'UTC'})),
      [call(()=>days[0].toLocaleDateString('en-US',{timeStyle:'short',timeZone:'UTC'})),new Date(NaN).toLocaleDateString('en-US',utc),call(()=>Date.prototype.toLocaleDateString.call({} as Date,'en-US'))]);
    out['toLocaleTimeString']=days.map(d=>d.toLocaleTimeString('en-US',{hourCycle:'h23',timeZone:'UTC'})).concat(days.map(d=>d.toLocaleTimeString('de-DE',utc)),
      days.map(d=>d.toLocaleTimeString('en-US',{hour:'2-digit',minute:'2-digit',hourCycle:'h23',timeZone:'UTC'})),[call(()=>days[0].toLocaleTimeString('en-US',{dateStyle:'short',timeZone:'UTC'}))]);
    out['Date toLocaleString']=days.map(d=>d.toLocaleString('en-US',{hourCycle:'h23',timeZone:'UTC'})).concat(days.map(d=>d.toLocaleString('de-DE',utc)),
      days.map(d=>d.toLocaleString('en-US',{month:'short',timeZone:'UTC'})),days.map(d=>d.toLocaleString('en-US',{dateStyle:'medium',timeZone:'UTC'})));
    for(const locale of ['en-US','de-DE','fr-FR']) {
      out[locale+' currency']=[1234.5,-0.41,166.07].map(v=>new Intl.NumberFormat(locale,{style:'currency',currency:'USD'}).format(v));
      out[locale+' percent']=[0.0041,-0.0041,0.5].map(v=>new Intl.NumberFormat(locale,{style:'percent',minimumFractionDigits:2}).format(v));
      out[locale+' resolved']=[new Intl.NumberFormat(locale,{notation:'compact'}).resolvedOptions().notation as string];
    }
    // A half rounds away from zero, from the number's shortest digits
    // (X72: Apple's engine rounded USD 0.825 to even, `$0.82`), in each
    // style and digit option, and by the formatter's roundingMode.
    const halves=[0.825,1.005,2.675,0.125,-0.825], usd={style:'currency',currency:'USD'} as const;
    const format=(options:object,v:number)=>{try{return new Intl.NumberFormat('en-US',options).format(v);}catch(e){return (e as Error).name;}};
    out['en-US currency halves']=halves.map(v=>format(usd,v));
    out['en-US currency halfExpand']=halves.map(v=>format({...usd,roundingMode:'halfExpand'},v));
    out['en-US currency toLocaleString']=halves.map(v=>v.toLocaleString('en-US',usd));
    out['de-DE currency halves']=halves.map(v=>new Intl.NumberFormat('de-DE',usd).format(v));
    out['en-US yen halves']=[1234.5,-1234.5,0.5,2.5].map(v=>format({style:'currency',currency:'JPY'},v));
    out['en-US percent halves']=[0.125,0.0125,-0.125,0.995,0.005].map(v=>format({style:'percent'},v));
    out['en-US percent2 halves']=[0.00125,-0.00125,0.012345].map(v=>format({style:'percent',minimumFractionDigits:2},v));
    out['en-US significant halves']=[0.125,12.5,1250,-0.0125,9.95,0.000125].map(v=>format({maximumSignificantDigits:2},v));
    out['en-US scientific halves']=[1234.5,0.00012345,1.0005,-1234.5].map(v=>format({notation:'scientific'},v));
    out['en-US decimal halves']=[1.0005,-1.0005,2.5,0.1+0.2,1234567890123.4567,-0.0001].map(v=>format({},v));
    out['en-US integer halves']=[2.5,3.5,-2.5,0.5].map(v=>format({maximumFractionDigits:0},v));
    for(const mode of ['ceil','floor','expand','trunc','halfCeil','halfFloor','halfExpand','halfTrunc','halfEven','up'])
      out['en-US roundingMode '+mode]=[1.25,-1.25,1.35,1.21,-1.21,1.2,-0.01].map(v=>format({maximumFractionDigits:1,roundingMode:mode},v))
        .concat([1250,-1250,1350,12500].map(v=>format({notation:'compact',roundingMode:mode},v)));
    // Compact the prelude prints in full (a currency or percent, a long
    // display, a locale it has no table for), at values Chrome's compact
    // prints with the same digits.
    out['en-US compact currency halves']=[0.825,0.125,-0.825].map(v=>format({...usd,notation:'compact'},v));
    for(const mode of ['halfEven','floor','ceil'])
      out['en-US compact currency '+mode]=[0.825,0.829,-0.821].map(v=>format({...usd,notation:'compact',roundingMode:mode},v));
    out['en-US compact percent halves']=[0.125,-0.125,0.165].map(v=>format({style:'percent',notation:'compact'},v));
    out['en-US compact long halves']=[0.0125,-0.0125,0.0145].map(v=>format({notation:'compact',compactDisplay:'long'},v));
    out['sk-SK compact halves']=[0.0125,-0.0125,0.0145].map(v=>new Intl.NumberFormat('sk-SK',{notation:'compact'}).format(v));
    const nf=new Intl.NumberFormat('en-US');
    out['en-US resolved roundingMode']=[nf,new Intl.NumberFormat('en-US',{roundingMode:'floor'} as Intl.NumberFormatOptions),new Intl.NumberFormat('en-US',{notation:'compact',roundingMode:'halfEven'} as Intl.NumberFormatOptions)]
      .map(f=>String((f.resolvedOptions() as {roundingMode?:string}).roundingMode)).concat([String(nf.format===nf.format),typeof Object.getOwnPropertyDescriptor(Intl.NumberFormat.prototype,'format')?.get]);
    // Intl.Locale and its week (#118): the tag, every field, and getWeekInfo(), or the error's name.
    const fields=['baseName','language','script','region','variants','calendar','caseFirst','collation','firstDayOfWeek','hourCycle','numeric','numberingSystem'];
    const locale=(tag:unknown,options?:object):string[]=>{
      try{const l:any=new (Intl as any).Locale(tag,options);return [String(l),...fields.map(f=>String(l[f])),JSON.stringify(l.getWeekInfo()),Object.prototype.toString.call(l),JSON.stringify(l)];}
      catch(e){return [(e as Error).name];}
    };
    for(const tag of ['en-US','de-DE','ar-EG','ar-SA','ja-JP','ko-KR','en-GB','fa-IR','hi-IN','he-IL','ps-AF','dv-MV','ug-UG','en-AU','fr-CA','pt-BR','pt-PT','es-419','en-001',
      'en','de','ar','fa','he','zh','zh-Hant','pt','und','sr-Latn','pa-Arab','fil','yue','EN-latn-us-u-ca-gregory-hc-h12','en-u-fw-mon','en-US-u-fw-sat','en-u-fw-xyz',
      'de-u-rg-uszzzz','en-u-kn','en-u-kn-false','en-u-nu-arab-ca-islamic','de-DE-1996-fonipa','sl-rozaj-biske-1994','en-a-foo-b-bar-u-ca-gregory','en-US-x-foo','en-t-de',
      'en-u-attr-ca-gregory','en-u-ca-gregory-ca-buddhist','','not_a_tag','en-','i-klingon','x-private','en-US-US','en-1996-1996','en-u','root',
      'ar-AE','en-AE','en-US-u-ca-iso8601','ar-EG-u-ca-iso8601','en-US-u-ca-iso8601-fw-sun','en-u-ca','en-u-kf','en-u-hc','en-u-co','en-u-nu','en-u-fw',
      'und-Arab','und-Deva','en-Shaw','pi-Thai','en-t-12','en-t-en-foo','en-t-de-k0-tech','en-t-k0-tech-h0-hybrid','en-t-ja-Latn-JP'])
      out['Locale '+tag]=locale(tag);
    for(const [tag,options] of [['en',{region:'GB',calendar:'gregory',hourCycle:'h23',firstDayOfWeek:'mon'}],['en',{numeric:true}],['en',{numeric:false}],['en',{firstDayOfWeek:0}],
      ['en',{firstDayOfWeek:7}],['en',{firstDayOfWeek:'monday'}],['en',{calendar:'Gregory'}],['en',{hourCycle:'h25'}],['en',{caseFirst:false}],['en-US',{language:'de'}],
      ['en',{region:'419'}],['en',{language:'en-US'}],['en-u-ca-buddhist',{calendar:'gregory'}],['ar',{region:'SA'}],['de',{region:'US'}],
      ['en',{variants:'fonipa'}],['en-1996',{variants:'FONIPA-1994'}],['en',{variants:''}],['en',{variants:'fonipa-fonipa'}],['en',{variants:'ab'}]] as [string,object][])
      out['Locale '+tag+' '+JSON.stringify(options)]=locale(tag,options);
    for(const tag of [undefined,null,5]) out['Locale '+String(tag)]=locale(tag);
    out['Locale en with null options']=locale('en',null as unknown as object);
    out['Locale length']=[String((Intl as any).Locale.length)];
    out['Locale of a Locale']=locale(new (Intl as any).Locale('de-DE-u-ca-gregory'),{region:'AT'});
    return JSON.stringify(out);
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
// An async mode replaces the call's envelope; a sync one the reply its settle
// gives after the checkpoint (a value given at once is a call too).
function wire(text:string,mode:string):unknown {
  const stringify=JSON.stringify;
  JSON.stringify=((value:unknown,replacer:never,space:never)=>{
    if(!mode.startsWith('async')&&(value as {tag?:number}|null)?.tag===3) return stringify(value,replacer,space);
    JSON.stringify=stringify;
    stringify(value,replacer,space);
    return text;
  }) as typeof JSON.stringify;
  const value=mode.includes('capture')?'a\\\0é😀\u2028\u2029'.repeat(8192):'settled';
  return mode.startsWith('async')?Promise.resolve(value):value;
}
(globalThis as any).exact={abi:1,appId:'test.pure',grants:'',answer:(source:string,args:unknown[]=[])=>source==='wire'?wire(String(args[0]),String(args[1])):source==='transfer'?transfer(String(args[0])):exercise(source)};
