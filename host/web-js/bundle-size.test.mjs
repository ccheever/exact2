import { test } from 'bun:test';
import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';

test('RealWorld production app.js omits the file-body path', () => {
  const path = process.env.EXACT_REALWORLD_JS;
  assert.ok(path, 'build RealWorld with --production and set EXACT_REALWORLD_JS to its app.js');
  const js = readFileSync(path, 'utf8');
  assert.ok(!js.includes('exactBodyFrom must be an app:/ path'), 'file-body validation was linked into app.js');
});
