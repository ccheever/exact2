// A popover opened at the press (LLP 1021 D2): its author reads the click's `clientX`/`clientY` and writes them as
// its `left`/`top`, as a page without Electron's `Menu.popup({x, y})` does. In the neutral look its invoker no longer
// places it (neutral.css's `position-area` gives way), and where the point leaves too little room it opens the
// other way from it, as a context menu at the pointer does (navigation.js `insetAt`), from where the insets are as
// they change. One with no inset still opens at its invoker, and one with an authored `position-area` is placed by it. Chrome is the oracle; the page's stylesheet is a default build's (look.mjs).
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';
import { withLook } from '../look.mjs';

const read = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const page = `<style>${withLook(read('../index.html')).match(/<style>([\s\S]*?)<\/style>/)[1]}</style>
<div id="exact-root"><button id="open" style="margin:40px">Members</button>
  <div popover="auto" role="menu" id="menu" style="margin:0"><button type="button">Ada</button><button type="button">Grace</button></div></div>
<script type="module">${read('../navigation.js')}
  const open = document.getElementById('open'), menu = document.getElementById('menu');
  // A keyboard's click is at 0, 0: the menu opens at its invoker, as the oracle's does.
  open.addEventListener('click', (event) => {
    const at = event.clientX || event.clientY;
    menu.style.left = at ? event.clientX + 'px' : '';
    menu.style.top = at ? event.clientY + 'px' : '';
    menu.showPopover({ source: open });
  });
  window.ready = true;
</script>`;

test('chromium: a popover whose author wrote left and top opens at that point, and the other way near an edge', async () => {
  const browser = await suiteBrowser('chromium');
  try {
    const tab = await browser.newPage({ viewport: { width: 600, height: 400 } });
    await tab.setContent(page);
    await tab.waitForFunction('window.ready === true');
    const box = async () => {
      await tab.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
      const r = await tab.evaluate(() => { const b = document.getElementById('menu').getBoundingClientRect(); return [b.left, b.top, b.right, b.bottom]; });
      await tab.keyboard.press('Escape');
      await tab.waitForFunction(() => !document.getElementById('menu').matches(':popover-open'));
      return r;
    };
    const invoker = await tab.locator('#open').boundingBox();
    await tab.mouse.click(invoker.x + 30, invoker.y + 8);
    const [left, top] = await box();
    expect([left, top]).toEqual([invoker.x + 30, invoker.y + 8]);
    // Closed (its toggle event a task later), the point it was placed by is gone; the author's insets stay.
    await tab.waitForFunction(() => document.getElementById('menu').style.getPropertyValue('--exact-inset-x') === '');
    expect(await tab.evaluate(() => document.getElementById('menu').style.left)).toBe(`${invoker.x + 30}px`);
    // Pressed from the keyboard: at the invoker, under it from its start, as one with no inset is.
    await tab.evaluate(() => { const m = document.getElementById('menu'); m.style.left = m.style.top = ''; });
    await tab.focus('#open');
    await tab.keyboard.press('Enter');
    const [kx, ky] = await box();
    expect([kx, ky]).toEqual([invoker.x, invoker.y + invoker.height]);
    // Near the corner the menu opens up and to the start of the point, inside the viewport.
    await tab.evaluate(() => { const o = document.getElementById('open'); o.style.position = 'fixed'; o.style.left = '540px'; o.style.top = '370px'; o.style.margin = '0'; });
    await tab.mouse.click(580, 380);
    const [, , right, bottom] = await box();
    expect([right, bottom]).toEqual([580, 380]);
    // Insets the author moves while it is open move the point it flips from too.
    await tab.mouse.click(560, 375);
    await tab.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
    await tab.evaluate(() => { const m = document.getElementById('menu'); m.style.left = '590px'; m.style.top = '395px'; });
    await tab.waitForFunction(() => document.getElementById('menu').style.getPropertyValue('--exact-inset-x') === '590px');
    const [, , movedRight, movedBottom] = await box();
    expect([movedRight, movedBottom]).toEqual([590, 395]);
    // Insets written away while it is open: the point goes, and its invoker places it again (the sheet's area).
    const opened = async () => { await tab.mouse.click(560, 375); await tab.waitForFunction(() => document.getElementById('menu').style.getPropertyValue('--exact-inset-x') === '560px'); };
    const area = () => tab.evaluate(() => getComputedStyle(document.getElementById('menu')).positionArea);
    await opened();
    await tab.evaluate(() => { const m = document.getElementById('menu'); m.style.left = m.style.top = ''; });
    await tab.waitForFunction(() => document.getElementById('menu').style.getPropertyValue('--exact-inset-x') === '');
    expect(await area()).not.toBe('none');
    await box();
    // An area its author adds while it is open overrules the point too.
    await opened();
    await tab.evaluate(() => document.getElementById('menu').style.setProperty('position-area', 'top'));
    await tab.waitForFunction(() => document.getElementById('menu').style.getPropertyValue('--exact-inset-x') === '');
    expect(await area()).not.toBe('none'); // the area, or the flip of it the browser chose
    await box();
    await tab.evaluate(() => document.getElementById('menu').style.removeProperty('position-area'));
    // With an authored position-area, CSS's: the area places it, and no point is left to flip from.
    await tab.evaluate(() => { const m = document.getElementById('menu'); m.style.setProperty('position-area', 'top'); });
    await tab.mouse.click(560, 375);
    await tab.waitForFunction(() => document.getElementById('menu').matches(':popover-open'));
    expect(await tab.evaluate(() => document.getElementById('menu').style.getPropertyValue('--exact-inset-x'))).toBe('');
  } finally { await browser.close(); }
}, 30000);
