// A control's label is no text to select: a button, a tab, a pressable box or link, or an element in a control's role
// selects nothing in Chrome, Firefox and WebKit, while prose (its links and pressed runs, Markdown text), a field's
// value, the Markdown editor and an author's opt-in inside a control still select. The nodes are typed by hand in the
// hosts' shape; `user_select_is_written_for_webkit_too` covers the writers.
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { withLook } from '../look.mjs';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

const web = new URL('../', import.meta.url);
const sheet = withLook(readFileSync(new URL('index.html', web), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
const PAGE = `<!doctype html><html><head><style>${sheet}</style></head><body><div id="exact-root">
  <div data-exact-text id="prose">Every word here selects, <a data-view href="/docs" id="inline-link">a link</a> and <span data-view tabindex="0" data-exact-on="press" id="inline-press">a pressed run</span> too</div>
  <a data-view href="/about" id="text-link"><div data-exact-text id="text-link-label">About</div></a>
  <div data-exact-text data-exact-on="press" id="pressed-text">Tap to retry</div>
  <div data-exact-text markup="markdown" id="markdown-text">Markdown *text* selects</div>
  <div role="tablist">
    <button role="tab" aria-selected="true" data-exact-on="press" id="tab"><div data-exact-text id="tab-label">Status</div><div data-exact-text id="opt-in" style="-webkit-user-select:text;user-select:text">ABC-123</div></button>
    <div role="tab" aria-selected="false" data-exact-on="press" id="div-tab"><div data-exact-text id="div-tab-label">Settings</div><div markup="markdown" contenteditable="false" class="exact-markdown-editor" id="markdown-editor">Read *only*</div></div>
  </div>
  <button type="button" data-exact-on="press" id="plain"><div data-exact-text id="plain-label">Refresh</div><div data-exact-text markup="markdown" id="button-markdown">*Now*</div></button>
  <button type="button" data-native data-button-style="bordered" id="native" style="display:grid"><span data-exact-text id="native-label">Save</span></button>
  <div data-exact-on="press" id="card" style="-webkit-user-select:none;user-select:none;cursor:pointer;text-transform:uppercase"><div data-exact-text id="card-label">Card</div><div data-exact-text markup="markdown" id="card-markdown">Its *body*</div></div>
  <a data-view href="/next" id="row-link"><div data-exact-text id="row-link-label">Next</div></a>
  <div role="switch" aria-checked="false" id="switch"><div data-exact-text id="switch-label">Wi-Fi</div></div>
  <div role="menuitem" id="menuitem"><div data-exact-text id="menuitem-label">Rename</div></div>
  <div role="option" id="option"><div data-exact-text id="option-label">Small</div></div>
  <input data-native id="field" value="typed words">
  <input data-native data-exact-on="press" id="pressed-field" value="pressed words">
</div></body></html>`;

test('a control selects no text; prose, a field, the Markdown editor and an author’s opt-in still do', async () => {
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: () => new Response(PAGE, { headers: { 'content-type': 'text/html; charset=utf-8' } }) });
  const playwright = await import('playwright-core');
  try {
    for (const engine of ['chromium', 'firefox', 'webkit']) {
      const browser = await playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: browserPath().executable } : {}) });
      try {
        const page = await browser.newPage();
        await page.goto(server.url.href);
        const want = {
          tab: 'none', 'tab-label': 'none', 'div-tab': 'none', 'div-tab-label': 'none', 'plain-label': 'none', 'button-markdown': 'none', 'native-label': 'none',
          'card-label': 'none', 'card-markdown': 'none', 'row-link-label': 'none', 'switch-label': 'none', 'menuitem-label': 'none', 'option-label': 'none',
          'text-link-label': 'none', 'pressed-text': 'none',
          prose: 'text', 'inline-link': 'text', 'inline-press': 'text', 'markdown-text': 'text', field: 'text', 'pressed-field': 'text', 'opt-in': 'text', 'markdown-editor': 'text',
        };
        // What the engine would select from each: WebKit selects by its own property, which inherits and whose
        // `auto` selects; elsewhere the used `user-select`, `auto` being the parent's value (CSS UI 4)
        const got = await page.evaluate(([ids, webkit]) => {
          const used = e => {
            const s = getComputedStyle(e);
            if (webkit) return s.webkitUserSelect === 'none' ? 'none' : 'text';
            return s.userSelect !== 'auto' || !e.parentElement ? s.userSelect : used(e.parentElement);
          };
          return Object.fromEntries(ids.map(id => [id, used(document.getElementById(id)) === 'none' ? 'none' : 'text']));
        }, [Object.keys(want), engine === 'webkit']);
        expect([engine, got]).toEqual([engine, want]);
        // A text's runs are its text: selecting the paragraph selects its link and its pressed run with it.
        expect([engine, await page.evaluate(() => { getSelection().selectAllChildren(document.getElementById('prose')); return getSelection().toString().replace(/\s+/g, ' '); })])
          .toEqual([engine, 'Every word here selects, a link and a pressed run too']);
        // Markdown text inherits what its author said around it, as the page's other text does.
        expect([engine, await page.evaluate(() => { const s = getComputedStyle(document.getElementById('card-markdown')); return [s.cursor, s.textTransform]; })]).toEqual([engine, ['pointer', 'uppercase']]);
      } finally {
        await browser.close();
      }
    }
  } finally {
    server.stop(true);
  }
}, 120000);
