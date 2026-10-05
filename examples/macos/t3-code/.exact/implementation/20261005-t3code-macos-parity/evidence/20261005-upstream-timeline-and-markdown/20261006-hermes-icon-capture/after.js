globalThis.result='pending';
async function sync() {
 let cache = new Map();
 let pending = new Map();
 const waits=[];
 for (const key of ['a', null, 'b', null]) {
  if (!key) continue;
  const request=load(key,cache,pending);
  pending.set(key,request); waits.push(request);
 }
 await Promise.all(waits);
 return cache.size;
}
async function load(key,destination,requests) {
 await Promise.resolve();
 destination.set(key,{src:'ok'});
 requests.delete(key);
}
sync().then(v=>globalThis.result='PASS '+v,e=>globalThis.result='FAIL '+e.stack);
