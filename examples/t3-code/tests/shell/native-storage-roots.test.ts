// Agent storage stays inside the roots supplied by Exact.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';
import { appPath } from '../support/app-files';

const source = (file: string) => Bun.file(appPath(file)).text();

describe('what #327 retires', () => {
  test('the module keeps its files where its context says (named roots under --storage), no TMPDIR re-rooting', async () => {
    const swift = await Promise.all(readdirSync(appPath('modules/apple')).filter(name => name.endsWith('.swift')).map(name => source(`modules/apple/${name}`)));
    expect(swift.filter(text => text.includes('T3Storage'))).toEqual([]);
    const module = await source('modules/apple/T3Module.swift');
    expect(module).toContain('transport = T3Transport(persistent: !context.agent, dataDirectory: context.data,');
  });
});
