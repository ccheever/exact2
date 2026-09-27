// The shared recorder cases (LLP 1056 §4): `canvas/tests/cases.txt` run
// against a 2D context. `runCases(make)` is what the TypeScript recorder's
// test calls; `bun canvas/tests/cases.mjs --chrome` runs the same file in
// headless Chrome, whose answers the file records, and prints any case
// where Chrome disagrees with it.
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));

/** `[{name, lines: [{call, throws, query, want}]}]` from cases.txt. */
export function parseCases(text = readFileSync(resolve(HERE, 'cases.txt'), 'utf8')) {
  const cases = [];
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (!line || (line.startsWith('#') && !line.startsWith('##'))) continue;
    if (line.startsWith('## ')) { cases.push({ name: line.slice(3), lines: [] }); continue; }
    const c = cases[cases.length - 1];
    if (line.startsWith('? ')) {
      const [query, want] = line.slice(2).split(/\s*=>\s*/);
      c.lines.push({ query, want: want ?? '' });
    } else {
      const m = line.match(/^(.*?)\s*!\s*(\w+)$/);
      c.lines.push(m ? { call: m[1], throws: m[2] } : { call: line });
    }
  }
  return cases;
}

/** The source that runs one case against `ctx` and returns its failures. */
function caseSource(c) {
  const body = c.lines.map((l) => {
    if (l.query) return `{const v=ctx.${l.query.replace(/^getTransform\(\)$/, 'getTransform()')};const s=v&&typeof v==='object'&&'a' in v?[v.a,v.b,v.c,v.d,v.e,v.f].join(','):String(v);if(s!==${JSON.stringify(l.want)})fail.push(${JSON.stringify(l.query)}+' => '+s+', want '+${JSON.stringify(l.want)});}`;
    const call = l.call.replace(/^(\w+) = (.*)$/, (_, v, e) => (v === 'g' ? `g = ${e}` : `ctx.${v} = ${e.replace(/\bg\b/, 'g')}`));
    const stmt = /^(g = |g\.|ctx\.)/.test(call) ? call.replace(/^g = (?!ctx)/, 'g = ctx.') : `ctx.${call}`;
    if (l.throws) return `try{${stmt};fail.push(${JSON.stringify(l.call)}+' did not throw')}catch(e){if(e.name!==${JSON.stringify(l.throws)})fail.push(${JSON.stringify(l.call)}+' threw '+e.name)}`;
    return `try{${stmt}}catch(e){fail.push(${JSON.stringify(l.call)}+' threw '+e.name)}`;
  });
  return `(ctx)=>{const fail=[];let g;${body.join('\n')}\nreturn fail;}`;
}

/** Run every case against `make()`, a fresh context each: `[{name, fail}]`. */
export function runCases(make, cases = parseCases()) {
  return cases.map((c) => ({ name: c.name, fail: (0, eval)(caseSource(c))(make()) }));
}

if (process.argv.includes('--chrome')) {
  const cases = parseCases();
  const dir = mkdtempSync(resolve(tmpdir(), 'exact-canvas-cases-'));
  const page = resolve(dir, 'cases.html');
  const fns = cases.map((c) => `[${JSON.stringify(c.name)}, ${caseSource(c)}]`).join(',\n');
  writeFileSync(page, `<pre id=o></pre><script>const r=[${fns}].map(([n,f])=>({name:n,fail:f(document.createElement('canvas').getContext('2d'))}));document.getElementById('o').textContent=JSON.stringify(r)</script>`);
  const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
  const r = spawnSync(chrome, ['--headless=new', '--disable-gpu', `--user-data-dir=${resolve(dir, 'profile')}`, '--dump-dom', `file://${page}`], { encoding: 'utf8', timeout: 60000 });
  rmSync(dir, { recursive: true, force: true });
  const json = r.stdout.match(/<pre id="o">(.*)<\/pre>/s)?.[1]?.replace(/&quot;/g, '"').replace(/&gt;/g, '>').replace(/&lt;/g, '<').replace(/&amp;/g, '&');
  if (!json) { console.error('no answer from Chrome', r.stderr?.slice(0, 400)); process.exit(1); }
  const results = JSON.parse(json);
  let bad = 0;
  for (const { name, fail } of results) if (fail.length) { bad++; console.log(`${name}:\n  ${fail.join('\n  ')}`); }
  console.log(`chrome: ${results.length - bad}/${results.length} cases agree with cases.txt`);
  process.exit(bad ? 1 : 0);
}
