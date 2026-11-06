// AttachmentFilePreview.tsx decodeFilePreviewText and the signed-asset rule (T3 Code 1e2ecbd975, MIT; see LICENSE-T3),
// as attachment-files.ts reads an attachment's first megabyte.
import { describe, expect, test } from 'bun:test';
import { PREVIEW_BYTES, assetUrl, decodePreview } from '../../src/panel/attachment-files';

describe('an attachment\'s text preview', () => {
  test('binary data and non-UTF-8 text are refused with the reference\'s sentences', () => {
    expect(decodePreview(new Uint8Array([0x61, 0, 0x62]), false)).toEqual({ ok: false, message: 'This file contains binary data and cannot be shown as text.' });
    expect(decodePreview(new Uint8Array([0x63, 0xe9]), false)).toEqual({ ok: false, message: 'This file is not UTF-8 text. Open it in another app to view its contents.' });
  });
  test('a cut inside a multi-byte sequence drops its leading bytes; past the megabyte is cut', () => {
    const text = `a${'é'.repeat(PREVIEW_BYTES / 2)}`, bytes = new TextEncoder().encode(text);
    const preview = decodePreview(bytes, true);
    expect(preview.ok).toBe(true);
    if (preview.ok) expect([new TextEncoder().encode(preview.text).length, preview.truncated]).toEqual([PREVIEW_BYTES - 1, true]);
  });
  test('only a signed asset route of an http(s) server is read', () => {
    expect(assetUrl('http://127.0.0.1:1/api/assets/tok/a.md')).toBe('http://127.0.0.1:1/api/assets/tok/a.md');
    expect([assetUrl('file:///etc/passwd'), assetUrl('http://127.0.0.1:1/api/other'), assetUrl('http://user@host/api/assets/x'), assetUrl('nonsense')]).toEqual([null, null, null, null]);
  });
});
