// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): apps/desktop/src/preview/RecordingInput.test.ts, its 5
// tests (two describes, the it.each rows) under their own names. The functions live in the recording's page
// script, assets/browser-recording.js, which exports them when loaded by `require` instead of a page.
import { describe, expect, it } from 'bun:test';

type KeyPress = { key: string; metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean };
// eslint-disable-next-line @typescript-eslint/no-require-imports
const input = require('./assets/browser-recording.js') as {
  recordingKeyLabel: (input: KeyPress, isMac: boolean) => string | null;
  recordingKeysAreSensitive: (document: Document) => boolean;
};
const { recordingKeyLabel, recordingKeysAreSensitive } = input;

const key = (value: string, modifiers: Partial<{ metaKey: boolean; ctrlKey: boolean; altKey: boolean; shiftKey: boolean }> = {}): KeyPress => ({
  key: value, metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...modifiers,
});

describe('recording key labels', () => {
  it('formats macOS and other-platform shortcuts', () => {
    expect(recordingKeyLabel(key('c', { metaKey: true }), true)).toBe('⌘C');
    expect(recordingKeyLabel(key('c', { ctrlKey: true }), false)).toBe('Ctrl + C');
    expect(recordingKeyLabel(key('Tab', { altKey: true, shiftKey: true }), true)).toBe('⌥⇧⇥');
  });
  it('shows held modifiers once and labels navigation keys', () => {
    expect(recordingKeyLabel(key('Meta', { metaKey: true }), true)).toBe('⌘');
    expect(recordingKeyLabel(key('Shift', { shiftKey: true }), false)).toBe('Shift');
    expect(recordingKeyLabel(key('ArrowLeft'), true)).toBe('←');
    expect(recordingKeyLabel(key(' '), false)).toBe('Space');
  });
  it.each(['Dead', 'Unidentified', 'Process', ''])('excludes composition key %s', value => {
    expect(recordingKeyLabel(key(value), true)).toBeNull();
  });
});

describe('recording key privacy', () => {
  const field = (type: string) => ({ tagName: 'INPUT', getAttribute: () => type });
  const sensitive = (activeElement: unknown) => recordingKeysAreSensitive({ activeElement } as Document);
  it('excludes password fields and their shadow-root focus', () => {
    expect(sensitive(field('password'))).toBe(true);
    expect(sensitive({ shadowRoot: { activeElement: field('password') } })).toBe(true);
    expect(sensitive(field('text'))).toBe(false);
  });
  it('excludes iframe focus whose field cannot be inspected', () => {
    expect(sensitive({ tagName: 'IFRAME' })).toBe(true);
    expect(sensitive({ tagName: 'SECRET-FIELD' })).toBe(true);
  });
});
