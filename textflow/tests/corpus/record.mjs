// Re-record `breaks.txt`: where Chrome, Safari's WebKit and Firefox start each
// line of each corpus paragraph in a box of width 0. With nothing fitting, a
// browser breaks at every opportunity it has, so its line starts are its
// break-opportunity set, and fonts decide nothing. Beside them, each engine's
// `Intl.Segmenter` word boundaries between two letters of Thai, Lao, Khmer or
// Myanmar, which the web host hands the walker. `tests/it/corpus.rs` scores
// the walker against each engine.
//
//   bun textflow/tests/corpus/record.mjs [--pretext <checkout>] [chrome] [webkit] [firefox]
//
// Names no engine: records all three. `--pretext` re-excerpts the texts, which
// drops every engine not recorded in the same run. Texts are excerpts of
// Pretext's corpora (github.com/chenglou/pretext, `corpora/`, MIT, © Pretext
// contributors); `sources.json` there names each text's origin. CHROME and
// FIREFOX override the browsers; WebKit is the system's, through webkit.swift.
import { execFileSync, spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const here = new URL('.', import.meta.url).pathname;
const out = join(here, 'breaks.txt');
const ENGINES = ['chrome', 'webkit', 'firefox'];
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const firefox = process.env.FIREFOX ?? '/Applications/Firefox.app/Contents/MacOS/firefox';
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
      paragraphs.push({ text, data: {} });
      kept += text.length;
    }
    return { id: s.id, lang: s.language, dir: s.direction, font: `${s.font_size_px}px ${s.font_family}`, paragraphs };
  });
}

// The file: "# engine <name> <version>" lines, then per corpus "@ id lang dir font",
// then per paragraph its text and "= <key> <offsets…>" lines; "= <key> =" repeats
// Chrome's line of the same kind.
function load() {
  const corpora = [], versions = {};
  for (const line of readFileSync(out, 'utf8').split('\n')) {
    if (line.startsWith('# engine ')) {
      const [name, ...version] = line.slice(9).split(' ');
      versions[name] = version.join(' ');
    } else if (line.startsWith('#') || !line) {
      continue;
    } else if (line.startsWith('@ ')) {
      const [id, lang, dir, ...font] = line.slice(2).split(' ');
      corpora.push({ id, lang, dir, font: font.join(' '), paragraphs: [] });
    } else if (line.startsWith('= ')) {
      const [key, ...offsets] = line.slice(2).split(' ');
      const data = corpora.at(-1).paragraphs.at(-1).data;
      data[key] = offsets[0] === '=' ? data[key.replace(/^\w+/, 'chrome')] : offsets.filter(Boolean).map(Number);
    } else {
      corpora.at(-1).paragraphs.push({ text: line, data: {} });
    }
  }
  return { corpora, versions };
}

// Runs in the page: per paragraph, the UTF-8 byte offset of the first
// non-collapsible character of every line after the first, and its SA words.
function measure(corpora) {
  const collapsible = /[ \t\n\r\f]/;
  const complex = /[\p{sc=Thai}\p{sc=Lao}\p{sc=Khmer}\p{sc=Myanmar}\p{sc=Tai_Tham}\p{sc=Tai_Viet}\p{sc=New_Tai_Lue}]/u;
  const segmenter = new Intl.Segmenter(undefined, { granularity: 'word' });
  const utf8 = (text, i) => new TextEncoder().encode(text.slice(0, i)).length;
  return corpora.map(c => c.paragraphs.map(text => {
    const words = [...segmenter.segment(text)].map(s => s.index)
      .filter(i => i > 0 && complex.test(text[i - 1]) && complex.test(text[i])).map(i => utf8(text, i));
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
        // After a taken soft hyphen, the first rect is the generated hyphen's.
        const rects = range.getClientRects();
        const rect = rects[rects.length - 1];
        if (rect && rect.top > top + half) {
          if (top !== -Infinity) starts.push(bytes);
          top = rect.top;
        }
      }
      bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
      i += n;
    }
    div.remove();
    return { starts, words };
  }));
}

function page(corpora) {
  const data = JSON.stringify(corpora.map(c => ({ ...c, paragraphs: c.paragraphs.map(p => p.text) })));
  return `<!doctype html><meta charset=utf-8><body><script>
${measure};
let payload;
try { payload = JSON.stringify({ agent: navigator.userAgent, paragraphs: measure(${data.replace(/</g, '\\u003c')}) }); }
catch (e) { payload = JSON.stringify({ error: String(e.stack ?? e) }); }
if (window.webkit?.messageHandlers?.result) window.webkit.messageHandlers.result.postMessage(payload);
else fetch('/result', { method: 'POST', body: payload });
</script>`;
}

