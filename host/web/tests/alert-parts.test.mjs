// A grouped alert on the web (`alertPart`, contract/lower/src/alert_parts.rs) in Chrome, Firefox and WebKit: a long
// message scrolls in its own box, the words clear of a gutter at its inline end for the scroll indicator, while the
// title and buttons stay in view and the dialog does not scroll; a short one lays out, and its texts take their
// colors, as the same alert written flat, its texts beside that gutter; and the message box (alert-message.js)
// starts at its top each time the alert opens and is a Tab stop while it overflows, on a page rendered ahead too.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { withLook } from '../look.mjs';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

const web = new URL('../', import.meta.url);
const sheet = withLook(readFileSync(new URL('index.html', web), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
const long = Array.from({ length: 120 }, (_, i) => `Sentence ${i} says why the save failed.`).join(' ');
const text = (id, words) => `<div data-exact-text id="${id}">${words}</div>`;
const button = (id, words, press = false) => `<button type="button" data-native data-button-style="bordered" id="${id}" style="display:grid"${press ? ' data-exact-on="press"' : ''}><span data-exact-text>${words}</span></button>`;
// The compiler's boxes: a `column`, a group named by the title, and a `row`
const message = (rows, title) => `<div data-alertpart="message" role="group" aria-label="${title}" style="display:flex;flex-direction:column">${rows}</div>`;
const actions = rows => `<div data-alertpart="actions" style="display:flex;flex-direction:row">${rows}</div>`;
const inherits = id => text(id, 'The alert’s.').replace('<div', '<div style="color:inherit"');
const PAGE = `<!doctype html><html><head><style>${sheet}</style></head><body><div id="exact-root">
  <button id="open-failed" commandfor="failed" command="show-modal">Open</button>
  <dialog id="failed" role="alertdialog" aria-label="Couldn’t Save">${message(text('failed-message', long), 'Couldn’t Save')}${actions(button('failed-ok', 'OK'))}</dialog>
  <dialog id="remove" role="alertdialog">${text('remove-title', 'Remove Book?').replace('<div', '<div data-alertpart="title"')}${message(text('remove-message', long), 'Remove Book?')}${actions(button('remove-cancel', 'Cancel') + button('remove-ok', 'Remove', true))}</dialog>
  <dialog id="rename" role="alertdialog" aria-label="Rename">${message(text('rename-message', long), 'Rename')}<input data-native id="rename-field" aria-label="Name">${actions(button('rename-cancel', 'Cancel') + button('rename-ok', 'Rename', true))}</dialog>
  <dialog id="short" role="alertdialog" aria-label="Remove Book?">${message(text('short-message', 'It is removed from every shelf.'), 'Remove Book?')}${actions(button('short-cancel', 'Cancel') + button('short-ok', 'Remove', true))}</dialog>
  <dialog id="rtl" role="alertdialog" aria-label="Couldn’t Save" style="direction:rtl">${message(text('rtl-message', long), 'Couldn’t Save')}${actions(button('rtl-ok', 'OK'))}</dialog>
  <dialog id="flat" role="alertdialog" aria-label="Remove Book?">${text('flat-message', 'It is removed from every shelf.')}${button('flat-cancel', 'Cancel')}${button('flat-ok', 'Remove', true)}</dialog>
  <dialog id="linked" role="alertdialog" aria-label="Couldn’t Sync">${message('<div data-exact-text id="linked-message">See <a id="linked-link" href="https://status.example">the status page</a>.</div>', 'Couldn’t Sync')}${actions(button('linked-ok', 'OK'))}</dialog>
  <dialog id="tinted" role="alertdialog" aria-label="Note" style="color:red">${message(inherits('tinted-inherit') + text('tinted-plain', 'Its own.'), 'Note')}${actions(button('tinted-ok', 'OK'))}</dialog>
  <dialog id="tinted-flat" role="alertdialog" aria-label="Note" style="color:red">${inherits('tinted-flat-inherit')}${text('tinted-flat-plain', 'Its own.')}${button('tinted-flat-ok', 'OK')}</dialog>
</div><script type="module">
  import '/alert-message.js';
  // The alert \`id\` written flat after it (its rows out of the compiler's boxes, its ids ending \`-flat\`), less its \`part\` box
  window.flat = (id, part) => {
    const alert = document.getElementById(id), copy = alert.cloneNode(true);
    for (const el of [copy, ...copy.querySelectorAll('[id]')]) el.id += '-flat';
    if (part) copy.querySelector(\`[data-alertpart="\${part}"]\`).remove();
    copy.querySelector('[data-alertpart="title"]')?.removeAttribute('data-alertpart');
    for (const box of copy.querySelectorAll('[data-alertpart="message"], [data-alertpart="actions"]')) box.replaceWith(...box.childNodes);
    alert.after(copy);
    return copy;
  };
  window.ready = true;
</script></body></html>`;

// Each engine's page at each size, with the shell's sheet and the message's module
async function inEngines(run) {
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(request) {
    const path = new URL(request.url).pathname;
    if (path === '/alert-message.js') return new Response(Bun.file(new URL('alert-message.js', web)), { headers: { 'content-type': 'text/javascript' } });
    return new Response(PAGE, { headers: { 'content-type': 'text/html; charset=utf-8' } });
  } });
  const playwright = await import('playwright-core');
  try {
    for (const engine of ['chromium', 'firefox', 'webkit']) {
      const browser = await playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: browserPath().executable } : {}) });
      try {
        for (const [width, height] of [[390, 844], [844, 390]]) {
          const page = await browser.newPage({ viewport: { width, height } });
          await page.goto(server.url.href);
          await page.waitForFunction(() => window.ready === true);
          await run(page, `${engine} ${width}x${height}`, engine);
          await page.close();
        }
      } finally {
        await browser.close();
      }
    }
  } finally {
    server.stop(true);
  }
}

