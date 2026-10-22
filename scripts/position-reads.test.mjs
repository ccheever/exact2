// HTML's top layer on kernel hosts: an open or closed popover lays out
// absolute (kernel/src/style/top_layer.rs), so how a box is positioned for
// layout is not always its authored `position`. Every layout read goes
// through one accessor: `style::top_layer::layout_position` (or `of`) in the
// kernel, `NodeRef::layout_position` on the Rust hosts, `NodeView.layoutPosition`
// in Swift. This scan fails on a direct read of the authored row anywhere
// else in the kernel and the Apple and Linux hosts, so a new layout reader
// can't skip the rule. The web hosts are out of scope: they write the
// authored row as CSS, and the browser's UA sheet applies the top layer.
// `bun test ./scripts/position-reads.test.mjs`.
import { test } from 'bun:test';
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

// Where the rule's readers live, and what a direct read looks like there:
// in Rust the field itself (as a word, so a destructuring, a rename, a
// spaced or split access all count) or its row id, in code outside comments,
// strings and tests; in Swift the row's name.
const RUST = /\bposition_type\b|\bStyleId::PositionType\b/;
const SCOPES = [
  { dir: 'kernel/src', ext: '.rs', read: RUST },
  { dir: 'host/apple/src', ext: '.rs', read: RUST },
  { dir: 'host/linux/src', ext: '.rs', read: RUST },
  { dir: 'host/apple/Sources', ext: '.swift', read: /"position_type"/ },
];

// The reads that mean the author's value, each with why. An entry matches
// one line of its file by a piece of its text, and must still be there.
export const ALLOWED = [
  { file: 'kernel/src/style/top_layer.rs', text: 'arena.computed_source(slot, crate::StyleId::PositionType)', why: 'the accessor itself: the row with its platform start' },
  { file: 'kernel/src/style/top_layer.rs', text: 'let authored = style.position_type;', why: 'the accessor itself' },
  { file: 'kernel/src/style/top_layer.rs', text: 'style.written.has(crate::StyleId::PositionType)', why: "the accessor itself: whether the author wrote the sticky" },
  { file: 'kernel/src/style.rs', text: 's.position = match self.position_type', why: "the engine mapping of the authored rows (to_taffy); top_layer::apply overrides it right after, in cover::apply" },
  { file: 'kernel/src/style.rs', text: 's.inset = if self.position_type == PositionType::Sticky', why: "a sticky box's insets aren't offsets (to_taffy); a box sticky for layout is sticky as written" },
  { file: 'kernel/src/arena/defaults.rs', text: '(StyleId::PositionType, "-exact-header-position")', why: "the neutral route header's platform start: a row it sets, not a read" },
  { file: 'kernel/src/arena/defaults.rs', text: '.computed_source(slot, StyleId::PositionType)', why: "the sticky machinery's own registry (refresh_sticky); a box sticky for layout is sticky as written" },
  { file: 'kernel/src/arena/defaults.rs', text: '.position_type', why: "the sticky machinery's own registry (refresh_sticky), the line above's field" },
  { file: 'kernel/src/kernel/sticky.rs', text: 'style.position_type != PositionType::Sticky', why: "the sticky machinery's own constraint (sticky_constraint); a box sticky for layout is sticky as written" },
  { file: 'host/apple/src/style.rs', text: 'computed.position_type == exact_kernel::PositionType::Sticky', why: 'the style rows the Swift side gets: which rows a sticky box crosses with' },
  { file: 'host/linux/src/paint.rs', text: '|s| s.position_type)', why: "the sticky machinery's paint offset (LLP 1083 D4); a box sticky for layout is sticky as written" },
  { file: 'host/linux/src/scroll_anchor.rs', text: 'StyleId::PositionType,', why: 'a row list, not a value: a change of the row is a layout change' },
  { file: 'host/apple/Sources/ExactKit/LayoutPosition.swift', text: 'let authored = style["position_type"]', why: 'the accessor itself' },
  { file: 'host/apple/Sources/ExactKit/LayoutPosition.swift', text: '.contains(.string("position_type"))', why: 'the accessor itself: whether the author wrote the sticky' },
  { file: 'host/apple/Sources/ExactKit/ScrollAnchoring.swift', text: '"inset", "position_type",', why: "a key list, not a value: a change of the authored row is a layout change" },
  { file: 'host/apple/Sources/ExactKit/PositionArea.swift', text: 'popover.style["position_type"] == nil', why: "whether the author wrote `position` decides between the UA's fixed and absolute (CSS Positioned Layout 4, top layer styling)" },
  { file: 'host/apple/Sources/ExactCore/FlatPaint.swift', text: 'case "position_type":', why: 'flat leaves carry only data props (FlatLeaves.onlyData), never a popover, so the row is how they lay out' },
  { file: 'host/apple/Sources/ExactCore/FlatPaint.swift', text: 'positioned = ["relative", "absolute"].contains(style["position_type"]', why: 'flat leaves carry only data props (FlatLeaves.onlyData), never a popover, so the row is how they lay out' },
];

