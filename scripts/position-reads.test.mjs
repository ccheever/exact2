// HTML's top layer on kernel hosts: an open or closed popover lays out
// absolute (kernel/src/style/top_layer.rs), so how a box is positioned for
// layout is not always its authored `position`. Every layout read goes
// through one accessor: `style::top_layer::layout_position` (or `of`) in the
// kernel, `NodeRef::layout_position` on the Rust hosts, `NodeView.layoutPosition`
// in Swift. This scan fails on a direct read of the authored row anywhere
// else in the kernel and the Apple and Linux hosts, so a new layout reader
// can't skip the rule. The web hosts are out of scope: they write the
// authored row as CSS, and the browser's UA sheet applies the top layer.
// It is a tripwire for ordinary code, not a proof (theme2 12:59): what the
// repo's own idiom writes is caught, each form with a case in the scratch
// tree below. Its stated limits, not chased: a name a macro builds (Rust's
// `concat!`, a generated identifier), a string built at run time (Swift's
// "position" + "_type"), reflection, a re-export of the row id under
// another module's name; and a comment inside a Swift interpolation is
// scanned as code, so a read written there is a false hit, never a miss.
// The compile-time guard is QUEUE'd with the computed-style default (B).
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
// spaced or split access all count) or its row id `StyleId::PositionType`
// (spaced or split across lines), in code outside comments, strings and
// tests; in Swift the row's name, in code outside comments.
const RUST = /\bposition_type\b|(?:\b(?:StyleId|Self)|<\s*(?:StyleId|Self)\s*>)\s*::\s*PositionType\b/g;
// The row id reached any other way is refused outright, as no read may
// hide behind it: a `use` group naming it, a rename in a `use` of StyleId
// (`PositionType as P`, `self as Ids`, `StyleId as S`), a type alias of
// StyleId, and, where a glob brings StyleId's rows in bare, `PositionType`
// as anything but a path (`PositionType::…`, `crate::PositionType`, the
// enum's).
const REFUSED = [
  [/\buse\b[^;]*\bStyleId\s*::\s*\{[^;]*\bPositionType\b[^;]*;/g, 'a `use` group naming StyleId::PositionType: write it out'],
  [/\buse\b[^;]*\bStyleId\b[^;]*\bas\b[^;]*;/g, 'a rename in a `use` of StyleId'],
  [/\btype\s+\w+[^=;{}]*=\s*[\w:\s<>]*?\bStyleId\b/g, 'a type alias of StyleId'],
  [/\buse\b[^;]*\bPositionType\s+as\b[^;]*;/g, 'a rename of PositionType'],
  [/\bpub(?:\s*\([^)]*\))?\s+use\b[^;]*\bStyleId\s*::\s*(?:\*|\{|PositionType\b)[^;]*;/g, "a re-export of StyleId's rows"],
];
const GLOB = /\bStyleId\s*::\s*(?:\*|\{[^}]*\*[^}]*\})/;
// A glob or group of StyleId's rows in a module's own scope (a file's top,
// or a `mod … { }`) puts them where a path (`super::PositionType`,
// `rows::PositionType`) or a child's `use super::*` reaches them bare:
// refused. In any other block (a function's body, a const's initializer)
// they reach nothing past it, and a bare `PositionType` there counts (BARE).
const ROWS_USE = /\buse\b[^;]*\bStyleId\s*::\s*(?:\*|\{)[^;]*;/g;

/** Whether offset `i` of Rust `code` is in a module's own scope: the
 * innermost enclosing `{` opens a `mod`, or there is none (a file's top). */
export function inModuleScope(code, i) {
  const stack = [];
  for (let j = 0; j < i; j++) {
    if (code[j] === '{') stack.push(/\bmod\s+\w+\s*$/.test(code.slice(Math.max(0, j - 200), j)));
    else if (code[j] === '}') stack.pop();
  }
  return stack.length === 0 || stack[stack.length - 1];
}
const BARE = /(?<!::\s*)\bPositionType\b(?!\s*::)/g;

const SCOPES = [
  { dir: 'kernel/src', ext: '.rs', read: RUST },
  { dir: 'host/apple/src', ext: '.rs', read: RUST },
  { dir: 'host/linux/src', ext: '.rs', read: RUST },
  { dir: 'host/apple/Sources', ext: '.swift', read: /"position_type"|"{3}\s*position_type\s*"{3}/g },
];

// The reads that mean the author's value, each with why. An entry matches
// one line of its file by a piece of its text, and must still be there.
export const ALLOWED = [
  { file: 'kernel/src/style/top_layer.rs', text: 'arena.computed_source(slot, crate::StyleId::PositionType)', why: 'the accessor itself: the row with its platform start' },
  { file: 'kernel/src/style/top_layer.rs', text: 'let authored = style.position_type;', why: 'the accessor itself' },
  { file: 'kernel/src/style/top_layer.rs', text: 'style.written.has(crate::StyleId::PositionType)', why: 'the accessor itself: whether the author wrote the sticky' },
  { file: 'kernel/src/style.rs', text: 's.position = match self.position_type', why: 'the engine mapping of the rows (to_taffy); top_layer::apply makes a top-layer box absolute right after, in cover::apply' },
  { file: 'kernel/src/style.rs', text: 's.inset = if self.position_type == PositionType::Sticky', why: "the engine mapping of the rows (to_taffy): a sticky row's insets aren't offsets; top_layer::apply makes a top-layer box's offsets again" },
  { file: 'kernel/src/arena/defaults.rs', text: '(StyleId::PositionType, "-exact-header-position")', why: "the neutral route header's platform start: a row it sets, not a read" },
  { file: 'kernel/src/kernel/cover.rs', text: 'StyleId::PositionType,', why: "a row list, not a value: a change of the row moves a box on the bottom edge's chain (style_changed); a popover's prop change does too (top_layer::prop_changed)" },
  { file: 'host/linux/src/scroll_anchor.rs', text: 'StyleId::PositionType,', why: 'a row list, not a value: a change of the row is a layout change' },
  { file: 'host/apple/Sources/ExactKit/LayoutPosition.swift', text: 'let authored = style["position_type"]', why: 'the accessor itself' },
  { file: 'host/apple/Sources/ExactKit/LayoutPosition.swift', text: '.contains(.string("position_type"))', why: 'the accessor itself: whether the author wrote the sticky' },
  { file: 'host/apple/Sources/ExactKit/ScrollAnchoring.swift', text: '"inset", "position_type",', why: 'a key list, not a value: a change of the row is a layout change' },
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
    if (!rust && (c === '"' || (c === '#' && /^#+"/.test(text.slice(i, i + 260))))) {
      // A Swift string, kept as it is (the row's name is read as one), so a
      // comment's opener inside one opens nothing.
      const j = swiftString(text, i);
      out += text.slice(i, j);
      i = j;
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

// The end of the Swift string literal at `i`: `"…"`, `"""…"""`, or a raw
// one with as many #s each side, whose escape is `\` with as many #s; an
// interpolation `\(…)` (`\#(…)` raw) runs to its matching paren, a string
// inside it read as a string.
function swiftString(text, i) {
  const hashes = /^#*/.exec(text.slice(i))[0];
  const open = text.startsWith('"""', i + hashes.length) ? '"""' : '"';
  const close = open + hashes, escape = `\\${hashes}`;
  let j = i + hashes.length + open.length;
  while (j < text.length && !text.startsWith(close, j)) {
    if (!text.startsWith(escape, j)) { j++; continue; }
    j += escape.length;
    if (text[j] !== '(') { j++; continue; }
    for (let depth = 0; j < text.length;) {
      if (text[j] === '"' || (text[j] === '#' && /^#+"/.test(text.slice(j, j + 260)))) { j = swiftString(text, j); continue; }
      // A comment in the interpolation's code: its parens and quotes count for nothing.
      if (text.startsWith('//', j)) { const e = text.indexOf('\n', j); j = e < 0 ? text.length : e; continue; }
      if (text.startsWith('/*', j)) {
        for (let c = 0; j < text.length;) {
          if (text.startsWith('/*', j)) { c++; j += 2; } else if (text.startsWith('*/', j)) { j += 2; if (--c === 0) break; } else j++;
        }
        continue;
      }
      if (text[j] === '(') depth++;
      else if (text[j] === ')' && --depth === 0) { j++; break; }
      j++;
    }
  }
  return Math.min(text.length, j + close.length);
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
      const scanned = rust ? source(path, code(text, true)) : code(text, false), lines = text.split('\n');
      // Over the whole file, so a read split across lines counts, on the
      // line it starts.
      const lineOf = i => scanned.slice(0, i).split('\n').length;
      const inUse = i => { const u = scanned.lastIndexOf('use ', i); return u >= 0 && !scanned.slice(u, i).includes(';') && /(^|[\s{;}])$/.test(scanned.slice(0, u)); };
      const at = new Set([...scanned.matchAll(read)].map(m => lineOf(m.index)));
      if (rust && GLOB.test(scanned)) for (const m of scanned.matchAll(BARE)) if (!inUse(m.index)) at.add(lineOf(m.index));
      if (rust) for (const [re, why] of REFUSED) for (const m of scanned.matchAll(re)) out.push({ file, line: lineOf(m.index), text: lines[lineOf(m.index) - 1].trim(), refused: why });
      if (rust) for (const m of scanned.matchAll(ROWS_USE)) if (inModuleScope(scanned, m.index)) out.push({ file, line: lineOf(m.index), text: lines[lineOf(m.index) - 1].trim(), refused: "a glob or group of StyleId's rows in a module's scope: import them in the function" });
      for (const line of [...at].sort((a, b) => a - b)) out.push({ file, line, text: lines[line - 1].trim() });
    }
  }
  const seen = new Set();
  return out.sort((a, b) => (a.file === b.file ? a.line - b.line : 0)).filter(r => !seen.has(`${r.file}:${r.line}`) && seen.add(`${r.file}:${r.line}`));
}

test('every direct read of the authored position is on the allowlist, with its reason', () => {
  const stray = directReads().filter(r => r.refused || !ALLOWED.some(a => a.file === r.file && r.text.includes(a.text)));
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
      'fn spaced(s: &Row) { s.get(StyleId :: PositionType); }',
      'fn split(s: &Row) { s.get(StyleId::',
      '    PositionType); }',
      'use exact_kernel::StyleId::{Display,',
      '    PositionType};',
      'fn bare(s: &Row) { s.get(PositionType); }',
      'fn typed(p: PositionType) -> PositionType { if p == PositionType::Static { p } else { crate::PositionType::Sticky } }',
      'use crate::StyleId as S;',
    ].join('\n'));
    writeFileSync(join(root, 'kernel/src/glob.rs'), [
      'use crate::StyleId::*;',
      'fn a(s: &Row) { s.get(PositionType); }',
      'fn b() -> crate::PositionType { crate::PositionType::Static }',
      'fn c() { let _ = Req { id: PositionType }; }',
    ].join('\n'));
    writeFileSync(join(root, 'kernel/src/rename.rs'), [
      'use crate::StyleId::{PositionType as P};',
      'use crate::StyleId::{self as Ids};',
      'type Id = crate::StyleId;',
      'type Ids2<T> = crate ::',
      '    StyleId;',
      '    use crate::StyleId::{self, *};',
      'fn e(s: &Row) { s.get(PositionType); }',
      '    use PositionType as Q;',
      '    pub use crate::StyleId::*;',
      'impl StyleId { fn f(s: &Row) { s.get(Self::PositionType); } }',
      'fn d(s: &Row) { s.get(P); s.get(Ids::Top); }',
    ].join('\n'));
    writeFileSync(join(root, 'kernel/src/parent.rs'), ['use crate::StyleId::*;', 'mod child;'].join('\n'));
    writeFileSync(join(root, 'kernel/src/nested.rs'), [
      'mod layout {',
      '    use crate::StyleId::*;',
      '    mod child { fn read(s: &Row) { let _ = s.get(super::PositionType); } }',
      '}',
      'fn local() {',
      '    let _ = <StyleId>::PositionType;',
      '}',
      '',
      '',
      '',
      'type Id<T> where T: Clone = crate::StyleId;',
    ].join('\n'));
    // A module in a function's body: its glob is a module's.
    writeFileSync(join(root, 'kernel/src/fnmod.rs'), ['fn layout(s: &Row) {', '    mod rows {', '        use crate::StyleId::*;', '        pub fn read(s: &Row) { let _ = s.get(self::PositionType); }', '    }', '}'].join('\n'));
    // A grouped glob alone in its file, in a function's body.
    writeFileSync(join(root, 'kernel/src/grouped.rs'), ['fn local(s: &Row) {', '    use crate::StyleId::{self, *};', '    let _ = s.get(PositionType);', '}'].join('\n'));
    writeFileSync(join(root, 'host/linux/src/scan_tests.rs'), 'fn t(s: &StyleProps) { s.position_type; }');
    writeFileSync(join(root, 'host/apple/Sources/New.swift'), [
      'let p = view.style["position_type"]?.string',
      '// view.style["position_type"] in a comment',
      'let marker = "/*"',
      'let q = view.style["position_type"]?.string',
      'let url = "http://x"; let r = view.style["position_type"]',
      'let doc = """',
      '  /* not a comment */ "',
      '  """',
      'let s = view.style["position_type"] /* a comment: view.style["position_type"] */',
      'let raw = #"a \\" /* "#; let t = view.style["position_type"]',
      'let marker2 = "\\(String("/*"))"; let u = view.style["position_type"]',
      'let raw2 = #"x \\#"# /* still a string "#; let v = view.style["position_type"]',
      'let marker3 = "\\(/* ) */ "/*")"',
      'let w = view.style["position_type"]',
      'let x = view.style["""',
      'position_type',
      '"""]',
    ].join('\n'));
    assert.deepEqual(directReads(root).map(r => `${r.file}:${r.line}`).sort(), [
      ...[2, 3, 6, 9, 10, 22, 23, 24, 26, 27, 29, 33].map(line => `kernel/src/new.rs:${line}`),
      ...[1, 2, 4].map(line => `kernel/src/glob.rs:${line}`),
      'kernel/src/parent.rs:1',
      ...[2, 6, 11].map(line => `kernel/src/nested.rs:${line}`),
      'kernel/src/grouped.rs:3',
      'kernel/src/fnmod.rs:3',
      ...[1, 2, 3, 4, 6, 7, 8, 9, 10].map(line => `kernel/src/rename.rs:${line}`),
      ...[1, 4, 5, 9, 10, 11, 12, 14, 15].map(line => `host/apple/Sources/New.swift:${line}`),
    ].sort());
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
