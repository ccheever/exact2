// The reference's code colours, from the pinned @pierre/highlights 0.2.0 (Apache-2.0), which the reference T3 Code
// (7c3b3b3605ef, lib/syntaxHighlighting.ts) highlights all code with: 80 languages, each a lexer in its
// dist/highlights.wasm; a language without one paints plain.
//
//   data/highlights.wasm                the lexers, which the data crate runs (data/src/highlights.rs)
//   data/LICENSE-highlights.md and data/NOTICE-highlights.md, beside them
//   src/highlight/highlights-tables.ts  each language name and alias with its lexer id (dist/languages.js), and
//                                       each token type's colour class (dist/token-types.js)
//   src/highlight/syntax-ink.contract   each class's ink, slant and weight (synColor, synSlant, synWeight)
//
// The inks: dark is the package's pierre-dark; light is the reference's t3-light (lib/readableSyntaxTheme.ts), each
// pierre-light colour darkened in 4% steps until it reaches 4.5:1 on a surface of luminance 0.69, and 6:1 for
// #1ca1c7, #636363 and #1a85d4. A token type takes its theme's style for its name or its nearest dotted parent
// (lib/theme.ts resolveThemeSyntax); the types whose two styles agree share one class.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const highlight = join(example, 'src', 'highlight');
const highlights = join(here, 'node_modules', '@pierre', 'highlights');
const dist = join(highlights, 'dist');

/** Each class by the first token type of its style group (a new type's group needs a name here). */
const CLASS_NAMES = {
  attribute: 'attr', boolean: 'num', comment: 'com', constant: 'const', constructor: 'type', 'diff.minus': 'del',
  'diff.plus': 'ins', emphasis: 'em', 'emphasis.strong': 'strong', function: 'fn', keyword: 'kw', 'keyword.operator': 'op',
  label: 'label', predictive: 'predict', primary: 'link', property: 'var', punctuation: 'punct', selector: 'tag',
  string: 'str', 'string.escape': 'esc', 'string.regex': 'regex', title: 'title', 'variable.parameter': 'param',
};

/** lib/theme.ts resolveThemeSyntax: the nearest scope's font settings with the nearest colour, by dotted parents. */
function resolveSyntax(syntax, name) {
  let settings;
  for (let key = name; key !== '';) {
    const value = syntax[key];
    if (typeof value === 'string') return { ...settings, color: value };
    if (value != null && typeof value === 'object') {
      settings ??= value;
      if (value.color != null) return { ...settings, color: value.color };
    }
    const dot = key.lastIndexOf('.');
    key = dot < 0 ? '' : key.slice(0, dot);
  }
  return settings;
}
const foreground = theme => (theme.style['editor.foreground'] ?? theme.style.foreground).toLowerCase();
/** lib/theme.ts resolveThemeStyle, for a syntax type: its colour (the foreground when only a font style is set), whether
 *  it is italic, and its weight (0 for none). */
function styleOf(theme, name, ink = color => color) {
  if (name === 'foreground' || name === 'background') return { color: foreground(theme), italic: false, weight: 0 };
  const { color, font_style: fontStyle, font_weight: fontWeight } = resolveSyntax(theme.style.syntax ?? {}, name) ?? {};
  return { color: ink((color ?? foreground(theme)).toLowerCase()), italic: fontStyle === 'italic', weight: fontWeight ?? 0 };
}

// lib/readableSyntaxTheme.ts
const channel = value => { const c = value / 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
const contrast = rgb => (0.69 + 0.05) / (0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]) + 0.05);
const FURTHER = new Set(['#1ca1c7', '#636363', '#1a85d4']);
/** readableSyntaxColor: the t3-light colour for a pierre-light colour. */
export function readableSyntaxColor(color) {
  const match = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(color);
  if (!match) return color;
  const min = FURTHER.has(color.toLowerCase()) ? 6 : 4.5;
  let rgb = [0, 2, 4].map(start => Number.parseInt(match[1].slice(start, start + 2), 16));
  if (contrast(rgb) >= min) return color;
  while (contrast(rgb) < min) rgb = rgb.map(value => Math.floor(value * 0.96));
  return `#${rgb.map(value => value.toString(16).padStart(2, '0')).join('')}${match[2] ?? ''}`;
}

