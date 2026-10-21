// The benchmark's documents: the corpus (the 40 largest llp/*.md at the commit
// LLP 1044 measured) and the generated stress documents. Byte-identical to the
// ones behind LLP 1044 §3: the generators were Python, so Python's Mersenne
// Twister and its choice/randint/random are reproduced here.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const CORPUS_COMMIT = '9dbaded0';

// CPython's random.Random: MT19937 seeded by init_by_array.
class PyRandom {
  constructor(seed) {
    this.mt = new Uint32Array(624); this.i = 625;
    const key = [];
    for (let n = BigInt(seed); key.length === 0 || n > 0n; n >>= 32n) key.push(Number(n & 0xffffffffn));
    this.init(19650218);
    let i = 1, j = 0;
    for (let k = Math.max(624, key.length); k; k--) {
      const p = this.mt[i - 1] ^ (this.mt[i - 1] >>> 30);
      this.mt[i] = ((this.mt[i] ^ Math.imul(p, 1664525)) + key[j] + j) >>> 0;
      i++; j++;
      if (i >= 624) { this.mt[0] = this.mt[623]; i = 1; }
      if (j >= key.length) j = 0;
    }
    for (let k = 623; k; k--) {
      const p = this.mt[i - 1] ^ (this.mt[i - 1] >>> 30);
      this.mt[i] = ((this.mt[i] ^ Math.imul(p, 1566083941)) - i) >>> 0;
      i++;
      if (i >= 624) { this.mt[0] = this.mt[623]; i = 1; }
    }
    this.mt[0] = 0x80000000;
  }
  init(s) {
    this.mt[0] = s >>> 0;
    for (let i = 1; i < 624; i++) this.mt[i] = (Math.imul(1812433253, this.mt[i - 1] ^ (this.mt[i - 1] >>> 30)) + i) >>> 0;
    this.i = 624;
  }
  u32() {
    if (this.i >= 624) {
      for (let k = 0; k < 624; k++) {
        const y = (this.mt[k] & 0x80000000) | (this.mt[(k + 1) % 624] & 0x7fffffff);
        this.mt[k] = this.mt[(k + 397) % 624] ^ (y >>> 1) ^ (y & 1 ? 0x9908b0df : 0);
      }
      this.i = 0;
    }
    let y = this.mt[this.i++];
    y ^= y >>> 11; y ^= (y << 7) & 0x9d2c5680; y ^= (y << 15) & 0xefc60000; y ^= y >>> 18;
    return y >>> 0;
  }
  random() { return ((this.u32() >>> 5) * 67108864 + (this.u32() >>> 6)) / 9007199254740992; }
  below(n) { // _randbelow, n < 2^32
    const k = 32 - Math.clz32(n);
    for (;;) { const r = this.u32() >>> (32 - k); if (r < n) return r; }
  }
  randint(a, b) { return a + this.below(b - a + 1); }
  choice(xs) { return xs[this.below(xs.length)]; }
}

const bytes = s => Buffer.byteLength(s);
const cps = s => [...s].length;
// str.capitalize: the first character upper, the rest lower.
const capitalize = s => { const [h, ...t] = [...s]; return h.toUpperCase() + t.join('').toLowerCase(); };

