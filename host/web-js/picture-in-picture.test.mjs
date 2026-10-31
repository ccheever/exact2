import { test, expect } from 'bun:test';
import { suiteBrowser } from '../../scripts/agent-launch.mjs';

// `requestPictureInPicture(id)` on the JS target (LLP 1042 §9): the image's
// picture floats as a canvas stream's video; a picture it loads next is drawn
// into the same stream; the window's close is the image's
// `leavepictureinpicture`, and a removed image closes it with no event.
test("an image floats in picture in picture and hears it enter and leave", async () => {
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/picture-in-picture.js') return new Response(Bun.file(new URL('./picture-in-picture.js', import.meta.url)));
    return new Response(`<button id="float">Float</button><img id="page"><output id="log"></output>
      <script type="module">
        import { float, exit } from '/picture-in-picture.js';
        window.exitPip = () => exit(line => { log.textContent += line + ';'; });
        const picture = color => { const c = Object.assign(document.createElement('canvas'), { width: 320, height: 200 }); const g = c.getContext('2d'); g.fillStyle = color; g.fillRect(0, 0, 320, 200); return c.toDataURL(); };
        const img = document.querySelector('#page'), log = document.querySelector('#log');
        img.src = picture('red');
        for (const kind of ['enterpictureinpicture', 'leavepictureinpicture']) img.addEventListener(kind, () => { log.textContent += kind + ';'; });
        window.next = () => { img.src = picture('blue'); };
        document.querySelector('#float').onclick = () => float(img, 'page', line => { log.textContent += line + ';'; });
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelector('#page').naturalWidth === 320);
    await page.locator('#float').click();
    await page.waitForFunction(() => document.querySelector('#log').textContent !== '');
    const log = await page.locator('#log').textContent();
    if (log.includes('refused')) {
      // A browser without picture in picture says so, and nothing is left floating.
      expect(log).toMatch(/^requestPictureInPicture "page" refused: /);
      expect(await page.evaluate(() => document.querySelectorAll('video').length)).toBe(0);
      return;
    }
    expect(log).toBe('enterpictureinpicture;');
    const floating = await page.evaluate(() => {
      const video = document.pictureInPictureElement;
      return { video: video?.localName, track: video?.srcObject?.getVideoTracks()[0]?.readyState, size: [video?.videoWidth, video?.videoHeight] };
    });
    expect(floating).toEqual({ video: 'video', track: 'live', size: [320, 200] });
    // The window's own close (its button) is a leave.
    await page.evaluate(() => document.exitPictureInPicture());
    await page.waitForFunction(() => document.querySelector('#log').textContent.endsWith('leavepictureinpicture;'));
    expect(await page.evaluate(() => document.querySelectorAll('video').length)).toBe(0);
    // exitPictureInPicture() closes it as the window's close does; with none open it says so.
    await page.locator('#float').click();
    await page.waitForFunction(() => document.pictureInPictureElement !== null);
    await page.evaluate(() => window.exitPip());
    await page.waitForFunction(() => document.pictureInPictureElement === null);
    await page.evaluate(() => window.exitPip());
    expect(await page.locator('#log').textContent()).toBe('enterpictureinpicture;leavepictureinpicture;enterpictureinpicture;leavepictureinpicture;exitPictureInPicture ignored: no image is in picture in picture;');
    await page.evaluate(() => { document.querySelector('#log').textContent = ''; });
    // Floated again, then the image leaves the document: the window closes, and no one hears it.
    await page.locator('#float').click();
    await page.waitForFunction(() => document.pictureInPictureElement !== null);
    await page.evaluate(() => { window.next(); document.querySelector('#page').remove(); });
    await page.waitForFunction(() => document.pictureInPictureElement === null, null, { timeout: 5000 });
    expect(await page.locator('#log').textContent()).toBe('enterpictureinpicture;');
  } finally {
    await browser?.close();
    server.stop(true);
  }
}, 30000);

// A picture from another origin without CORS would taint the canvas, whose
// stream then sends no frames: it is refused, and nothing is left floating.
test("a picture from another origin without CORS is refused", async () => {
  const other = Bun.serve({ port: 0, fetch: () => new Response(Bun.file(new URL('../../apps/canvas-gallery/assets/tile.png', import.meta.url))) });
  const server = Bun.serve({ port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/picture-in-picture.js') return new Response(Bun.file(new URL('./picture-in-picture.js', import.meta.url)));
    return new Response(`<button id="float">Float</button><img id="page" src="${other.url.href}icon.png"><output id="log"></output>
      <script type="module">
        import { float } from '/picture-in-picture.js';
        const img = document.querySelector('#page'), log = document.querySelector('#log');
        img.addEventListener('enterpictureinpicture', () => { log.textContent += 'enter;'; });
        img.addEventListener('pictureinpictureerror', e => { log.textContent += 'error event: ' + e.detail + ';'; });
        document.querySelector('#float').onclick = () => float(img, 'page', line => { log.textContent += line + ';'; });
      </script>`, { headers: { 'content-type': 'text/html' } });
  } });
  let browser;
  try {
    browser = await suiteBrowser();
    const page = await browser.newPage();
    await page.goto(server.url.href);
    await page.waitForFunction(() => document.querySelector('#page').naturalWidth > 0);
    await page.locator('#float').click();
    await page.waitForFunction(() => document.querySelector('#log').textContent !== '');
    const reason = 'a picture from another origin needs CORS (Access-Control-Allow-Origin)';
    expect(await page.locator('#log').textContent()).toBe(`requestPictureInPicture "page" refused: ${reason};error event: ${reason};`);
    expect(await page.evaluate(() => [document.pictureInPictureElement, document.querySelectorAll('video').length])).toEqual([null, 0]);
  } finally {
    await browser?.close();
    server.stop(true); other.stop(true);
  }
}, 30000);

test("an element that is not an image is refused", async () => {
  const { float } = await import('./picture-in-picture.js');
  const said = [];
  float(null, 'missing', line => said.push(line));
  float({ localName: 'div' }, 'box', line => said.push(line));
  expect(said).toEqual(['requestPictureInPicture "missing" refused: no live node with that id', 'requestPictureInPicture "box" refused: not an image']);
});
