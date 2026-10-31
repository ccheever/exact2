import { expect, test } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../../scripts/agent-launch.mjs';

// Exercise real opaque Windows, JSON-object SDK messages, and response CSP.
test('bundled adapter authenticates its parent and guest document before converting JSON', async () => {
  const adapter = new URL('../../assets/inline-document/index.html', import.meta.url);
  const server = Bun.serve({ port: 0, fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === '/adapter') return new Response(Bun.file(adapter), { headers: { 'content-type': 'text/html' } });
    if (url.pathname.startsWith('/guest')) return new Response(`<!doctype html><body>SDK guest<script>
      window.received=[];addEventListener('message',e=>{if(e.source!==parent||e.origin!=='null'||typeof e.data!=='object')return;received.push(e.data);parent.postMessage({jsonrpc:'2.0',method:'guest/received',params:e.data},'*')});
      parent.postMessage({jsonrpc:'2.0',id:1,method:'ui/initialize',params:{}},'*');
    </script>`, { headers: { 'content-type': 'text/html', 'content-security-policy': "default-src 'none'; script-src 'unsafe-inline'; connect-src 'none'; frame-src 'none'; sandbox allow-scripts allow-forms allow-popups allow-downloads" } });
    return new Response(`<!doctype html><main></main><script>
      window.rogueDone=false;addEventListener('message',e=>{if(e.data==='rogue-done')rogueDone=true});window.messages=[];window.token='mounted-document-1';
      const frame=document.createElement('iframe');frame.id='adapter';frame.sandbox='allow-scripts';frame.src='/adapter#'+encodeURIComponent(JSON.stringify({src:location.origin+'/guest',token}));
      addEventListener('message',e=>{if(e.source!==frame.contentWindow||e.origin!=='null'||typeof e.data!=='string')return;const v=JSON.parse(e.data);if(v.t3Document===token)messages.push(v.message)});
      document.querySelector('main').append(frame);
      window.send=message=>frame.contentWindow.postMessage(JSON.stringify({t3Document:token,message}),'*');
    </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => messages.some(v=>v.method==='t3/adapter-ready'));
    expect(await page.evaluate(() => messages.find(v=>v.method==='ui/initialize'))).toMatchObject({ jsonrpc:'2.0', id:1 });
    const guest = page.frames().find(f=>new URL(f.url()).pathname==='/guest');
    expect(guest).toBeDefined();
    await page.evaluate(() => send({jsonrpc:'2.0',id:1,result:{hostContext:{theme:'dark'}}}));
    await page.waitForFunction(() => messages.some(v=>v.method==='guest/received'));
    expect(await guest.evaluate(() => received)).toEqual([{jsonrpc:'2.0',id:1,result:{hostContext:{theme:'dark'}}}]);
    // A sibling has the same opaque origin but a different Window identity.
    await page.evaluate(() => {
      const rogue=document.createElement('iframe');rogue.sandbox='allow-scripts';rogue.srcdoc=`<script>parent.frames[0].postMessage(JSON.stringify({t3Document:'mounted-document-1',message:{jsonrpc:'2.0',method:'forged'}}),'*');parent.postMessage('rogue-done','*')<\/script>`;
      document.querySelector('main').append(rogue);
      document.querySelector('#adapter').contentWindow.postMessage(JSON.stringify({t3Document:'old-document',message:{jsonrpc:'2.0',method:'stale'}}),'*');
    });
    await page.waitForFunction(() => rogueDone);
    expect(await guest.evaluate(() => received)).toHaveLength(1);
    expect(await guest.evaluate(async () => { try { await fetch('/blocked'); return false; } catch { return true; } })).toBe(true);
    // The unchanged WindowProxy is revoked after the replacement load is observed.
    await guest.goto(server.url.href+'guest-next');
    await page.evaluate(() => send({jsonrpc:'2.0',method:'after-navigation'}));
    expect(await guest.evaluate(() => received)).toEqual([]);
  } finally { await browser?.close(); server.stop(true); }
}, 30_000);