// Open `id` and wait for the frame the message's module keeps it in
const open = (page, id) => page.evaluate(id => new Promise(done => {
  document.getElementById(id).showModal();
  requestAnimationFrame(() => requestAnimationFrame(done));
}), id);
const close = (page, id) => page.evaluate(id => document.getElementById(id).close(), id);
const frames = () => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done)));
const stopOf = (page, id) => page.evaluate(id => document.querySelector(`#${id} [data-alertpart="message"]`).getAttribute('tabindex'), id);

test('a long message scrolls in its own box, and the title, field and buttons stay in view', async () => {
  await inEngines(async (page, where) => {
    for (const [id, kept] of [['failed', ['failed-ok']], ['remove', ['remove-title', 'remove-cancel', 'remove-ok']], ['rename', ['rename-field', 'rename-cancel', 'rename-ok']], ['rtl', ['rtl-ok']]]) {
      await open(page, id);
      const got = await page.evaluate(([id, kept]) => {
        const dialog = document.getElementById(id), m = dialog.querySelector('[data-alertpart="message"]');
        const d = dialog.getBoundingClientRect(), top = m.getBoundingClientRect().top;
        const inView = e => { const r = e.getBoundingClientRect(); return r.top >= d.top && r.bottom <= d.bottom && r.bottom <= innerHeight && r.left >= d.left && r.right <= d.right; };
        return {
          dialogScrolls: dialog.scrollTop > 0 || dialog.scrollHeight - dialog.clientHeight > 1 || dialog.scrollWidth - dialog.clientWidth > 1,
          messageScrolls: m.scrollHeight > m.clientHeight, messageAtTop: m.scrollTop === 0 && top >= d.top,
          // The `aria-label` title is the dialog's `::before`, above the message
          titleAbove: getComputedStyle(dialog, '::before').content === 'none' || top - d.top > 20,
          outOfView: kept.filter(k => !inView(document.getElementById(k))),
          stop: m.getAttribute('tabindex'),
          // The gutters between the words and the box's left and right edges (inside any classic scrollbar): an
          // overlay indicator draws at the inline end
          gutter: [m.firstElementChild.getBoundingClientRect().left - m.getBoundingClientRect().left - m.clientLeft, m.getBoundingClientRect().left + m.clientLeft + m.clientWidth - m.firstElementChild.getBoundingClientRect().right].map(Math.round),
        };
      }, [id, kept]);
      expect([where, id, got]).toEqual([where, id, { dialogScrolls: false, messageScrolls: true, messageAtTop: true, titleAbove: true, outOfView: [], stop: '0', gutter: id === 'rtl' ? [8, 0] : [0, 8] }]);
      await close(page, id);
    }
  });
}, 120000);