function seed1044(out) {
  const r = new PyRandom(1044);
  const words = 'the quick brown fox jumps over a lazy dog while layout engines measure every glyph and line break carefully before painting text into layers'.split(' ');
  const uni = ['naïve', 'café', 'Übermaß', '日本語の文章', '中文段落', '한국어', 'Ελληνικά', 'русский', 'עברית', 'العربية', '🙂', '👩🏽‍💻', '🚀', '✨'];
  const sentence = (n, u = 0) => { const o = []; for (let i = 0; i < n; i++) o.push(r.random() < u ? r.choice(uni) : r.choice(words)); return capitalize(o.join(' ')) + '.'; };
  const para = (n, u = 0) => { const o = []; for (let i = 0; i < n; i++) o.push(sentence(r.randint(8, 20), u)); return o.join(' '); };
  const fill = (target, gen) => { const parts = []; let size = 0; while (size < target) { const p = gen(); parts.push(p); size += bytes(p) + 2; } return parts.join('\n\n') + '\n'; };
  for (const [name, sz] of [['paragraph-1m', 1 << 20], ['paragraph-3m', Math.trunc(3.5 * (1 << 20))]]) {
    const body = []; let size = 0;
    while (size < sz) { const s = sentence(r.randint(8, 20), 0.03); body.push(s); size += bytes(s) + 1; }
    out[name] = '# One paragraph\n\n' + body.join(' ') + '\n';
  }
  { const lines = []; let size = 0;
    while (size < 1 << 20) {
      const indent = '    '.repeat(r.randint(0, 4)), n = r.randint(3, 24), ws = [];
      for (let i = 0; i < n; i++) ws.push(r.choice(words));
      const l = indent + ws.join(' ') + ';'; lines.push(l); size += cps(l) + 1;
    }
    out['code-1m'] = '# One code block\n\n```rust\n' + lines.join('\n') + '\n```\n'; }
  { const rows = ['| # | name | description | value |', '|---|---|---|---|']; let size = 0;
    for (let i = 0; size < 1 << 20; i++) {
      const a = r.choice(words), b = r.choice(words), d = sentence(r.randint(3, 12)), v = r.randint(0, 99999);
      const row = `| ${i} | ${a} ${b} | ${d} | ${v} |`; rows.push(row); size += cps(row) + 1;
    }
    out['table-1m'] = '# Giant table\n\n' + rows.join('\n') + '\n'; }
  { const blocks = []; let size = 0;
    for (let i = 0; size < 1 << 20; i++) {
      const b = i % 5 === 0 ? `## Heading ${i}` : `Short ${r.choice(words)} ${r.choice(words)} ${i}.`;
      blocks.push(b); size += cps(b) + 2;
    }
    out['many-blocks-1m'] = blocks.join('\n\n') + '\n'; }
  out['dense-inline-1m'] = '# Dense inline\n\n' + fill(1 << 20, () => {
    const o = [], n = r.randint(40, 90);
    for (let k = 0; k < n; k++) { const w = r.choice(words); o.push([w, `**${w}**`, `*${w}*`, `\`${w}\``, `[${w}](https://example.com/${w})`, `***${w}***`][k % 6]); }
    return o.join(' ');
  });
  const tok = 'abcdefghijklmnopqrstuvwxyz0123456789-_/';
  out['long-tokens-1m'] = '# Long tokens\n\n' + fill(1 << 20, () => {
    const o = [];
    for (let i = 0; i < 30; i++) {
      if (r.random() < 0.5) { const n = r.randint(80, 400); let t = ''; for (let k = 0; k < n; k++) t += r.choice(tok); o.push('https://example.com/' + t); }
      else o.push(r.choice(words));
    }
    return o.join(' ');
  });
  out['scripts-1m'] = '# Scripts\n\n' + fill(1 << 20, () => para(r.randint(2, 6), 0.35));
  out['dense-plain-1m'] = out['dense-inline-1m'].replace(/\[([^\]]+)\]\([^)]+\)/g, '$1').replaceAll('***', '').replaceAll('**', '').replaceAll('*', '').replaceAll('`', '');
}

function malformed(out) {
  const r = new PyRandom(7), words = 'the quick brown fox jumps over a lazy dog'.split(' ');
  for (const [name, unit] of [['malformed-bracket-64k', '[x'], ['malformed-autolink-64k', 'x<']]) {
    const parts = []; let n = 0;
    while (n < 64 * 1024) { const s = r.random() < 0.5 ? unit : r.choice(words); parts.push(s); n += cps(s) + 1; }
    out[name] = '# Malformed\n\n' + parts.join(' ') + '\n';
  }
  out['dense-bracket-64k'] = '# Malformed\n\n' + '[x'.repeat(32768) + '\n';
  out['dense-autolink-64k'] = '# Malformed\n\n' + 'x<'.repeat(32768) + '\n';
}

const greek = 'alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega'.split(' ');
const header = cols => ['| ' + Array.from({ length: cols }, (_, c) => `col ${c}`).join(' | ') + ' |', '|' + Array(cols).fill('---').join('|') + '|'];

