// The clone's icon data, generated from the packages the reference T3 Code (1e2ecbd975) draws its
// icons with, pinned in package.json and bun.lock at the integrity values the reference locks:
// Lucide (lucide 0.564.0 for each icon's geometry, lucide-react 0.564.0 for the names
// `lucide-react/dynamic` offers, aliases included; ISC) and Pierre's file icons (@pierre/trees
// 1.0.0-beta.4, Apache-2.0: the "complete" sprite, its colour per token, its file-name and extension
// tables). generate.mjs writes the files; see LICENSE-T3.
//
//   settings-b-lucide.ts          every Lucide icon as one 24-grid stroke path (the project icon picker)
//   pages-icons.contract          the pages' Lucide glyphs (PAGE_GLYPHS) and two of their own, one path each
//   timeline-files.contract       FileTypeIcon: each Pierre icon's paths, opacities, fill rules and tint
//   timeline-files-tables.ts      Pierre's file-name and extension tables (timeline-files.ts resolves with them)
//   macos/pierre-icons.tsv        the same tables for the Markdown module's file chips (pierre_icons.rs)
//   assets/composer-icons.json    the composer's chip icons as SVG (T3ComposerIcons.swift draws them)

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const example = join(here, '..', '..');
const modules = join(here, 'node_modules');
const read = (path) => readFileSync(join(modules, path), 'utf8');
const version = (name) => JSON.parse(read(`${name}/package.json`)).version;

