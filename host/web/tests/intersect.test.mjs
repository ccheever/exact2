// LLP 1005: the adapter must preserve Chrome's IntersectionObserver entries.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { chromium } from 'playwright-core';
import { chromium as installedChrome } from '../../../scripts/agent-launch.mjs';

test('intersection matches Chrome for margins, scroll, clipping, zero area and removal', async () => {
  const source = readFileSync(new URL('../intersect-glue.js', import.meta.url), 'utf8');
  const browser = await chromium.launch({ executablePath: installedChrome().executable, headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 600, height: 500 } });
    await page.setContent(`<style>body{margin:0} #port{width:200px;height:100px;overflow:auto}
      #clip{height:260px;overflow:clip} .gap{height:150px} .row{width:100px;height:50px}</style>
      <div id="port"><div id="clip"><div class="gap"></div><div id="a" class="row"></div>
      <div id="b" class="row"></div><div id="zero" style="width:0;height:0"></div></div>
      <div style="height:300px"></div></div><div id="viewport" class="row"></div>`);
    await page.evaluate(async source => {
      const { observeIntersect } = await import(URL.createObjectURL(new Blob([source], { type: 'text/javascript' })));
      globalThis.observeIntersect = observeIntersect;
      globalThis.actual = []; globalThis.oracle = []; globalThis.stops = [];
      const entry = e => [e.target.id, e.isIntersecting, e.intersectionRatio];
      const root = document.querySelector('#port');
      for (const [ids, margin, port] of [[['a', 'b', 'zero'], '10% 0px 20px', root], [['viewport'], '-5px', null]]) {
        const observer = new IntersectionObserver(es => oracle.push(...es.map(entry)), { root: port, rootMargin: margin, threshold: 0 });
        for (const id of ids) {
          const el = document.getElementById(id);
          el.setAttribute('data-intersectmargin', margin);
          stops.push(observeIntersect(el, e => actual.push([id, e.isIntersecting, e.intersectionRatio])));
          observer.observe(el);
        }
      }
    }, source);
    const settled = async () => {
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      await page.waitForFunction(() => actual.length === oracle.length && actual.length >= 4);
      const result = await page.evaluate(() => ({ actual, oracle }));
      // The platform does not order callbacks across different observers.
      // Entry order within each root/margin group must remain exact.
      for (const viewport of [false, true]) {
        expect(result.actual.filter(e => (e[0] === 'viewport') === viewport))
          .toEqual(result.oracle.filter(e => (e[0] === 'viewport') === viewport));
      }
      return result.actual;
    };
    expect((await settled()).find(e => e[0] === 'a')).toEqual(['a', false, 0]);
    const percentageRoot = await page.evaluate(() => new Promise(resolve => {
      const observer = new IntersectionObserver(([e]) => {
        observer.disconnect();
        resolve([e.rootBounds.left, e.rootBounds.top, e.rootBounds.right, e.rootBounds.bottom]);
      }, { root: document.getElementById('port'), rootMargin: '10%' });
      observer.observe(document.getElementById('a'));
    }));
    // The native sampler follows this browser result, including its per-axis
    // percentage basis (different from the W3C draft's all-width wording).
    expect(percentageRoot).toEqual([-20, -10, 220, 110]);
    await page.evaluate(() => document.getElementById('port').scrollTop = 60);
    await page.waitForFunction(() => actual.some(e => e[0] === 'a' && e[1]));
    await settled();
    await page.evaluate(() => document.getElementById('port').scrollTop = 180);
    await page.waitForFunction(() => actual.some(e => e[0] === 'zero' && e[1]));
    const before = await settled();
    // Moving inside threshold zero changes the ratio without delivering it.
    await page.evaluate(() => document.getElementById('port').scrollTop = 185);
    expect(await settled()).toEqual(before);
    await page.evaluate(() => document.getElementById('port').scrollTop = 350);
    await page.waitForFunction(() => actual.filter(e => e[0] === 'a').length >= 3);
    await settled();
    await page.evaluate(() => { stops.forEach(stop => stop()); document.getElementById('a').remove(); });
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const count = await page.evaluate(() => actual.length);
    await page.evaluate(() => document.getElementById('port').scrollTop = 0);
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    expect(await page.evaluate(() => actual.length)).toBe(count);
    // A changed scroll owner starts a fresh observation; detaching cancels
    // without a leave, and a later mount can observe the same element anew.
    await page.evaluate(() => {
      const outer = document.createElement('div');
      outer.style.cssText = 'width:200px;height:100px;overflow:auto';
      const row = document.createElement('div'); row.style.cssText = 'width:50px;height:50px';
      outer.append(row); document.body.append(outer);
      globalThis.moved = []; globalThis.moving = row; globalThis.outer = outer;
      observeIntersect(row, e => moved.push([e.isIntersecting, e.intersectionRatio]));
    });
    await page.waitForFunction(() => moved.length === 1);
    expect(await page.evaluate(() => moved)).toEqual([[true, 1]]);
    await page.evaluate(() => {
      const inner = document.createElement('div'); inner.style.cssText = 'height:20px;overflow:hidden';
      outer.append(inner); inner.append(moving);
    });
    await page.waitForFunction(() => moved.length === 2);
    expect(await page.evaluate(() => moved[1][0])).toBe(true);
    expect(await page.evaluate(() => moved[1][1])).toBeCloseTo(0.4, 6);
    await page.evaluate(() => moving.remove());
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    expect(await page.evaluate(() => moved.length)).toBe(2);
    await page.evaluate(() => {
      outer.append(moving);
      observeIntersect(moving, e => moved.push([e.isIntersecting, e.intersectionRatio]));
    });
    await page.waitForFunction(() => moved.length === 3);
    expect(await page.evaluate(() => moved[2])).toEqual([true, 1]);
  } finally { await browser.close(); }
}, 120000);


