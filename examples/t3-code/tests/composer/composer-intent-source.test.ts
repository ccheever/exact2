import { describe, expect, test } from 'bun:test';
import { composerIntentAnswer, composerIntentSource, type ComposerIntent } from '../../src/composer/composer-intent-source';
import { composerSourceSession } from '../../src/composer/composer-rich-state';

const key = (value: string): ComposerIntent => ({ kind: 'key', mode: 'rich', snapshotOwner: 'snapshot', owner: 'draft', value, serial: 3,
  start: value.length, end: value.length, composing: false, key: 'Enter', shift: false, command: false });
describe('guarded composer intents', () => {
  test('source boundary decodes the admitted command and refuses malformed variants', () => {
    const client = { snapshotOwner: 'snapshot' };
    composerSourceSession(client).observe('draft', 'text');
    const args = ['snapshot', 'draft', 'text', 1, 4, 4, 'key', '', 'Enter', false, false, false, 'rich'];
    expect(composerIntentAnswer(client, args).kind).toBe('send');
    expect(composerIntentAnswer(client, args.slice(0, -1)).kind).toBe('stale');
    expect(composerIntentAnswer(client, [...args.slice(0, 6), 'unknown', ...args.slice(7)]).kind).toBe('stale');
    expect(composerIntentAnswer(client, ['snapshot', 'draft', 'text', 1, 4, 4, 'toggle', true, '', false, false, false]).kind).toBe('stale');
  });
  test('carries exact source/owner/serial with a canonical one-command edit', () => {
    const client = { snapshotOwner: 'snapshot' }, intent = key('```ts');
    composerSourceSession(client).observe(intent.owner, intent.value);
    expect(composerIntentSource(client, intent)).toEqual({ owner: 'draft', value: '```ts', serial: 3, kind: 'edit', mode: 'rich',
      start: 0, end: 5, text: '```ts\n\n```', selectionStart: 6, selectionEnd: 6, preserveSelection: false });
  });
  test('refuses composition, invalid ranges, or a source/owner no longer observed', () => {
    const client = { snapshotOwner: 'snapshot' }, intent = key('text');
    composerSourceSession(client).observe(intent.owner, intent.value);
    for (const changed of [
      { composing: true }, { snapshotOwner: 'previous' }, { owner: 'another-draft' }, { value: 'old' },
      { start: -1 }, { end: 100 }, { serial: Number.NaN },
    ]) expect(composerIntentSource(client, { ...intent, ...changed }).kind).toBe('stale');
    expect(composerIntentSource(client, intent).kind).toBe('send');
  });
  test('header intents preserve native selection and refuse a stale owner', () => {
    const client = { snapshotOwner: 'snapshot' }, value = '```ts\nx\n```\n- [ ] task';
    composerSourceSession(client).observe('draft', value);
    const language: ComposerIntent = { ...key(value), kind: 'language', start: 0, end: 5, language: 'rust' };
    expect(composerIntentSource(client, language)).toMatchObject({ kind: 'edit', start: 3, end: 5, text: 'rust', preserveSelection: true });
    expect(composerIntentSource(client, { ...language, owner: 'old' }).kind).toBe('stale');
    expect(composerIntentAnswer(client, ['snapshot', 'draft', value, 4, 12, 22, 'toggle', 'checked', '', false, false, false, 'rich']).kind).toBe('stale');
  });
  test('formatting is the Markdown field\'s own command, never a source edit', () => {
    const client = { snapshotOwner: 'snapshot' }, intent: ComposerIntent = { ...key('word'), key: 'Bold', start: 0, end: 4, command: true };
    composerSourceSession(client).observe('draft', intent.value);
    expect(composerIntentSource(client, intent).kind).toBe('default');
    for (const mode of ['plain', 'literal'] as const) {
      composerSourceSession(client).observe('draft', intent.value, '', mode);
      expect(composerIntentSource(client, { ...intent, mode }).kind).toBe('default');
    }
  });
});
