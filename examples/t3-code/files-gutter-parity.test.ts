// files-gutter-parity: the Files preview's gutter as T3 Code (1e2ecbd975) draws it over @pierre/diffs (FG-1: Pierre's "+"
// at the number's right edge; FG-2: a drag over the numbers selects lines and the draft opens at the end of any selection),
// split view's hatched empty side (FG-3), and a press on any gutter closing a skill chip's details (FG-4). The reference's
// values were read over CDP (FilePreviewPanel's File, DiffPanel's split Cargo.lock), light and dark.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { T3Client } from './client';
import { fileComment, fileCommentLines, parseFileLineId, fileLineId } from './diff-file-comments';
import { closeChipFromPress } from './composer-chip-popover';
import { diffReview } from './diff-review';
import { prCodeLocal } from './pages-pr-code';
import { sourceGutter } from './r9-device-crumbs';
import { obj, type Obj } from './domain';
import type { Native } from './protocol';

const source = (file: string) => readFileSync(join(import.meta.dir, file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };
const files = source('r4-surfaces-files.contract');
const code = component(files, 'R4CodeRow');
const rows = source('diff-rows.contract');
const cell = component(rows, 'DiffCell');

/** CDP (1e2ecbd975), light then dark. */
const REFERENCE = {
  // FilePreviewPanel's [data-utility-button] on a hovered line of fixture.txt (a one-digit gutter at x 741, 33.3 wide).
  plus: { left: 764.47 - 741, size: 20, radius: 4, background: '#009fff', plus: ['#ffffff', '#111111'] },
  // DiffPanel split, Cargo.lock's 14 added lines: [data-gutter-buffer] flat; [data-content-buffer] transparent over the
  // code surface with repeating-linear-gradient(-45deg, transparent 0 4.242px, buffer 4.242px 5.656px), 8px tiles at 5px 0.
  buffer: { gutter: ['#f8f8f8', '#131313'], stripe: ['#e6e6e6', '#1d1d1d'], surface: ['#fcfcfc', '#0a0a0a'], period: 8, stripeWidth: 2, phase: 6 },
};

const text = 'one\ntwo\nthree\nfour\nfive\n';
const lines = (body: string) => body.split('\n').map((line, index) => ({ id: String(index + 1), number: String(index + 1), runs: [{ id: '0', text: line, syntax: '' }] }));
/** A native that records its requests; its skill chip is open as press `seq` until it is closed. */
function harness(chip: { open: boolean; seq: number } = { open: false, seq: 0 }) {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    if (request.op === 'editorChip') return { ok: true, generation: 1, value: { seq: chip.seq, open: chip.open } };
    if (request.op === 'editorChipClose') { if (request.seq === chip.seq) chip.open = false; return { ok: true, generation: 1, value: { seq: chip.seq, open: chip.open } }; }
    return { ok: true, generation: 1, value: { applied: true } };
  } };
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 't1', projectId: 'p1' });
  const op = (name: string, value = '') => fileComment(client, native, name, 'a.txt', value, text, 'panel');
  const view = () => fileCommentLines(client, 'a.txt', text, lines(text), false);
  const marks = () => view().filter(line => line.kind === 'line').map(line => `${line.line}${line.selected ? '*' : ''}${line.pin ? '+' : ''}`);
  const draft = () => view().find(line => line.kind === 'draft');
  const closes = () => calls.filter(call => call.op === 'editorChipClose').map(call => call.seq);
  return { client, native, calls, op, view, marks, draft, closes, chip };
}