// Serve the page, start the engine on it, and resolve with what the page posts.
// Only the child started here is stopped.
async function run(engine, html, dir) {
  if (engine === 'webkit') {
    const file = join(dir, 'page.html'), binary = join(dir, 'webkit');
    writeFileSync(file, html);
    execFileSync('swiftc', ['-O', join(here, 'webkit.swift'), '-o', binary], { stdio: 'inherit' });
    return execFileSync(binary, [file], { encoding: 'utf8', maxBuffer: 64 << 20, timeout: 300_000 });
  }
  let child;
  const server = createServer();
  try {
    const posted = new Promise((ok, fail) => {
      server.on('request', (request, response) => {
        if (request.method === 'POST') {
          let body = '';
          request.setEncoding('utf8');
          request.on('data', chunk => { body += chunk; });
          request.on('end', () => { response.end(); ok(body); });
        } else {
          response.setHeader('content-type', 'text/html; charset=utf-8');
          response.end(html);
        }
      });
      setTimeout(() => fail(new Error(`${engine} posted nothing in 5 minutes`)), 300_000).unref();
    });
    await new Promise(ok => server.listen(0, '127.0.0.1', ok));
    const url = `http://127.0.0.1:${server.address().port}/`;
    const profile = join(dir, `${engine}-profile`);
    mkdirSync(profile);
    if (engine === 'chrome') {
      child = spawn(chrome, ['--headless=new', '--no-sandbox', '--disable-extensions',
        '--disable-background-networking', '--disable-component-update', '--no-first-run',
        '--no-default-browser-check', `--user-data-dir=${profile}`, url], { stdio: 'ignore' });
    } else {
      writeFileSync(join(profile, 'user.js'), [
        'browser.shell.checkDefaultBrowser', 'datareporting.policy.dataSubmissionEnabled',
        'toolkit.telemetry.reportingpolicy.firstRun', 'app.update.enabled',
      ].map(p => `user_pref("${p}", false);\n`).join(''));
      child = spawn(firefox, ['--headless', '--no-remote', '--profile', profile, url], { stdio: 'ignore' });
    }
    child.once('error', error => { throw error; });
    return await posted;
  } finally {
    child?.kill();
    server.close();
  }
}

function version(engine) {
  if (engine === 'chrome') return execFileSync(chrome, ['--version'], { encoding: 'utf8' }).trim();
  if (engine === 'firefox') return execFileSync(firefox, ['--version'], { encoding: 'utf8' }).trim();
  const plist = (path, key) => execFileSync('defaults', ['read', path, key], { encoding: 'utf8' }).trim();
  return `Safari ${plist('/Applications/Safari.app/Contents/Info', 'CFBundleShortVersionString')}`
    + ` (WebKit ${plist('/System/Library/Frameworks/WebKit.framework/Resources/Info', 'CFBundleVersion')})`;
}

const args = process.argv.slice(2);
const pretext = args.includes('--pretext') ? args[args.indexOf('--pretext') + 1] : null;
const engines = ENGINES.filter(e => args.includes(e));
const { corpora, versions } = pretext ? { corpora: excerpt(pretext), versions: {} } : load();
const dir = mkdtempSync(join(tmpdir(), 'textflow-corpus-'));
try {
  for (const engine of engines.length ? engines : ENGINES) {
    if (engine !== 'webkit' && !existsSync(engine === 'chrome' ? chrome : firefox)) throw new Error(`${engine} is not installed; set ${engine.toUpperCase()}`);
    const { paragraphs, error } = JSON.parse(await run(engine, page(corpora), dir));
    if (error) throw new Error(`${engine}: ${error}`);
    corpora.forEach((c, i) => c.paragraphs.forEach((p, j) => {
      p.data[engine] = paragraphs[i][j].starts;
      p.data[`${engine}-words`] = paragraphs[i][j].words;
    }));
    versions[engine] = version(engine);
    console.log(`${versions[engine]}: ${paragraphs.flat().reduce((n, p) => n + p.starts.length, 0)} line starts, `
      + `${paragraphs.flat().reduce((n, p) => n + p.words.length, 0)} SA word boundaries`);
  }
  let text = `# Line starts at width 0 (UTF-8 byte offsets) per engine, recorded by record.mjs,\n`
    + `# and each engine's Intl.Segmenter SA word boundaries ("<engine>-words").\n`
    + `# Excerpts of Pretext's corpora (github.com/chenglou/pretext, MIT, © Pretext contributors).\n`;
  for (const engine of ENGINES) if (versions[engine]) text += `# engine ${engine} ${versions[engine]}\n`;
  for (const c of corpora) {
    text += `@ ${c.id} ${c.lang} ${c.dir} ${c.font}\n`;
    for (const p of c.paragraphs) {
      text += `${p.text}\n`;
      for (const engine of ENGINES) {
        for (const key of [engine, `${engine}-words`]) {
          const offsets = p.data[key], chromes = p.data[key.replace(/^\w+/, 'chrome')];
          if (!offsets) continue;
          const same = engine !== 'chrome' && chromes && offsets.join() === chromes.join();
          text += `= ${key} ${same ? '=' : offsets.join(' ')}`.trimEnd() + '\n';
        }
      }
    }
  }
  writeFileSync(out, text);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
