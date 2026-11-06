// The gutter hit-test id agrees with the id parsed by the data module.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { lineCellId } from '../../src/diff/diff-line-drag';
import { appPath } from '../support/app-files';

const source = (file: string) => readFileSync(appPath(file), 'utf8');
const component = (text: string, name: string) => { const start = text.indexOf(`component ${name}\n`); const next = text.indexOf('\ncomponent ', start + 1); return text.slice(start, next < 0 ? undefined : next); };

describe('the gutter hears the pointer', () => {
  const cell = component(source('diff-rows.contract'), 'DiffCell');

  test('each cell carries the id elementFromPoint names it by, as the panels parse it', () => {
    expect(cell).toContain('row id=`dl:${side}:${line}:${path}`');
    expect(cell).toContain('overId = `dl:${side}:${line}:${path}`');
    expect(lineCellId('additions', 3, 'docs/usage.md')).toBe('dl:additions:3:docs/usage.md');
  });
});
