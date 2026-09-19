import { chromium } from 'playwright';
import { mkdir, readFile, readdir, writeFile, stat } from 'node:fs/promises';
import { gzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';
const args = Object.fromEntries(process.argv.slice(2).map(a => { const i = a.indexOf('='); return [a.slice(0,i), a.slice(i+1)]; }));
if (!args.url || !args.dist || !args.engine) throw Error('url=... dist=... engine=... out=... required');
const out = resolve(args.out || `results/${args.engine}`);
await mkdir(out,{recursive:true});
const quantile = (a,p) => [...a].sort((a,b)=>a-b)[Math.min(a.length-1,Math.floor(p*a.length))];
async function files(dir) {
  const found=[];
  for (const name of await readdir(dir)) {
    const p=dir+'/'+name;
    if ((await stat(p)).isDirectory()) found.push(...await files(p)); else found.push(p);
  }
  return found;
}
const sizes=[];
for(const file of await files(args.dist)) {
  const bytes=await readFile(file);
  sizes.push({file:file.slice(args.dist.length+1),bytes:bytes.length,gzipBytes:gzipSync(bytes).length,sha256:createHash('sha256').update(bytes).digest('hex')});
}
console.log(JSON.stringify({stage:"package-scanned",engine:args.engine,files:sizes.length}));
const launches=[];
for(let trial=0;trial<Number(args.trials || 5);trial++) {
  console.log(JSON.stringify({stage:"launch",trial,engine:args.engine}));
  const browser=await chromium.launch({executablePath:process.env.CHROMIUM||'/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true,args:['--no-sandbox','--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']});
  try {
    const page=await browser.newPage({viewport:{width:1280,height:800}});
    page.setDefaultTimeout(90000);
    const start=performance.now();
    await page.goto(args.url.replace(/[?&]agent=1/,''));
    await page.waitForFunction(()=>!!window.lanterns?.command);
    const deadline=performance.now()+90000;
    while(!(await page.evaluate(()=>window.lanterns.command({op:'ready'}))).ready) {
      if(performance.now()>deadline)throw Error('Asset readiness timed out');
      await new Promise(resolve=>setTimeout(resolve,20));
    }
    const navigationToReadyMs=performance.now()-start;
    console.log(JSON.stringify({stage:"ready",trial,navigationToReadyMs}));
    await page.evaluate(()=>window.lanterns.command({op:'start'}));
    await page.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));
    const navigationToStartedPaintOpportunityMs=performance.now()-start;
    const frameIntervals=await page.evaluate(()=>new Promise(resolve=>{
      const intervals=[];let previous=performance.now(),warmup=30;
      function sample(now){if(warmup-->0){}else intervals.push(now-previous);previous=now;if(intervals.length<120)requestAnimationFrame(sample);else resolve(intervals)}
      requestAnimationFrame(sample);
    }));
    const cdp=await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    const metrics=Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m=>[m.name,m.value]));
    let summedBrowserRssKiB=null,rssError=null;
    try {
      const session=await browser.newBrowserCDPSession();
      const ids=(await session.send('SystemInfo.getProcessInfo')).processInfo.map(p=>p.id);
      summedBrowserRssKiB=execFileSync('ps',['-o','rss=','-p',ids.join(',')],{encoding:'utf8'}).trim().split(/\s+/).reduce((n,x)=>n+Number(x),0);
    } catch(e) {rssError=String(e)}
    launches.push({trial,navigationToReadyMs,navigationToStartedPaintOpportunityMs,
      frameIntervalP50Ms:quantile(frameIntervals,.5),frameIntervalP95Ms:quantile(frameIntervals,.95),frameIntervals,
      jsHeapUsedBytes:metrics.JSHeapUsedSize,summedBrowserRssKiB,rssError,
      paints:await page.evaluate(()=>performance.getEntriesByType('paint').map(x=>({name:x.name,startTime:x.startTime}))),
      resources:await page.evaluate(()=>performance.getEntriesByType('resource').map(x=>({name:x.name,transferSize:x.transferSize,encodedBodySize:x.encodedBodySize,decodedBodySize:x.decodedBodySize}))),
      state:await page.evaluate(()=>window.lanterns.command({op:'state'}))});
    await writeFile(out+'/partial.json',JSON.stringify({sizes,launches},null,2));
    console.log(JSON.stringify({stage:'sampled',trial}));
  } finally {await browser.close()}
}
const result={engine:args.engine,url:args.url,recordedAt:new Date().toISOString(),
  conditions:'Fresh Chromium process per trial, local server, warm OS cache,1280x800,SwiftShader software rendering; no phone or native package.',
  definitions:{ready:'Adapter asset readiness, not independently measured first pixel',started:'Start command plus two rAF callbacks: paint opportunity, not GPU completion',frame:'120 requestAnimationFrame intervals after30 warmup frames; scheduling/presentation cadence, not GPU execution time',memory:'JS heap excludes much engine/Wasm/GPU memory. Sum of browser-process RSS includes browser overhead and may double-count shared pages.'},
  size:{files:sizes,totalBytes:sizes.reduce((n,x)=>n+x.bytes,0),totalGzipBytes:sizes.reduce((n,x)=>n+x.gzipBytes,0),
    payloadExcludingMapsBytes:sizes.filter(x=>!x.file.endsWith('.map')).reduce((n,x)=>n+x.bytes,0),
    payloadExcludingMapsGzipBytes:sizes.filter(x=>!x.file.endsWith('.map')).reduce((n,x)=>n+x.gzipBytes,0)},launches};
await writeFile(out+'/measurements.json',JSON.stringify(result,null,2));
console.log(JSON.stringify({engine:args.engine,bytes:result.size.totalBytes,gzipBytes:result.size.totalGzipBytes,readyMedianMs:quantile(launches.map(x=>x.navigationToReadyMs),.5),frameMedianMs:quantile(launches.map(x=>x.frameIntervalP50Ms),.5)}));
