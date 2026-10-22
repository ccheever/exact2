// Ported from T3 Code 74541bab17 (MIT, see LICENSE-T3): packages/shared/src/preview.test.ts, its
// `isLoopbackHost`, `normalizePreviewUrl`, `resolveAddressBarInput` and `resolveSearchUrlTemplate` tests under their own
// names (`it.each` rows as loops;
// `newPreviewTabId` stays with the server, which issues tab ids). Then the load-failed page's words
// (apps/web/src/components/preview/PreviewUnreachable.tsx, errorCodeMessages.ts), which the
// reference renders without a unit test of its own.
import { describe, expect, it } from 'bun:test';
import { BROWSER_SEARCH_ENGINES, describePreviewError, isLoopbackHost, isValidSearchUrlTemplate, normalizePreviewUrl, PreviewUrlNormalizationError, previewErrorLabel, previewHost, resolveAddressBarInput,
  resolveSearchUrlTemplate } from '../../src/browser/browser-url';

describe('resolveAddressBarInput', () => {
  for (const [input, expected] of [
    ['localhost:3000/?next=https://example.com', 'http://localhost:3000/?next=https://example.com'],
    ['127.0.0.1:3000/#https://example.com', 'http://127.0.0.1:3000/#https://example.com'],
    ['example.com/https://example.org', 'https://example.com/https://example.org'],
  ] as const) {
    it(`opens the outer address when it contains another URL: ${input}`, () => {
      expect(resolveAddressBarInput(input)).toBe(expected);
    });
  }

  it('opens what users type as addresses', () => {
    expect(resolveAddressBarInput('localhost:5173')).toBe('http://localhost:5173/');
    expect(resolveAddressBarInput('127.0.0.1:3000/path')).toBe('http://127.0.0.1:3000/path');
    expect(resolveAddressBarInput('my-box.tailnet.ts.net')).toBe('https://my-box.tailnet.ts.net/');
    expect(resolveAddressBarInput('cnn.com')).toBe('https://cnn.com/');
    expect(resolveAddressBarInput('devbox:8080')).toBe('https://devbox:8080/');
    expect(resolveAddressBarInput(' https://example.com/a b ')).toBe('https://example.com/a%20b');
    expect(resolveAddressBarInput('example.com:8080')).toBe('https://example.com:8080/');
    expect(resolveAddressBarInput('[::1]:3000')).toBe('http://[::1]:3000/');
    expect(resolveAddressBarInput('192.168.1.5:3000')).toBe('https://192.168.1.5:3000/');
  });

  it('searches for text that is not an address', () => {
    expect(resolveAddressBarInput('weather')).toBe('https://duckduckgo.com/?q=weather');
    expect(resolveAddressBarInput('how to center a div')).toBe('https://duckduckgo.com/?q=how%20to%20center%20a%20div');
    expect(resolveAddressBarInput('what is cnn.com')).toBe('https://duckduckgo.com/?q=what%20is%20cnn.com');
    expect(resolveAddressBarInput('what is https://example.com')).toBe('https://duckduckgo.com/?q=what%20is%20https%3A%2F%2Fexample.com');
    expect(resolveAddressBarInput('what is 10:30')).toBe('https://duckduckgo.com/?q=what%20is%2010%3A30');
    expect(resolveAddressBarInput('note: buy milk')).toBe('https://duckduckgo.com/?q=note%3A%20buy%20milk');
  });

  it('still rejects empty input and unsupported schemes', () => {
    expect(() => resolveAddressBarInput('  ')).toThrow(PreviewUrlNormalizationError);
    expect(() => resolveAddressBarInput('ftp://example.com')).toThrow(PreviewUrlNormalizationError);
    expect(() => resolveAddressBarInput('mailto:alice@example.com')).toThrow(PreviewUrlNormalizationError);
    expect(() => resolveAddressBarInput('data:text/plain,hello')).toThrow(PreviewUrlNormalizationError);
    // A known scheme with a numeric payload is not a host and port.
    for (const input of ['ftp:21', 'tel:5551234']) expect(() => resolveAddressBarInput(input)).toThrow(PreviewUrlNormalizationError);
    expect(resolveAddressBarInput('devbox:8080')).toBe('https://devbox:8080/');
  });

  it('searches with the given template and still opens addresses directly', () => {
    const google = resolveSearchUrlTemplate({ browserSearchEngine: 'google', browserCustomSearchUrl: '' });
    expect(resolveAddressBarInput('how to center a div', google)).toBe('https://www.google.com/search?q=how%20to%20center%20a%20div');
    expect(resolveAddressBarInput('cnn.com', google)).toBe('https://cnn.com/');
    expect(resolveAddressBarInput('a&b', 'https://search.example/?q=%s&lang=en')).toBe('https://search.example/?q=a%26b&lang=en');
  });
});

describe('resolveSearchUrlTemplate', () => {
  it('uses a valid custom template and falls back to DuckDuckGo for an unusable one', () => {
    expect(resolveSearchUrlTemplate({ browserSearchEngine: 'custom', browserCustomSearchUrl: ' https://search.example/?q=%s ' })).toBe('https://search.example/?q=%s');
    for (const browserCustomSearchUrl of ['https://search.example/?q=', "javascript:alert('%s')", 'https://%s.example.com/', 'https://%s@search.example/?q=%s']) {
      expect(resolveSearchUrlTemplate({ browserSearchEngine: 'custom', browserCustomSearchUrl })).toBe('https://duckduckgo.com/?q=%s');
    }
  });

  // Clone row: one query through every engine the setting offers, as the address bar sends it.
  it('searches each engine for a query', () => {
    const urls = Object.fromEntries(BROWSER_SEARCH_ENGINES.map(engine => [engine, resolveAddressBarInput('t3 code', resolveSearchUrlTemplate({ browserSearchEngine: engine, browserCustomSearchUrl: 'https://search.example/find?q=%s' }))]));
    expect(urls).toEqual({
      duckduckgo: 'https://duckduckgo.com/?q=t3%20code', google: 'https://www.google.com/search?q=t3%20code', bing: 'https://www.bing.com/search?q=t3%20code',
      brave: 'https://search.brave.com/search?q=t3%20code', kagi: 'https://kagi.com/search?q=t3%20code', ecosia: 'https://www.ecosia.org/search?q=t3%20code',
      custom: 'https://search.example/find?q=t3%20code',
    });
    expect(isValidSearchUrlTemplate('http://search.example/?q=%s')).toBe(true);
  });
});

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
