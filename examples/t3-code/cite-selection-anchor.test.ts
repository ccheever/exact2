// cite-selection-anchor (G4-03a, G4-03b/KW-9, DCP-E): Cite's place, its Tab, and where the comment opens after it, against
// T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): components/chat/AssistantSelectionToolbar.tsx (the capture-phase Tab, Escape,
// `removeAllRanges()` after `onCite`, the position from `rects.item(rects.length - 1)` and the toolbar's 8 pt hold),
// lib/selectionActions.ts (`resolveSelectionActionPosition`, the release point of `observeSelectionActions`) and
// AssistantCitationChip.tsx (`popupAnchor`: the range's last client rect, `side="bottom" align="end"`). The reference's
// numbers were read over CDP on the lane reference with the lane's Cite fixture answer, 1280×840 (scenarios A to E).
//
// The functions run from their `.contract` lines; the answer's and the window's actions run with Contract's action
// semantics (contract-action-fixture.ts).
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { contractComponent, type Env } from './contract-action-fixture';
import { contractFn } from './contract-fn-fixture';

const source = (file: string) => readFileSync(`${import.meta.dir}/${file}`, 'utf8');

/** The index of the bracket closing the one at `open`, over (), [], {} and template literals' `${…}`. */
function closing(s: string, open: number): number {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    const c = s[i]!;
    if (c === '"') { i = s.indexOf('"', i + 1); continue; }
    if ('([{'.includes(c)) depth++;
    else if (')]}'.includes(c) && --depth === 0) return i;
  }
  throw new Error(`unclosed bracket in ${s}`);
}

/**
 * A one-line Contract expression as JavaScript, for the few forms these functions use beyond contract-fn-fixture's: `match x
 * { case some(r) => a, case none => b }` over an option (`at` gives `undefined` for none), a shape's constructor with named
 * arguments (`CitePart(block=key, …)` is the record), `and`, `or`, `not`.
 */
function toJs(expr: string, shapes: string[]): string {
  let s = expr;
  for (let at = s.indexOf('match '); at >= 0; at = s.indexOf('match ')) {
    const brace = s.indexOf(' { case some(', at), end = closing(s, brace + 1);
    const subject = s.slice(at + 'match '.length, brace);
    const name = s.slice(brace + ' { case some('.length, s.indexOf(')', brace));
    const arms = s.slice(s.indexOf(') => ', brace) + ') => '.length, end);
    const split = arms.lastIndexOf(', case none => ');
    s = `${s.slice(0, at)}((${name}) => ${name} === undefined ? (${arms.slice(split + ', case none => '.length).trim()}) : (${arms.slice(0, split)}))(${subject})${s.slice(end + 1)}`;
  }
  for (const shape of shapes) {
    for (let at = s.indexOf(`${shape}(`); at >= 0; at = s.indexOf(`${shape}(`)) {
      const open = at + shape.length, end = closing(s, open);
      s = `${s.slice(0, at)}({ ${s.slice(open + 1, end).replace(/\b([a-z]\w*)=(?![=>])/g, '$1: ')} })${s.slice(end + 1)}`;
    }
  }
  return s.replace(/\band\b/g, '&&').replace(/\bor\b/g, '||').replace(/\bnot\b/g, '!');
}

const std = {
  min: Math.min, max: Math.max, length: (x: { length: number }) => x.length, at: (xs: unknown[], i: number) => xs.at(i),
  slice: (x: string | unknown[], a: number, b?: number) => x.slice(a, b), filter: (xs: unknown[], f: (x: never) => boolean) => xs.filter(f as never),
  map: (xs: unknown[], f: (x: never) => unknown) => xs.map(f as never), join: (xs: unknown[], j: string) => xs.join(j),
  concat: (a: unknown[], b: unknown[]) => [...a, ...b], indexOf: (xs: unknown[] | string, x: never) => xs.indexOf(x),
};

/** Every one-line `fn` of a `.contract` file as JavaScript, each seeing the others. */
function contractFns(file: string, extra: Env = {}): Record<string, (...args: never[]) => unknown> {
  const fns: Record<string, (...args: never[]) => unknown> = {};
  const scope: Env = { ...std, ...extra };
  for (const m of source(file).matchAll(/^fn (\w+)\(([^)]*)\): [^=]+ = (.+)$/gm)) {
    const params = [...m[2]!.matchAll(/(\w+):/g)].map(p => p[1]!);
    const body = toJs(m[3]!, ['CitePart', 'CiteOffer']);
    fns[m[1]!] = (...args: never[]) => new Function(...Object.keys(scope), ...params, `return (${body});`)(...Object.values(scope), ...args);
    scope[m[1]!] = fns[m[1]!];
  }
  return fns;
}