// ── Lucide ───────────────────────────────────────────────────────────────────────────────────
/** Every name `lucide-react/dynamic` offers (aliases included), sorted, with the icon file it loads. */
function lucideNames() {
  const source = read('lucide-react/dist/esm/dynamicIconImports.js');
  const pairs = [...source.matchAll(/^\s+"([^"]+)": \(\) => import\('\.\/icons\/([^']+)\.js'\),?$/gm)].map(m => [m[1], m[2]]);
  if (pairs.length < 1500) throw new Error(`lucide-react's dynamicIconImports.js lists ${pairs.length} icons`);
  return new Map(pairs.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
}
const nodes = new Map();
/** An icon's node list ([tag, attributes] pairs) as lucide draws it. */
async function lucideNode(file) {
  if (!nodes.has(file)) nodes.set(file, (await import(join(modules, 'lucide/dist/esm/lucide/src/icons', `${file}.js`))).default);
  return nodes.get(file);
}

const NUMBER = '(-?(?:\\d+\\.?\\d*|\\.\\d+)(?:e[-+]?\\d+)?)';
const LEADING_MOVE = new RegExp(`^m\\s*${NUMBER}[\\s,]*${NUMBER}[\\s,]*`);
const fmt = n => String(Math.round(n * 1e6) / 1e6);
/** A path's `d` as one subpath of the joined path: a leading relative moveto starts at the origin, so
 *  it becomes absolute and its implicit pairs relative linetos. Absolute paths stay as they are. */
function pathD(d) {
  const move = LEADING_MOVE.exec(d);
  if (!move) return d;
  const rest = d.slice(move[0].length);
  return `M${move[1]} ${move[2]}` + (rest && /^[-\d.]/.test(rest) ? 'l' : '') + rest;
}
/** One element of an icon as path data. */
function element([tag, a]) {
  const n = name => Number(a[name] ?? 0);
  switch (tag) {
    case 'path': return pathD(a.d);
    case 'circle': case 'ellipse': {
      const rx = tag === 'circle' ? n('r') : n('rx'), ry = tag === 'circle' ? rx : n('ry');
      return `M${fmt(n('cx') - rx)} ${fmt(n('cy'))}a${fmt(rx)} ${fmt(ry)} 0 1 0 ${fmt(2 * rx)} 0a${fmt(rx)} ${fmt(ry)} 0 1 0 ${fmt(-2 * rx)} 0`;
    }
    case 'line': return `M${a.x1} ${a.y1}L${a.x2} ${a.y2}`;
    case 'polyline': case 'polygon': {
      const p = a.points.trim().split(/[\s,]+/);
      let d = '';
      for (let at = 0; at < p.length; at += 2) d += `${at ? 'L' : 'M'}${p[at]} ${p[at + 1]}`;
      return d + (tag === 'polygon' ? 'Z' : '');
    }
    case 'rect': {
      const x = n('x'), y = n('y'), w = n('width'), h = n('height');
      const rx = a.rx !== undefined ? n('rx') : n('ry'), ry = a.ry !== undefined ? n('ry') : rx;
      if (!rx) return `M${fmt(x)} ${fmt(y)}h${fmt(w)}v${fmt(h)}h${fmt(-w)}Z`;
      const arc = (dx, dy) => `a${fmt(rx)} ${fmt(ry)} 0 0 1 ${fmt(dx)} ${fmt(dy)}`;
      return `M${fmt(x + rx)} ${fmt(y)}h${fmt(w - 2 * rx)}${arc(rx, ry)}v${fmt(h - 2 * ry)}${arc(-rx, ry)}h${fmt(-(w - 2 * rx))}${arc(-rx, -ry)}v${fmt(-(h - 2 * ry))}${arc(rx, -ry)}Z`;
    }
  }
  throw new Error(`a Lucide <${tag}> has no path form`);
}
/** An icon flattened to one path (a stroke icon: each element its own subpath). */
const flatten = node => node.map(element).join('');
/** An icon's elements as SVG markup, attributes in lucide's order. */
const markup = node => node.map(([tag, a]) => `<${tag}${Object.entries(a).map(([k, v]) => ` ${k}="${v}"`).join('')}/>`).join('');

// ── Contract lookups ─────────────────────────────────────────────────────────────────────────
const q = JSON.stringify;
/** A value lookup as a fn body: `includes` groups of ⌈√n⌉ keys, each a ternary chain, so a call's
 *  expression stays shallow (view-depth-under-test-stack). */
function lookup(entries, fallback, key = 'token') {
  const size = Math.ceil(Math.sqrt(entries.length));
  const groups = [];
  for (let at = 0; at < entries.length; at += size) groups.push(entries.slice(at, at + size));
  return groups.map(group => {
    const chain = group.length === 1 ? group[0][1]
      : `(${group.slice(0, -1).map(([k, v]) => `${key} == ${q(k)} ? ${v} : `).join('')}${group[group.length - 1][1]})`;
    return `[${group.map(([k]) => q(k)).join(', ')}].includes(${key}) ? ${chain} : `;
  }).join('') + fallback;
}
/** A short lookup inline: a ternary chain. */
const inline = (entries, fallback, key = 'token') => entries.map(([k, v]) => `${key} == ${q(k)} ? ${v} : `).join('') + fallback;

// ── The project icon picker: every Lucide icon ──────────────────────────────────────────────
async function lucideSet() {
  const entries = [];
  for (const [name, file] of lucideNames()) entries.push(`${q(name)}:${q(flatten(await lucideNode(file)))}`);
  const lines = [];
  for (let at = 0; at < entries.length; at += 12) lines.push(entries.slice(at, at + 12).join(',') + (at + 12 < entries.length ? ',' : ''));
  const literal = line => `'${line.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
  return `// Generated by examples/t3-code/tools/generate/gen-icons.mjs; do not edit. Every icon lucide-react@${version('lucide-react')}'s
// dynamic iconNames offer (aliases included), with lucide@${version('lucide')}'s geometry (ISC), for the project
// icon picker: each icon as one 24-grid stroke path.
export const LUCIDE_PATHS: Record<string, string> = JSON.parse('{' +
${lines.map(line => `  ${literal(line)} +`).join('\n')}
  '}');
`;
}

// ── The pages' glyphs ────────────────────────────────────────────────────────────────────────
/** The Lucide icons the pages draw (pull requests, usage, onboarding, landings), in PgIcon's order. */
const PAGE_GLYPHS = ['arrow-left', 'plus', 'message-square-dashed', 'link', 'folder-plus', 'info', 'refresh-cw', 'chevron-down',
  'chevron-right', 'arrow-right', 'check', 'monitor', 'cloud', 'terminal', 'copy', 'arrow-up-down', 'arrow-down-up', 'list-filter',
  'plug', 'plug-2', 'search', 'x', 'circle-check', 'circle-x', 'circle-dashed', 'circle-slash', 'users', 'external-link', 'ellipsis',
  'panel-right', 'tag', 'user-plus', 'file-text', 'git-branch', 'triangle-alert', 'git-pull-request', 'git-pull-request-arrow',
  'git-pull-request-draft', 'git-merge', 'git-pull-request-closed', 'circle-dot', 'sliders-horizontal', 'git-commit-horizontal',
  'message-square', 'pencil', 'list-checks', 'user-lock', 'clock', 'calendar-arrow-down', 'calendar-arrow-up', 'maximize-2',
  'minimize-2', 'layers', 'eye', 'eye-off', 'pen-line', 'user-round', 'user-check', 'user-round-x', 'folder-git-2', 'file-diff',
  'arrow-up-right', 'message-circle-question-mark', 'book-open', 'hammer', 'rotate-ccw', 'laptop', 'server', 'square-terminal',
  'link-2', 'loader-circle', 'unlink-2', 'user', 'folder'];
/** The pages' own filled marks: the pull request list's sort triangle and the status dot. */
const PAGE_MARKS = {
  'triangle-fill-filled': 'M21.73 18l-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z',
  'dot-filled': 'M12 7a5 5 0 1 0 0 10a5 5 0 1 0 0-10Z',
};
/** And the one Lucide icon drawn after them. */
const PAGE_LAST = ['play'];
/** Names per fn: a fn of 40 is six ⌈√40⌉ groups deep. */
const PER_FN = 40;

async function pageIcons() {
  const files = lucideNames();
  const entries = [];
  for (const name of [...PAGE_GLYPHS, ...Object.keys(PAGE_MARKS), ...PAGE_LAST]) {
    if (PAGE_MARKS[name]) { entries.push([name, q(PAGE_MARKS[name])]); continue; }
    if (!files.has(name)) throw new Error(`lucide has no icon ${name}`);
    entries.push([name, q(flatten(await lucideNode(files.get(name))))]);
  }
  const fns = [];
  for (let at = 0; at < entries.length; at += PER_FN) fns.push(`fn pgPath${fns.length}(name: string): string = ${lookup(entries.slice(at, at + PER_FN), '""', 'name')}`);
  const pick = fns.map((_, index) => `pgPath${index}(name) != "" ? pgPath${index}(name) : `).join('') + '""';
  const svg = weight => `svg width=\`\${size / 16}rem\` height=\`\${size / 16}rem\` viewBox="0 0 24 24" color=color fill="none" stroke="currentColor" stroke-width=${weight} stroke-linecap="round" stroke-linejoin="round" flex-shrink=0 aria-hidden=true`;
  return `// Generated by examples/t3-code/tools/generate/gen-icons.mjs; do not edit. Lucide geometry (lucide@${version('lucide')},
// ISC) and the pages' own marks (pull requests, usage, onboarding, landings), each icon flattened
// to one path. A new icon is a name in gen-icons.mjs's PAGE_GLYPHS.
${fns.join('\n')}
fn pgPath(name: string): string = ${pick}
fn pgFilled(name: string): bool = ${Object.keys(PAGE_MARKS).map(name => `name == ${q(name)}`).join(' or ')}

component PgIcon
  props
    name: string
    size: number
    color: string
  view
    ${svg(2)}
      path d=pgPath(name) fill=(pgFilled(name) ? "currentColor" : "none")

// The same geometry at another stroke weight (the conflict badge draws at 2.5).
component PgGlyph
  props
    name: string
    size: number
    color: string
    weight: number
  view
    ${svg('weight')}
      path d=pgPath(name) fill=(pgFilled(name) ? "currentColor" : "none")
`;
}

// ── Pierre's file icons ─────────────────────────────────────────────────────────────────────
/** @pierre/trees' built-in icon module's tables (its private constants, read from its source). */
function pierreModule() {
  const source = read('@pierre/trees/dist/builtInIcons.js').replace(/^export \{[^}]*\};.*$/m, '').replace(/^\/\/# sourceMappingURL.*$/m, '');
  return new Function(`${source}\nreturn { BUILT_IN_FILE_NAME_TOKENS, BUILT_IN_FILE_EXTENSION_TOKENS, COMPLETE_EXTENSION_OVERRIDES, BUILT_IN_SVG_SPRITE_SHEETS };`)();
}
/** T3's own sprite (apps/web/src/pierre-icons.ts T3_FILE_ICON_SPRITE: AGENTS.md and pnpm's files; MIT, see LICENSE-T3). */
const T3_SPRITE = `
  <symbol id="t3-file-icon-agents" viewBox="0 0 32 32">
    <path fill="currentColor" d="M27.2 16c0-6.19-5.01-11.2-11.2-11.2C9.81 4.8 4.8 9.81 4.8 16S9.81 27.2 16 27.2c6.19 0 11.2-5.01 11.2-11.2Zm-5.6 2.1a1.4 1.4 0 1 1 0 2.8h-4.2a1.4 1.4 0 1 1 0-2.8Zm-11.2-6.8c.622-.373 1.42-.208 1.84.361l.079.119 2.1 3.5.088.171c.15.351.15.748 0 1.1l-.088.171-2.1 3.5a1.4 1.4 0 0 1-2.4-1.44L11.59 16l-1.67-2.78-.067-.127c-.302-.642-.075-1.42.547-1.79ZM30 16c0 7.73-6.27 14-14 14S2 23.73 2 16 8.27 2 16 2s14 6.27 14 14Z" />
  </symbol>
  <symbol id="t3-file-icon-pnpm" viewBox="0 0 32 32">
    <path fill="#f9ad00" d="M30 10.75h-8.749V2H30Zm-9.626 0h-8.75V2h8.75Zm-9.625 0H2V2h8.749ZM30 20.375h-8.749v-8.75H30Z" />
    <path fill="currentColor" d="M20.374 20.375h-8.75v-8.75h8.75Zm0 9.625h-8.75v-8.75h8.75ZM30 30h-8.749v-8.75H30Zm-19.251 0H2v-8.75h8.749Z" />
  </symbol>`;
/** T3_PIERRE_ICONS.byFileName: two remap a built-in icon, drawn as its shape without its colour
 *  (token `<icon>-plain`), three name T3's own symbols. */
const T3_BY_NAME = { 'package.json': 'npm-plain', 'tsconfig.json': 'typescript-plain', 'agents.md': 'agents', 'pnpm-lock.yaml': 'pnpm', 'pnpm-workspace.yaml': 'pnpm' };

const attributes = tag => Object.fromEntries([...tag.matchAll(/([\w-]+)="([^"]*)"/g)].map(m => [m[1], m[2]]));
/** Every symbol as { viewBox, inner, paths: [{ d, opacity, rule, fill }] }, by token. */
function pierreSymbols(pierre) {
  const symbols = new Map();
  const sheet = pierre.BUILT_IN_SVG_SPRITE_SHEETS.complete + T3_SPRITE;
  for (const [, prefix, token, viewBox, inner] of sheet.matchAll(/<symbol id="(file-tree-builtin|t3-file-icon)-([\w-]+)" viewBox="([^"]+)">([\s\S]*?)<\/symbol>/g)) {
    const paths = [...inner.matchAll(/<path\s([^>]*?)\s*\/>/g)].map(([, tag]) => {
      const a = attributes(tag);
      // A gradient fill (nextjs: currentColor fading to nothing) is drawn as its mean, half opacity.
      const gradient = a.fill?.startsWith('url(');
      return { d: a.d, opacity: gradient ? 0.5 : Number(a.opacity ?? 1), rule: a['fill-rule'] ?? 'nonzero', fill: gradient ? 'currentColor' : (a.fill ?? 'currentColor') };
    });
    symbols.set(token, { prefix, viewBox, inner, paths });
  }
  for (const token of Object.values(T3_BY_NAME)) if (token.endsWith('-plain')) symbols.set(token, symbols.get(token.slice(0, -'-plain'.length)));
  return symbols;
}
/** Each token's light-dark tint from the colored set's stylesheet ([data-icon-token] rules). */
function pierreTints() {
  const css = JSON.parse(/var style_default = (".*");/s.exec(read('@pierre/trees/dist/style.js'))[1]);
  const palette = Object.fromEntries([...css.matchAll(/--trees-icon-([\w-]+): (light-dark\([^)]*\));/g)].map(m => [m[1], m[2]]));
  const tokenColour = Object.fromEntries([...css.matchAll(/--trees-file-icon-color-([\w-]+): var\(\s*--[\w-]+,\s*var\(--trees-icon-([\w-]+)\)\s*\)/g)].map(m => [m[1], palette[m[2]]]));
  const ruled = [...css.matchAll(/\[data-icon-token='([\w-]+)'\] \{\s*color: var\(--trees-file-icon-color-([\w-]+)\);/g)].map(m => [m[1], tokenColour[m[2]]]);
  return { tints: new Map(ruled), fallback: tokenColour.default };
}
const number = n => (Number.isInteger(n) ? n.toFixed(1) : String(n));

function fileTypeIcon(symbols, tints) {
  const tokens = [...symbols.keys()].sort();
  const layers = Math.max(...tokens.map(token => symbols.get(token).paths.length));
  const fns = [], derives = [];
  const derive = (name, fnName, type, entries, fallback) => {
    if (entries.length > 8) {
      fns.push(`fn ${fnName}(token: string): ${type} = ${lookup(entries, fallback)}`);
      derives.push(`  derive ${name} = ${fnName}(token)`);
    } else derives.push(`  derive ${name} = ${inline(entries, fallback)}`);
  };
  const boxes = tokens.filter(token => symbols.get(token).viewBox !== '0 0 16 16').map(token => [token, q(symbols.get(token).viewBox)]);
  derives.push(`  derive box = ${inline(boxes, '"0 0 16 16"')}`);
  // A token tinted the fallback's grey (default, text) is the fallback's.
  derive('tint', 'fileTypeIconTint', 'string', [...tints.tints].filter(([token, tint]) => symbols.has(token) && tint !== tints.fallback).sort(([a], [b]) => (a < b ? -1 : 1)).map(([token, tint]) => [token, q(tint)]), q(tints.fallback));
  for (let layer = 0; layer < layers; layer++) {
    const at = token => symbols.get(token).paths[layer];
    const having = tokens.filter(at);
    derive(`d${layer}`, `fileTypeIconD${layer}`, 'string', having.map(token => [token, q(at(token).d)]), '""');
    derive(`o${layer}`, `fileTypeIconO${layer}`, 'number', having.filter(token => at(token).opacity !== 1).map(token => [token, number(at(token).opacity)]), '1.0');
    derive(`r${layer}`, `fileTypeIconR${layer}`, 'string', having.filter(token => at(token).rule !== 'nonzero').map(token => [token, q(at(token).rule)]), '"nonzero"');
    derive(`f${layer}`, `fileTypeIconF${layer}`, 'string', having.filter(token => at(token).fill !== 'currentColor').map(token => [token, q(at(token).fill)]), '"currentColor"');
  }
  const paths = Array.from({ length: layers }, (_, layer) => `      if d${layer} != ""\n        path d=d${layer} fill=f${layer} fill-opacity=o${layer} fill-rule=r${layer}`);
  return `// Generated by examples/t3-code/tools/generate/gen-icons.mjs; do not edit. Pierre file icons (the
// "complete" set of @pierre/trees@${version('@pierre/trees')} plus T3's agents/pnpm sprites (Apache-2.0;
// T3's, MIT; see LICENSE-T3). timeline-files.ts resolves a path or fence language to the token; PierreEntryIcon
// colours it per scheme.

// FileTypeIcon's lookups are fns: a call reads its argument once, so a caller's deep expression is not
// nested under every comparison (view-depth-under-test-stack).
${fns.join('\n')}

component FileTypeIcon
  props
    token: string
    size: number
${derives.join('\n')}
  view
    svg width=\`\${size / 16}rem\` height=\`\${size / 16}rem\` viewBox=box color=tint fill="none" flex-shrink=0 aria-hidden=true
${paths.join('\n')}
`;
}

function fileTables(pierre) {
  const byExtension = pierre.BUILT_IN_FILE_EXTENSION_TOKENS;
  const ts = `// Generated by examples/t3-code/tools/generate/gen-icons.mjs; do not edit. @pierre/trees@${version('@pierre/trees')}'s
// file-name and extension tables (createFileTreeIconResolver's built-in ones; Apache-2.0, see LICENSE-T3).
// timeline-files.ts resolves a path with them.

export const BY_NAME: Record<string, string> = ${q(pierre.BUILT_IN_FILE_NAME_TOKENS)};
export const BY_EXTENSION: Record<string, string> = ${q(byExtension)};
export const COMPLETE: Record<string, string> = ${q(pierre.COMPLETE_EXTENSION_OVERRIDES)};
`;
  const rows = (kind, table) => Object.entries(table).map(([key, token]) => `${kind}\t${key}\t${token}\n`).join('');
  const tsv = `# Generated by examples/t3-code/tools/generate/gen-icons.mjs; do not edit. @pierre/trees@${version('@pierre/trees')}'s
# file-name, complete-set extension and extension tables (Apache-2.0, see LICENSE-T3), one row each:
# kind, key, token. pierre_icons.rs resolves a path with them.
${rows('name', pierre.BUILT_IN_FILE_NAME_TOKENS)}${rows('complete', pierre.COMPLETE_EXTENSION_OVERRIDES)}${rows('extension', byExtension)}`;
  return { ts, tsv };
}

/** The Lucide glyphs the composer's chips draw (T3ComposerIcons.name), by chip name: icon and stroke width. */
const COMPOSER_GLYPHS = {
  film: ['film', 2], image: ['image', 2], 'mouse-pointer-click': ['mouse-pointer-click', 2], 'messages-square': ['messages-square', 2],
  skill: ['box', 1.85], quote: ['quote', 2], pencil: ['pencil', 2], folder: ['folder', 2], file: ['file', 2],
  'git-pull-request': ['git-pull-request', 2], 'git-pull-request-draft': ['git-pull-request-draft', 2], 'git-merge': ['git-merge', 2],
  'git-pull-request-closed': ['git-pull-request-closed', 2], 'message-square-text': ['message-square-text', 2],
  terminal: ['terminal', 2], 'circle-dashed': ['circle-dashed', 2],
};

async function composerIcons(symbols, tints) {
  const files = lucideNames();
  const svg = {}, colors = {}, lucide = {};
  for (const [token, symbol] of [...symbols].sort(([a], [b]) => (a < b ? -1 : 1))) {
    if (symbol.prefix !== 'file-tree-builtin' || token.endsWith('-plain')) continue;
    // As a browser serializes the symbol's content inside an <svg>: elements closed, whitespace one space.
    const inner = symbol.inner.trim().replace(/\s*\n\s*/g, ' ').replace(/<(\w+)([^<>]*?)\s*\/>/g, '<$1$2></$1>');
    svg[token] = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${symbol.viewBox}">${inner}</svg>`;
  }
  // A token the stylesheet gives no colour (default, font) draws in the fallback's grey.
  for (const token of Object.keys(svg)) {
    const [, light, dark] = /light-dark\(([^,]+), ([^)]+)\)/.exec(tints.tints.get(token) ?? tints.fallback);
    colors[token] = [light, dark];
  }
  for (const [chip, [icon, width]] of Object.entries(COMPOSER_GLYPHS)) lucide[chip] = [markup(await lucideNode(files.get(icon))), width];
  return JSON.stringify({
    about: `Generated by examples/t3-code/tools/generate/gen-icons.mjs; do not edit. The composer's chip icons (T3ComposerIcons.swift): Pierre's complete set as SVG with its light and dark tint (@pierre/trees@${version('@pierre/trees')}, Apache-2.0, see LICENSE-T3) and Lucide glyphs with their stroke width (lucide@${version('lucide')}, ISC).`,
    pierre: svg, colors, lucide,
  }, null, 1) + '\n';
}

export async function generate() {
  const pierre = pierreModule(), symbols = pierreSymbols(pierre), tints = pierreTints();
  const tables = fileTables(pierre);
  return [
    { file: join(example, 'settings-b-lucide.ts'), text: await lucideSet() },
    { file: join(example, 'pages-icons.contract'), text: await pageIcons() },
    { file: join(example, 'timeline-files.contract'), text: fileTypeIcon(symbols, tints) },
    { file: join(example, 'timeline-files-tables.ts'), text: tables.ts },
    { file: join(example, 'macos', 'pierre-icons.tsv'), text: tables.tsv },
    { file: join(example, 'assets', 'composer-icons.json'), text: await composerIcons(symbols, tints) },
  ];
}