/** Each token type's class, and each class's light and dark ink, slant and weight. */
export async function classes() {
  const types = (await import(join(dist, 'token-types.js'))).default;
  const dark = (await import(join(dist, 'themes', 'pierre-dark.js'))).default;
  const light = (await import(join(dist, 'themes', 'pierre-light.js'))).default;
  const plain = { light: styleOf(light, 'foreground'), dark: styleOf(dark, 'foreground') };
  const groups = new Map(), typeClasses = [];
  for (const name of types) {
    const style = { light: styleOf(light, name, readableSyntaxColor), dark: styleOf(dark, name) };
    const italic = style.light.italic || style.dark.italic, weight = Math.max(style.light.weight, style.dark.weight);
    const isPlain = style.light.color === readableSyntaxColor(plain.light.color) && style.dark.color === plain.dark.color && !italic && !weight;
    const key = isPlain || name === 'background' ? '' : `${style.light.color} ${style.dark.color} ${italic} ${weight}`;
    if (!groups.has(key)) {
      const cls = key === '' ? '' : CLASS_NAMES[name];
      if (cls === undefined) throw new Error(`@pierre/highlights' token type ${name} starts a style group with no class name (CLASS_NAMES)`);
      groups.set(key, { cls, light: style.light.color, dark: style.dark.color, italic, weight });
    }
    typeClasses.push(groups.get(key).cls);
  }
  const styled = [...groups.values()].filter(group => group.cls !== '');
  return { typeClasses, styled, plain: { light: plain.light.color, dark: plain.dark.color } };
}

const header = (comment, text) => `// Generated by tools/generate/gen-highlights.mjs from @pierre/highlights ${version()} (Apache-2.0;\n// data/LICENSE-highlights.md). ${comment}\n${text}`;
const version = () => JSON.parse(readFileSync(join(highlights, 'package.json'), 'utf8')).version;

export async function generate() {
  const languages = (await import(join(dist, 'languages.js'))).default;
  const { typeClasses, styled, plain } = await classes();
  const ids = Object.entries(languages).sort(([a], [b]) => (a < b ? -1 : 1)).map(([name, id]) => `  ${JSON.stringify(name)}: ${id},`);
  const tables = header('Do not edit.',
    `/** Each language name and alias a lexer has, lower-cased, with its lexer id; 0 is plain text (dist/languages.js). */\n`
    + `export const LEXER_IDS: Readonly<Record<string, number>> = {\n${ids.join('\n')}\n};\n`
    + `/** Each token type's colour class by its index (dist/token-types.js): syntax-ink.contract paints it. */\n`
    + `export const TYPE_CLASSES: readonly string[] = ${JSON.stringify(typeClasses)};\n`);
  // One list of `class=ink` entries, the plain ink first (the class ""), read by prefix: inlined at every call, it costs
  // one list where a chain of comparisons would cost one per class.
  const entries = [`"=light-dark(${readableSyntaxColor(plain.light)}, ${plain.dark})"`, ...styled.map(group => `"${group.cls}=light-dark(${group.light}, ${group.dark})"`)];
  const named = list => `[${list.map(group => JSON.stringify(group.cls)).join(', ')}]`;
  const weight = Math.max(...styled.map(group => group.weight));
  const contract = header('Each colour class of highlights-tables.ts, as t3-light and pierre-dark paint it.',
    `fn synColor(cls: string): string = [${entries.join(', ')}].filter((entry) => entry.startsWith(\`\${cls}=\`)).join("").slice(cls.length + 1)\n`
    + `fn synSlant(cls: string): string = ${named(styled.filter(group => group.italic))}.includes(cls) ? "italic" : "normal"\n`
    + `fn synWeight(cls: string): number = ${named(styled.filter(group => group.weight === weight))}.includes(cls) ? ${weight} : 400\n`);
  return [
    { file: join(example, 'data', 'highlights.wasm'), text: readFileSync(join(dist, 'highlights.wasm')) },
    { file: join(example, 'data', 'LICENSE-highlights.md'), text: readFileSync(join(highlights, 'LICENSE.md'), 'utf8') },
    { file: join(example, 'data', 'NOTICE-highlights.md'), text: readFileSync(join(highlights, 'NOTICE.md'), 'utf8') },
    { file: join(highlight, 'highlights-tables.ts'), text: tables },
    { file: join(highlight, 'syntax-ink.contract'), text: contract },
  ];
}

/** The Shiki path's generated grammars and word lists, which nothing writes any more. */
export function stale() {
  if (!existsSync(highlight)) return [];
  return readdirSync(highlight).filter(name => /^grammar(-\d+)?\.ts$|^css-words\.ts$/.test(name)).map(name => join(highlight, name));
}