test('a short grouped alert lays out as the same alert written flat, its texts beside a gutter, and its message is no stop', async () => {
  await inEngines(async (page, where) => {
    const boxes = async id => {
      await open(page, id);
      const got = await page.evaluate(id => {
        const dialog = document.getElementById(id), box = e => { const r = e.getBoundingClientRect(); return [r.left, r.top, r.width, r.height].map(Math.round); };
        return { dialog: box(dialog), texts: [...dialog.querySelectorAll('[data-exact-text]:not(button *)')].map(box), buttons: [...dialog.querySelectorAll('button')].map(box), stop: dialog.querySelector('[data-alertpart="message"]')?.getAttribute('tabindex') ?? null };
      }, id);
      await close(page, id);
      return got;
    };
    // The same, but the message's texts are as wide as the box less its gutter for a scroll indicator
    const flat = await boxes('flat'), grouped = await boxes('short');
    flat.texts = flat.texts.map(([x, y, w, h]) => [x, y, w - 8, h]);
    expect([where, grouped]).toEqual([where, flat]);
  });
}, 120000);

// From outside the dialog, a grouped alert opens with its focus on a prompt's field, else on the button the browser
// chooses for the same alert written flat, as the native alerts open: the message is inert until the alert's first
// frame, so neither a link in it (which the flat alert would focus first) nor a long message's box (a scroller with
// nothing focusable in it, which Chrome would) is chosen.
test("a grouped alert opens with its focus on its field or the flat alert's button, passing over its message", async () => {
  await inEngines(async (page, where) => {
    const got = await page.evaluate(() => {
      const focus = id => {
        document.getElementById('open-failed').focus();
        const dialog = document.getElementById(id);
        dialog.showModal();
        const at = document.activeElement.id;
        dialog.close();
        return at;
      };
      return ['failed', 'remove', 'rename', 'linked'].map(id => [id, focus(id), focus(flat(id).id).replace(/-flat$/, ''), document.querySelector(`#${id} [data-alertpart="message"]`).inert]);
    });
    expect([where, got]).toEqual([where, [['failed', 'failed-ok', 'failed-ok', false], ['remove', 'remove-cancel', 'remove-cancel', false], ['rename', 'rename-field', 'rename-field', false], ['linked', 'linked-ok', 'linked-link', false]]]);
  });
}, 120000);

// What an `if` left false leaves in a part box (nothing, or the JS target's markers) takes no room, though the
// compiler writes the box's `display`: the alert lays out as the same alert written flat without that part, an empty
// message's titled by its label or by a text, and an empty actions box's ending at its message
test('a message or actions box its `if` leaves empty takes no room', async () => {
  await inEngines(async (page, where) => {
    const got = await page.evaluate(() => [['short', 'message'], ['remove', 'message'], ['short', 'actions']].map(([id, part]) => {
      const alert = document.getElementById(id), copy = flat(id, part);
      const emptied = alert.querySelector(`[data-alertpart="${part}"]`), was = [...emptied.childNodes];
      emptied.replaceChildren(document.createComment(''));
      const measure = dialog => {
        dialog.showModal();
        const top = dialog.getBoundingClientRect().top;
        const got = [dialog.getBoundingClientRect().height, ...[...dialog.querySelectorAll('button')].map(b => b.getBoundingClientRect().top - top)];
        dialog.close();
        return got;
      };
      const out = [id, part, getComputedStyle(emptied).display, measure(alert), measure(copy)];
      emptied.replaceChildren(...was);
      copy.remove();
      return out;
    }));
    for (const [id, part, display, grouped, flat] of got) expect([where, id, part, display, grouped]).toEqual([where, id, part, 'none', flat]);
  });
}, 120000);