describe('FG-1: the Files preview\'s "+" is Pierre\'s utility button at the number\'s right edge', () => {
  test('a 20pt #009fff square, radius 4, its plus in the surface\'s colour, at the reference\'s x', () => {
    expect(code).toContain(`position="absolute" left=(gutter - 9.83) top=0 width=${REFERENCE.plus.size} height=${REFERENCE.plus.size} padding=0 border-radius=${REFERENCE.plus.radius} border-width=0`);
    expect(code).toContain(`background-color="${REFERENCE.plus.background}"`);
    expect(code).toContain(`svg width=16 height=16 viewBox="0 0 16 16" color="light-dark(${REFERENCE.plus.plus.join(', ')})"`);
    // The one-digit gutter of a two-line file (fixture.txt) and a four-digit one: the number's right edge less 1ch + 2pt.
    expect(Math.abs(sourceGutter(2) - 9.83 - REFERENCE.plus.left)).toBeLessThan(0.1);
    expect(sourceGutter(2000) - 9.83).toBeCloseTo(sourceGutter(2000) - (7.8267 + 2), 1);
    expect(code).not.toContain('#1b4ed8');
    expect(code).not.toContain('left=2 top=2 width=16 height=16');
  });
  test('it is drawn on the selection\'s bottom line while one is held, else on the hovered line', () => {
    expect(code).toContain('when not open and (line.pin or not line.pinned or dragging)');
    expect(code).toContain('opacity=(line.pin or (hovering and not line.pinned) ? 1 : 0)');
  });
  test('the number takes no pointer and no text selection, so a press in the middle of it selects (it no longer lands on the "+")', () => {
    expect(code).toContain('text line.number position="absolute" left=0 top=0 width=gutter padding-right=9.83 box-sizing="border-box" pointer-events="none" user-select="none" aria-hidden=true');
    expect(code).toContain('box width=gutter flex-shrink=0 align-self="stretch" min-height="20px" press=comment pointerdown=down(false) pointermove=move pointerup=up aria-label=`Select line ${line.number}` testId=`file-line-${line.number}`');
    // The number comes after the code, as the diff's DiffNumber; the cell holds no text.
    expect(code.indexOf('text line.number')).toBeGreaterThan(code.indexOf('each run in line.runs'));
  });
});

describe('FG-2: a drag over the numbers selects lines, and the draft opens at the end of any line selection', () => {
  test('the row is named by its id; the gutter cell and the "+" hold the pointer and report the line under it', () => {
    expect(code).toContain('row id=`fl:${line.line}:${path}` width="100%"');
    expect(code).toContain('let hit = match elementFromPoint(e.clientX, e.clientY) { case some(id) => id, case none => "" }');
    expect(code).toContain('local("surface-files-comment-to", path, hit)');
    expect(code).toContain('button press=comment pointerdown=down(true) pointermove=move pointerup=up aria-label=`Comment on line ${line.number}`');
    expect(fileLineId(12, 'src/a b.ts')).toBe('fl:12:src/a b.ts');
    expect(parseFileLineId('fl:12:src/a b.ts')).toEqual({ path: 'src/a b.ts', line: 12 });
    expect(parseFileLineId('dl:additions:12:src/a.ts')).toBeNull();
    expect(parseFileLineId('fl:0:a.txt')).toBeNull();
  });
  test('the release and a press that opens the draft let go of the focus the press gave the gutter, so the draft takes it', () => {
    // A pressable node takes the focus on macOS, and autofocus waits while a node holds it (the after drive's first run:
    // the draft opened without the focus); FilePreviewPanel's beginComment blurs and the draft's textarea autofocuses.
    expect(code).toContain('  action up(e: PointerEvent)\n    if dragging\n      dragging = false\n      blur()\n      local("surface-files-comment-end", path, "")');
    expect(code).toContain('    if dragging and e.buttons == 0\n      dragging = false\n      blur()\n      local("surface-files-comment-end", path, "")');
    expect(code).toContain('  action comment\n    if not open\n      blur()\n      begin(line.line)');
    expect(source('diff-comments.contract')).toContain('textarea id="diff-comment-draft-text" value=text input=edit key=keys focus=focusIn blur=focusOut autofocus=true');
  });
  test('a click on a number selects its line and opens the draft under it at the release', async () => {
    const h = harness();
    await h.op('drag', '3');
    expect([h.marks(), h.draft()]).toEqual([['1', '2', '3*+', '4', '5', '6'], undefined]);
    await h.op('end');
    expect(h.draft()).toMatchObject({ line: 3, label: 'L3' });
    expect(h.marks()).toEqual(['1', '2', '3*', '4', '5', '6']);
    // The gutter cell's press after the release finds the draft open: one draft.
    await h.op('begin', '3');
    expect(h.view().filter(line => line.kind === 'draft')).toHaveLength(1);
  });
  test('a drag 1 → 3 paints its lines as it goes, the "+" on the bottom line, and opens the draft at the release', async () => {
    const h = harness();
    await h.op('drag', '1');
    await h.op('to', 'fl:2:a.txt');
    await h.op('to', 'fl:3:other.txt'); // another file's line: nothing
    await h.op('to', 'files-surface'); // not a line: nothing
    expect(h.marks()).toEqual(['1*', '2*+', '3', '4', '5', '6']);
    await h.op('to', 'fl:3:a.txt');
    expect([h.marks(), h.draft()]).toEqual([['1*', '2*', '3*+', '4', '5', '6'], undefined]);
    await h.op('end');
    expect(h.draft()).toMatchObject({ line: 3, label: 'L1 to L3' });
    // An open draft holds the gutter (enableLineSelection: false): a press changes nothing.
    await h.op('drag', '5'); await h.op('end');
    expect(h.draft()).toMatchObject({ line: 3, label: 'L1 to L3' });
    await h.op('cancel');
    expect([h.marks(), h.draft()]).toEqual([['1', '2', '3', '4', '5', '6'], undefined]);
  });
  test('a drag upwards keeps its anchor; the draft goes under the range\'s last line', async () => {
    const h = harness();
    await h.op('drag', '4'); await h.op('to', 'fl:2:a.txt');
    expect(h.marks()).toEqual(['1', '2*', '3*', '4*+', '5', '6']);
    await h.op('end');
    expect(h.draft()).toMatchObject({ line: 4, label: 'L2 to L4' });
  });
  test('a press on the "+" selects its line and opens the draft at the release; a keyboard press opens it too', async () => {
    const h = harness();
    await h.op('gutter', '2');
    expect(h.draft()).toBeUndefined();
    await h.op('end'); await h.op('begin', '2'); // the "+"'s click after its release
    expect(h.view().filter(line => line.kind === 'draft').map(line => line.label)).toEqual(['L2']);
    await h.op('cancel');
    await h.op('begin', '5'); // no pointer gesture: the line's draft
    expect(h.draft()).toMatchObject({ line: 5, label: 'L5' });
  });
  test('a "+" press while the gesture is in flight is its release (one draft)', async () => {
    const h = harness();
    await h.op('gutter', '2'); await h.op('to', 'fl:4:a.txt'); await h.op('begin', '2');
    expect(h.view().filter(line => line.kind === 'draft').map(line => line.label)).toEqual(['L2 to L4']);
  });
  test('the old press-only ops are gone', async () => {
    const h = harness();
    await h.op('line', '3'); await h.op('line-shift', '5');
    expect(h.marks().filter(mark => mark.includes('*'))).toEqual([]);
    expect(code).not.toContain('surface-files-comment-line');
  });
});