// The reference's lib/selectionActions.ts resolveSelectionActionPosition and AssistantSelectionToolbar's layout effect
// (the toolbar held 8 pt inside the window by its own size), verbatim in behavior.
function referencePosition(bounds: { left: number; top: number; width: number; height: number }, selectionRect: { right: number; bottom: number } | null,
  pointer: { x: number; y: number } | null, viewport: { width: number; height: number }, toolbar: { width: number; height: number }) {
  const preferred = pointer ?? { x: selectionRect?.right ?? bounds.left + bounds.width - 140, y: selectionRect ? selectionRect.bottom + 4 : bounds.top + 12 };
  const x = Math.max(8, Math.min(Math.max(bounds.left, preferred.x), bounds.left + bounds.width, viewport.width - 8));
  const y = Math.max(8, Math.min(Math.max(bounds.top, preferred.y), bounds.top + bounds.height, viewport.height - 8));
  return { left: Math.max(8, Math.min(x, viewport.width - toolbar.width - 8)), top: Math.max(8, Math.min(y, viewport.height - toolbar.height - 8)) };
}

const geometry = (x: number, y: number, width: number, height: number) => ({ x, y, width, height, unavailable: false });
const cite = contractFns('diff-cite.contract');
// The clone's timeline box at 1280×840 (the lane's layout: `transcript` 256,52 1024×788) and the window.
const transcript = geometry(256, 52, 1024, 788);
const place = (px: number, py: number, tooLong = false) => {
  const x = cite.citeAxis!(px as never, transcript.x as never, transcript.width as never, false as never) as number;
  const y = cite.citeAxis!(py as never, transcript.y as never, transcript.height as never, false as never) as number;
  return { left: cite.citeActionLeft!(x as never, 1280 as never, tooLong as never), top: cite.citeActionTop!(y as never, 840 as never) };
};
const reference = (pointer: { x: number; y: number } | null, rect: { right: number; bottom: number } | null, tooLong = false) =>
  referencePosition({ left: 256, top: 52, width: 1024, height: 788 }, rect, pointer, { width: 1280, height: 840 }, { width: tooLong ? 138 : 66, height: 28 });

