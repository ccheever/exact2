// Re-record `chrome.txt`: where Chrome starts each line of each corpus paragraph
// in a box of width 0. With nothing fitting, the browser breaks at every
// opportunity it has, so its line starts are its break-opportunity set, and
// fonts decide nothing. `tests/it/corpus.rs` compares the walker against it.
//
//   bun textflow/tests/corpus/record.mjs <pretext checkout>   # re-excerpt and record
//   bun textflow/tests/corpus/record.mjs                      # record the kept excerpts
//
// Texts are excerpts of Pretext's corpora (github.com/chenglou/pretext,
// `corpora/`, MIT, © Pretext contributors); `sources.json` there names each
// text's origin. CHROME overrides the browser.
import { execFileSync, spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const here = new URL('.', import.meta.url).pathname;
const out = join(here, 'chrome.txt');
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const BUDGET = 2400; // UTF-16 units kept per corpus: enough to meet each script's rules

function excerpt(pretext) {
  const sources = JSON.parse(readFileSync(join(pretext, 'corpora/sources.json'), 'utf8'));
  return sources.map(s => {
    const paragraphs = [];
    let kept = 0;
    for (const line of readFileSync(join(pretext, s.output), 'utf8').split('\n')) {
      let text = line.replace(/[ \t\r]+$/, '');
      if (!text.trim() || kept >= BUDGET) continue;
      if (kept + text.length > BUDGET) {
        const cut = text.lastIndexOf(' ', BUDGET - kept);
        text = text.slice(0, cut > 0 ? cut : BUDGET - kept).replace(/[\uD800-\uDBFF]$/, '');
      }
      paragraphs.push(text);
      kept += text.length;
    }
    return { id: s.id, lang: s.language, dir: s.direction, font: `${s.font_size_px}px ${s.font_family}`, paragraphs };
  });
}

function kept() {
  const corpora = [];
  const lines = readFileSync(out, 'utf8').split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].startsWith('@ ')) {
      const [id, lang, dir, ...font] = lines[i].slice(2).split(' ');
      corpora.push({ id, lang, dir, font: font.join(' '), paragraphs: [] });
    } else if (lines[i] && !lines[i].startsWith('#')) {
      corpora.at(-1).paragraphs.push(lines[i]);
      i++; // the recorded starts
    }
  }
  return corpora;
}

// Runs in the page: the UTF-8 byte offset of the first non-collapsible
// character of every line after the first.
function measure(corpora) {
  const collapsible = /[ \t\n\r\f]/;
  return corpora.map(c => c.paragraphs.map(text => {
    const div = document.createElement('div');
    div.lang = c.lang;
    div.dir = c.dir;
    div.style.cssText = `width:0;font:${c.font};line-height:4;white-space:normal;overflow-wrap:normal;word-break:normal;line-break:auto`;
    div.textContent = text;
    document.body.append(div);
    const node = div.firstChild;
    const range = document.createRange();
    const half = parseFloat(getComputedStyle(div).lineHeight) / 2;
    const starts = [];
    let top = -Infinity;
    let bytes = 0;
    for (let i = 0; i < text.length;) {
      const cp = text.codePointAt(i);
      const n = cp > 0xffff ? 2 : 1;
      if (!collapsible.test(text[i])) {
        range.setStart(node, i);
        range.setEnd(node, i + n);
        const rect = range.getClientRects()[0];
        if (rect && rect.top > top + half) {
          if (top !== -Infinity) starts.push(bytes);
          top = rect.top;
        }
      }
      bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
      i += n;
    }
    div.remove();
    return starts;
  }));
}

const corpora = process.argv[2] ? excerpt(process.argv[2]) : kept();
const dir = mkdtempSync(join(tmpdir(), 'textflow-corpus-'));
try {
  const page = join(dir, 'record.html');
  const data = JSON.stringify(corpora).replace(/</g, '\\u003c');
  writeFileSync(page, `<!doctype html><meta charset=utf-8><body><script>
${measure};
const result = document.createElement('script');
result.type = 'application/json';
result.id = 'result';
try {
  result.textContent = JSON.stringify({ starts: measure(${data}) });
} catch (e) {
  result.textContent = JSON.stringify({ error: String(e.stack ?? e) });
}
document.body.append(result);
</script>`);
  // Headless Chrome prints the DOM and then stays up; stop the child we started.
  const child = spawn(chrome, ['--headless=new', '--no-sandbox', '--disable-extensions',
    '--disable-background-networking', '--disable-component-update', '--no-first-run',
    '--no-default-browser-check', `--user-data-dir=${join(dir, 'profile')}`, '--dump-dom',
    `file://${page}`], { stdio: ['ignore', 'pipe', 'ignore'] });
  let dom = '';
  child.stdout.setEncoding('utf8');
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(new Error('Chrome printed no DOM in 5 minutes')), 300_000);
    child.stdout.on('data', chunk => {
      dom += chunk;
      if (dom.includes('</html>')) { clearTimeout(timer); ok(); }
    });
    child.once('error', fail);
    child.once('exit', () => { clearTimeout(timer); ok(); });
  }).finally(() => child.kill());
  const { starts, error } = JSON.parse(dom.match(/<script type="application\/json" id="result">(.*?)<\/script>/s)[1]);
  if (error) throw new Error(error);
  const version = execFileSync(chrome, ['--version'], { encoding: 'utf8' }).trim();
  let text = `# Chrome's line starts at width 0 (UTF-8 byte offsets), recorded by record.mjs in ${version}.\n`
    + `# Excerpts of Pretext's corpora (github.com/chenglou/pretext, MIT, © Pretext contributors).\n`
    + `# Each corpus: "@ id lang dir font", then per paragraph its text and its starts.\n`;
  corpora.forEach((c, i) => {
    text += `@ ${c.id} ${c.lang} ${c.dir} ${c.font}\n`;
    c.paragraphs.forEach((p, j) => { text += `${p}\n${starts[i][j].join(' ')}\n`; });
  });
  writeFileSync(out, text);
  console.log(`${version}: ${corpora.length} corpora, ${corpora.reduce((n, c) => n + c.paragraphs.length, 0)} paragraphs, ${starts.flat(2).length} line starts -> ${out}`);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
