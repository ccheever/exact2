// The web draws Exact's colour roles in Apple's palette in Chrome, Firefox and WebKit, light and dark, as the hosts
// write a role (`var(--exact-<role>, <the kernel's fallback>)`): the system's own where the browser has WebKit's
// `-apple-system-*` keyword for it (Safari), else the schema's Apple pair, which the backgrounds always are (WebKit
// on a Mac resolves all six to one colour). The look's floors hold over them, the web's check reports them, and the
// glue reports them to the kernel before the boot (glue.js, src/colors.rs).
import { test, expect } from 'bun:test';
import { readFileSync } from 'node:fs';
import { withLook } from '../look.mjs';
import { chromium as browserPath } from '../../../scripts/agent-launch.mjs';

const web = new URL('../', import.meta.url);
const sheet = withLook(readFileSync(new URL('index.html', web), 'utf8')).match(/<style>([\s\S]*?)<\/style>/)[1];
const { colors } = JSON.parse(readFileSync(new URL('../../../kernel/tables/schema.json', import.meta.url), 'utf8'));
const row = name => colors.find(r => r[0] === name);
// A label, two backgrounds, a fill WebKit does not name, a system colour it does and one with no keyword
const ROLES = ['secondary-label', 'secondary-background', 'grouped-background', 'tertiary-fill', 'system-green', 'system-cyan'];
// The glue's report, as it runs before the boot
const PROBE = readFileSync(new URL('glue.js', web), 'utf8').match(/const appleRoles = [^\n]*?wasm\.exact_report_colors\(writeIn\(JSON\.stringify\(colors\)\)\); \}/)[0];
const swatch = (name, scheme) => {
  const [, , , , , , light, dark] = row(name);
  return `<i id="${name}-${scheme}" style="color: var(--exact-${name}, light-dark(${light}, ${dark}))">x</i>`;
};
// A colour as the engine computes it, in a scheme; a pixel's colour over layers painted in turn; WCAG's contrast.
const HELPERS = `
  window.read = (value, scheme) => { const i = document.createElement('i'); i.style.cssText = (scheme ? 'color-scheme:' + scheme + ';' : '') + 'color:' + value; document.getElementById('exact-root').append(i); const c = getComputedStyle(i).color; i.remove(); return c; };
  const x = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
  window.paint = (...layers) => { x.clearRect(0, 0, 1, 1); for (const l of layers) { x.fillStyle = l; x.fillRect(0, 0, 1, 1); } return [...x.getImageData(0, 0, 1, 1).data]; };
  const lum = (c) => { const [r, g, b] = c.slice(0, 3).map((v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
  window.ratio = (a, b) => { const [p, q] = [lum(a), lum(b)].sort((m, n) => n - m); return (p + 0.05) / (q + 0.05); };`;
const PAGE = `<!doctype html><html><head><style>${sheet}</style></head><body><div id="exact-root">
  ${['light', 'dark'].map(scheme => `<div style="color-scheme:${scheme}">${ROLES.map(n => swatch(n, scheme)).join('')}</div>`).join('')}
</div><script>${HELPERS}</script></body></html>`;