describe('G4-03b: Cite at the release point, else at the selection\'s last line (resolveSelectionActionPosition)', () => {
  test('a pointer\'s selection: Cite\'s top-left is where it was released (the reference\'s A, B, C and D)', () => {
    // A: a drag from "parser" to the end of "paragraph", released at (420, 208): the reference's Cite at 420, 208.
    expect(place(420, 208)).toEqual({ left: 420, top: 208 });
    // B: the closing paragraph's two lines, released at its end (501.16, 385.52); C: the code, (607.98, 282.2);
    // D: a double-click on "closing" at (316.11, 362.77). The reference drew Cite at 501,385 / 607,282 / 316,362 (Chrome
    // snaps the fixed box to whole pixels).
    for (const [x, y] of [[501.16, 385.52], [607.98, 282.2], [316.11, 362.77]] as const) expect(place(x, y)).toEqual(reference({ x, y }, null));
  });

  test('a selection made otherwise: its last line\'s right and bottom + 4; both held in the timeline and 8 pt inside the window', () => {
    // The reference's A without a pointer: the last rect ended at 420, its bottom 216.3.
    expect(place(420, 216.3 + 4)).toEqual({ left: 420, top: 220.3 });
    // Held: left of the timeline, past its right edge (the window's right, less Cite's width and 8), below the window.
    for (const [x, y] of [[100, 300], [1279, 300], [600, 900], [600, 10], [1275, 835]] as const) {
      expect(place(x, y)).toEqual(reference({ x, y }, null));
      expect(place(x, y, true)).toEqual(reference({ x, y }, null, true));
    }
    expect(place(1279, 300)).toEqual({ left: 1280 - 66 - 8, top: 300 });
  });

  test('a text\'s part keeps its last selected line from the text\'s top-left; the end part is the one ending last', () => {
    const box = geometry(280, 351.42, 672, 45.5);
    const rects = [geometry(280, 352.5, 672, 21), geometry(280, 375.25, 221.2, 21)];
    expect(cite.citePart!('2' as never, 160 as never, 300 as never, rects as never, box as never)).toEqual({ block: '2', start: 160, end: 300, lx: 0, ly: 375.25 - 351.42, lw: 221.2, lh: 21 });
    // A reader paragraph reports no rectangle: the text's own frame stands for its line.
    expect(cite.citePart!('2' as never, 160 as never, 300 as never, [] as never, box as never)).toMatchObject({ lw: -1 });
    const parts = [{ block: '0-3', start: 16, end: 23, lx: 0, ly: 0, lw: 45, lh: 21 }, { block: '0-21', start: 150, end: 159, lx: 0, ly: 0, lw: 66.3, lh: 21 },
      { block: '0-5', start: 29, end: 39, lx: 0, ly: 0, lw: 87, lh: 19 }];
    expect(cite.citeEndKey!(parts as never)).toBe('0-21');
    const word = geometry(353.67, 196.65, 70.14, 22.75);
    expect(cite.citeLineRight!(parts as never, word as never)).toBeCloseTo(353.67 + 66.3, 5);
    expect(cite.citeLineBottom!(parts as never, word as never)).toBeCloseTo(196.65 + 21, 5);
    expect(cite.citeLineRight!([{ ...parts[1]!, lw: -1 }] as never, word as never)).toBeCloseTo(353.67 + 70.14, 5);
  });

  test('the comment after Cite opens under the selection\'s last line, align end, its text measured again (popupAnchor)', async () => {
    const offer = cite.citeOfferFor!('["t","a"]' as never, [{ block: '2', start: 160, end: 300, lx: 0, ly: 23.83, lw: 221.2, lh: 16.5 }] as never, 'stream' as never, 501 as never, 385 as never) as Env;
    expect(offer).toMatchObject({ source: '["t","a"]', ranges: '160-300', end: 'cite-["t","a"]-2', x: 501, y: 385, lx: 0, ly: 23.83, lw: 221.2, lh: 16.5 });
    // The timeline scrolled 40 pt up since: the line moves with its text.
    const frame = cite.citeLineFrame!(offer as never, geometry(280, 311.42, 672, 45.5) as never) as string;
    expect(frame).toBe(`280,${311.42 + 23.83},221.2,16.5`);
    expect(cite.citeLineFrame!(offer as never, { ...geometry(0, 0, 0, 0), unavailable: true } as never)).toBe('');
    expect(cite.citeLineFrame!({ ...offer, lw: -1 } as never, geometry(280, 311.42, 672, 45.5) as never)).toBe('280,311.42,672,45.5');
    // citation-comment.contract places the popup from that frame as the reference did: B's last rect ended at 501.2 with its
    // bottom at 393.8, and the reference's popup was 181.2..501.2 wide from 397.8.
    const left = await contractFn('citation-comment.contract', 'citationPopoverLeft', ['chip', 'width', 'viewportWidth'], { max: Math.max, min: Math.min });
    const anchor = { anchor: 'source', ax: 280, ay: 377.3, aw: 221.2, ah: 16.5 };
    expect(left(anchor, 320, 1280)).toBeCloseTo(181.2, 5);
    expect(anchor.ay + anchor.ah + 4).toBeCloseTo(397.8, 5);
  });
});

// The fixture answer's blocks, as diff-cite-popover.test.ts lays them out (the closing paragraph is block 2).
const blocks = [{ id: '0', kind: 'paragraph', text: '', href: '', runs: [{ id: '0', text: 'Cite check.' }] }, { id: '2', kind: 'paragraph', text: '', href: '',
  runs: [{ id: '0', text: 'A closing paragraph after the code block, long enough to wrap onto a second line in the timeline lane so a selection can end on its last line.' }] }];

