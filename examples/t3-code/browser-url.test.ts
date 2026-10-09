// Ported from T3 Code 1e2ecbd975 (MIT, see LICENSE-T3): packages/shared/src/preview.test.ts, its
// `isLoopbackHost` and `normalizePreviewUrl` tests under their own names (`it.each` rows as loops;
// `newPreviewTabId` stays with the server, which issues tab ids). Then the load-failed page's words
// (apps/web/src/components/preview/PreviewUnreachable.tsx, errorCodeMessages.ts), which the
// reference renders without a unit test of its own.
import { describe, expect, it } from 'bun:test';
import { describePreviewError, isLoopbackHost, normalizePreviewUrl, PreviewUrlNormalizationError, previewErrorLabel, previewHost } from './browser-url';

describe('isLoopbackHost', () => {
  for (const host of ['localhost', '127.0.0.1', '0.0.0.0', '::1', '[::1]']) {
    it(`${host} is loopback`, () => { expect(isLoopbackHost(host)).toBe(true); });
  }
  for (const host of ['example.com', '192.168.1.10', '10.0.0.1', '']) {
    it(`${host} is not loopback`, () => { expect(isLoopbackHost(host)).toBe(false); });
  }
});

describe('normalizePreviewUrl', () => {
  it('treats bare loopback hosts as http', () => {
    expect(normalizePreviewUrl('localhost:5173')).toBe('http://localhost:5173/');
    expect(normalizePreviewUrl('127.0.0.1:3000')).toBe('http://127.0.0.1:3000/');
  });

  it('treats bare public hosts as https', () => {
    expect(normalizePreviewUrl('example.com')).toBe('https://example.com/');
  });

  it('respects explicit schemes', () => {
    expect(normalizePreviewUrl('https://localhost:5173')).toBe('https://localhost:5173/');
    expect(normalizePreviewUrl('http://example.com/path?q=1')).toBe('http://example.com/path?q=1');
  });

  it('rejects empty input', () => {
    try {
      normalizePreviewUrl('   ');
      throw new Error('expected URL normalization to fail');
    } catch (error) {
      expect(error).toBeInstanceOf(PreviewUrlNormalizationError);
      expect(error).toMatchObject({ inputLength: 3, reason: 'empty' });
      expect(error).not.toHaveProperty('rawUrl');
      expect('cause' in (error as object)).toBe(false);
    }
  });

  it('rejects unsupported protocols', () => {
    try {
      normalizePreviewUrl('ftp://example.com');
      throw new Error('expected URL normalization to fail');
    } catch (error) {
      expect(error).toBeInstanceOf(PreviewUrlNormalizationError);
      expect(error).toMatchObject({ inputLength: 'ftp://example.com'.length, reason: 'unsupported-protocol', protocol: 'ftp:' });
    }
  });

  it('rejects unparseable input without retaining credentials or tokens', () => {
    const rawUrl = 'https://user:password@example.com:bad/path?access_token=secret#fragment';
    try {
      normalizePreviewUrl(rawUrl);
      throw new Error('expected URL normalization to fail');
    } catch (error) {
      expect(error).toBeInstanceOf(PreviewUrlNormalizationError);
      expect(error).toMatchObject({ inputLength: rawUrl.length, reason: 'parse', protocol: 'https:' });
      expect(error).not.toHaveProperty('rawUrl');
      expect((error as PreviewUrlNormalizationError).cause).toBeInstanceOf(Error);
      expect((error as PreviewUrlNormalizationError).message).not.toContain(((error as PreviewUrlNormalizationError).cause as Error).message);
      expect((error as PreviewUrlNormalizationError).message).not.toMatch(/user|password|access_token|secret|fragment/);
    }
  });
});

describe('the load-failed page (PreviewUnreachable)', () => {
  it('words a known Chromium error name, else the name, else "Network error"', () => {
    expect(describePreviewError('ERR_CONNECTION_REFUSED')).toBe('Connection refused');
    expect(describePreviewError('ERR_NAME_NOT_RESOLVED')).toBe('DNS address could not be found');
    expect(describePreviewError('ERR_SOMETHING_ELSE')).toBe('ERR_SOMETHING_ELSE');
    expect(describePreviewError('')).toBe('Network error');
  });

  it('names the host, else the URL, and labels the error by name, else by code', () => {
    expect(previewHost('http://localhost:16651/missing')).toBe('localhost:16651');
    expect(previewHost('not a url')).toBe('not a url');
    expect(previewErrorLabel(-1004, 'ERR_CONNECTION_REFUSED')).toBe('ERR_CONNECTION_REFUSED');
    expect(previewErrorLabel(-1004, '')).toBe('ERR_1004');
    expect(previewErrorLabel(0, '')).toBe('ERR_FAILED');
  });
});
