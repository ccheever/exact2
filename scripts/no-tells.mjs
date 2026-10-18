#!/usr/bin/env bun
/**
 * no-tells — an app's `.contract` files say no colour, font size or font weight
 * of their own (LLP 1115, "Write the web, ship the platform").
 *
 *   bun scripts/no-tells.mjs <app name | app directory | file.contract | guide.md …>
 *
 * A Markdown file is read for its fenced `contract` blocks, as the recipes are
 * (docs/recipes, LLP 1116 D3: a recipe builders copy carries no tells).
 *
 * A literal colour (`#8e8e93`, `rgb(…)`, `hsl(…)`, `red`) on a colour property, or
 * a literal `font-size`/`font-weight`, is a value copied from one platform's
 * screenshot: it misses dark mode, Increased Contrast, Dynamic Type and the next
 * OS. Leave it unsaid, or name what it is: a colour role (`CanvasText`,
 * `AccentColor`, `-exact-secondary-label`, `-exact-separator`,
 * `-exact-system-red`), a heading (`role="heading" aria-level=N`) or a text style
 * (`font="-exact-footnote"`, `font-size="-exact-caption1"`). Layout (sizes,
 * padding, gaps, radii) is CSS and the author's, and is not looked at.
 *
 * The one authored colour is the accent (LLP 1116.002 D1): a role, or one
 * `light-dark(light, dark)` pair on `accent-color`, which is not flagged. A pair
 * anywhere else is, as a bare literal is everywhere: the compiler's
 * `lower-colour-one-scheme` warning names the bare ones (outside `svg`), so what
 * passes here never earns it.
 *
 * The app's `theme` (LLP 1095 D4) is where its colours are said: its entries'
 * literals are the palette, not tells, and a colour property naming an entry
 * (`color="brand"`, `light-dark(brand, ink)`) names what the colour is. Lowering
 * refuses an entry spelled as a CSS colour name, so a name here is never one.
 * So is an app's design (`app.tamber`, or the file `app.json`'s `"tamber"`
 * names, LLP 1121): a token name is not a literal, and neither is an
 * expression over tokens and roles (`color-mix(in oklab, accent.fill 40%,
 * surface.card)`, `rgb(from surface.card r g b / 0.5)`), which the build folds.
 *
 * A numeric readout is design, not a tell (LLP 1116 D3): a literal `font-size` on
 * a node that also sets `font-variant-numeric="tabular-nums"` (a timer's or a
 * total's big number) is not flagged. A line that must keep a literal ends with
 * `// no-tells: <why>`. `overflow-wrap="anywhere"` on prose is one too (LLP 1116.021
 * D6, `brokenWordsIn`). Exit 0 when there is nothing to say, 1 with every
 * finding, 2 on a bad invocation. `apps/shelf` is held to it by its
 * `app.test.ts`.
 */

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

const ROOT = resolve(import.meta.dir, '..');

// CSS's named colours (CSS Color 4 §6.1), which a colour property would paint.
const NAMED = new Set(`aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue
blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan
darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen
darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey
darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite
forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink
indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral
lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen
lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta
maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue
mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin
navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen
paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red
rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue
slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white
whitesmoke yellow yellowgreen`.split(/\s+/));

// A property whose value is (or holds) a colour: `color`, `background-color`,
// `border`, `border-top-color`, `outline`, `-exact-tint-color`, `accent-color`,
// `caret-color`, `fill`, `stroke`, `box-shadow`, `text-shadow`, `column-rule`,
// `stop-color`, `text-decoration-color`, `background` …
const COLOUR_PROPERTY = /^(-exact-)?(.*-)?(color|colour|background|border(-(top|right|bottom|left|block|inline)(-(start|end))?)?|outline|fill|stroke|shadow|column-rule)$/;
const COLOUR_FUNCTION = /\b(rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/i;
const HEX = /(^|[^\w&-])#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})\b/i;
// A text style, the only value a font property may name here.
const TEXT_STYLE = /^(-exact-|-apple-system-)[a-z0-9-]+$/;

/** The source with `//` comments blanked, strings kept, offsets unchanged. */
function uncomment(text) {
  let out = '', quote = null;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quote) {
      out += c;
      if (c === '\\') { out += text[++i] ?? ''; continue; }
      if (c === quote) quote = null;
      else if (c === '\n' && quote === '"') quote = null;
    } else if (c === '"' || c === '`') { quote = c; out += c; }
    else if (c === '/' && text[i + 1] === '/') { while (i < text.length && text[i] !== '\n') { out += ' '; i++; } if (i < text.length) out += '\n'; }
    else out += c;
  }
  return out;
}

