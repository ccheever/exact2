import { test, expect } from 'bun:test';
import { chromium } from 'playwright-core';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

// Oracle for InlineFrameMacTests and InlineFrameIOSTests. Keep these metrics
// tied to the browser's inline elements, including its empty inline box.
test('inline client rectangles exclude hanging spaces and use the addressed font', async () => {
  const browser = await chromium.launch({ executablePath: browserPath().executable, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent(`<div style="font:16px/20px Arial;width:260px">Hi<span id="empty"></span> there</div>
      <div style="font:16px/60px Arial;width:260px"><span id="small" style="font-size:12px">link</span><span style="font-size:48px">BIG</span></div>
      <div style="font:16px/20px Arial;width:40px">Hi<span id="wrap"> there</span></div>`);
    const got = await page.evaluate(() => Object.fromEntries(['empty', 'small', 'wrap'].map(id => {
      const el = document.getElementById(id), parent = el.parentElement.getBoundingClientRect();
      const rect = r => [r.x - parent.x, r.y - parent.y, r.width, r.height];
      return [id, { frame: rect(el.getBoundingClientRect()), fragments: [...el.getClientRects()].map(rect) }];
    })));
    for (const [id, rect] of Object.entries({ empty: [15.109375, 1, 0, 17], small: [0, 35, 18.015625, 14], wrap: [0, 21, 36.46875, 17] })) {
      expect(got[id].frame).toEqual(rect);
      expect(got[id].fragments).toEqual([rect]);
    }
  } finally { await browser.close(); }
}, 60000);
