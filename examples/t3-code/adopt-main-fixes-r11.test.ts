// Agent storage stays inside the roots supplied by Exact.
import { describe, expect, test } from 'bun:test';
import { readdirSync } from 'node:fs';

const source = (file: string) => Bun.file(new URL(`./${file}`, import.meta.url)).text();

describe('what #327 retires', () => {
  test('the module keeps its files where its context says (named roots under --storage), no TMPDIR re-rooting', async () => {
    const dir = new URL('./modules/apple/', import.meta.url);
    const swift = await Promise.all(readdirSync(dir).filter(name => name.endsWith('.swift')).map(name => Bun.file(new URL(name, dir)).text()));
    expect(swift.filter(text => text.includes('T3Storage'))).toEqual([]);
    const module = await source('modules/apple/T3Module.swift');
    expect(module).toContain('transport = T3Transport(persistent: !context.agent, dataDirectory: context.data,');
  });
});
