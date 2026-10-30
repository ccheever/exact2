import { test, expect } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';
import { webCompiler } from './module.mjs';

test('bound springs animate paint and type on ordinary and pressed rows', async () => {
  if (webCompiler().cmd === 'cargo') throw new Error('build the compiler first: cargo build -p exact-web-js');
  const root = resolve(import.meta.dir, '../..');
  mkdirSync(resolve(root, 'target'), { recursive: true });
  const dir = mkdtempSync(resolve(root, 'target/spring-transition-'));
  let server, browser;
  try {
    writeFileSync(resolve(dir, 'app.json'), JSON.stringify({ name: 'Spring transition', app: { id: 'test.spring-transition', name: 'Spring transition' } }));
    writeFileSync(resolve(dir, 'app.contract'), `component App
  state changed = false
  state curve = "all -exact-spring() 0s 0.1s, font-weight 2s linear 0s"
  state typeCurve = "font-weight 2s linear, font-weight -exact-spring(100, 20, 1) 0s 125ms"
  action change
    changed = ! changed
  view
    column
      button "Change" press=change testId="change"
      text "Ordinary" testId="ordinary" color=(changed ? "#ffffff" : "#000000") font-weight=(changed ? 800 : 200) transition=curve
      text "Pressed" testId="pressed" press=change -exact-press-scale=0.9 color=(changed ? "#ffffff" : "#000000") font-weight=(changed ? 800 : 200) transition=curve
      text "Spring type" testId="type" font-weight=(changed ? 800 : 200) transition=typeCurve
`);
    const out = resolve(dir, 'dist');
    const built = spawnSync(process.execPath, ['host/web-js/build.mjs', dir, '--out', out, '--render', 'none'], { cwd: root, encoding: 'utf8' });
    expect(built.status, built.stdout + built.stderr).toBe(0);
    server = Bun.serve({ port: 0, hostname: '127.0.0.1', fetch: request => new Response(Bun.file(resolve(out, new URL(request.url).pathname.slice(1) || 'index.html'))) });
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelector('[data-testid="ordinary"]'));
    const observed = await page.evaluate(() => {
      const elements = ['ordinary', 'pressed'].map(id => document.querySelector(`[data-testid="${id}"]`));
      elements.forEach(el => getComputedStyle(el).color);
      document.querySelector('[data-testid="change"]').click();
      const paint = elements.map(el => {
        const animations = el.getAnimations();
        const color = animations.find(a => a.transitionProperty === 'color');
        const weight = animations.find(a => a.transitionProperty === 'font-weight');
        const result = { properties: animations.map(a => a.transitionProperty).sort(), running: color?.playState, easing: color?.effect.getTiming().easing, delay: color?.effect.getTiming().delay, weightDuration: weight?.effect.getTiming().duration };
        for (const a of animations) { a.pause(); a.currentTime = a.transitionProperty === 'color' ? 200 : 1000; }
        result.color = getComputedStyle(el).color;
        result.weight = getComputedStyle(el).fontWeight;
        animations.forEach(a => a.finish());
        result.finalColor = getComputedStyle(el).color;
        return result;
      });
      const type = document.querySelector('[data-testid="type"]');
      const animation = type.getAnimations().find(a => a.transitionProperty === 'font-weight');
      const typography = { running: animation?.playState, easing: animation?.effect.getTiming().easing, delay: animation?.effect.getTiming().delay };
      if (animation) { animation.pause(); animation.currentTime = 300; }
      typography.weight = Number(getComputedStyle(type).fontWeight);
      animation?.finish();
      typography.finalWeight = Number(getComputedStyle(type).fontWeight);
      return { paint, typography };
    });
    for (const value of observed.paint) {
      expect(value.properties).toEqual(['color', 'font-weight']);
      expect(value.running).toBe('running');
      expect(value.easing).toStartWith('linear(');
      expect(value.delay).toBeCloseTo(100);
      expect(value.color).not.toBe('rgb(0, 0, 0)');
      expect(value.color).not.toBe('rgb(255, 255, 255)');
      expect(value.finalColor).toBe('rgb(255, 255, 255)');
      expect(value.weightDuration).toBe(2000);
      expect(Number(value.weight)).toBeCloseTo(500);
    }
    expect(observed.typography.running).toBe('running');
    expect(observed.typography.easing).toStartWith('linear(');
    expect(observed.typography.delay).toBeCloseTo(125);
    expect(observed.typography.weight).toBeGreaterThan(200);
    expect(observed.typography.weight).toBeLessThan(800);
    expect(observed.typography.finalWeight).toBe(800);
  } finally {
    await browser?.close();
    server?.stop(true);
    rmSync(dir, { recursive: true, force: true });
  }
}, 120000);