test('intersection initial entries follow scroll, resize and registration order', async () => {
  const source = readFileSync(new URL('../intersect-glue.js', import.meta.url), 'utf8');
  const browser = await chromium.launch({ executablePath: installedChrome().executable, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent(`<style>body{margin:0}.port{width:200px;height:100px;overflow:hidden}.row{width:100px;height:50px}</style>
      <div class="port" id="scroll"><div style="height:150px"></div><div class="row" id="scrolled"></div><div style="height:300px"></div></div>
      <div class="port"><div id="gap" style="height:80px"></div><div class="row" id="resized"></div></div>
      <div class="port"><div style="height:200px"></div><div class="row" id="margin" data-intersectmargin="200PX"></div></div>
      <div class="port" id="ordered"><div class="row" id="first"></div><div class="row" id="second"></div><div class="row" id="third"></div></div>`);
    await page.evaluate(async source => {
      const { observeIntersect } = await import(URL.createObjectURL(new Blob([source], { type: 'text/javascript' })));
      globalThis.entries = [];
      const resize = new ResizeObserver(() => { document.getElementById('gap').style.height = '0px'; });
      resize.observe(document.getElementById('gap'));
      for (const id of ['scrolled', 'resized', 'margin', 'first', 'second', 'third']) {
        observeIntersect(document.getElementById(id), e => entries.push([id, e.isIntersecting, e.intersectionRatio]));
      }
      document.getElementById('scroll').scrollTop = 150;
      document.getElementById('ordered').prepend(document.getElementById('third'));
    }, source);
    await page.waitForFunction(() => entries.length === 6);
    const entries = await page.evaluate(() => globalThis.entries);
    for (const id of ['scrolled', 'resized', 'margin']) expect(entries.find(e => e[0] === id)).toEqual([id, true, 1]);
    expect(entries.filter(e => ['first', 'second', 'third'].includes(e[0])).map(e => e[0])).toEqual(['first', 'second', 'third']);
  } finally { await browser.close(); }
}, 120000);

test('invalid margins cannot block later intersection targets', async () => {
  const source = readFileSync(new URL('../intersect-glue.js', import.meta.url), 'utf8');
  const browser = await chromium.launch({ executablePath: installedChrome().executable, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent('<div id="bad"></div><div id="good"></div><div id="later"></div>');
    const result = await page.evaluate(async source => {
      const { observeIntersect } = await import(URL.createObjectURL(new Blob([source], { type: 'text/javascript' })));
      globalThis.observeIntersect = observeIntersect;
      globalThis.entries = [];
      const invalid = ['1.px', '1.%', '.px', '1e', '1epx', '1e+px', '+-1px', '1.e2px'];
      const refused = invalid.map(rootMargin => {
        try { new IntersectionObserver(() => {}, { rootMargin }).disconnect(); return null; }
        catch (error) { return error.name; }
      });
      const accepted = ['+1px', '-.5%', '+.5PX', '1.0px', '1e2px', '1E-2%', '.5e+2px'].map(rootMargin => {
        try { new IntersectionObserver(() => {}, { rootMargin }).disconnect(); return true; }
        catch { return false; }
      });
      const errors = [];
      for (const id of ['bad', 'good']) {
        const el = document.getElementById(id);
        if (id === 'bad') el.setAttribute('data-intersectmargin', '1.px');
        try { observeIntersect(el, e => entries.push([id, e.isIntersecting])); }
        catch (error) { errors.push([id, error.name]); }
      }
      return { refused, accepted, errors };
    }, source);
    expect(result.refused).toEqual(Array(8).fill('SyntaxError'));
    expect(result.accepted).toEqual(Array(7).fill(true));
    expect(result.errors).toEqual([]);
    await page.waitForFunction(() => entries.some(e => e[0] === 'good'));
    // A previously valid target can also become invalid during mutation sync.
    await page.evaluate(() => {
      document.getElementById('good').setAttribute('data-intersectmargin', '1.px');
      observeIntersect(document.getElementById('later'), e => entries.push(['later', e.isIntersecting]));
    });
    await page.waitForFunction(() => entries.some(e => e[0] === 'later'));
    expect(await page.evaluate(() => entries)).toEqual([['good', true], ['later', true]]);
    // Retiring a failed registration must still allow an explicit retry.
    await page.evaluate(() => {
      const el = document.getElementById('bad');
      el.setAttribute('data-intersectmargin', '0px');
      observeIntersect(el, e => entries.push(['bad', e.isIntersecting]));
    });
    await page.waitForFunction(() => entries.some(e => e[0] === 'bad'));
  } finally { await browser.close(); }
}, 120000);