function answer() {
  const md = contractComponent('markdown.contract', 'AnswerMarkdown');
  const fns = contractFns('markdown.contract');
  const calls: unknown[][] = [];
  const spy = (name: string) => (...args: unknown[]) => { calls.push([name, ...args]); };
  const state: Env = { parts: [], held: false, moved: false };
  const frames: Record<string, ReturnType<typeof geometry>> = { transcript, 'cite-["t","a"]-2': geometry(280, 351.42, 672, 45.5) };
  const scope = (offered = '') => ({
    ...std, ...cite, citeOffset: fns.citeOffset, citeRunOffset: fns.citeRunOffset, citeStream: fns.citeStream,
    document: { blocks }, code: [], source: '["t","a"]', citeOffered: offered,
    frame: (id: string) => frames[id] ?? { ...geometry(0, 0, 0, 0), unavailable: true },
    citeOfferSet: spy('offer'), citeWithdraw: spy('withdraw'), outsidePressDown: spy('windowDown'), outsidePressUp: spy('windowUp'),
    preventDefault: spy('preventDefault'), stopPropagation: spy('stopPropagation'), focus: spy('focus'),
  });
  const selection = (text: string, start: number, end: number, rects: ReturnType<typeof geometry>[]) => ({ text, start, end, rects });
  const line2 = [geometry(280, 352.5, 672, 21), geometry(280, 375.25, 221.2, 21)];
  return { md, state, scope, calls, selection, line2, offers: () => calls.filter(c => c[0] === 'offer').map(c => c[1] as Env) };
}

describe('G4-03b: the answer offers Cite as observeSelectionActions does', () => {
  test('a drag: Cite hides at the press, waits through the drag, and is offered at the release point', () => {
    const a = answer();
    a.md.call('down', a.state, a.scope('["t","a"]'), [{ buttons: 1, clientX: 280.5, clientY: 362.8 }]);
    // The window hears the press (and hides Cite there: below).
    expect(a.calls).toEqual([['windowDown', { buttons: 1, clientX: 280.5, clientY: 362.8 }]]);
    expect(a.state).toMatchObject({ held: true, moved: false });
    a.md.call('selected', a.state, a.scope(), ['2', '', a.selection('A closing paragraph', 0, 19, [geometry(280, 352.5, 140, 21)])]);
    a.md.call('selected', a.state, a.scope(), ['2', '', a.selection('A closing … last line.', 0, 140, a.line2)]);
    expect(a.offers()).toEqual([]);
    expect(a.state.moved).toBe(true);
    a.md.call('up', a.state, a.scope(), [{ buttons: 0, clientX: 501.16, clientY: 385.52 }]);
    expect(a.calls.at(-2)).toEqual(['windowUp']);
    // The stream: "Cite check." then a line break: the closing paragraph starts at 12.
    expect(a.offers()).toEqual([expect.objectContaining({ source: '["t","a"]', ranges: '12-152', x: 501.16, y: 385.52, end: 'cite-["t","a"]-2', lx: 0, lw: 221.2, lh: 21 })]);
    expect(a.offers()[0]!.ly).toBeCloseTo(375.25 - 351.42, 5);
    expect(a.state).toMatchObject({ held: false, moved: false });
  });

  test('a press that changes nothing (a button in the answer, a click on empty text) offers nothing at its release', () => {
    const a = answer();
    a.state.parts = [{ block: '2', start: 12, end: 31, lx: 0, ly: 1, lw: 140, lh: 21 }];
    a.md.call('down', a.state, a.scope('["t","a"]'), [{ buttons: 1, clientX: 900, clientY: 300 }]);
    a.md.call('up', a.state, a.scope(), [{ buttons: 0, clientX: 900, clientY: 300 }]);
    expect(a.offers()).toEqual([]);
    expect(a.calls.map(c => c[0])).toEqual(['windowDown', 'windowUp']);
  });

  test('a selection made without the pointer is offered at once at its last line; cleared, it is withdrawn', () => {
    const a = answer();
    a.md.call('selected', a.state, a.scope(), ['2', '', a.selection('A closing … last line.', 0, 140, a.line2)]);
    expect(a.offers()).toEqual([expect.objectContaining({ x: 280 + 221.2, y: 375.25 + 21 + 4 })]);
    a.md.call('selected', a.state, a.scope('["t","a"]'), ['2', '', a.selection('', 0, 0, [])]);
    expect(a.state.parts).toEqual([]);
    expect(a.calls.at(-1)).toEqual(['withdraw', '["t","a"]']);
  });
});

