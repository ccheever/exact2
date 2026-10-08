import { expect, test } from 'bun:test';
import { mobileComposerAttachmentWireKindAndMime as wire, mobileUploadedAttachmentReference as reference } from './mobile-attachment-policy';
import { queuedEditImageMime } from './queued-edit-upload';

test('definite MIME beats misleading image filename in both callers', () => {
  const file = { kind: 'file' as const, name: 'report.png', mimeType: 'application/pdf' };
  expect(wire(file)).toEqual({ type: 'file', mimeType: 'application/pdf' });
  expect(queuedEditImageMime(file)).toBe('');
  expect(reference({ ...file, sizeBytes: 25 }, 'remote')).toEqual({ type: 'file', id: 'remote', name: 'report.png', mimeType: 'application/pdf', sizeBytes: 25 });
});
test('Files images promote only from supported or generic MIME evidence', () => {
  for (const mimeType of ['', 'application/octet-stream', 'BINARY/OCTET-STREAM', 'application/unknown; charset=utf-8']) {
    expect(wire({ kind: 'file', name: 'photo.JPG ', mimeType })).toEqual({ type: 'image', mimeType: 'image/jpeg' });
  }
  expect(wire({ kind: 'file', name: 'photo.png', mimeType: ' image/webp ; charset=binary' })).toEqual({ type: 'image', mimeType: 'image/webp' });
  expect(queuedEditImageMime({ name: 'photo.png', mimeType: ' IMAGE/JPEG; charset=binary' })).toBe('image/jpeg');
  for (const mimeType of ['image/svg+xml', 'image/heic', 'text/plain']) {
    expect(wire({ kind: 'file', name: 'photo.png', mimeType })).toEqual({ type: 'file', mimeType });
    expect(() => wire({ kind: 'image', name: 'photo.png', mimeType })).toThrow("Unsupported image type for 'photo.png'.");
  }
});
test('modern references preserve metadata and convert only pasted-text file source', () => {
  const file = { kind: 'file' as const, name: 'paste.txt', mimeType: 'text/plain;charset=utf-8', sizeBytes: 7 };
  for (const source of ['pasted-text', { _tag: 'pasted-text' }]) {
    expect(reference({ ...file, source }, 'remote')).toEqual({ type: 'file', id: 'remote', name: file.name, mimeType: file.mimeType, sizeBytes: 7, source: { _tag: 'pasted-text' } });
  }
  for (const source of [undefined, 'attached', {}, { _tag: 'unknown' }]) {
    expect(reference({ ...file, source }, 'remote')).not.toHaveProperty('source');
  }
});
test('modern image reference follows source omission including valid snapshot metadata', () => {
  const source = { kind: 'snap-shot', capturedAt: '2026-10-08T00:00:00.000Z', appName: 'Test', windowTitle: '' };
  for (const kind of ['image', 'file'] as const) {
    expect(reference({ kind, name: 'photo.png', mimeType: 'image/png', sizeBytes: 42, source }, 'remote')).toEqual({ type: 'image', id: 'remote', name: 'photo.png', mimeType: 'image/png', sizeBytes: 42 });
  }
});
