// B5's precedence guard (LLP 1071.001 part B, ~/x2theme/x2p4-b5-plan.md §2 row 8): no rule of the shell's own sheets
// reaches two ids unless it is `!important`. An app's classes sit at two ids and a class (emit.rs), a lowered node's
// arm class at three ids and a class (lower.js), so a one-id shell rule loses to both as it loses to an inline row,
// and an important one beats all three alike. A shell rule at two ids would outrank a class where it did not outrank
// the inline write: this test names it.
import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { withLook } from '../look.mjs';
import { suiteBrowser } from '../../../scripts/agent-launch.mjs';

const WEB = resolve(import.meta.dir, '..'), WEB_JS = resolve(WEB, '../web-js');
// The shell's sheet with its look, and the sheets the glue writes itself (their text, as the modules hold it).
const shell = withLook(readFileSync(resolve(WEB, 'index.html'), 'utf8')).match(/<style>([\s\S]*?)<\/style>/g).map(s => s.replace(/<\/?style>/g, '')).join('\n');
const injected = [
  [resolve(WEB, 'input-glue.js'), /textContent: "(\[data-exact-snap\][^"]*)"/],
  [resolve(WEB_JS, 'modes.js'), /sheet\.textContent = "([^"]*)"/],
  [resolve(WEB, 'glue.js'), /symbolStyle\.textContent = '([^']*)'/],
].map(([file, at]) => { const m = at.exec(readFileSync(file, 'utf8')); if (!m) throw new Error(`${file}: its sheet's text moved`); return m[1]; });

test('no non-important shell rule holds two ids outside :where()', async () => {
  const suite = await suiteBrowser();
  try {
    const page = await (await suite.newContext()).newPage();
    await page.setContent(`<style>${shell}</style>${injected.map(t => `<style>${t}</style>`).join('')}<div id="exact-root"></div>`);
    const found = await page.evaluate(() => {
      // A selector's ids outside :where(), its arguments dropped (they weigh nothing).
      const ids = sel => { let out = '', depth = 0, skip = -1; for (let i = 0; i < sel.length; i++) { if (sel.startsWith(':where(', i) && skip < 0) { skip = depth; depth++; i += 6; continue; } const c = sel[i]; if (c === '(') depth++; if (c === ')') { depth--; if (depth === skip) { skip = -1; continue; } } if (skip < 0) out += c; } return (out.match(/#[A-Za-z_][\w-]*/g) ?? []).length; };
      // A list's selectors (top-level commas); a nested rule's, each under each of its parent's.
      const list = sel => { const out = []; let depth = 0, at = 0; for (let i = 0; i < sel.length; i++) { const c = sel[i]; if (c === '(') depth++; else if (c === ')') depth--; else if (c === ',' && !depth) { out.push(sel.slice(at, i).trim()); at = i + 1; } } out.push(sel.slice(at).trim()); return out; };
      const out = [], walk = (rules, parents) => { for (const r of rules) {
        const sels = r.selectorText ? (parents.length ? parents.flatMap(p => list(r.selectorText).map(c => c.includes('&') ? c.replaceAll('&', p) : `${p} ${c}`)) : list(r.selectorText)) : parents;
        if (r.style) for (const p of r.style) if (r.style.getPropertyPriority(p) !== 'important') for (const sel of sels) if (ids(sel) >= 2) out.push(`${sel} { ${p} }`);
        if (r.cssRules) walk(r.cssRules, sels);
      } };
      let n = 0;
      for (const s of document.styleSheets) { n += s.cssRules.length; walk(s.cssRules, []); }
      return { rules: n, twoIds: [...new Set(out)] };
    });
    expect(found.rules).toBeGreaterThan(300);
    expect(found.twoIds).toEqual([]);
  } finally { await suite.close(); }
});