test("a grouped message's texts take their colors as a flat alert's do", async () => {
  await inEngines(async (page, where) => {
    const colors = async id => {
      await open(page, id);
      const got = await page.evaluate(id => [...document.querySelectorAll(`#${id} [data-exact-text]:not(button *)`)].map(e => getComputedStyle(e).color), id);
      await close(page, id);
      return got;
    };
    const grouped = await colors('tinted');
    expect([where, grouped]).toEqual([where, await colors('tinted-flat')]);
    // The text that inherits takes the alert's color; the other the secondary label
    expect([where, grouped[0], grouped[1] === 'rgb(255, 0, 0)']).toEqual([where, 'rgb(255, 0, 0)', false]);
  });
}, 120000);

test('the message starts at its top each time the alert opens, by a command or an invoker', async () => {
  await inEngines(async (page, where) => {
    const scrolled = () => page.evaluate(() => document.querySelector('#failed [data-alertpart="message"]').scrollTop);
    const scroll = () => page.evaluate(() => { const m = document.querySelector('#failed [data-alertpart="message"]'); m.scrollTop = m.scrollHeight; });
    await open(page, 'failed');
    await scroll();
    await close(page, 'failed');
    await open(page, 'failed');
    expect([where, await scrolled()]).toEqual([where, 0]);
    await scroll();
    await close(page, 'failed');
    await page.click('#open-failed');
    await page.waitForFunction(() => document.getElementById('failed').open);
    await page.evaluate(frames);
    expect([where, await scrolled()]).toEqual([where, 0]);
    await close(page, 'failed');
  });
}, 120000);

test('Shift+Tab from the button reaches the message, which the keys then scroll', async () => {
  await inEngines(async (page, where) => {
    await open(page, 'failed');
    await page.focus('#failed-ok');
    await page.keyboard.press('Shift+Tab');
    expect([where, await page.evaluate(() => document.activeElement?.dataset.alertpart ?? document.activeElement?.id)]).toEqual([where, 'message']);
    // Page Down: WebKit's arrows scroll no focused box, on any page
    await page.keyboard.press('PageDown');
    await page.waitForFunction(() => document.querySelector('#failed [data-alertpart="message"]').scrollTop > 0);
    await close(page, 'failed');
  });
}, 120000);

// The browser picks the focus as it would without the module (which in Firefox can differ between openings)
test('a stop left from the last opening is gone when the dialog chooses its focus', async () => {
  await inEngines(async (page, where) => {
    await page.evaluate(long => { document.getElementById('short-message').textContent = long; }, long);
    await open(page, 'short');
    expect([where, await stopOf(page, 'short')]).toEqual([where, '0']);
    await close(page, 'short');
    await page.focus('#open-failed'); // outside, so the opening moves it in every engine
    await page.evaluate(() => document.addEventListener('focusin', () => {
      window.stopAtFocus = document.querySelector('#short [data-alertpart="message"]').getAttribute('tabindex');
    }, { capture: true, once: true }));
    await open(page, 'short');
    expect([where, await page.evaluate(() => window.stopAtFocus)]).toEqual([where, null]);
    await close(page, 'short');
  });
}, 120000);

