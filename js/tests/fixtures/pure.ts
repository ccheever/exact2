// The same utility corpus runs in Hermes and Chrome. Returning JSON keeps the
// Contract signature small; the separate transfer corpus also uses Bun's JSON.
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
  if(mode==='reverseHook'){
    Object.defineProperty(Array.prototype,'reverse',{value(){throw new Error('application reverse hook');},configurable:true});
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
  if(mode==='object') {
    let reads=0;
    const shared={text:big};
    return {get first(){reads++;return big;},second:{toJSON(){return big;}},
      aliases:[shared,shared],get reads(){return reads;},omitted:undefined};
  }
  throw new Error(mode);
}
(globalThis as any).exact={abi:1,appId:'test.pure',grants:'',answer:(source:string,args:unknown[]=[])=>source==='transfer'?transfer(String(args[0])):exercise(source)};