describe('FG-3: split view\'s empty side is hatched', () => {
  test('the gutter part is flat and the code part carries the stripes in the reference\'s colours, the row the code surface', () => {
    expect(cell).not.toContain('#e4e4e4');
    expect(cell).toContain(`tone == "empty" ? "light-dark(${REFERENCE.buffer.gutter.join(', ')})" : "#00000000")`);
    expect(cell).toContain(`tone == "addition" ? diffInk(scheme, "add-row") : "light-dark(${REFERENCE.buffer.surface.join(', ')})")`);
    expect(cell).toContain('when tone == "empty"\n        box position="absolute" left=((gutter - 2) * size / 13 + 2) top=0 right=0 bottom=0 overflow="hidden" pointer-events="none" aria-hidden=true');
    expect(cell).toContain('rect x=0 y=0 width="100%" height="100%" fill="url(#diff-buffer-stripes)"');
    expect(cell.match(new RegExp(`fill="light-dark\\(${REFERENCE.buffer.stripe.join(', ')}\\)"`, 'g'))).toHaveLength(2);
  });
  test('45° stripes rising to the right: the reference\'s share of the surface and phase, a period that divides the line', () => {
    const pattern = /pattern id="diff-buffer-stripes" patternUnits="userSpaceOnUse" width=(\d+) height=(\d+)/.exec(cell);
    expect(pattern).not.toBeNull();
    const period = Number(pattern![1]);
    expect(Number(pattern![2])).toBe(period);
    expect(20 % period).toBe(0); // a row at a time: the next row starts where this one's stripes end
    const polygons = [...cell.matchAll(/polygon points="([^"]+)"/g)].map(match => match[1]!.split(' ').map(pair => pair.split(',').map(Number) as [number, number]));
    expect(polygons).toHaveLength(2);
    // Each polygon is a band of constant x + y (a "/" stripe), the second the first one period on.
    const sums = polygons.map(points => points.map(([x, y]) => x + y));
    const bands = sums.map(values => [Math.min(...values), Math.max(...values)] as const);
    for (const points of polygons) for (const [x, y] of points) expect([x >= 0 && x <= period, y >= 0 && y <= period]).toEqual([true, true]);
    expect(bands[1]![0] - bands[0]![0]).toBe(period);
    const width = bands[0]![1] - bands[0]![0];
    expect(width / period).toBeCloseTo(REFERENCE.buffer.stripeWidth / REFERENCE.buffer.period, 5);
    expect(((bands[0]![0] + bands[0]![1]) / 2) % period).toBeCloseTo(REFERENCE.buffer.phase % period, 5);
  });
});