// What can make a message start or stop overflowing while the alert is open, with no box changing size for some
test("the message's stop follows its text, its look, an ancestor's look and an ancestor's mode", async () => {
  await inEngines(async (page, where) => {
    await page.evaluate(() => document.head.insertAdjacentHTML('beforeend', '<style>[data-x-modes="space=wide"] #short-message { letter-spacing: 2em }</style>'));
    const causes = {
      text: [long => { document.getElementById('short-message').textContent = long; }, () => { document.getElementById('short-message').textContent = 'It is removed from every shelf.'; }],
      look: [() => { document.getElementById('short-message').style.letterSpacing = '2em'; }, () => { document.getElementById('short-message').style.letterSpacing = ''; }],
      'ancestor look': [() => { document.getElementById('exact-root').style.letterSpacing = '2em'; }, () => { document.getElementById('exact-root').style.letterSpacing = ''; }],
      'ancestor mode': [() => document.getElementById('exact-root').setAttribute('data-x-modes', 'space=wide'), () => document.getElementById('exact-root').setAttribute('data-x-modes', 'space=narrow')],
    };
    for (const [cause, [on, off]] of Object.entries(causes)) {
      await page.evaluate(() => { document.getElementById('short-message').style.whiteSpace = 'nowrap'; });
      await open(page, 'short');
      expect([where, cause, await stopOf(page, 'short')]).toEqual([where, cause, null]);
      await page.evaluate(on, long);
      await page.waitForFunction(() => document.querySelector('#short [data-alertpart="message"]').getAttribute('tabindex') === '0');
      await page.evaluate(off);
      await page.waitForFunction(() => !document.querySelector('#short [data-alertpart="message"]').hasAttribute('tabindex'));
      await close(page, 'short');
    }
  });
}, 120000);

test("a box the page replaces before the alert's first frame is the one watched", async () => {
  await inEngines(async (page, where) => {
    const got = await page.evaluate(([long, frames]) => new Promise(done => {
      const dialog = document.getElementById('short');
      dialog.showModal();
      const fresh = document.createElement('div');
      fresh.dataset.alertpart = 'message';
      fresh.style.cssText = 'display:flex;flex-direction:column';
      fresh.innerHTML = `<div data-exact-text>${long}</div>`;
      dialog.querySelector('[data-alertpart="message"]').replaceWith(fresh);
      eval(frames)().then(() => done(fresh.getAttribute('tabindex')));
    }), [long, frames.toString()]);
    expect([where, got]).toEqual([where, '0']);
    await close(page, 'short');
  });
}, 120000);

test('an alert taken out of the page while open is no longer watched', async () => {
  await inEngines(async (page, where) => {
    await open(page, 'short');
    // Taken out, then put back: what watched it let go when it left, and only an opening watches it again
    const stop = await page.evaluate(async ([long, frames]) => {
      const wait = eval(frames), dialog = document.getElementById('short'), message = dialog.querySelector('[data-alertpart="message"]');
      dialog.remove();
      await wait();
      document.getElementById('exact-root').append(dialog);
      dialog.style.maxHeight = '300px'; // no longer modal, so the user agent's ceiling is gone
      message.firstElementChild.textContent = long;
      await wait();
      return [dialog.open, message.scrollHeight > message.clientHeight, message.getAttribute('tabindex')];
    }, [long, frames.toString()]);
    expect([where, stop]).toEqual([where, [true, true, null]]);
  });
}, 120000);

// Its ancestors' child lists hear it leave: the empty box is hidden and has no size for a ResizeObserver to watch
test('an alert with an empty message, taken out of the page with its ancestor, is no longer watched', async () => {
  await inEngines(async (page, where) => {
    await page.evaluate(() => {
      const dialog = document.getElementById('short');
      window.wrapper = document.createElement('div');
      document.getElementById('exact-root').append(wrapper);
      wrapper.append(dialog);
      document.getElementById('short-message').remove(); // the message box empty: hidden, with no size to change
    });
    await open(page, 'short');
    await page.evaluate(() => wrapper.remove());
    await page.evaluate(frames);
    // Put back with a long message: what watched it let go when it left
    const stop = await page.evaluate(async long => {
      document.getElementById('exact-root').append(wrapper);
      const dialog = document.getElementById('short'), message = dialog.querySelector('[data-alertpart="message"]');
      dialog.style.maxHeight = '300px'; // no longer modal, so the user agent's ceiling is gone
      message.insertAdjacentHTML('beforeend', `<div data-exact-text>${long}</div>`);
      await new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done)));
      return [dialog.open, message.scrollHeight > message.clientHeight, message.getAttribute('tabindex')];
    }, long);
    expect([where, stop]).toEqual([where, [true, true, null]]);
  });
}, 120000);

