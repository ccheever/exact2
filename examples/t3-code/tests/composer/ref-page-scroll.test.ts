// paginatedBranches.ts shouldLoadNextBranchPageAfterScroll and BranchPicker.tsx maybeFetchNextBranchPage (T3 Code
// 1e2ecbd975, MIT; see LICENSE-T3), as ref-paging.contract RefPageScroll runs them on the list's scroll events.
import { describe, expect, test } from 'bun:test';
import { contractComponent } from '../support/contract-action-fixture';

const list = contractComponent('ref-paging.contract', 'RefPageScroll');
const at = (scrollTop: number, scrollHeight = 1000) => ({ scrollTop, scrollHeight, clientHeight: 224 });

function run(events: { scrollTop: number; scrollHeight: number; clientHeight: number }[]) {
  const asked: string[] = [], state = list.state();
  const scope = { name: 'strip-refs', refScrollEnd: (name: string) => { asked.push(name); } };
  for (const e of events) list.call('scrolled', state, scope, [0, e.scrollTop, e]);
  return asked;
}

describe('a ref list asks for its next page', () => {
  test('a scroll toward the end that lands within 96px of it', () => {
    expect(run([at(600), at(690)])).toEqual(['strip-refs']); // 1000 - 690 - 224 = 86
  });
  test('not further from the end, nor scrolling back, nor on the first event', () => {
    expect(run([at(500), at(600)])).toEqual([]); // 176 from the end
    expect(run([at(776), at(700)])).toEqual([]); // toward the start
    expect(run([at(700)])).toEqual([]); // no earlier position
  });
  test('once per content height: the page that grows the list arms the next ask', () => {
    expect(run([at(600), at(690), at(700), at(776), at(1700, 2000), at(1770, 2000)])).toEqual(['strip-refs', 'strip-refs']);
  });
});
