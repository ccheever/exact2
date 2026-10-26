// The design tool's held choice (apps/design-tool/choice.ts, tamber/1.18 `choose`), pure: a card's base and the
// page's flips over it, a rebase by another client's choice making the page's flips obsolete before anything sends
// them, a newer record kept while the page's own choose is on its way and settled when it lands, an undone tick still
// sending its card's choice, a failed choose said, an override's items turned over together, and a new server run.
import { expect, test } from 'bun:test';
import { chosenOf, heldChoice } from '../apps/design-tool/choice.ts';

const r = { tokens: 2, offers: [{ items: [2, 3] }] }, ops = [0, 1, 2, 3], card = (picked, id = 'p') => chosenOf(id, picked, r, ops, { proposal: id });
const later = () => { let done; const p = new Promise(ok => { done = ok; }); return { p, done }; };

test('a base from the server, the page\'s flips over it; an override turned over whole; a rebase makes flips obsolete at once', () => {
  heldChoice.saw([{ proposal: 'p', choice: [0, 2], choiceAt: 1 }], 'e1');
  expect(card('')).toEqual([0, 2]);
  // (offer [2,3] began partly in: ticking it turns all of it in, as its unticked box says)
  expect(card('p:2,3')).toEqual([0, 2, 3]);
  expect(card('p:0')).toEqual([2]);
  // Another client chooses [1]: the base moves, and the page's flip over the old base counts no more, before any drop.
  heldChoice.saw([{ proposal: 'p', choice: [1], choiceAt: 2 }], 'e1');
  expect(card('p:0')).toEqual([1]);
  expect(heldChoice.drop(['p:0'])).toEqual(['p:0']);
  expect(card('p:0')).toEqual([1]);
  // Once dropped from picked, a new flip counts again.
  heldChoice.drop([]);
  expect(card('p:0')).toEqual([0, 1]);
  expect(heldChoice.at('p')).toBe(2);
});

test('a newer record kept while the page\'s own choose is on its way: its echo moves nothing, another\'s rebases when it lands', async () => {
  heldChoice.saw([{ proposal: 'q', choiceAt: 0 }], 'e2');
  const one = later();
  const sent = heldChoice.send('q', [0], () => one.p);
  // Bo's choice (at 2) lands in the list before the page's own answer (at 1): kept, not yet applied.
  heldChoice.saw([{ proposal: 'q', choice: [3], choiceAt: 2 }], 'e2');
  expect(heldChoice.base('q')).toBe(null);
  one.done({ choiceAt: 1 });
  expect(await sent).toBe(true);
  await Promise.resolve();
  expect(heldChoice.base('q')).toEqual([3]);
  expect(heldChoice.pendingDrop()).toBe(true);
  heldChoice.drop([]);
  // The page's own echo (the record of its own choose) moves no base.
  const two = later();
  const again = heldChoice.send('q', [1], () => two.p);
  heldChoice.saw([{ proposal: 'q', choice: [1], choiceAt: 3 }], 'e2');
  two.done({ choiceAt: 3 });
  await again; await Promise.resolve();
  expect(heldChoice.base('q')).toEqual([3]);
  expect(heldChoice.at('q')).toBe(3);
});

test('a tick undone before its answer still sends the card\'s choice; a failed choose says so; a new run forgets, late answers ignored', async () => {
  heldChoice.saw([{ proposal: 'u', choiceAt: 0 }], 'e3');
  const asked = [];
  const ask = items => { asked.push(items); return Promise.resolve({ choiceAt: asked.length }); };
  heldChoice.send('u', [0], ask);
  expect(heldChoice.touchedFor('u')).toBe(true);
  await heldChoice.send('u', [], ask);
  expect(asked).toEqual([[0], []]);
  expect(await heldChoice.send('u', [1], () => Promise.resolve(null))).toBe(false);
  const slow = later();
  const late = heldChoice.send('u', [2], () => slow.p);
  heldChoice.saw([{ proposal: 'u', choice: [5], choiceAt: 9 }], 'e4');
  slow.done({ choiceAt: 10 });
  expect(await late).toBe(false);
  expect(heldChoice.base('u')).toEqual([5]);
  expect(heldChoice.at('u')).toBe(9);
});
