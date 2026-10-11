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
 * A numeric readout is design, not a tell (LLP 1116 D3): a literal `font-size` on
 * a node that also sets `font-variant-numeric="tabular-nums"` (a timer's or a
 * total's big number) is not flagged. A line that must keep one (a brand colour
 * the platform has no role for) ends with `// no-tells: <why>`. Exit 0 when there is nothing to say, 1 with every
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

function colourIn(literal) {
  if (HEX.test(literal)) return literal.match(HEX)[0].trim();
  if (COLOUR_FUNCTION.test(literal)) return literal.match(COLOUR_FUNCTION)[0] + '…)';
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

/** Every tell in one `.contract` source: `{ line, column, property, value, kind }`. */
export function tellsIn(source) {
  const text = uncomment(source), lines = source.split('\n'), found = [];
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
        const colour = colourIn(literal);
        if (colour) { found.push({ line, column, property: name, value, kind: 'colour', colour }); break; }
      }
    }
  }
  return found;
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

/** Every tell in an app's (or a file's) Contract, each with its file; a Markdown
 * file's lines are the document's. */
export function tells(target) {
  return contractFiles(target).flatMap(file => file.endsWith('.md')
    ? contractBlocks(readFileSync(file, 'utf8')).flatMap(b => tellsIn(b.source).map(t => ({ file, ...t, line: t.line + b.at })))
    : tellsIn(readFileSync(file, 'utf8')).map(t => ({ file, ...t })));
}

export const ADVICE = `These are tells (LLP 1115, "Write the web, ship the platform"): values one platform's
screenshot had, which miss dark mode, Increased Contrast, Dynamic Type and the next OS.
- Colour: leave it unsaid (text, tint, backgrounds, separators and controls are the
  platform's), or name a role: CanvasText, AccentColor, -exact-secondary-label,
  -exact-separator, -exact-fill, -exact-grouped-background, -exact-system-red …
  (docs/contract-for-agents.md, "Colours: say a role, not a value").
- Type: leave body text unsaid; say a heading (text role="heading" aria-level=1..4) or
  a text style (font="-exact-footnote", -exact-headline, -exact-caption1 …)
  (docs/contract-for-agents.md, "Type: say a heading, not a size").
apps/shelf is the recipe. A brand colour with no role keeps its line with
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
    const what = t.kind === 'colour' ? `a literal colour (${t.colour})` : 'a font size or weight that is not a text style';
    console.log(`${relative(process.cwd(), t.file)}:${t.line}:${t.column}: ${t.property}=${t.value}: ${what}`);
  }
  if (found.length) {
    console.log(`\n${found.length} tell${found.length === 1 ? '' : 's'}. ${ADVICE}`);
    process.exit(1);
  }
  const files = targets.flatMap(contractFiles).length;
  console.log(`no-tells: ${files} file${files === 1 ? '' : 's'} of Contract, no literal colour, font size or weight`);
}