function* files(dir, ext) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* files(path, ext);
    else if (entry.name.endsWith(ext)) yield path;
  }
}

// `text` with its comments blanked (and, for Rust, its strings and char
// literals too), each to spaces but its line breaks, so lines keep their
// numbers. A Rust string that is the row's name stays as the word.
export function code(text, rust) {
  let out = '';
  for (let i = 0; i < text.length;) {
    const c = text[i], next = text[i + 1];
    const blank = (j) => { out += text.slice(i, j).replace(/[^\n]/g, ' '); i = j; };
    if (c === '/' && next === '/') { const j = text.indexOf('\n', i); blank(j < 0 ? text.length : j); continue; }
    if (c === '/' && next === '*') {
      // Rust and Swift block comments nest.
      let depth = 0, j = i;
      while (j < text.length) {
        if (text.startsWith('/*', j)) { depth++; j += 2; } else if (text.startsWith('*/', j)) { j += 2; if (--depth === 0) break; } else j++;
      }
      blank(j);
      continue;
    }
    if (rust) {
      const raw = /^b?r(#*)"/.exec(text.slice(i, i + 260));
      if (raw && !/\w/.test(text[i - 1] ?? '')) {
        const end = text.indexOf(`"${raw[1]}`, i + raw[0].length);
        const j = end < 0 ? text.length : end + 1 + raw[1].length;
        const body = text.slice(i + raw[0].length, end);
        if (body === 'position_type') { out += 'position_type'.padEnd(j - i); i = j; } else blank(j);
        continue;
      }
      if (c === '"' || (c === 'b' && next === '"' && !/\w/.test(text[i - 1] ?? ''))) {
        let j = i + (c === 'b' ? 2 : 1);
        while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
        j++;
        const body = text.slice(i + (c === 'b' ? 2 : 1), j - 1);
        if (body === 'position_type') { out += 'position_type'.padEnd(j - i); i = j; } else blank(j);
        continue;
      }
      // A char literal, not a lifetime: one char or one escape, then a quote.
      const ch = c === "'" && /^'(?:\\u\{[0-9a-fA-F]+\}|\\.|[^\\'\n])'/.exec(text.slice(i, i + 12));
      if (ch) { blank(i + ch[0].length); continue; }
    }
    out += c;
    i++;
  }
  return out;
}

// A Rust file's test code: a `_tests.rs` or `tests.rs` file, or an inline
// `#[cfg(test)] mod … { … }`, blanked to its line breaks. `text` is already
// `code`, so a brace in a string or a comment counts for nothing.
function source(path, text) {
  if (/(^|_)tests\.rs$/.test(path.split('/').pop())) return '';
  const inline = /^\s*#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod \w+\s*\{/gm;
  let out = '', from = 0;
  for (let m; (m = inline.exec(text));) {
    let depth = 0, at = m.index + m[0].length - 1;
    for (; at < text.length; at++) {
      if (text[at] === '{') depth++;
      else if (text[at] === '}' && --depth === 0) break;
    }
    out += text.slice(from, m.index) + text.slice(m.index, at + 1).replace(/[^\n]/g, '');
    from = inline.lastIndex = at + 1;
  }
  return out + text.slice(from);
}

