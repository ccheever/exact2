// Chrome's `position-area` box (LLP 1021 D2), the table the Apple hosts'
// painted popovers are held to (PositionAreaRegionIOSTests, …MacTests): in a
// 400×600 viewport, a 40×20 anchor and a popover of twelve 50×18 boxes in a
// wrapping row, so no font decides a width. Each option's box is sized in its
// region (the inset-modified containing block: the anchor's and the
// viewport's grid cell, reaching the anchor past the viewport), fit-content
// with min-content winning, and an option fits by its own region, so a box
// near the edge narrows where one sized in the viewport would flip.
import { test, expect } from 'bun:test';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

// [anchor x, anchor y, position-area, position-try-fallbacks, x, y, width, height, boxes (12 if unsaid)]
export const TABLE = [
  [0, 100, 'bottom span-right', 'none', 0, 120, 400, 36],
  [150, 100, 'bottom span-right', 'none', 150, 120, 250, 54],
  [150, 100, 'bottom span-right', 'flip-inline', 150, 120, 250, 54],
  [300, 100, 'bottom span-right', 'none', 300, 120, 100, 108],
  [300, 100, 'bottom span-right', 'flip-inline', 300, 120, 100, 108],
  [380, 100, 'bottom span-right', 'none', 370, 120, 50, 216],
  [380, 100, 'bottom span-right', 'flip-inline', 0, 120, 420, 36],
  [150, 100, 'bottom', 'none', 0, 120, 400, 36],
  [150, 100, 'center', 'none', 145, 2, 50, 216],
  [10, 100, 'center', 'none', 5, 2, 50, 216],
  [150, 550, 'bottom span-right', 'flip-block', 150, 496, 250, 54],
  [150, 100, 'right', 'none', 190, 83, 210, 54],
  [150, 100, 'left', 'none', 0, 74, 150, 72],
  [150, 100, 'top span-left', 'none', 0, 28, 190, 72],
  [300, 100, 'bottom span-left', 'none', 0, 120, 340, 36],
  [20, 100, 'bottom span-left', 'flip-inline', 0, 120, 60, 216],
  // No option fits: shifted inside the window, as CSS Box Alignment's overflow alignment does.
  [150, 550, 'bottom span-right', 'none', 150, 546, 250, 54],
  // Six boxes: laid out once in the viewport (300 wide) the base would overflow its region and the
  // flip fit, and stick; sized in its 100-wide region the base fits, and Chrome keeps it.
  [300, 100, 'bottom span-right', 'flip-inline', 300, 120, 100, 54, 6],
  [300, 560, 'bottom span-right', 'flip-block', 300, 506, 100, 54, 6],
  [60, 100, 'bottom span-left', 'flip-inline', 0, 120, 100, 54, 6],
];

/** A row in a right-to-left document: the anchor, the area's sides and the box mirrored (Chrome's, checked). */
export const mirrored = ([ax, ay, area, fallbacks, x, y, w, h, boxes]) =>
  [400 - ax - 40, ay, area.replace(/left|right/g, side => (side === 'left' ? 'right' : 'left')), fallbacks, 400 - x - w, y, w, h, boxes];

for (const rtl of [false, true]) test(`Chrome sizes a position-area box in its option's region, as the table says${rtl ? ', mirrored right to left' : ''}`, async () => {
  const browser = await suiteBrowser('chromium', { timeout: 10000 });
  try {
    const page = await browser.newPage({ viewport: { width: 400, height: 600 } });
    await page.setContent(`<!doctype html><html dir=${rtl ? 'rtl' : 'ltr'}><style>
      body{margin:0} .a{position:absolute;width:40px;height:20px;anchor-name:--a}
      .p{position:fixed;position-anchor:--a;margin:0;padding:0;border:0;inset:auto;display:flex;flex-wrap:wrap}
      .w{width:50px;height:18px;flex:none}
    </style><div class=a id=a></div><div class=p id=p popover></div>`);
    for (const [ax, ay, area, fallbacks, x, y, w, h, boxes = 12] of rtl ? TABLE.map(mirrored) : TABLE) {
      const box = [x, y, w, h];
      const got = await page.evaluate(([ax, ay, area, fallbacks, boxes]) => {
        const a = document.getElementById('a'), p = document.getElementById('p');
        p.innerHTML = '<div class=w></div>'.repeat(boxes);
        a.style.left = `${ax}px`; a.style.top = `${ay}px`;
        p.style.positionArea = area; p.style.positionTryFallbacks = fallbacks;
        p.hidePopover(); p.showPopover();
        const r = p.getBoundingClientRect();
        return [r.x, r.y, r.width, r.height].map(Math.round);
      }, [ax, ay, area, fallbacks, boxes]);
      expect([ax, ay, area, fallbacks, ...got]).toEqual([ax, ay, area, fallbacks, ...box]);
    }
  } finally {
    await browser.close();
  }
}, 60000);
