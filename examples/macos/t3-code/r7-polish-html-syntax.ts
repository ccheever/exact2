// Lane r7-polish: HTML source as the reference colours it. Every source surface (the attachment's
// HTML source, a Files preview, a diff, a chat code block) is @pierre/diffs over Shiki with the
// pierre-light / pierre-dark themes (ReadOnlySourcePreview.tsx: resolveDiffThemeName), and Shiki's
// html grammar embeds the css grammar in <style> and the javascript grammar in <script> and in
// on* attribute values. The classes are the ones `synColor` (markdown.contract) already maps to
// those themes' colours; the scopes below were read from Shiki 4.2's own tokens for the same text:
//   <!doctype html>  `<!` punct · `doctype` tag (entity.name.tag) · `html` attr · `>` punct
//   tag brackets punct, names tag, attribute names attr (entity.other.attribute-name), `=` punct,
//   quoted and unquoted values str; style="…" stays one string; on*="…" is str quotes around JS
//   &amp; → `&` punct · `amp` tag · `;` punct; &#169; → `&` punct · `#169;` tag; <!-- --> com
//   CSS: selectors tag, `.class` attr, `#id` fn, pseudo-classes op, combinators and brackets
//   punct; property names op (support.type.property-name; a custom property var, an unknown one
//   plain); values: numbers num with their unit tag, #hex and keyword values flag, functions fn,
//   strings str (escapes esc), !important and at-rule names kw, url(bare) and keyframe names param.
import type { Cls, Token } from './timeline-highlight';
import { CSS_PROPERTY, CSS_VALUE } from './r7-polish-css-words';
import { closingTag, scriptBodyEnd, scriptTokens, styleBodyEnd } from './r9-device-html-end';

const bareScriptEnd = (text: string, index: number) => { const close = text.slice(index).search(/<\/script/i); return close < 0 ? text.length : index + close; };

type Script = (text: string) => Token[];

function push(tokens: Token[], text: string, cls: Cls) {
  if (!text) return;
  const last = tokens[tokens.length - 1];
  if (last && last.cls === cls) last.text += text; else tokens.push({ text, cls });
}
function append(tokens: Token[], more: readonly Token[]) { for (const token of more) push(tokens, token.text, token.cls); }
function match(re: RegExp, text: string, index: number): string {
  re.lastIndex = index;
  const found = re.exec(text);
  return found ? found[0] : '';
}

