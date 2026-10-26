// The design tool's pinned review digests (apps/design-tool/reviewed.ts, the third hands-on pass's #2): pinned when a
// draft is first proposed and kept through its re-proposals; another draft, its end or a new run drops it; one's own
// write moves it along only when it held the files that write read; a refusal re-pins to the server's files.
import { beforeEach, expect, test } from 'bun:test';
import { reviewed } from '../apps/design-tool/reviewed.ts';

const A = { 'app.tamber': 'a1', 'cards.tamber': 'c1' };
beforeEach(() => reviewed.end());

test('pinned at the first proposal and kept through re-proposals; another draft drops it', () => {
  expect(reviewed.pin('k', A)).toEqual(A);
  expect(reviewed.pin('k', { ...A, 'app.tamber': 'a2' })).toEqual(A);
  reviewed.pin('other', { ...A, 'app.tamber': 'a2' });
  expect(reviewed.get('k')).toBeUndefined();
});

test("one's own write moves the pin along when it held the files read; another's write in between does not", () => {
  reviewed.pin('k', A);
  reviewed.advance({ 'app.tamber': 'a1' }, { 'app.tamber': 'a2', 'cards.tamber': 'c1' });
  expect(reviewed.get('k')).toEqual({ ...A, 'app.tamber': 'a2' });
  // Someone else wrote a3 meanwhile: our write read a3, the pin still has a2, and keeps it (Apply is refused, by name).
  reviewed.advance({ 'app.tamber': 'a3' }, { 'app.tamber': 'a4' });
  expect(reviewed.get('k')).toEqual({ ...A, 'app.tamber': 'a2' });
  // A write of another file leaves the pin's other entries be.
  reviewed.advance({ 'cards.tamber': 'c1' }, { 'cards.tamber': 'c2' });
  expect(reviewed.get('k')).toEqual({ 'app.tamber': 'a2', 'cards.tamber': 'c2' });
});

test('a slow answer for a proposal since replaced pins nothing and drops nothing', () => {
  const old = reviewed.ask(), now = reviewed.ask();
  expect(reviewed.pin('211', A, now)).toEqual(A);
  // 210's answer lands after 211's review.
  expect(reviewed.pin('210', { ...A, 'app.tamber': 'b' }, old)).toEqual({ ...A, 'app.tamber': 'b' });
  expect(reviewed.get('211')).toEqual(A);
  expect(reviewed.get('210')).toBeUndefined();
  // And 211's re-proposal after another client's write keeps the first pin.
  expect(reviewed.pin('211', { ...A, 'app.tamber': 'c' }, reviewed.ask())).toEqual(A);
});

test("one's own write while a proposal is on its way: the pin it makes is moved along at once", () => {
  const t = reviewed.ask();
  reviewed.advance({ 'app.tamber': 'a1' }, { 'app.tamber': 'a2', 'cards.tamber': 'c1' });
  // The proposal, asked before the write, answers with the files it read before it.
  expect(reviewed.pin('k', A, t)).toEqual({ ...A, 'app.tamber': 'a2' });
});

test('a refusal re-pins to the server\'s files; the draft\'s end or a new run drops the pin', () => {
  reviewed.pin('k', A);
  reviewed.repin('k', { ...A, 'app.tamber': 'a9' });
  expect(reviewed.get('k')?.['app.tamber']).toBe('a9');
  reviewed.end('k');
  expect(reviewed.get('k')).toBeUndefined();
  reviewed.pin('k', A); reviewed.end();
  expect(reviewed.get('k')).toBeUndefined();
});