describe('G4-03a: a plain Tab focuses Cite (AssistantSelectionToolbar.tsx:87-107)', () => {
  const key = (k: string, mods: Partial<Record<'shiftKey' | 'altKey' | 'ctrlKey' | 'metaKey', boolean>> = {}) =>
    ({ key: k, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, code: '', repeat: false, ...mods });
  const parts = [{ block: '2', start: 12, end: 31, lx: 0, ly: 1, lw: 140, lh: 21 }];

  test('while this answer\'s Cite shows: preventDefault, stopPropagation, focus on Cite', () => {
    const a = answer();
    a.state.parts = parts;
    a.md.call('keys', a.state, a.scope('["t","a"]'), ['Tab', key('Tab')]);
    expect(a.calls).toEqual([['preventDefault'], ['stopPropagation'], ['focus', 'cite-action']]);
  });

  test('not with a modifier, not when no Cite of this answer shows, not when Cite is disabled (too long)', () => {
    for (const mods of [{ shiftKey: true }, { altKey: true }, { ctrlKey: true }, { metaKey: true }]) {
      const a = answer();
      a.state.parts = parts;
      a.md.call('keys', a.state, a.scope('["t","a"]'), ['Tab', key('Tab', mods)]);
      expect(a.calls).toEqual([]);
    }
    const hidden = answer();
    hidden.state.parts = parts;
    hidden.md.call('keys', hidden.state, hidden.scope('["t","other"]'), ['Tab', key('Tab')]);
    expect(hidden.calls).toEqual([]);
    const long = answer();
    long.state.parts = [{ block: '2', start: 0, end: 9000, lx: 0, ly: 0, lw: 1, lh: 1 }];
    const big = { ...long.scope('["t","a"]'), document: { blocks: [{ id: '2', kind: 'paragraph', text: '', href: '', runs: [{ id: '0', text: 'x'.repeat(9000) }] }] } };
    long.md.call('keys', long.state, big, ['Tab', key('Tab')]);
    expect(long.calls).toEqual([]);
  });

  test('Escape in the answer hides its Cite and keeps the selection (no preventDefault); Escape on Cite hides it too', () => {
    const a = answer();
    a.state.parts = parts;
    a.md.call('keys', a.state, a.scope('["t","a"]'), ['Escape', key('Escape')]);
    expect(a.calls).toEqual([['withdraw', '["t","a"]']]);
    expect(a.state.parts).toEqual(parts);
    const button = contractComponent('diff-cite.contract', 'CiteAction');
    const calls: string[] = [];
    button.call('keys', { hovering: false }, { preventDefault: () => calls.push('preventDefault'), dismiss: () => calls.push('dismiss') }, ['Escape']);
    expect(calls).toEqual(['preventDefault', 'dismiss']);
  });

  test('the Tab\'s focus draws Cite\'s ring (focus-visible:ring-2 ring-offset-1); the ring goes with the focus', () => {
    const button = contractComponent('diff-cite.contract', 'CiteAction');
    const state: Env = { hovering: false, focused: false };
    button.call('focusChanged', state, {}, [true]);
    expect(state.focused).toBe(true);
    button.call('focusChanged', state, {}, [false]);
    expect(state.focused).toBe(false);
    expect(source('diff-cite.contract')).toContain('focus=focusChanged(true) blur=focusChanged(false) disabled=tooLong');
    expect(source('diff-cite.contract')).toContain('box-shadow=(focused ? "0 0 0 1px light-dark(#fcfcfc, #0a0a0a), 0 0 0 3px light-dark(#1b4ed8, #346bf1), 0 8px 24px -12px #00000059" : "0 8px 24px -12px #00000059")');
  });
});

