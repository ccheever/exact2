// The Contract grammar as VS Code runs it: vscode-textmate over the
// Oniguruma VM VS Code ships. A sample states the scopes a reader sees; every
// `.contract` file in the repository tokenizes without an `invalid` scope and
// ends where it began, so no string, template or block runs past a file's end.
//
//   bun test editors/vscode
//
// The grammar's word lists are checked against the compiler's tables by
// `cargo test -p contract --test it vscode_grammar`.
import { test, expect } from 'bun:test';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import textmate from 'vscode-textmate';
import oniguruma from 'vscode-oniguruma';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '../..');
const SCOPE = 'source.exact-contract';

const wasm = readFileSync(resolve(ROOT, 'node_modules/vscode-oniguruma/release/onig.wasm'));
await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
const registry = new textmate.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (sources) => new oniguruma.OnigScanner(sources),
    createOnigString: (text) => new oniguruma.OnigString(text),
  }),
  loadGrammar: async (scope) => scope === SCOPE
    ? textmate.parseRawGrammar(readFileSync(resolve(HERE, 'syntaxes/exact-contract.tmLanguage.json'), 'utf8'), 'exact-contract.tmLanguage.json')
    : null,
});
const grammar = await registry.loadGrammar(SCOPE);

/** Each line's tokens, `{ text, scopes }`, and the rule stack after the last. */
function tokenize(source) {
  let stack = textmate.INITIAL;
  const lines = source.split('\n').map((line) => {
    const { tokens, ruleStack } = grammar.tokenizeLine(line, stack);
    stack = ruleStack;
    return tokens.map((t) => ({ text: line.slice(t.startIndex, t.endIndex), scopes: t.scopes }));
  });
  return { lines, stack };
}

/** The scopes of the first token on `line` (0-based) whose text is `text`;
 * when no token is that text, the line's tokens, so the case reads as wrong. */
function scopesOf(lines, line, text) {
  const token = lines[line].find((t) => t.text === text);
  return token ? token.scopes.join(' ') : `no such token in ${JSON.stringify(lines[line].map((t) => t.text))}`;
}

const SAMPLE = `use Theme from "@exact/reading"
// a comment
timeline Swipe
sound "assets/tick.wav"
color-profile --press src="assets/press.icc"
component Player
  state paused = false
  resource weather = forecast(lat, lon with revision) as shape Forecast
  action enterFullscreen
    requestFullscreen("video")
  view
    column gap=12 resize=measured
      hr
      video "assets/motion.mp4" id="video" fullscreenchange=changed pointerdown=down
      text \`Zone \${zone + 1}\` color="#ffffff"
      when paused
        Badge(label="Paused")
      else
        text "Playing"
test "plays"
  tap "toggle"
  expect text "Play"`;

test('the sample reads as Contract', () => {
  const { lines } = tokenize(SAMPLE);
  const cases = [
    [0, 'use', 'keyword.control.import'],
    [0, 'Theme', 'entity.name.type'],
    [0, '"', 'string.quoted.double'],
    [1, '// a comment', 'comment.line.double-slash'],
    [2, 'timeline', 'keyword.declaration'],
    [2, 'Swipe', 'entity.name.type'],
    [3, 'sound', 'keyword.declaration'],
    [4, 'color-profile', 'keyword.declaration'],
    [4, '--press', 'entity.name.type'],
    [5, 'component', 'keyword.declaration'],
    [5, 'Player', 'entity.name.type'],
    [6, 'state', 'keyword.declaration'],
    [6, 'false', 'constant.language'],
    [7, 'resource', 'keyword.declaration'],
    [7, 'with', 'keyword.control'],
    [7, 'forecast', 'entity.name.function'],
    [8, 'action', 'keyword.declaration'],
    [8, 'enterFullscreen', 'entity.name.function'],
    [9, 'requestFullscreen', 'support.function.builtin'],
    [11, 'column', 'support.type.view'],
    [11, 'gap', 'entity.other.attribute-name'],
    [11, '12', 'constant.numeric'],
    [11, 'resize', 'support.function.event'],
    [12, 'hr', 'support.type.view'],
    [13, 'video', 'support.type.view'],
    [13, 'fullscreenchange', 'support.function.event'],
    [13, 'pointerdown', 'support.function.event'],
    [14, '`', 'string.quoted.template'],
    [14, 'zone', 'variable.other'],
    [15, 'when', 'keyword.control'],
    [16, 'Badge', 'entity.name.type.component'],
    [17, 'else', 'keyword.control'],
    [19, 'test', 'keyword.declaration'],
    [20, 'tap', 'keyword.control.test'],
    [21, 'expect', 'keyword.control.test'],
  ];
  const wrong = cases.filter(([line, text, scope]) => !scopesOf(lines, line, text).includes(`${scope}.exact-contract`))
    .map(([line, text, scope]) => `line ${line} ${JSON.stringify(text)}: want ${scope}, got ${scopesOf(lines, line, text)}`);
  expect(wrong).toEqual([]);
});

/** Every `.contract` file under `dir`, skipping build output and packages. */
function contracts(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'target' || name.startsWith('.')) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) contracts(path, out);
    else if (name.endsWith('.contract')) out.push(path);
  }
  return out;
}

test('every Contract file in the repository tokenizes cleanly', () => {
  const files = ['apps', 'packages', 'contract', 'host', 'examples'].flatMap((d) => {
    try { return contracts(resolve(ROOT, d)); } catch { return []; }
  });
  expect(files.length).toBeGreaterThan(50);
  const problems = [];
  for (const file of files) {
    const name = relative(ROOT, file);
    const { lines, stack } = tokenize(readFileSync(file, 'utf8'));
    lines.forEach((tokens, i) => {
      for (const t of tokens) if (t.scopes.some((s) => s.startsWith('invalid'))) problems.push(`${name}:${i + 1} ${JSON.stringify(t.text)} is ${t.scopes.at(-1)}`);
    });
    // A declaration after the last line is one only if nothing is left open.
    const after = grammar.tokenizeLine('component Sentinel', stack).tokens[0];
    if (!after.scopes.includes('keyword.declaration.exact-contract')) problems.push(`${name}: ends inside ${after.scopes.at(-1)}`);
  }
  expect(problems).toEqual([]);
});