describe('FG-4: a press on a gutter closes a skill chip\'s details', () => {
  test('closeChipFromPress closes the newest open press, and nothing when none is open', async () => {
    const open = harness({ open: true, seq: 7 });
    await closeChipFromPress(open.native);
    expect([open.closes(), open.chip.open]).toEqual([[7], false]);
    const shut = harness({ open: false, seq: 7 });
    await closeChipFromPress(shut.native);
    expect(shut.closes()).toEqual([]);
  });
  test('the Files gutter: a press that starts a drag and one while the draft is open close it; its moves and release do not', async () => {
    const h = harness({ open: true, seq: 3 });
    await h.op('drag', '1');
    expect(h.closes()).toEqual([3]);
    h.chip.open = true; h.chip.seq = 4;
    await h.op('to', 'fl:2:a.txt'); await h.op('end');
    expect(h.closes()).toEqual([3]);
    await h.op('press'); // the draft is open: the press starts nothing
    expect(h.closes()).toEqual([3, 4]);
    h.chip.open = true; h.chip.seq = 5;
    await h.op('cancel'); await h.op('gutter', '2');
    expect(h.closes()).toEqual([3, 4, 5]);
  });
  test('the thread\'s Diff panel: a number, a "+" and a press that starts no drag', async () => {
    for (const op of ['drag:additions', 'drag:deletions:shift', 'gutter:additions', 'press']) {
      const h = harness({ open: true, seq: 9 });
      await diffReview(h.client, h.native, op, 'a.ts', op === 'press' ? 0 : 2);
      expect([op, h.closes()]).toEqual([op, [9]]);
    }
    const h = harness({ open: true, seq: 9 });
    await diffReview(h.client, h.native, 'to', 'dl:additions:3:a.ts', 0); await diffReview(h.client, h.native, 'end', 'a.ts', 0);
    expect(h.closes()).toEqual([]);
  });
  test('the pull request surface\'s Code tab beside the composer', async () => {
    const h = harness({ open: true, seq: 2 });
    const ctx = { client: { environmentId: 'env', local: {} } as unknown as T3Client, reference: { projectId: 'p1', repository: 'acme/app', number: 1 }, detail: null, native: h.native };
    await prCodeLocal(ctx, 'drag', 'diffreview|press|0|a.ts');
    expect(h.closes()).toEqual([2]);
  });
  test('every gutter reports its primary presses on the queued sends the drags take', () => {
    expect(cell).toContain('    else if e.buttons == 1\n      command("diffreview", "press", path, line)');
    expect(code).toContain('    else if e.buttons == 1\n      local("surface-files-comment-press", path, "")');
    expect(source('app.contract')).toContain('else if op == "diffreview" and (startsWith(id, "drag:") or startsWith(id, "gutter:") or startsWith(id, "comment:") or id == "to" or id == "end" or id == "press")\n');
    expect(source('app.contract')).toContain('    if startsWith(op, "surface-files-comment-")\n      send fileCommentChanged = command(`chatlocal:${op}`, id, value, 0)');
    expect(source('pages-pr-code.contract')).toContain('if startsWith(id, "drag:") or startsWith(id, "gutter:") or id == "to" or id == "end" or id == "press"\n      local("pr-code-drag"');
  });
});
