import { expect, test } from 'bun:test';
import { completeSave, requestSave, saveSnapshot } from '../../src/media/save-file';

test('multi-file app downloads wait for each Save panel to finish and ignore stale completions', () => {
  const client = {};
  requestSave(client, 'app:/tmp/one', 'one.csv'); requestSave(client, 'app:/tmp/two', 'two.csv');
  expect(saveSnapshot(client).saveRequest).toEqual({ from: 'app:/tmp/one', name: 'one.csv', nonce: 1 });
  completeSave(client, 1);
  expect(saveSnapshot(client).saveRequest).toEqual({ from: 'app:/tmp/two', name: 'two.csv', nonce: 2 });
  completeSave(client, 1);
  expect(saveSnapshot(client).saveRequest.nonce).toBe(2);
  completeSave(client, 2);
  expect(saveSnapshot(client).saveRequest.from).toBe('');
  requestSave(client, 'app:/tmp/three', 'three.csv');
  expect(saveSnapshot(client).saveRequest.nonce).toBe(3);
});
