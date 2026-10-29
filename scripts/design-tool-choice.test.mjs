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
  // (offer [2,3] partly in is off, as its box shows; ticking it turns all of it in)
  expect(card('')).toEqual([0]);
  expect(card('p:2,3')).toEqual([0, 2, 3]);
  expect(card('p:0')).toEqual([]);
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
  // (false: the server's newest is Bo's, not this page's, which it doesn't overwrite)
  expect(await sent).toBe(false);
  await Promise.resolve();
  expect(heldChoice.base('q')).toEqual([3]);
  expect(heldChoice.shownStale()).toBe(true);
  heldChoice.drop([]); heldChoice.takeShown();
  // The page's own echo (the record of its own choose) moves no base.
  const two = later();
  const again = heldChoice.send('q', [1], () => two.p);
  await Promise.resolve(); await Promise.resolve(); // (its ask on its way, as any record of it comes after)
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

test('round 2: obsolete flips said until dropped; a lost answer sends again; a proposal briefly missing keeps its base; a send never overwrites a newer foreign choice; gen counts rebases; the cards are rebuilt', async () => {
  heldChoice.saw([{ proposal: 'r', choice: [0], choiceAt: 1 }], 'e5');
  heldChoice.takeShown();
  heldChoice.saw([{ proposal: 'r', choice: [1], choiceAt: 2 }], 'e5');
  expect(heldChoice.gen('r')).toBe(1);
  expect(heldChoice.shownStale()).toBe(true);
  heldChoice.takeShown();
  // (said again while picked still has them, a superseded answer losing nothing)
  expect(heldChoice.drop(['r:0'])).toEqual(['r:0']);
  expect(heldChoice.drop(['r:0'])).toEqual(['r:0']);
  expect(heldChoice.drop([])).toEqual([]);
  expect(heldChoice.stale('r:0')).toBe(false);
  // A choose whose answer is lost: the next one with the earlier items sends again.
  const asked = [];
  await heldChoice.send('r', [0], items => { asked.push(items); return Promise.resolve({ choiceAt: 3 }); });
  await heldChoice.send('r', [1], items => { asked.push(items); return Promise.resolve(null); });
  await heldChoice.send('r', [0], items => { asked.push(items); return Promise.resolve({ choiceAt: 5 }); });
  expect(asked).toEqual([[0], [1], [0]]);
  // Missing from one list (out being accepted), back in the next: the same base.
  heldChoice.saw([], 'e5');
  heldChoice.saw([{ proposal: 'r', choice: [0], choiceAt: 5 }], 'e5');
  expect(heldChoice.base('r')).toEqual([1]);
  expect(heldChoice.gen('r')).toBe(1);
  // Another client's choice lands while one of the page's is queued: the queued one doesn't overwrite it.
  const slow = later(), queuedAsk = [];
  const first = heldChoice.send('r', [2], () => slow.p);
  const second = heldChoice.send('r', [3], items => { queuedAsk.push(items); return Promise.resolve({ choiceAt: 9 }); });
  heldChoice.saw([{ proposal: 'r', choice: [4], choiceAt: 7 }], 'e5');
  slow.done({ choiceAt: 6 });
  expect(await first).toBe(false);
  expect(await second).toBe(false);
  expect(queuedAsk).toEqual([]);
  await Promise.resolve();
  expect(heldChoice.base('r')).toEqual([4]);
  expect(heldChoice.gen('r')).toBe(2);
});

test('round 3: a card is in step only as the page shows it built from its moved base, its obsolete flips gone; a partly chosen override is off, as shown', () => {
  heldChoice.saw([{ proposal: 's', choice: [0], choiceAt: 1 }], 'e6');
  heldChoice.takeShown();
  const built = heldChoice.gen('s');
  expect(heldChoice.inStep('s', 's:1', built)).toBe(true);
  heldChoice.saw([{ proposal: 's', choice: [0, 1], choiceAt: 2 }], 'e6');
  // (the page still shows the card built before, and picked still has a flip over it)
  expect(heldChoice.inStep('s', 's:1', built)).toBe(false);
  heldChoice.takeShown();
  expect(heldChoice.inStep('s', '', built)).toBe(false); // (built again, but not yet on the page)
  expect(heldChoice.inStep('s', 's:1', heldChoice.gen('s'))).toBe(false);
  expect(heldChoice.drop(['s:1'])).toEqual(['s:1']);
  expect(heldChoice.drop([])).toEqual([]);
  expect(heldChoice.inStep('s', '', heldChoice.gen('s'))).toBe(true);
  // Override [2,3] with only 2 in the base: off, as its box shows it, so neither is chosen.
  heldChoice.saw([{ proposal: 't', choice: [0, 2], choiceAt: 1 }], 'e6');
  expect(card('', 't')).toEqual([0]);
});