function wide(out) {
  const r = new PyRandom(7);
  for (const [cols, rows] of [[16, 10], [64, 10], [256, 10], [32, 2000]]) {
    const lines = [`# Wide table ${cols}x${rows}`, '', 'Some prose before the table so the page has a heading and a paragraph.', '', ...header(cols)];
    for (let row = 0; row < rows; row++) { const cells = []; for (let c = 0; c < cols; c++) cells.push(`${r.choice(greek)} ${row}.${c}`); lines.push('| ' + cells.join(' | ') + ' |'); }
    lines.push('', 'Prose after the table.', '');
    out[`wide-${cols}x${rows}`] = lines.join('\n');
  }
  const r9 = new PyRandom(9), lines = ['# Wide table 256x200', '', 'Prose before.', '', ...header(256)];
  for (let row = 0; row < 200; row++) { const cells = []; for (let c = 0; c < 256; c++) cells.push(`${r9.choice(greek)} ${row}.${c}`); lines.push('| ' + cells.join(' | ') + ' |'); }
  out['wide-256x200'] = lines.join('\n') + '\n\nProse after.\n';
}

function unicode(out) {
  const r = new PyRandom(11);
  const range = (a, b) => Array.from({ length: b - a }, (_, i) => String.fromCodePoint(a + i));
  const emoji = [...range(0x1F600, 0x1F64F), ...range(0x1F300, 0x1F3FF), ...range(0x1F680, 0x1F6C5)];
  const zwj = ['👩‍💻', '👨‍👩‍👧‍👦', '🏳️‍🌈', '🧑🏽‍🚀', '👍🏿', '🇯🇵', '🇺🇸', '❤️‍🔥'];
  const words = 'the quick brown fox jumps over a lazy dog while reading markdown documents'.split(' ');
  let paras = [], size = 0;
  while (size < 1_000_000) {
    const toks = [], n = r.randint(30, 90);
    for (let i = 0; i < n; i++) { const x = r.random(); toks.push(x < 0.35 ? r.choice(emoji) : x < 0.45 ? r.choice(zwj) : r.choice(words)); }
    const p = toks.join(' '); paras.push(p); size += bytes(p) + 2;
  }
  out['emoji-1m'] = '# Emoji\n\n' + paras.join('\n\n') + '\n';
  const marks = range(0x0300, 0x036F);
  paras = []; size = 0;
  while (size < 1_000_000) {
    const toks = [], n = r.randint(20, 60);
    for (let i = 0; i < n; i++) {
      let w = '';
      for (let g = r.randint(2, 6); g; g--) { w += r.choice('abcdefghij'); for (let m = r.randint(1, 12); m; m--) w += r.choice(marks); }
      toks.push(w);
    }
    const p = toks.join(' '); paras.push(p); size += bytes(p) + 2;
  }
  out['zalgo-1m'] = '# Combining\n\n' + paras.join('\n\n') + '\n';
}

/** Writes every document under `dir` (once; they never change) and returns `dir`. */
export function writeDocs(root, dir) {
  const stamp = resolve(dir, '.complete');
  if (existsSync(stamp) && readFileSync(stamp, 'utf8') === CORPUS_COMMIT) return dir;
  mkdirSync(dir, { recursive: true });
  const git = (...args) => execFileSync('git', ['-C', root, ...args], { maxBuffer: 1 << 28 });
  // `ls -S llp/*.md | head -40` at the commit: largest first, ties by name.
  const files = git('ls-tree', '-l', CORPUS_COMMIT, 'llp/').toString().trim().split('\n')
    .map(l => l.split(/\s+/)).filter(f => f[4].endsWith('.md'))
    .map(f => ({ size: +f[3], path: f[4] })).sort((a, b) => b.size - a.size || (a.path < b.path ? -1 : 1)).slice(0, 40);
  writeFileSync(resolve(dir, 'corpus.md'), Buffer.concat(files.flatMap(f => [git('show', `${CORPUS_COMMIT}:${f.path}`), Buffer.from('\n\n')])));
  writeFileSync(resolve(dir, 'readme.md'), git('show', `${CORPUS_COMMIT}:README.md`));
  const out = {};
  seed1044(out); malformed(out); wide(out); unicode(out);
  for (const [name, text] of Object.entries(out)) writeFileSync(resolve(dir, name + '.md'), text);
  writeFileSync(stamp, CORPUS_COMMIT);
  return dir;
}
