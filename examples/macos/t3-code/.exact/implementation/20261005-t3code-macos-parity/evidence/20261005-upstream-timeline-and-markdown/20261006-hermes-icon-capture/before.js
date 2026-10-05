globalThis.result='pending';
async function sync() {
 let cache = new Map();
 let pending = new Map();
 const waits=[];
 for (const key of ['a', null]) {
  if (!key) continue;
  const destination=cache, requests=pending;
  const request=(async()=> {
   await Promise.resolve();
   destination.set(key, {src:'ok'});
   requests.delete(key);
  })();
  pending.set(key,request); waits.push(request);
 }
 await Promise.all(waits);
 return cache.size;
}
sync().then(v=>globalThis.result='PASS '+v,e=>globalThis.result='FAIL '+e.stack);