/** The source with the root `theme` block's lines blanked, offsets unchanged:
 * its literals are the app's palette (LLP 1095 D4). */
function withoutTheme(text) {
  // Only a line that starts outside any bracket or string is a
  // declaration's: a `theme` word continuing an expression is not one.
  let inTheme = false, depth = 0, quote = null;
  return text.split('\n').map(line => {
    const top = depth === 0 && quote === null;
    for (let i = 0; i < line.length; i++) {
      const c = line[i];
      if (quote) { if (c === '\\') i++; else if (c === quote) quote = null; }
      else if (c === '"' || c === '`') quote = c;
      else if ('([{'.includes(c)) depth++;
      else if (')]}'.includes(c)) depth = Math.max(0, depth - 1);
    }
    if (quote === '"') quote = null;
    if (top && /^theme\s*$/.test(line)) { inTheme = true; return line; }
    if (inTheme && (line.trim() === '' || /^\s/.test(line))) return line.replace(/\S/g, ' ');
    inTheme = false;
    return line;
  }).join('\n');
}

/** The root `theme` block's entry names, as `withoutTheme` finds the block. */
function themeNames(text) {
  const blank = withoutTheme(text).split('\n'), names = [];
  text.split('\n').forEach((line, i) => {
    const entry = line !== blank[i] && /^\s+([^\s=]+)\s*=/.exec(line);
    if (entry) names.push(entry[1]);
  });
  return names;
}

/** Each `name=value` attribute: the value a quoted string, a template, a
 * balanced parenthesised expression or a bare word. */