export function directReads(root = ROOT) {
  const out = [];
  for (const { dir, ext, read } of SCOPES) {
    for (const path of files(join(root, dir), ext)) {
      const file = relative(root, path);
      const text = readFileSync(path, 'utf8'), rust = ext === '.rs';
      const scanned = (rust ? source(path, code(text, true)) : code(text, false)).split('\n'), lines = text.split('\n');
      scanned.forEach((line, i) => {
        if (read.test(line)) out.push({ file, line: i + 1, text: lines[i].trim() });
      });
    }
  }
  return out;
}

test('every direct read of the authored position is on the allowlist, with its reason', () => {
  const stray = directReads().filter(r => !ALLOWED.some(a => a.file === r.file && r.text.includes(a.text)));
  assert.deepEqual(stray, [], 'a layout read goes through layout_position / layoutPosition (kernel/src/style/top_layer.rs); a read that means the author\'s value goes on ALLOWED with why');
});

test('each allowlist entry names one read that is still there', () => {
  const reads = directReads();
  for (const a of ALLOWED) {
    assert.ok(a.why, `${a.file}: ${a.text} has no reason`);
    const hits = reads.filter(r => r.file === a.file && r.text.includes(a.text));
    assert.equal(hits.length, 1, `${a.file}: "${a.text}" matches ${hits.length} reads`);
  }
});

test('the scan finds a new read in any form, and skips comments, strings and test code', () => {
  const root = mkdtempSync(join(tmpdir(), 'iosfix-position-reads-'));
  try {
    for (const { dir } of SCOPES) mkdirSync(join(root, dir), { recursive: true });
    writeFileSync(join(root, 'kernel/src/new.rs'), [
      '// s.position_type in a comment, /* nested */ too',
      'fn f(s: &StyleProps) -> bool { s.position_type == PositionType::Absolute }',
      'fn g(StyleProps { position_type: p, .. }: &StyleProps) {}',
      'fn h(s: &StyleProps) {',
      '    let StyleProps {',
      '        position_type,',
      '        ..',
      '    } = s;',
      '    let _ = s . position_type;',
      '    let _ = s.row(StyleId::PositionType);',
      '    let _ = "s.position_type in a string";',
      '}',
      '/* s.position_type in a block comment',
      '   /* nested */ still a comment s.position_type */',
      '#[cfg(test)]',
      'mod tests {',
      '    const OPEN: &str = "{";',
      "    const C: char = '{';",
      '    // an unmatched { in a comment',
      '    fn t(s: &StyleProps) { if s.position_type == PositionType::Static {} }',
      '}',
      'fn after(s: &StyleProps) -> bool { s.position_type == PositionType::Sticky }',
      'fn named(row: &Row) -> bool { row.name == "position_type" }',
      "fn life<'a>(s: &'a StyleProps) -> bool { s.position_type == PositionType::Relative }",
      'const RAW: &str = r#"s.position_type { "#;',
    ].join('\n'));
    writeFileSync(join(root, 'host/linux/src/scan_tests.rs'), 'fn t(s: &StyleProps) { s.position_type; }');
    writeFileSync(join(root, 'host/apple/Sources/New.swift'), [
      'let p = view.style["position_type"]?.string',
      '// view.style["position_type"] in a comment',
    ].join('\n'));
    assert.deepEqual(directReads(root).map(r => `${r.file}:${r.line}`), [
      ...[2, 3, 6, 9, 10, 22, 23, 24].map(line => `kernel/src/new.rs:${line}`),
      'host/apple/Sources/New.swift:1',
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
