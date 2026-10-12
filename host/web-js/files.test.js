import { test, expect, mock } from 'bun:test';

mock.module('./rt.js', () => ({
  journal: [], clock: { now: 0, agent: true }, nextTicket: () => 1,
  inflight: { n: 0 }, Hosts: {}, OnHooks: {}, viewId: () => 1, Views: new Map(), data: {},
}));
mock.module('./pointer.js', () => ({ record: () => {} }));
mock.module('./navigation.js', () => ({ reportPlace: () => '' }));
mock.module('./admission.js', () => ({ appGrantSet: () => new Set(), coversPath: () => true }));

const { accepts, deliveredType, tokens } = await import('./files.js');

test('file input accept admits any type only when unrestricted', () => {
  const absent = tokens({ getAttribute: () => null });
  const empty = tokens({ getAttribute: () => '' });
  const mixed = tokens({ getAttribute: () => 'image/jpeg, */*' });
  expect(absent).toEqual([]);
  expect(empty).toEqual([]);
  expect(mixed).toEqual(['image/jpeg', '*/*']);
  for (const name of ['archive.xyz', 'notes.txt']) {
    expect(accepts(absent, name, 'application/octet-stream')).toBe(true);
    expect(accepts(empty, name, 'application/octet-stream')).toBe(true);
    expect(accepts(['*/*'], name, 'application/octet-stream')).toBe(true);
    expect(accepts(mixed, name, 'application/octet-stream')).toBe(true);
  }
  expect(accepts(['image/jpeg', '.txt'], 'archive.xyz', 'application/octet-stream')).toBe(false);
  expect(accepts(['image/jpeg', '.txt'], 'notes.txt', 'text/plain')).toBe(true);
});

test('a wildcard in a mixed accept preserves the original HEIC type', () => {
  expect(deliveredType('image/heic', ['image/jpeg', '*/*'])).toBe('image/heic');
  expect(deliveredType('image/heif', ['image/jpeg', '*/*'])).toBe('image/heif');
  expect(deliveredType('image/heic', ['image/jpeg'])).toBe('image/jpeg');
});
