// realinput-1010f-followups: what the real-input session realinput-1010f found, checked where a unit test reaches it.
// RF-3's drags run in diff-line-drag.test.ts, pages-pr-code.test.ts and diff-review.test.ts; RF-1 and RF-5 in the AppKit
// tests (macos/tests/contextmenu text-menu.swift, macos/tests/activity); RF-4 and RF-3's input under the agent drive.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { lineCellId } from './diff-line-drag';

const source = (file: string) => readFileSync(join(import.meta.dir, file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };

describe('RF-3: the gutter hears the pointer', () => {
  const cell = component(source('diff-rows.contract'), 'DiffCell');
  test('the number and the "+" start a drag, follow the pointer and end it; the press no longer only selects', () => {
    expect(cell).toContain('pointerdown=down(false) pointermove=move pointerup=up testId=`diff-line-${path}-${side}-${line}`');
    expect(cell).toContain('button press=comment pointerdown=down(true) pointermove=move pointerup=up');
    expect(cell).not.toContain('press=pick');
    expect(cell).toContain('elementFromPoint(e.clientX, e.clientY)');
  });
  test('a drag\'s press, moves and release go out in turn on their own queued send, in both panels', () => {
    const app = source('app.contract');
    expect(app).toContain('mutation lineDragChanged as shape Change queue refreshes data');
    // The "+"'s press (`comment:` in the Diff panel, `pr-code-begin` in the Code tab) queues behind its drag's sends.
    expect(app).toContain('else if op == "diffreview" and (startsWith(id, "drag:") or startsWith(id, "gutter:") or startsWith(id, "comment:") or id == "to" or id == "end")\n');
    expect(app).toContain('      send lineDragChanged = command(op, id, value, n)');
    expect(app).toContain('    else if op == "pr-code-drag" or op == "pr-code-begin"\n      send lineDragChanged = command(`chatlocal:${op}`, id, value, 0)');
    const code = source('pages-pr-code.contract');
    expect(code).toContain('    if startsWith(id, "drag:") or startsWith(id, "gutter:") or id == "to" or id == "end"\n      local("pr-code-drag", ref, `${op}|${id}|${n}|${value}`)');
  });
  test('a file with a selection draws its "+" on the selection\'s bottom line only; a cell whose drag is in flight keeps it mounted', () => {
    expect(cell).toContain('when line > 0 and not open and (pin or not pinned or dragging)\n        button press=comment');
    expect(cell).toContain('opacity=(pin or (hovering and not pinned) ? 1 : 0)');
    const row = component(source('diff-rows.contract'), 'DiffRow');
    expect(row).toContain('selected=item.leftSelected, pinned=item.pinned, pin=item.leftPin,');
    expect(row).toContain('selected=item.rightSelected, pinned=item.pinned, pin=item.rightPin,');
    expect(row).toContain('selected=item.selected, pinned=item.pinned, pin=item.pin,');
  });
  test('each cell carries the id elementFromPoint names it by, as the panels parse it', () => {
    expect(cell).toContain('row id=`dl:${side}:${line}:${path}`');
    expect(cell).toContain('overId = `dl:${side}:${line}:${path}`');
    expect(lineCellId('additions', 3, 'docs/usage.md')).toBe('dl:additions:3:docs/usage.md');
  });
});

test('RF-4: the pending card\'s dashed border takes no press (it is positioned, so it paints over the trash)', () => {
  const card = component(source('pages-pr-threads.contract'), 'PrdPendingCard');
  expect(card).toContain('box position="absolute" left=0.5 top=0.5 right=0.5 bottom=0.5 pointer-events="none" aria-hidden=true');
  expect(card).toContain('button press=discard hover=hover aria-label="Discard this comment"');
});