const TAG_NAME = /[A-Za-z][\w:.-]*/y;
const ATTR_NAME = /[^\s"'<>\/=]+/y;
const UNQUOTED = /[^\s"'<>`=]+/y;
const SPACE = /\s+/y;
const ENTITY = /&(?:#\d+;|#[xX][\da-fA-F]+;|[A-Za-z][A-Za-z\d]*;)/y;
const JS_TYPES = /^(?:|module|text\/javascript|application\/javascript|text\/ecmascript|application\/ecmascript|text\/babel|text\/jsx)$/i;

/** One quoted or unquoted attribute value; an event handler's quoted value is JavaScript. */
function attributeValue(tokens: Token[], text: string, index: number, name: string, script: Script): number {
  const quote = text[index];
  if (quote === '"' || quote === "'") {
    const close = text.indexOf(quote, index + 1), end = close < 0 ? text.length : close + 1;
    const inner = text.slice(index + 1, close < 0 ? text.length : close);
    if (/^on/i.test(name) && inner) {
      push(tokens, quote, 'str'); append(tokens, script(inner)); if (close >= 0) push(tokens, quote, 'str');
    } else push(tokens, text.slice(index, end), 'str');
    return end;
  }
  const bare = match(UNQUOTED, text, index);
  push(tokens, bare, 'str');
  return index + bare.length;
}

/** The inside of a start tag (or `<!…>` / `<?…?>`) up to and including its closing bracket. */
function attributes(tokens: Token[], text: string, index: number, script: Script, attrs: Map<string, string>, closer: string): number {
  while (index < text.length) {
    if (text.startsWith(closer, index)) { push(tokens, closer, 'punct'); return index + closer.length; }
    if (closer !== '>' && text[index] === '>') { push(tokens, '>', 'punct'); return index + 1; }
    if (text.startsWith('/>', index)) { push(tokens, '/>', 'punct'); return index + 2; }
    if (text[index] === '<') return index;
    const space = match(SPACE, text, index);
    if (space) { push(tokens, space, 'punct'); index += space.length; continue; }
    const name = match(ATTR_NAME, text, index);
    if (!name) { push(tokens, text[index]!, 'punct'); index++; continue; }
    push(tokens, name, 'attr'); index += name.length;
    const gap = match(SPACE, text, index);
    if (text[index + gap.length] !== '=') { attrs.set(name.toLowerCase(), ''); continue; }
    push(tokens, gap, 'punct'); push(tokens, '=', 'punct'); index += gap.length + 1;
    const after = match(SPACE, text, index);
    push(tokens, after, 'punct'); index += after.length;
    const start = index;
    index = attributeValue(tokens, text, index, name, script);
    attrs.set(name.toLowerCase(), text.slice(start, index).replace(/^["']|["']$/g, ''));
  }
  return index;
}

/** Tokens for an HTML document with its embedded style sheets and scripts. */
export function htmlTokens(text: string, script: Script): Token[] {
  const tokens: Token[] = [];
  let index = 0;
  while (index < text.length) {
    const character = text[index]!;
    if (character === '<') {
      if (text.startsWith('<!--', index)) {
        const close = text.indexOf('-->', index + 4), end = close < 0 ? text.length : close + 3;
        push(tokens, text.slice(index, end), 'com'); index = end; continue;
      }
      const opener = text.startsWith('</', index) ? '</' : text.startsWith('<!', index) ? '<!' : text.startsWith('<?', index) ? '<?' : '<';
      const name = match(TAG_NAME, text, index + opener.length);
      if (name) {
        push(tokens, opener, 'punct'); push(tokens, name, 'tag');
        const attrs = new Map<string, string>();
        index = attributes(tokens, text, index + opener.length + name.length, script, attrs, opener === '<?' ? '?>' : '>');
        const element = name.toLowerCase();
        if (opener === '<' && (element === 'script' || element === 'style') && text[index - 1] === '>' && text[index - 2] !== '/') {
          // Lane r9-device: the body ends where Shiki's grammar ends it (r9-device-html-end.ts).
          const js = element === 'script' && JS_TYPES.test(attrs.get('type') ?? '');
          const end = element === 'style' ? styleBodyEnd(text, index) : js ? scriptBodyEnd(text, index) : bareScriptEnd(text, index);
          const body = text.slice(index, end);
          if (element === 'style') append(tokens, cssTokens(body));
          else if (js) append(tokens, scriptTokens(body, script));
          else push(tokens, body, '');
          index = end;
          if (end < text.length) { const close = closingTag(text, end, element); append(tokens, close.tokens); index = close.end; }
        }
        continue;
      }
    }
    if (character === '&') {
      const entity = match(ENTITY, text, index);
      if (entity) {
        push(tokens, '&', 'punct');
        if (entity[1] === '#') push(tokens, entity.slice(1), 'tag');
        else { push(tokens, entity.slice(1, -1), 'tag'); push(tokens, ';', 'punct'); }
        index += entity.length; continue;
      }
    }
    push(tokens, character, ''); index++;
  }
  return tokens;
}

// --- CSS (Shiki's css grammar, as the pierre themes colour it) ---
const IDENT = /-?(?:[A-Za-z_]|\\.)(?:[\w-]|\\.)*|--[\w-]*/y;
const NUMBER = /[-+]?(?:\d*\.\d+|\d+)(?:[eE][-+]?\d+)?/y;
const UNIT = /%|[A-Za-z]+/y;
const HEX = /#[\da-fA-F]{3,8}(?![\w-])/y;
const BLOCK_RULES = /^(?:media|supports|document|layer|container|scope|starting-style)$/i;

type CssMode = 'selector' | 'props' | 'value' | 'header' | 'frames';

function cssString(tokens: Token[], text: string, index: number): number {
  const quote = text[index]!;
  let at = index + 1, run = quote;
  while (at < text.length && text[at] !== quote && text[at] !== '\n') {
    if (text[at] === '\\' && at + 1 < text.length) { push(tokens, run, 'str'); run = ''; push(tokens, text.slice(at, at + 2), 'esc'); at += 2; continue; }
    run += text[at]; at++;
  }
  if (text[at] === quote) { run += quote; at++; }
  push(tokens, run, 'str');
  return at;
}

/** Whether a property-list word starts a nested rule: its `{` comes before any `;` or `}`. */
function opensRule(text: string, index: number): boolean {
  const rest = text.slice(index).search(/[;{}]/);
  return rest >= 0 && text[index + rest] === '{';
}

export function cssTokens(text: string): Token[] {
  const tokens: Token[] = [];
  const stack: CssMode[] = [];
  let mode: CssMode = 'selector', headerReturn: CssMode = 'selector', atRule = '', depth = 0, fn = '';
  let index = 0;
  while (index < text.length) {
    const character = text[index]!;
    if (text.startsWith('/*', index)) {
      const close = text.indexOf('*/', index + 2), end = close < 0 ? text.length : close + 2;
      push(tokens, text.slice(index, end), 'com'); index = end; continue;
    }
    const space = match(SPACE, text, index);
    if (space) { push(tokens, space, ''); index += space.length; continue; }
    if (character === '"' || character === "'") { index = cssString(tokens, text, index); continue; }
    if (character === '{') {
      push(tokens, '{', 'punct'); index++; depth = 0; fn = '';
      if (mode === 'header') { stack.push(headerReturn); mode = BLOCK_RULES.test(atRule) ? 'selector' : /keyframes$/i.test(atRule) ? 'frames' : 'props'; }
      else { stack.push(mode === 'value' ? 'props' : mode); mode = 'props'; }
      continue;
    }
    if (character === '}') { push(tokens, '}', 'punct'); index++; mode = stack.pop() ?? 'selector'; continue; }
    if (character === '@' && (mode === 'selector' || mode === 'props')) {
      const name = match(IDENT, text, index + 1);
      push(tokens, '@', 'punct'); push(tokens, name, 'kw'); index += 1 + name.length;
      headerReturn = mode; mode = 'header'; atRule = name; depth = 0; fn = '';
      continue;
    }
    if (mode === 'frames') {
      const offset = match(/[^{}\s]+/y, text, index);
      push(tokens, offset, ''); index += offset.length; continue;
    }
    if (mode === 'selector' || (mode === 'props' && !text.startsWith('--', index) && opensRule(text, index))) { index = selector(tokens, text, index); continue; }
    if (mode === 'props') {
      const name = match(IDENT, text, index);
      if (name) { push(tokens, name, name.startsWith('--') ? 'var' : CSS_PROPERTY.test(name) ? 'op' : ''); index += name.length; continue; }
      if (character === ':') { push(tokens, ':', 'punct'); mode = 'value'; depth = 0; fn = ''; index++; continue; }
      push(tokens, character, character === ';' ? 'punct' : ''); index++; continue;
    }
    // A property value or an at-rule header.
    if (character === ';') { push(tokens, ';', 'punct'); index++; mode = mode === 'value' ? 'props' : headerReturn; continue; }
    if (character === '(') {
      push(tokens, '(', 'punct'); depth++; index++;
      if (fn.toLowerCase() === 'url' && !/^\s*["']/.test(text.slice(index))) {
        const bare = match(/[^)\s]+/y, text, index);
        push(tokens, bare, 'param'); index += bare.length;
      }
      continue;
    }
    if (character === ')') { push(tokens, ')', 'punct'); depth = Math.max(0, depth - 1); index++; fn = ''; continue; }
    if (character === ',') { push(tokens, ',', 'punct'); index++; continue; }
    const hex = mode === 'value' ? match(HEX, text, index) : '';
    if (hex) { push(tokens, hex, 'flag'); index += hex.length; continue; }
    const previous = text[index - 1] ?? '';
    const number = /[\w)%.]/.test(previous) && /[-+]/.test(character) ? '' : match(NUMBER, text, index);
    if (number) {
      push(tokens, number, 'num'); index += number.length;
      const unit = match(UNIT, text, index);
      push(tokens, unit, 'tag'); index += unit.length; continue;
    }
    const important = character === '!' ? match(/!\s*important/iy, text, index) : '';
    if (important) { push(tokens, important, 'kw'); index += important.length; continue; }
    const word = match(IDENT, text, index);
    if (word) {
      index += word.length;
      if (text[index] === '(') { fn = word; push(tokens, word, 'fn'); continue; }
      if (mode === 'header') { push(tokens, word, /keyframes$/i.test(atRule) ? 'param' : depth > 0 ? 'punct' : ''); continue; }
      push(tokens, word, word.startsWith('--') ? 'var' : CSS_VALUE.test(word) ? 'flag' : ''); continue;
    }
    if (mode === 'header' && character === ':') { push(tokens, ':', 'punct'); index++; continue; }
    push(tokens, character, '+*'.includes(character) || (character === '-' && depth > 0) || (character === '/' && depth > 0) ? 'op' : ''); index++;
  }
  return tokens;
}

/** A selector up to (not including) its `{`, `;` or `}`. */
function selector(tokens: Token[], text: string, index: number): number {
  while (index < text.length) {
    const character = text[index]!;
    if (character === '{' || character === '}' || character === ';' || text.startsWith('/*', index)) return index;
    const space = match(SPACE, text, index);
    if (space) { push(tokens, space, 'kw'); index += space.length; continue; }
    if (character === '.' || character === '#') {
      const name = match(IDENT, text, index + 1);
      push(tokens, character, 'punct'); push(tokens, name, character === '.' ? 'attr' : 'fn'); index += 1 + name.length; continue;
    }
    if (character === ':') {
      const colons = text.startsWith('::', index) ? '::' : ':', name = match(IDENT, text, index + colons.length);
      push(tokens, colons, 'punct'); push(tokens, name, 'op'); index += colons.length + name.length; continue;
    }
    if (character === '[') {
      const close = text.indexOf(']', index), end = close < 0 ? text.length : close + 1;
      push(tokens, '[', 'punct');
      const inner = text.slice(index + 1, close < 0 ? text.length : close);
      const parts = /^(\s*[^\s~|^$*=\]]+)(\s*[~|^$*]?=\s*)?(.*)$/s.exec(inner);
      if (parts) {
        push(tokens, parts[1]!, 'attr'); push(tokens, parts[2] ?? '', 'punct');
        const value = parts[3] ?? '';
        if (/^["']/.test(value)) cssString(tokens, value, 0); else push(tokens, value, 'str');
      } else push(tokens, inner, '');
      if (close >= 0) push(tokens, ']', 'punct');
      index = end; continue;
    }
    if (character === '*') { push(tokens, '*', 'tag'); index++; continue; }
    if ('>+~,()'.includes(character)) { push(tokens, character, 'punct'); index++; continue; }
    const name = match(IDENT, text, index);
    if (name) { push(tokens, name, 'tag'); index += name.length; continue; }
    push(tokens, character, 'kw'); index++;
  }
  return index;
}