// A page rendered ahead with a long grouped alert a native invoker opens, its scripts (`head`) and stand-ins (`files`)
// served as asked, in Chrome and WebKit
async function renderedAhead({ head = '', tail = '', files, slow = {} }, drive) {
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, async fetch(request) {
    const path = new URL(request.url).pathname;
    if (slow[path]) await Bun.sleep(slow[path]);
    if (path in files) return new Response(files[path], { headers: { 'content-type': 'text/javascript' } });
    if (path.endsWith('.js')) return new Response('', { headers: { 'content-type': 'text/javascript' } });
    if (path !== '/') return new Response('', { status: 404 });
    return new Response(`<!doctype html><html><head><style>${sheet}</style>${head}</head><body><div id="exact-root">
      <button id="ask" commandfor="failed" command="show-modal">Ask</button>
      <dialog id="failed" role="alertdialog" aria-label="Couldn’t Save">${message(text('failed-message', long), 'Couldn’t Save')}${actions(button('failed-ok', 'OK'))}</dialog>
      </div><script type="application/vnd.exact.checkpoint" data-activate="interaction">{}</script>${tail}</body></html>`, { headers: { 'content-type': 'text/html; charset=utf-8' } });
  } });
  const playwright = await import('playwright-core');
  try {
    for (const engine of ['chromium', 'webkit']) {
      const browser = await playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: browserPath().executable } : {}) });
      try {
        await drive(await browser.newPage({ viewport: { width: 390, height: 844 } }), server.url.href, engine);
      } finally {
        await browser.close();
      }
    }
  } finally {
    server.stop(true);
  }
}

test('on a JS page rendered ahead, opening a grouped alert activates its runtime, which then watches the alert', async () => {
  const capture = readFileSync(new URL('../web-js/capture.js', web), 'utf8');
  const files = { '/app.js': "import './alert-message.js'; window.activated = true;", '/alert-message.js': readFileSync(new URL('alert-message.js', web), 'utf8') };
  await renderedAhead({ head: `<script>${capture}</script>`, files }, async (page, url, engine) => {
    await page.goto(url);
    await page.waitForTimeout(300);
    expect([engine, await page.evaluate(() => window.activated ?? false)]).toEqual([engine, false]);
    await page.click('#ask');
    await page.waitForFunction(() => window.activated === true);
    await page.waitForFunction(() => document.querySelector('#failed [data-alertpart="message"]').getAttribute('tabindex') === '0');
    // Opened before the module loaded, where Chrome put the focus on the box: it moves to the button
    expect([engine, await page.evaluate(() => document.activeElement.id)]).toEqual([engine, 'failed-ok']);
  });
}, 120000);

test('on a wasm page rendered ahead, opening a grouped alert starts its runtime, opened before its bootstrap loads too', async () => {
  const files = { '/document-glue.js': readFileSync(new URL('document-glue.js', web), 'utf8'), '/glue.js': 'window.activated = true;' };
  const tail = '<script type="module" src="/document-glue.js"></script>';
  await renderedAhead({ tail, files }, async (page, url, engine) => {
    await page.goto(url);
    await page.waitForFunction(() => !!globalThis.exact?.documentPage);
    await page.waitForTimeout(300);
    expect([engine, await page.evaluate(() => window.activated ?? false)]).toEqual([engine, false]);
    await page.click('#ask');
    await page.waitForFunction(() => window.activated === true);
  });
  // The bootstrap arrives after the person has opened the alert
  await renderedAhead({ tail, files, slow: { '/document-glue.js': 1500 } }, async (page, url, engine) => {
    await page.goto(url, { waitUntil: 'commit' });
    await page.click('#ask');
    expect([engine, await page.evaluate(() => !!globalThis.exact?.documentPage)]).toEqual([engine, false]);
    await page.waitForFunction(() => window.activated === true, null, { timeout: 15000 });
  });
}, 120000);
