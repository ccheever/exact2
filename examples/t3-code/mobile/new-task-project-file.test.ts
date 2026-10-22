import { expect, test } from 'bun:test';
import { mobileParseProjectFile as parse } from './new-task-project-file';
import { resolveProjectSettings } from './shared/scoped-settings-plan';

test('project JSONC preserves quoted syntax and trims only schema-trimmed fields', () => {
  expect(parse(`// header
    {"$schema":" https://t3.codes/schema/t3.json ", /* block */
      "iconPath":" icons/t3.svg ","defaultThreadEnvMode":"worktree",
      "worktreeSubmodules":"top-level","scripts":[{
        "name":" Start ","command":" echo 'a,] // /* literal */' ",
        "previewUrl":" https://example.test/a,} ","icon":"play",
        "runOnWorktreeCreate":true,"async":false,"autoOpenPreview":true,
      },],
    }`)).toEqual({ $schema: ' https://t3.codes/schema/t3.json ', iconPath: 'icons/t3.svg',
      defaultThreadEnvMode: 'worktree', worktreeSubmodules: 'top-level', scripts: [{ name: 'Start',
        command: "echo 'a,] // /* literal */'", previewUrl: 'https://example.test/a,}', icon: 'play',
        runOnWorktreeCreate: true, async: false, autoOpenPreview: true }] });
});

test('absent optional values remain absent and unknown keys are stripped', () => {
  expect(parse('{}')).toEqual({});
  expect(parse('{"unknown":null,"scripts":[{"name":"a","command":"b","other":12}]}'))
    .toEqual({ scripts: [{ name: 'a', command: 'b' }] });
  expect(parse('{"$schema":""}')).toEqual({ $schema: '' });
  expect(parse('{"scripts":[{"name":"a","command":"b"}]}')).toEqual({ scripts: [{ name: 'a', command: 'b' }] });
});

test('invalid recognized values reject the whole file, including an unrelated script', () => {
  for (const value of [null, [], true, 1, 'text', { $schema: null }, { iconPath: '' }, { iconPath: '  ' },
    { defaultThreadEnvMode: ' worktree ' }, { defaultThreadEnvMode: null }, { worktreeSubmodules: 'all' },
    { scripts: {} }, { scripts: [null] }, { scripts: [{ name: 'n' }] },
    ...['name', 'command', 'previewUrl'].map(key => ({ scripts: [{ name: 'n', command: 'c', [key]: ' \n ' }] })),
    ...['runOnWorktreeCreate', 'async', 'autoOpenPreview', 'icon'].map(key => ({ scripts: [{ name: 'n', command: 'c', [key]: null }] })),
    { scripts: [{ name: 'n', command: 'c', icon: 'terminal' }] }]) {
    const contents = value && typeof value === 'object' && !Array.isArray(value)
      ? { defaultThreadEnvMode: 'worktree', ...value } : value;
    expect(parse(JSON.stringify(contents))).toBeNull();
  }
});

test('source limits count encoded UTF-16 path length and script entries', () => {
  expect(parse(JSON.stringify({ iconPath: 'x'.repeat(512) }))?.iconPath).toBe('x'.repeat(512));
  expect(parse(JSON.stringify({ iconPath: ' '.repeat(512) + 'x' }))).toBeNull();
  expect(parse(JSON.stringify({ iconPath: '😀'.repeat(256) }))?.iconPath).toBe('😀'.repeat(256));
  expect(parse(JSON.stringify({ iconPath: '😀'.repeat(257) }))).toBeNull();
  const scripts = Array.from({ length: 50 }, () => ({ name: 'n', command: 'c' }));
  expect((parse(JSON.stringify({ scripts }))?.scripts as unknown[]).length).toBe(50);
  expect(parse(JSON.stringify({ scripts: [...scripts, scripts[0]] }))).toBeNull();
});

test('JSON5 syntax, truncation and unclosed comments remain invalid', () => {
  for (const value of ['', ' ', '[{}]', '{', '{a:1}', "{'iconPath':'x'}", '{} trailing', '{"scripts":[}',
    '{"iconPath":"x"} /* unfinished', '{"iconPath":NaN}', '\uFEFF{}']) expect(parse(value)).toBeNull();
});

test('the scoped resolver owns defaults and receives null for invalid files', () => {
  const settings = { defaultThreadEnvMode: null, worktreeSubmodules: null };
  expect(resolveProjectSettings(settings, 'project', parse('{}')).settings).toMatchObject({ defaultThreadEnvMode: 'local', worktreeSubmodules: 'recursive' });
  expect(resolveProjectSettings(settings, 'project', parse('{"defaultThreadEnvMode":"worktree",}')).settings.defaultThreadEnvMode).toBe('worktree');
  expect(resolveProjectSettings(settings, 'project', parse('{"defaultThreadEnvMode":"worktree","scripts":[{}]}')).settings.defaultThreadEnvMode).toBe('local');
});
