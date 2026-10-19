import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../scripts/agent-launch.mjs';

// The real browser performs origin checks and Window ownership; no DOM doubles.
test('iframe replies keep order and ownership, with no queue across replacement or navigation', async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/frame-messages.js') return new Response(Bun.file(new URL('./frame-messages.js', import.meta.url)));
    if (path === '/commands.js') return new Response(Bun.file(new URL('../web-js/commands.js', import.meta.url)));
    if (path === '/navigation.js') return new Response(Bun.file(new URL('./navigation.js', import.meta.url)));
    if (path === '/prompt-glue.js') return new Response(Bun.file(new URL('./prompt-glue.js', import.meta.url)));
    if (path.startsWith('/guest')) return new Response(`<body>Guest<script>window.received=[];addEventListener('message',e=>{received.push(e.data);document.body.textContent=JSON.stringify(received);parent.postMessage({seen:e.data},'*')})</script>`, { headers: { 'content-type': 'text/html' } });
    return new Response(`<main id="exact-root"></main><script type="module">
      import * as frames from '/frame-messages.js';
      import {commands} from '/commands.js';
      window.framesApi=frames; window.messages=[];window.logs=[];
      addEventListener('message',e=>messages.push(e.data));
      window.post=(text,id)=>commands(t=>logs.push(t)).postMessage(text,id,'iframe');
      window.mount=(id,sandbox=null)=>{const frame=document.createElement('iframe');frame.id=id;if(sandbox!==null)frame.setAttribute('sandbox',sandbox);frame.src='/guest-'+id;frames.commitFrameOrigin(frame);document.querySelector('main').append(frame);return frame};
      mount('known');mount('opaque','allow-scripts');mount('sibling');
    </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelectorAll('iframe').length === 3);
    await page.waitForFunction(() => [...document.querySelectorAll('iframe')].filter(f=>f.contentDocument?.readyState==='complete').length===2);
    await page.evaluate(() => { post('one','known');post('two','known');post('sandbox','opaque'); });
    await page.waitForFunction(() => messages.length===3);
    expect(await page.evaluate(() => document.querySelector('#known').contentWindow.received)).toEqual(['one','two']);
    expect(await page.evaluate(() => document.querySelector('#sibling').contentWindow.received)).toEqual([]);
    expect(await page.evaluate(() => messages)).toContainEqual({seen:'sandbox'});
    await page.evaluate(() => { post('queued-before-replacement','known');document.querySelector('#known').remove();post('absent','known');mount('known'); });
    await page.waitForFunction(() => document.querySelector('#known').contentDocument?.readyState==='complete');
    expect(await page.evaluate(() => document.querySelector('#known').contentWindow.received)).toEqual([]);
    await page.evaluate(() => { post('new','known'); });
    await page.waitForFunction(() => messages.some(m=>m.seen==='new'));
    await page.evaluate(() => { const f=document.querySelector('#known');f.contentWindow.location.href='/guest-navigation'; });
    await page.waitForFunction(() => document.querySelector('#known').contentWindow.location.pathname==='/guest-navigation' && document.querySelector('#known').contentDocument?.readyState==='complete');
    await page.evaluate(() => post('after-navigation','known'));
    expect(await page.evaluate(() => document.querySelector('#known').contentWindow.received)).toEqual([]);
    await page.evaluate(() => { const f=document.querySelector('#known');post('queued-before-source-change','known');f.src='/guest-reset';framesApi.commitFrameOrigin(f); });
    await page.waitForFunction(() => document.querySelector('#known').contentWindow.location.pathname==='/guest-reset' && document.querySelector('#known').contentDocument?.readyState==='complete');
    expect(await page.evaluate(() => document.querySelector('#known').contentWindow.received)).toEqual([]);
    await page.evaluate(() => post('reset','known'));
    await page.waitForFunction(() => messages.some(m=>m.seen==='reset'));
    await page.evaluate(() => mount('known'));
    await page.waitForFunction(() => document.querySelectorAll('#known').length===2);
    await page.evaluate(() => post('ambiguous','known'));
    expect(await page.evaluate(() => logs.some(t=>t.includes('no unique live iframe')))).toBe(true);
    expect(await page.evaluate(() => globalThis.exact?.pendingPosts ?? [])).toEqual([]);
  } finally { await browser?.close(); server.stop(true); }
}, 30_000);
