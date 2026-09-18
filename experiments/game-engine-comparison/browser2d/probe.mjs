import { chromium } from 'playwright';
import { writeFileSync, statSync } from 'node:fs';
const server=Bun.serve({port:0,fetch(req){const p=new URL(req.url).pathname;if(p==='/favicon.ico')return new Response(null,{status:204});return new Response(Bun.file('.'+(p==='/'?'/phaser.html':p)));}});
const browser=await chromium.launch({executablePath:'/home/ccheever/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome',headless:true,args:['--no-sandbox','--enable-unsafe-swiftshader']});
const results=[];
for(const engine of ['phaser','kaplay']){
 const runs=[];
 for(let run=0;run<5;run++){
  const page=await browser.newPage();const errors=[];page.on('pageerror',e=>{errors.push(String(e));console.log(engine,String(e))});
  const started=performance.now();await page.goto(`http://localhost:${server.port}/${engine}.html`);await page.waitForFunction(()=>window.probe);
  const startupMs=performance.now()-started;
  const result=await page.evaluate(()=>{let start=performance.now();probe.reset();const initial=probe.state();probe.input(true);probe.ticks(80);const held=probe.state();probe.input(false);probe.input(true);probe.ticks(1);probe.input(false);const moved=probe.state();const saved=probe.save();probe.reset();const reset=probe.state();probe.restore(saved);const restored=probe.state();return {initial,held,moved,reset,restored,saved,executionMs:performance.now()-start,passed:held.x===180&&held.collected===3&&moved.collected===4&&reset.x===20&&reset.collected===0&&restored.x===moved.x&&restored.collected===4};});
  if(run===0)await page.screenshot({path:`${engine}.png`});runs.push({startupMs,...result,errors});await page.close();
 }
 results.push({engine,runs});writeFileSync('partial.json',JSON.stringify(results,null,2));
}
writeFileSync('results.json',JSON.stringify({host:process.env.HOSTNAME,versions:{phaser:'4.2.1',kaplay:'3001.0.19',playwright:'1.63.0'},renderer:'Headless Chromium; Phaser Canvas, KAPLAY WebGL SwiftShader',results},null,2));
console.log(JSON.stringify(results,null,2));await browser.close();server.stop();if(results.some(e=>e.runs.some(r=>!r.passed||r.errors.length)))process.exit(1);
