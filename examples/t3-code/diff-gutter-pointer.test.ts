// What a person's pointer found in the diff gutter and the pending card, checked where a unit test reaches
// it. The gutter's drags also run in diff-line-drag.test.ts, pages-pr-code.test.ts and diff-review.test.ts; the shell
// menu's Services and the activity reporter's pointer in the AppKit tests (macos/tests/contextmenu text-menu.swift,
// macos/tests/activity); the card's press and the drags' input under an agent drive.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { lineCellId } from './diff-line-drag';

const source = (file: string) => readFileSync(join(import.meta.dir, file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };

describe('the gutter hears the pointer', () => {
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
    expect(app).toContain('else if op == "diffreview" and (id.startsWith("drag:") or id.startsWith("gutter:") or id.startsWith("comment:") or id == "to" or id == "end" or id == "press")\n');
    expect(app).toContain('      send lineDragChanged = command(op, id, value, n)');
    expect(app).toContain('    else if op == "pr-code-drag" or op == "pr-code-begin"\n      send lineDragChanged = command(`chatlocal:${op}`, id, value, 0)');
    const code = source('pages-pr-code.contract');
    expect(code).toContain('    if id.startsWith("drag:") or id.startsWith("gutter:") or id == "to" or id == "end" or id == "press"\n      local("pr-code-drag", ref, `${op}|${id}|${n}|${value}`)');
  });
  test('a file with a selection draws its "+" on the selection\'s bottom line only; a cell whose drag is in flight keeps it mounted', () => {
    expect(cell).toContain('if line > 0 and not open and (pin or not pinned or dragging)\n        button press=comment');
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

test('the pending card\'s dashed border takes no press: it is a border now, not a positioned overlay', () => {
  const card = component(source('pages-pr-threads.contract'), 'PrdPendingCard');
  expect(card).toContain('padding="0.75rem" box-sizing="border-box" border-radius="0.75rem" border-width=1 border-style="dashed" border-color=ptBorder()');
  expect(card).not.toMatch(/position="absolute"|stroke-dasharray|\bsvg width="100%"/);
  expect(card).toContain('button press=discard hover=hover aria-label="Discard this comment"');
});
