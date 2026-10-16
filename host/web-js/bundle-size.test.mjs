import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { need } from '../../scripts/coverage.mjs';

const check = need('fixture', process.env.EXACT_REALWORLD_JS ? null : 'build RealWorld with --production and set EXACT_REALWORLD_JS to its app.js');

check('RealWorld production app.js omits the file-body path', () => {
  const js = readFileSync(process.env.EXACT_REALWORLD_JS, 'utf8');
  assert.ok(!js.includes('exactBodyFrom must be an app:/ path'), 'file-body validation was linked into app.js');
});