test("every engine draws Exact's colour roles in Apple's palette", async () => {
  // The page, and the web's check (host/web-js/check.js, and the navigation glue it imports)
  const script = { '/check.js': '../web-js/check.js', '/navigation.js': 'navigation.js' };
  const server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch: (req) => { const file = script[new URL(req.url).pathname];
    return file ? new Response(readFileSync(new URL(file, web), 'utf8'), { headers: { 'content-type': 'text/javascript' } }) : new Response(PAGE, { headers: { 'content-type': 'text/html; charset=utf-8' } }); } });
  const playwright = await import('playwright-core');
  try {
    for (const engine of ['chromium', 'firefox', 'webkit']) {
      const browser = await playwright[engine].launch({ headless: true, ...(engine === 'chromium' ? { executablePath: browserPath().executable } : {}) });
      try {
        const page = await browser.newPage();
        await page.goto(server.url.href);
        for (const scheme of ['light', 'dark']) {
          const got = {}, want = {};
          for (const name of ROLES) {
            const [, alias, , , light, dark] = row(name);
            const own = alias && !name.endsWith('background') && await page.evaluate(alias => CSS.supports('color', alias), alias);
            got[name] = await page.evaluate(id => getComputedStyle(document.getElementById(id)).color, `${name}-${scheme}`);
            want[name] = await page.evaluate(([value, scheme]) => read(value, scheme), own ? [alias, scheme] : [scheme === 'light' ? light : dark]);
          }
          expect([engine, scheme, got]).toEqual([engine, scheme, want]);
          // Apple's own stays apart from an app's theme (LLP 1095 D4), which sets the role over it: the page reports it.
          const themed = await page.evaluate((scheme) => {
            const theme = Object.assign(document.createElement('style'), { textContent: ':root { --exact-system-green: rgb(255, 0, 0); }' });
            document.head.append(theme);
            const out = [read('var(--exact-system-green)', scheme), read('var(--exact-apple-system-green)', scheme)];
            theme.remove();
            return out;
          }, scheme);
          expect([engine, scheme, themed]).toEqual([engine, scheme, ['rgb(255, 0, 0)', want['system-green']]]);
          // The look's floors hold over Apple's colours: a control's edge reads at 3:1 on the page and on a card (the line
          // is the label's ink at 45%, though Safari's label on a Mac is 85% black); a destructive control's red reads at
          // 3:1 on its own fill over a card in every style; a red the theme says is drawn as written.
          const floors = await page.evaluate((scheme) => {
            const line = read('var(--exact-line)', scheme), card = read('var(--exact-secondary-grouped-background)', scheme);
            const edges = [read('var(--exact-background)', scheme), card].map((ground) => ratio(paint(ground, line), paint(ground)));
            const button = Object.assign(document.createElement('button'), { type: 'button' });
            for (const [k, v] of [['data-native', ''], ['data-destructive', 'true']]) button.setAttribute(k, v);
            button.style.colorScheme = scheme; document.getElementById('exact-root').append(button);
            const reds = ['gray', 'bordered', 'tinted', 'plain', 'filled'].map((style) => { button.setAttribute('data-button-style', style); const b = getComputedStyle(button);
              return style === 'filled' ? ratio(paint(card, b.backgroundColor), paint(card)) : ratio(paint(card, b.backgroundColor, b.color), paint(card, b.backgroundColor)); });
            button.setAttribute('data-button-style', 'plain');
            const theme = Object.assign(document.createElement('style'), { textContent: ':root{--exact-system-red:rgb(255, 120, 120);--exact-system-red-said:var(--exact-system-red);}' });
            document.head.append(theme);
            const themed = getComputedStyle(button).color;
            theme.remove(); button.remove();
            return { edges: edges.every((r) => r >= 3) || edges, reds: reds.every((r) => r >= 3) || reds, themed };
          }, scheme);
          expect([engine, scheme, floors]).toEqual([engine, scheme, { edges: true, reds: true, themed: 'rgb(255, 120, 120)' }]);
        }
        // The web's check says what the page draws each role in, light and dark, as the driver judges an ink by it
        // (scripts/agent-check.mjs): in Safari on a Mac the secondary label is half black, no pair's value.
        const palette = await page.evaluate(async (roles) => {
          const { observe } = await import('/check.js'), said = observe(new Map()).palette;
          const drawn = (name, scheme) => { const [r, g, b, a] = paint(read(`var(--exact-${name})`, scheme)); return [r, g, b, Math.round(a / 255 * 1000) / 1000]; };
          return roles.flatMap((name) => ['light', 'dark'].map((scheme) => JSON.stringify(said?.[scheme]?.[name]) === JSON.stringify(drawn(name, scheme)) || [name, scheme, said?.[scheme]?.[name], drawn(name, scheme)]));
        }, ROLES);
        expect([engine, palette.every((same) => same === true) || palette]).toEqual([engine, true]);
        // The report: every role the page holds, light and dark, as the page draws it; a role the page does not hold
        // is left out rather than reported as the ink around the probe, and a page with none reports nothing.
        const report = await page.evaluate(([probe, roles]) => {
          const run = () => { let said = null; new Function('wasm', 'writeIn', probe)({ exact_report_colors: (json) => { said = JSON.parse(json); } }, (text) => text); return said; };
          const all = run(), want = roles.flatMap((name) => ['light', 'dark'].map((scheme) => [name, scheme === 'dark', read(`var(--exact-apple-${name})`, scheme)]));
          document.documentElement.style.setProperty('--exact-apple-system-green', 'initial');
          const less = run().filter(([name]) => name === 'system-green').length;
          document.documentElement.style.setProperty('--exact-apple-roles', 'initial');
          const none = run();
          document.documentElement.removeAttribute('style');
          const sorted = (entries) => entries.map((e) => JSON.stringify(e)).sort();
          return [sorted(all.filter(([name]) => roles.includes(name))), sorted(want), all.length, less, none];
        }, [PROBE, ROLES]);
        const named = colors.filter(([name, , ios]) => /^[a-z]/.test(name) && !ios.startsWith('@tint')).length;
        expect([engine, report[0], report[2], report[3], report[4]]).toEqual([engine, report[1], named * 2, 0, null]);
      } finally {
        await browser.close();
      }
    }
  } finally {
    server.stop(true);
  }
}, 120000);
