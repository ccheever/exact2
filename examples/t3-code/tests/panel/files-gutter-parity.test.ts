// The Files preview's gutter as T3 Code (1e2ecbd975) draws it over @pierre/diffs (Pierre's "+"
// at the number's right edge; a drag over the numbers selects lines and the draft opens at the end of any selection),
// and split view's hatched empty side. The reference's
// values were read over CDP (FilePreviewPanel's File, DiffPanel's split Cargo.lock), light and dark.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { T3Client } from '../../src/client/client';
import { fileComment, fileCommentLines, parseFileLineId, fileLineId } from '../../src/diff/diff-file-comments';
import { sourceGutter } from '../../src/panel/files-crumbs-settle';
import { obj, type Obj } from '../../src/client/domain';
import type { Native } from '../../src/client/protocol';
import { appPath } from '../support/app-files';

const source = (file: string) => readFileSync(appPath(file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };
const files = source('files-surface.contract');
const code = component(files, 'FilesCodeRow');

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
/** A native that records its requests. */
function harness() {
  const calls: Obj[] = [];
  const native: Native = { available: true, watch() {}, async later(input) {
    const request = obj(input); calls.push(request);
    return { ok: true, generation: 1, value: { applied: true } };
  } };
  const client = new T3Client();
  Object.assign(client, { environmentId: 'env', threadId: 't1', projectId: 'p1' });
  const op = (name: string, value = '') => fileComment(client, native, name, 'a.txt', value, text, 'panel');
  const view = () => fileCommentLines(client, 'a.txt', text, lines(text), false);
  const marks = () => view().filter(line => line.kind === 'line').map(line => `${line.line}${line.selected ? '*' : ''}${line.pin ? '+' : ''}`);
  const draft = () => view().find(line => line.kind === 'draft');
  return { client, native, calls, op, view, marks, draft };
}

describe('the Files preview\'s "+" is Pierre\'s utility button at the number\'s right edge', () => {
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
});

describe('a drag over the numbers selects lines, and the draft opens at the end of any line selection', () => {
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