describe('the window draws Cite and sends it (app-window.contract)', () => {
  const window = contractComponent('app-window.contract', 'T3Window');
  const offer = { source: '["t","a"]', ranges: '12-152', stream: 'Cite check.\nA closing …', size: 140, x: 501, y: 385, end: 'cite-["t","a"]-2', lx: 0, ly: 23.83, lw: 221.2, lh: 21 };

  test('an offer shows on its thread; a withdrawal by another answer, or none, leaves it; the scroll hides it', () => {
    const state: Env = { citeOffer: {}, citeFor: '', citeThread: '' };
    window.call('citeOfferSet', state, { data: { threadId: 't' } }, [offer]);
    expect(state).toMatchObject({ citeOffer: offer, citeFor: '["t","a"]', citeThread: 't' });
    window.call('citeWithdraw', state, {}, ['["t","b"]']);
    expect(state.citeFor).toBe('["t","a"]');
    window.call('citeScrolled', state, {}, [0, 120]);
    expect(state.citeFor).toBe('');
    expect(source('app-window.contract')).toContain('derive citeShows = citeFor != "" and citeThread == data.threadId and not pageCover and not settingsOpen and not modal');
    expect(source('app-main.contract')).toContain('reachend=releaseFold scroll=citeScrolled');
  });

  test('a press anywhere but on Cite hides it (observeSelectionActions\' pointerdown); one on Cite keeps it for its press', () => {
    const at = (x: number, y: number) => {
      const state: Env = { outsideDown: false, citeFor: '["t","a"]' };
      window.call('outsidePressDown', state, { chipShown: false, detailsOpen: false, frame: (id: string) => id === 'cite-action' ? geometry(501, 385, 65.3, 28) : geometry(0, 0, 0, 0) },
        [{ buttons: 1, clientX: x, clientY: y }]);
      return state;
    };
    expect(at(530, 399)).toMatchObject({ outsideDown: true, citeFor: '["t","a"]' });
    expect(at(300, 362)).toMatchObject({ outsideDown: true, citeFor: '' });
    expect(at(1200, 30)).toMatchObject({ citeFor: '' });
  });

  test('the press sends the parts with the last line measured again, then clears the selection (removeAllRanges after onCite)', () => {
    const calls: unknown[][] = [];
    const state: Env = { citeOffer: offer, citeFor: '["t","a"]', citeThread: 't' };
    window.call('citePress', state, { ...cite, frame: () => geometry(280, 351.42, 672, 45.5), command: (...args: unknown[]) => calls.push(['command', ...args]),
      removeAllRanges: () => calls.push(['removeAllRanges']) }, []);
    expect(calls).toEqual([['command', 'diffreview', 'cite:["t","a"]', `12-152|280,${351.42 + 23.83},221.2,21|Cite check.\nA closing …`, 0], ['removeAllRanges']]);
    expect(state.citeFor).toBe('');
  });

  test('Cite is drawn by the window above the timeline (the reference portals it to the body), not inside the answer', () => {
    expect(source('app-window.contract')).toContain('when citeShows\n          CiteAction(offer=citeOffer, viewportWidth=viewport.width, viewportHeight=viewport.height, press=citePress, dismiss=citeWithdraw(citeOffer.source))');
    expect(source('markdown.contract')).toContain('column width="100%" pointerdown=down pointerup=up keycapture=keys\n      DiagramBlocks(');
    expect(source('diff-cite.contract')).toMatch(/button id="cite-action" retainFocus=true press=press key=keys hover=hover focus=focusChanged\(true\) blur=focusChanged\(false\) disabled=tooLong aria-label=\(tooLong \? "Selection is too long to cite" : "Cite selection in composer"\) testId=`cite-\$\{offer.source\}` position="absolute" left=citeActionLeft\(offer.x, viewportWidth, tooLong\) top=citeActionTop\(offer.y, viewportHeight\) z-index=50/);
  });
});

describe('DCP-E: the pencil\'s comment above its chip (no change: the clone already places it as the reference)', () => {
  test('side top, align end on the pencil, 4 pt off: the reference\'s E and the clone\'s base drive', async () => {
    const left = await contractFn('citation-comment.contract', 'citationPopoverLeft', ['chip', 'width', 'viewportWidth'], { max: Math.max, min: Math.min });
    const below = await contractFn('citation-comment.contract', 'citationPopoverBelow', ['chip', 'height', 'viewportHeight']);
    // The reference: the pencil 380.4..394.5 at 695.4; its popup 74.5..394.5, bottom 691.4.
    const reference = { anchor: 'chip', ax: 380.4, ay: 695.4, aw: 14.1, ah: 14.1 };
    expect(left(reference, 320, 1280)).toBeCloseTo(74.5, 5);
    expect(below(reference, 114, 840)).toBe(false);
    // The clone before this change (the lane drive, chip resource): the pencil 377..391 at 654.5; its popup 71..391, 536.5..650.5.
    const clone = { anchor: 'chip', ax: 377, ay: 654.5, aw: 14, ah: 14 };
    expect(left(clone, 320, 1280)).toBe(71);
    expect(below(clone, 114, 840)).toBe(false);
    expect(clone.ay - 4 - 114).toBe(536.5);
  });
});
