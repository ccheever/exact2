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

// Where the rule's readers live, and what a direct read looks like there.
// A Rust read is the field, or a destructuring of it.
const RUST = /\.position_type\b|[{,]\s*position_type\s*[,}]/;
const SCOPES = [
  { dir: 'kernel/src', ext: '.rs', read: RUST },
  { dir: 'host/apple/src', ext: '.rs', read: RUST },
  { dir: 'host/linux/src', ext: '.rs', read: RUST },
  { dir: 'host/apple/Sources', ext: '.swift', read: /"position_type"/ },
];

// The reads that mean the author's value, each with why. An entry matches
// one line of its file by a piece of its text, and must still be there.
export const ALLOWED = [
  { file: 'kernel/src/style/top_layer.rs', text: 'match style.position_type', why: 'the accessor itself' },
  { file: 'kernel/src/style.rs', text: 's.position = match self.position_type', why: "the engine mapping of the authored rows (to_taffy); top_layer::apply overrides it right after, in cover::apply" },
  { file: 'kernel/src/style.rs', text: 's.inset = if self.position_type == PositionType::Sticky', why: "a sticky box's insets aren't offsets (to_taffy); the top layer never applies to a sticky box" },
  { file: 'kernel/src/arena/defaults.rs', text: '.position_type', why: "the sticky machinery's own registry (refresh_sticky); the top layer never applies to a sticky box" },
  { file: 'kernel/src/kernel/sticky.rs', text: 'style.position_type != PositionType::Sticky', why: "the sticky machinery's own constraint (sticky_constraint); the top layer never applies to a sticky box" },
  { file: 'host/apple/src/style.rs', text: 'computed.position_type == exact_kernel::PositionType::Sticky', why: 'the style rows the Swift side gets: which rows a sticky box crosses with' },
  { file: 'host/linux/src/paint.rs', text: '|s| s.position_type)', why: "the sticky machinery's paint offset (LLP 1083 D4); the top layer never applies to a sticky box" },
  { file: 'host/apple/Sources/ExactKit/LayoutPosition.swift', text: 'style["position_type"]', why: 'the accessor itself' },
  { file: 'host/apple/Sources/ExactKit/ScrollAnchoring.swift', text: '"inset", "position_type",', why: "a key list, not a value: a change of the authored row is a layout change" },
  { file: 'host/apple/Sources/ExactKit/PositionArea.swift', text: 'popover.style["position_type"] == nil', why: "whether the author wrote `position` decides between the UA's fixed and absolute (CSS Positioned Layout 4, top layer styling)" },
  { file: 'host/apple/Sources/ExactCore/FlatPaint.swift', text: 'case "position_type":', why: 'flat leaves carry only data props (FlatLeaves.onlyData), never a popover, so authored is laid out' },
  { file: 'host/apple/Sources/ExactCore/FlatPaint.swift', text: 'positioned = ["relative", "absolute"].contains(style["position_type"]', why: 'flat leaves carry only data props (FlatLeaves.onlyData), never a popover, so authored is laid out' },
];

function* files(dir, ext) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* files(path, ext);
    else if (entry.name.endsWith(ext)) yield path;
  }
}

// A Rust file's test code: a `_tests.rs` or `tests.rs` file, or an inline
// `#[cfg(test)] mod … { … }`, blanked to its line breaks so lines keep their numbers.
function source(path, text) {
  if (/(^|_)tests\.rs$/.test(path.split('/').pop())) return '';
  const inline = /^#\[cfg\(test\)\]\s*\n\s*mod \w+\s*\{/gm;
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
      const lines = (ext === '.rs' ? source(path, readFileSync(path, 'utf8')) : readFileSync(path, 'utf8')).split('\n');
      lines.forEach((line, i) => {
        if (read.test(line) && !/^\s*\/\//.test(line)) out.push({ file, line: i + 1, text: line.trim() });
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

test('the scan finds a new read, and skips comments and test code', () => {
  const root = mkdtempSync(join(tmpdir(), 'iosfix-position-reads-'));
  try {
    for (const { dir } of SCOPES) mkdirSync(join(root, dir), { recursive: true });
    writeFileSync(join(root, 'kernel/src/new.rs'), [
      '// s.position_type in a comment',
      'fn f(s: &StyleProps) -> bool { s.position_type == PositionType::Absolute }',
      'fn g(StyleProps { position_type, .. }: &StyleProps) {}',
      '#[cfg(test)]',
      'mod tests {',
      '    fn t(s: &StyleProps) { if s.position_type == PositionType::Static {} }',
      '}',
    ].join('\n'));
    writeFileSync(join(root, 'host/linux/src/scan_tests.rs'), 'fn t(s: &StyleProps) { s.position_type; }');
    writeFileSync(join(root, 'host/apple/Sources/New.swift'), 'let p = view.style["position_type"]?.string');
    assert.deepEqual(directReads(root).map(r => `${r.file}:${r.line}`),
      ['kernel/src/new.rs:2', 'kernel/src/new.rs:3', 'host/apple/Sources/New.swift:1']);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