function* attributes(text) {
  const start = /(^|[\s(])(-?[a-zA-Z][\w-]*)=(?!=)/g;
  let m;
  while ((m = start.exec(text))) {
    const name = m[2], from = m.index + m[0].length;
    let i = from, depth = 0, quote = null;
    for (; i < text.length; i++) {
      const c = text[i];
      if (quote) { if (c === '\\') i++; else if (c === quote) { quote = null; if (!depth) { i++; break; } } continue; }
      if (c === '"' || c === '`') { quote = c; continue; }
      if (c === '(') depth++;
      else if (c === ')') { if (!depth) break; if (!--depth) { i++; break; } }
      else if (!depth && /\s/.test(c)) break;
    }
    yield { name, value: text.slice(from, i), at: m.index + m[1].length };
    start.lastIndex = Math.max(i, start.lastIndex);
  }
}

/** The string literals in a value (a bare word is one). */
function literals(value) {
  if (!/["`]/.test(value)) return /^[a-zA-Z#-]/.test(value) ? [value] : [];
  return [...value.matchAll(/"((?:[^"\\]|\\.)*)"|`((?:[^`\\]|\\.)*)`/g)].map(m => m[1] ?? m[2]);
}

/** The literal with each balanced `light-dark(…)` removed: the accent's pair. */
function withoutPairs(literal) {
  let out = literal, at;
  while ((at = out.search(/light-dark\(/i)) >= 0) {
    let depth = 0, end = at;
    for (; end < out.length; end++) {
      if (out[end] === '(') depth++;
      else if (out[end] === ')' && !--depth) break;
    }
    out = out.slice(0, at) + ' ' + out.slice(end + 1);
  }
  return out;
}

// A role, which a relative colour may take its colour from (`from CanvasText`):
// Exact's, or a CSS system colour, as roles match, without case.
const SYSTEM = new Set('canvas canvastext linktext visitedtext activetext buttonface buttontext buttonborder field fieldtext highlight highlighttext selecteditem selecteditemtext mark marktext graytext accentcolor accentcolortext'.split(' '));
const isRole = word => /^(-exact-|-apple-system-)[a-z0-9-]+$/i.test(word) || SYSTEM.has(word.toLowerCase());
// Each relative colour function's channel keywords (CSS Color 5 §4).
const KEYWORDS = { rgb: 'r g b', rgba: 'r g b', hsl: 'h s l', hsla: 'h s l', hwb: 'h w b', lab: 'l a b', lch: 'l c h', oklab: 'l a b', oklch: 'l c h', color: 'r g b x y z' };
/** The balanced text from `(` at `at`'s matching `)`, inclusive, or null. */
const balanced = (text, at) => { let depth = 0; for (let i = at; i < text.length; i++) { if (text[i] === '(') depth++; else if (text[i] === ')' && !--depth) return text.slice(at, i + 1); } return null; };

/** Whether the colour function whose `(` ends at `at` in `text` is relative to a
 * token or a role and keeps its channels (a keyword, or an expression over
 * one: `calc(l * 0.9)`): then the build folds it from the name. A literal
 * channel (`rgb(from brand 255 0 0)`) is a colour of its own. */
function relativeTo(text, fn, at, tokens) {
  // The origin: a token, a role, or an expression over them (`color-mix(…)`).
  const head = /^\s*from\s+([^\s,()]+)/i.exec(text.slice(at));
  if (!head) return false;
  let origin = head[0].length;
  if (text[at + origin] === '(') {
    const inner = balanced(text, at + origin);
    const words = (inner ?? '').split(/[\s,()]+/).filter(Boolean);
    if (!inner || colourIn(head[1] + inner, tokens) || !words.some(w => tokens.has(w) || isRole(w))) return false;
    origin += inner.length;
  } else if (!(tokens.has(head[1]) || isRole(head[1]))) return false;
  const keys = new Set([...KEYWORDS[fn.toLowerCase()].split(' '), 'alpha']);
  let rest = text.slice(at + origin).replace(/^\s+/, '');
  if (fn.toLowerCase() === 'color') rest = rest.replace(/^[\w-]+\s*/, '');
  // The channels, to `/` or the function's `)`, at its own depth.
  const words = [];
  let depth = 0, word = '';
  for (const c of rest) {
    if (depth === 0 && (c === ')' || c === '/')) break;
    if (c === '(') depth++;
    if (c === ')') depth--;
    if (depth === 0 && /\s/.test(c)) { if (word) words.push(word); word = ''; } else word += c;
  }
  if (word) words.push(word);
  return words.length > 0 && words.every(w => keys.has(w.toLowerCase()) || w.includes('(') && w.split(/[^a-z]+/i).some(k => keys.has(k.toLowerCase())));
}

function colourIn(literal, tokens = new Set()) {
  if (HEX.test(literal)) return '#' + literal.match(HEX)[2];
  // A colour function relative to a token or a role is an expression over it.
  // Only the outermost: one inside an origin is the origin's to answer.
  const outer = [];
  for (const m of literal.matchAll(new RegExp(COLOUR_FUNCTION.source, 'gi'))) if (!outer.some(o => m.index < o.end)) outer.push({ m, end: m.index + (balanced(literal, m.index + m[0].length - 1)?.length ?? 0) + m[0].length - 1 });
  const relative = outer.every(({ m }) => relativeTo(literal, m[1], m.index + m[0].length, tokens));
  if (COLOUR_FUNCTION.test(literal) && !relative) return literal.match(COLOUR_FUNCTION)[0] + '…)';
  const named = literal.split(/[\s,()]+/).find(word => NAMED.has(word.toLowerCase()));
  return named ?? null;
}

/** Each line's node, as the index of the line that starts it: a node's attributes
 * continue on deeper lines that start with `name=`, and a `style` block's rows
 * belong to it the same way. */
function nodeHeads(lines) {
  const heads = [];
  let head = 0, depth = -1;
  lines.forEach((line, i) => {
    const indent = line.length - line.trimStart().length;
    if (i > 0 && indent > depth && /^-?[a-zA-Z][\w-]*=(?!=)/.test(line.trim())) heads.push(head);
    else { head = i; depth = indent; heads.push(i); }
  });
  return heads;
}

/** Every tell in one `.contract` source: `{ line, column, property, value, kind }`;
 * `tokens`, the app design's token names. */
export function tellsIn(source, tokens = new Set()) {
  const text = withoutTheme(uncomment(source)), lines = source.split('\n'), found = [];
  // The root `theme` block's entries are tokens too (LLP 1095 D4, LLP 1121 D2).
  tokens = new Set([...tokens, ...themeNames(uncomment(source))]);
  // The nodes that are numeric readouts: their literal `font-size` is design.
  const heads = nodeHeads(text.split('\n'));
  const readouts = new Set(text.split('\n').flatMap((line, i) => /(^|\s)font-variant-numeric=["`]?tabular-nums\b/.test(line) ? [heads[i]] : []));
  const lineStarts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === '\n') lineStarts.push(i + 1);
  const locate = offset => { let l = 0; while (l + 1 < lineStarts.length && lineStarts[l + 1] <= offset) l++; return [l + 1, offset - lineStarts[l] + 1]; };
  for (const { name, value, at } of attributes(text)) {
    const [line, column] = locate(at);
    if (/\/\/\s*no-tells:\s*\S/.test(lines[line - 1] ?? '')) continue;
    const property = name.toLowerCase();
    if (property === 'font-size' && readouts.has(heads[line - 1])) continue;
    if (property === 'font-size' || property === 'font-weight' || property === 'font') {
      const words = literals(value);
      const computed = !words.length && value !== '';
      const bad = computed ? value : words.find(word => !TEXT_STYLE.test(word.trim()) && !['inherit', 'unset'].includes(word.trim()));
      if (bad !== undefined) found.push({ line, column, property: name, value, kind: 'type' });
    } else if (COLOUR_PROPERTY.test(property)) {
      for (const literal of literals(value)) {
        const colour = colourIn(property === 'accent-color' ? withoutPairs(literal) : literal, tokens);
        if (colour) { found.push({ line, column, property: name, value, kind: 'colour', colour }); break; }
      }
    }
  }
  for (const b of brokenWordsIn(source)) found.push({ line: b.line, column: b.column, property: 'overflow-wrap', value: b.value, kind: 'break' });
  return found.sort((a, b) => a.line - b.line || a.column - b.column);
}

// LLP 1116.020 D4: a font size that stays put as the reader's text size grows, which `verify` names: a bare number, px or pt,
// whole or as a ternary's branch. `tabular-nums` excuses none of it (a readout's designed size is a rem one), and a native
// control's own field sizes its text as the platform does. Exact's parts fixed on purpose (`RoundButton`'s title) are the
// library's, not the app's files.
const CONTROL_TAGS = new Set(['input', 'select', 'textarea', 'progress', 'meter']);
function fixedSize(value) {
  const sizes = literals(value).flatMap(word => { const m = /^(\d+(?:\.\d+)?)(px|pt)?$/.exec(word.trim()); return m ? [Number(m[1])] : []; });
  const bare = value.replace(/"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`/g, '""');
  const whole = /^\(?\s*(\d+(?:\.\d+)?)\s*\)?$/.exec(bare.trim());
  if (whole) sizes.push(Number(whole[1]));
  else for (const m of bare.matchAll(/[?:]\s*(\d+(?:\.\d+)?)\s*(?=[):]|$)/g)) sizes.push(Number(m[1]));
  return sizes.length ? Math.max(...sizes) : null;
}

/** Each fixed font size in one `.contract` source: `{ line, column, tag, value, size, testId }`, `testId` the node's literal one. */
export function fixedSizesIn(source) {
  const text = withoutTheme(uncomment(source)), rows = text.split('\n'), lines = source.split('\n'), heads = nodeHeads(rows), found = [];
  const lineStarts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === '\n') lineStarts.push(i + 1);
  const locate = offset => { let l = 0; while (l + 1 < lineStarts.length && lineStarts[l + 1] <= offset) l++; return [l + 1, offset - lineStarts[l] + 1]; };
  for (const { name, value, at } of attributes(text)) {
    if (name.toLowerCase() !== 'font-size') continue;
    const [line, column] = locate(at), head = heads[line - 1], tag = rows[head].trim().split(/\s+/)[0];
    if (/\/\/\s*no-tells:\s*\S/.test(lines[line - 1] ?? '') || CONTROL_TAGS.has(tag)) continue;
    const size = fixedSize(value);
    if (size == null) continue;
    let testId = null;
    for (let i = head; i < rows.length && (i === head || heads[i] === head) && testId == null; i++) testId = /(?:^|\s)testId=(?:"([^"]*)"|`([^`$]*)`)/.exec(rows[i])?.slice(1).find((v) => v != null) ?? null;
    found.push({ line, column, tag, value, size, testId });
  }
  return found;
}

/** A node's literal `testId`, read from its head line and the attribute lines under it. */
function testIdOf(rows, heads, head) {
  for (let i = head; i < rows.length && (i === head || heads[i] === head); i++) {
    const id = /(?:^|\s)testId=(?:"([^"]*)"|`([^`$]*)`)/.exec(rows[i])?.slice(1).find((v) => v != null);
    if (id != null) return id;
  }
  return null;
}

/** Each attribute of `source` with what `verify` needs to place it: `{ name, value, line, column, head, tag }`. */
function placed(source) {
  const text = withoutTheme(uncomment(source)), rows = text.split('\n'), lines = source.split('\n'), heads = nodeHeads(rows);
  const lineStarts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === '\n') lineStarts.push(i + 1);
  const locate = offset => { let l = 0; while (l + 1 < lineStarts.length && lineStarts[l + 1] <= offset) l++; return [l + 1, offset - lineStarts[l] + 1]; };
  const out = [];
  for (const { name, value, at } of attributes(text)) {
    const [line, column] = locate(at), head = heads[line - 1];
    if (/\/\/\s*no-tells:\s*\S/.test(lines[line - 1] ?? '')) continue;
    out.push({ name: name.toLowerCase(), value, line, column, head, tag: rows[head].trim().split(/\s+/)[0] });
  }
  return { out, rows, heads };
}

// @ref LLP 1116.021 D6 — `overflow-wrap="anywhere"` lets a line break inside any word, and lets its box shrink below its
// longest word, so prose, a number or a date breaks mid-word at the largest text size ("Octo/ber", "$15.2/1"): no
// hand-built UIKit label does. It is right for a URL or a long token, which this reads by structure: on a `link`, or on text
// set in a monospace face (a password, a hash, a key). Anything else that must keep it says why with `// no-tells: <why>`.
// `overflow-wrap="break-word"` breaks only a word too long for a line of its own, and is not named.
/** Each `overflow-wrap="anywhere"` that breaks prose: `{ line, column, tag, value, testId }`. */
export function brokenWordsIn(source) {
  const { out: attrs, rows, heads } = placed(source);
  const mono = new Set(attrs.filter(a => (a.name === 'font-family' || a.name === 'font') && literals(a.value).some(w => /monospace|-exact-monospace/i.test(w))).map(a => a.head));
  return attrs
    .filter(a => a.name === 'overflow-wrap' && literals(a.value).some(w => w.trim() === 'anywhere') && a.tag !== 'link' && !mono.has(a.head))
    .map(a => ({ line: a.line, column: a.column, tag: a.tag, value: a.value, testId: testIdOf(rows, heads, a.head) }));
}

// @ref LLP 1116.021 D6 — a symbol `image` sized in points beside text that grows stays small as Dynamic Type grows, as a
// fixed `font-size` does: an unsized symbol follows its text, and one sized by a `font-size` in rem grows with the root.
// "Beside text" is a `text` in the same parent box, at any depth, whose size is not fixed itself. A symbol alone (an
// icon-only button, a hero glyph) is the author's size; Exact's parts and the platform's controls are not the app's files.
/** Each symbol `width` or `height` in px, pt or a bare number beside text that scales: `{ line, column, property, value, size, testId }`. */
export function fixedSymbolsIn(source) {
  const { out: attrs, rows, heads } = placed(source);
  const indent = i => rows[i].length - rows[i].trimStart().length;
  const fixedFont = new Set(attrs.filter(a => a.name === 'font-size' && fixedSize(a.value) != null).map(a => a.head));
  const symbol = head => /^image\s+(?:"symbol:|`symbol:|\(.*"symbol:)/.test(rows[head].trim());
  // A text that scales beside the image: a sibling, or a line of a sibling `column` (a title over a subtitle). `if`, `else`,
  // `for` and `match` lines stand between nodes without being one.
  const flow = i => /^(if|else|for|match|case)\b/.test(rows[i].trim());
  const besideText = head => {
    let parent = head - 1;
    while (parent >= 0 && (heads[parent] !== parent || !rows[parent].trim() || flow(parent) || indent(parent) >= indent(head))) parent--;
    if (parent < 0) return false;
    const path = [];
    for (let i = parent + 1; i < rows.length; i++) {
      if (!rows[i].trim() || heads[i] !== i) continue;
      if (indent(i) <= indent(parent)) break;
      while (path.length && indent(path.at(-1)) >= indent(i)) path.pop();
      const nodes = path.filter(p => !flow(p));
      const scales = /^text\b/.test(rows[i].trim()) && !fixedFont.has(i);
      if (i !== head && scales && (nodes.length === 0 || (nodes.length === 1 && /^column\b/.test(rows[nodes[0]].trim())))) return true;
      path.push(i);
    }
    return false;
  };
  return attrs
    .filter(a => (a.name === 'width' || a.name === 'height') && a.tag === 'image' && symbol(a.head) && fixedSize(a.value) != null && besideText(a.head))
    .map(a => ({ line: a.line, column: a.column, property: a.name, value: a.value, size: fixedSize(a.value), testId: testIdOf(rows, heads, a.head) }));
}

/** The `.contract` files an argument names: an app (by name under apps/ or by
 * directory, recursively, skipping node_modules and build output) or files
 * (a Markdown file stands for its `contract` blocks). */
export function contractFiles(target) {
  const dir = existsSync(target) ? resolve(target) : resolve(ROOT, 'apps', target);
  if (!existsSync(dir)) throw new Error(`no app or file ${target}`);
  if (statSync(dir).isFile()) return [dir];
  const out = [];
  const walk = d => {
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      if (entry.name.startsWith('.') || ['node_modules', 'target', 'dist'].includes(entry.name)) continue;
      const path = join(d, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (entry.name.endsWith('.contract')) out.push(path);
    }
  };
  walk(dir);
  return out.sort();
}

/** A Markdown document's fenced `contract` blocks, each with the line before its body. */
function contractBlocks(text) {
  const blocks = [];
  let open = null;
  text.split('\n').forEach((line, i) => {
    if (open) { if (line.trim() === '```') { blocks.push(open); open = null; } else open.lines.push(line); }
    else if (line.startsWith('```')) open = line.slice(3).trim() === 'contract' ? { at: i + 1, lines: [] } : { skip: true, lines: [] };
  });
  return blocks.filter(b => !b.skip).map(b => ({ at: b.at, source: b.lines.join('\n') }));
}

/** The token names of the design beside `file`'s app (LLP 1121 D2): the
 * `"tamber"` file `app.json` names, else `app.tamber`, in the nearest
 * directory up that has either. Its `theme` block's entries, each `name = …`
 * at the block's own indent, with or without a kind (`name: color = …`). */
export function designTokens(file) {
  for (let dir = resolve(file, '..'); ; dir = resolve(dir, '..')) {
    let named = null;
    try { named = JSON.parse(readFileSync(join(dir, 'app.json'), 'utf8')).tamber ?? null; } catch {}
    const path = named ? join(dir, named) : join(dir, 'app.tamber');
    if (existsSync(path)) {
      // The block's entries share one indent, its first's; a `when` line is deeper.
      const names = new Set();
      let inTheme = false, indent = null;
      for (const line of uncomment(readFileSync(path, 'utf8')).split('\n')) {
        if (/^\S/.test(line)) { inTheme = /^theme\s*$/.test(line); indent = null; continue; }
        const entry = inTheme && /^(\s+)([^\s=:]+)\s*(?::\s*[\w-]+\s*)?=/.exec(line);
        if (!entry) continue;
        indent ??= entry[1];
        if (entry[1] === indent) names.add(entry[2]);
      }
      return names;
    }
    // The app's directory is the last one looked in.
    if (named || existsSync(join(dir, 'app.json')) || dir === ROOT || dir === resolve(dir, '..')) break;
  }
  return new Set();
}

/** Every tell in an app's (or a file's) Contract, each with its file; a Markdown
 * file's lines are the document's. */
export function tells(target) {
  return contractFiles(target).flatMap(file => file.endsWith('.md')
    ? contractBlocks(readFileSync(file, 'utf8')).flatMap(b => tellsIn(b.source).map(t => ({ file, ...t, line: t.line + b.at })))
    : tellsIn(readFileSync(file, 'utf8'), designTokens(file)).map(t => ({ file, ...t })));
}

export const ADVICE = `These are tells (LLP 1115, "Write the web, ship the platform"): values one platform's
screenshot had, which miss dark mode, Increased Contrast, Dynamic Type and the next OS.
- Colour: leave it unsaid (text, tint, backgrounds, separators and controls are the
  platform's), or name a role: CanvasText, AccentColor, -exact-secondary-label,
  -exact-separator, -exact-fill, -exact-grouped-background, -exact-system-red …
  Leave the accent unsaid too (the platform's); if written, a role or one
  light-dark(light, dark) pair on accent-color (docs/contract-for-agents.md, "Colours:
  say a role, not a value").
  An app's own palette is its design (app.tamber, LLP 1121) or its root \`theme\`
  block, and rows name its tokens.
- Type: leave body text unsaid; say a heading (h1 .. h4) or
  a text style (font="-exact-footnote", -exact-headline, -exact-caption1 …)
  (docs/contract-for-agents.md, "Type: say a heading, not a size").
- Breaks: overflow-wrap="anywhere" breaks inside words (Octo/ber); wrap between words (the
  default) or give the row room. It is for a URL or a long token: on a link, or in a
  monospace face.
apps/shelf is the recipe. A line that must keep a literal says why with
\`// no-tells: <why>\`.`;

if (import.meta.main) {
  const targets = process.argv.slice(2);
  if (!targets.length) {
    console.error('usage: bun scripts/no-tells.mjs <app name | app directory | file.contract …>');
    process.exit(2);
  }
  let found;
  try { found = targets.flatMap(tells); }
  catch (error) { console.error(`no-tells: ${error.message}`); process.exit(2); }
  for (const t of found) {
    const what = t.kind === 'colour' ? `a literal colour (${t.colour})` : t.kind === 'break' ? 'breaks inside words (Octo/ber)' : 'a font size or weight that is not a text style';
    console.log(`${relative(process.cwd(), t.file)}:${t.line}:${t.column}: ${t.property}=${t.value}: ${what}`);
  }
  if (found.length) {
    console.log(`\n${found.length} tell${found.length === 1 ? '' : 's'}. ${ADVICE}`);
    process.exit(1);
  }
  const files = targets.flatMap(contractFiles).length;
  console.log(`no-tells: ${files} file${files === 1 ? '' : 's'} of Contract, no literal colour, font size or weight`);
}
