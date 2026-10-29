// Replaced held proposals' ids (scripts/tamber-replaced.mjs, tamber/1.17): a replacement's event kept, a chain followed
// to the latest with its maker, other events ignored, and at most 256 kept: one past that answers as never held (null,
// so the server's 404).
import { expect, test } from 'bun:test';
import { replacedProposals } from './tamber-replaced.mjs';

test('a chain answers its latest replacement and maker; other drops are not replacements', () => {
  const r = replacedProposals();
  r.record({ event: 'proposal-dropped', proposal: 'p1', client: 'bea', reason: 'edited', replacedBy: 'p2', editedBy: 'ada' });
  r.record({ event: 'proposal-dropped', proposal: 'p2', client: 'ada', reason: 'replaced', replacedBy: 'p3' });
  r.record({ event: 'proposal-dropped', proposal: 'q', client: 'bea', reason: 'dropped' });
  expect(r.stale('p1').error).toMatchObject({ stale: true, replacedBy: 'p3', by: 'ada' });
  expect(r.stale('p1').status).toBe(409);
  expect(r.stale('q')).toBeNull();
  expect(r.stale('never')).toBeNull();
});

test('at most 256 kept, oldest first', () => {
  const r = replacedProposals();
  for (let i = 0; i <= 256; i++) r.record({ event: 'proposal-dropped', proposal: `p${i}`, client: 'a', reason: 'replaced', replacedBy: `n${i}` });
  expect(r.stale('p0')).toBeNull();
  expect(r.stale('p1').error.replacedBy).toBe('n1');
  expect(r.stale('p256').error.replacedBy).toBe('n256');
});