test('H2: the page\'s own choose whose answer was lost comes back as its own, not another client\'s; a same-name tab\'s different choice still rebases', async () => {
  heldChoice.saw([{ proposal: 'h', choiceAt: 0 }], 'e7', 'design-tool:Ada');
  heldChoice.takeShown();
  // The server records [0, 1] (at 1), and its answer never comes.
  expect(await heldChoice.send('h', [0, 1], () => Promise.resolve(null))).toBe(false);
  heldChoice.saw([{ proposal: 'h', choice: [0, 1], choiceBy: 'design-tool:Ada', choiceAt: 1 }], 'e7', 'design-tool:Ada');
  expect(heldChoice.gen('h')).toBe(0);
  expect(heldChoice.stale('h:1')).toBe(false);
  expect(heldChoice.shownStale()).toBe(false);
  // An accept names the record's choiceAt, read after the match: the server's stale check stays the authority.
  expect(heldChoice.at('h')).toBe(1);
  // Another tab under the same name chooses something else: that rebases, as another client's choice does.
  heldChoice.saw([{ proposal: 'h', choice: [1], choiceBy: 'design-tool:Ada', choiceAt: 2 }], 'e7', 'design-tool:Ada');
  expect(heldChoice.gen('h')).toBe(1);
  expect(heldChoice.base('h')).toEqual([1]);
});

test('H3: a flip made after another client\'s choice rebased the card counts; only the flips picked held when the rebase was taken are dropped', () => {
  heldChoice.saw([{ proposal: 'k', choice: [0], choiceAt: 1 }], 'e8', 'design-tool:Ada');
  heldChoice.takeShown();
  heldChoice.saw([{ proposal: 'k', choice: [1], choiceAt: 2, choiceBy: 'design-tool:Bo' }], 'e8', 'design-tool:Ada');
  // The page's next answer takes the flips it had then as obsolete.
  expect(heldChoice.drop(['k:2,3'])).toEqual(['k:2,3']);
  // A token unticked after that, before the page has dropped the old flip: it counts, and only the old one is dropped.
  expect(heldChoice.stale('k:1')).toBe(false);
  expect(heldChoice.drop(['k:2,3', 'k:1'])).toEqual(['k:2,3']);
  expect(card('k:2,3;k:1', 'k')).toEqual([]);
  heldChoice.drop(['k:1']);
  expect(heldChoice.inStep('k', 'k:1', heldChoice.gen('k'))).toBe(true);
});

test('H2, round 1 (Astra): a rebase ends the page\'s unanswered asks, so a same-name tab choosing them after it rebases; two lost answers both come back as the page\'s own', async () => {
  heldChoice.saw([{ proposal: 'm', choiceAt: 0 }], 'e9', 'design-tool:Ada');
  expect(await heldChoice.send('m', [0, 1], () => Promise.resolve(null))).toBe(false);
  heldChoice.saw([{ proposal: 'm', choice: [1], choiceBy: 'design-tool:Bo', choiceAt: 2 }], 'e9', 'design-tool:Ada');
  expect(heldChoice.gen('m')).toBe(1);
  heldChoice.saw([{ proposal: 'm', choice: [0, 1], choiceBy: 'design-tool:Ada', choiceAt: 3 }], 'e9', 'design-tool:Ada');
  expect(heldChoice.gen('m')).toBe(2);
  expect(heldChoice.base('m')).toEqual([0, 1]);
  // Two lost answers, and the poll shows the newest: the page's own.
  heldChoice.saw([{ proposal: 'n', choiceAt: 0 }], 'e9', 'design-tool:Ada');
  await heldChoice.send('n', [0], () => Promise.resolve(null));
  await heldChoice.send('n', [1], () => Promise.resolve(null));
  heldChoice.saw([{ proposal: 'n', choice: [1], choiceBy: 'design-tool:Ada', choiceAt: 2 }], 'e9', 'design-tool:Ada');
  expect(heldChoice.gen('n')).toBe(0);
  expect(heldChoice.at('n')).toBe(2);
});

test('H2, round 2 (Astra, Grok): an earlier lost ask\'s items chosen again by a same-name tab after the page moved on rebase, kept while the page\'s newer choose is on its way', async () => {
  // [0,1] lost, then [1] lost: a same-name tab's [0,1] (at 3) is another client's.
  heldChoice.saw([{ proposal: 'v', choiceAt: 0 }], 'e10', 'design-tool:Ada');
  await heldChoice.send('v', [0, 1], () => Promise.resolve(null));
  await heldChoice.send('v', [1], () => Promise.resolve(null));
  heldChoice.saw([{ proposal: 'v', choice: [0, 1], choiceBy: 'design-tool:Ada', choiceAt: 3 }], 'e10', 'design-tool:Ada');
  expect(heldChoice.gen('v')).toBe(1);
  expect(heldChoice.base('v')).toEqual([0, 1]);
  // [0] lost, [0,1] on its way (at 2): a same-name tab's [0] at 3 rebases once the page's own lands.
  heldChoice.saw([{ proposal: 'w', choiceAt: 0 }], 'e10', 'design-tool:Ada');
  await heldChoice.send('w', [0], () => Promise.resolve(null));
  const slow = later();
  const sent = heldChoice.send('w', [0, 1], () => slow.p);
  await Promise.resolve(); await Promise.resolve();
  heldChoice.saw([{ proposal: 'w', choice: [0], choiceBy: 'design-tool:Ada', choiceAt: 3 }], 'e10', 'design-tool:Ada');
  slow.done({ choiceAt: 2 });
  expect(await sent).toBe(false);
  await Promise.resolve();
  expect(heldChoice.gen('w')).toBe(1);
  expect(heldChoice.base('w')).toEqual([0]);
  expect(heldChoice.at('w')).toBe(3);
});
