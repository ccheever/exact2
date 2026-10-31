// The preview URL rules (T3 Code 74541bab17, MIT, see LICENSE-T3:
// packages/shared/src/preview.ts `isLoopbackHost`, `normalizePreviewUrl`, `resolveAddressBarInput`,
// `isValidSearchUrlTemplate`, `resolveSearchUrlTemplate`, `PreviewUrlNormalizationError`;
// apps/web/src/components/preview/errorCodeMessages.ts and previewConstants.ts
// `PREVIEW_ERROR_CODE_MESSAGES`). The URL field ("Search or enter URL") takes a bare loopback host as
// http, a bare public host as https, searches the chosen engine (Settings › Integrations › Browser ›
// Search engine) for text that is not an address, and refuses only empty input and an explicit
// unsupported scheme. The
// error is the reference's Effect TaggedError as a plain Error: the same fields and message, and
// never the raw URL (it can carry credentials or tokens).

import { isLocalLoopbackHost } from '../connections/host-classification';

/**
 * previewPdfAssetUrl (packages/shared/src/preview.ts, T3 Code 74541bab17): a server browser tab on one of the
 * environment's signed PDF assets shows the PDF from this client, since the server's headless browser cannot render
 * it (the server commits a placeholder page at the URL): the viewer page reads it (server-pdf-viewer.ts). A browser on
 * the host's loopback address reaches the asset through this client's own route to the environment (`httpBaseUrl`);
 * null for anything else.
 */
export function previewPdfAssetUrl(rawUrl: string, httpBaseUrl: string | null): string | null {
  if (!httpBaseUrl) return null;
  try {
    const url = new URL(rawUrl), baseUrl = new URL(httpBaseUrl);
    if ((url.protocol !== 'http:' && url.protocol !== 'https:') || !url.pathname.startsWith('/api/assets/') || !/\.pdf$/i.test(url.pathname)) return null;
    if (url.origin !== baseUrl.origin && !isLocalLoopbackHost(url.hostname) && url.hostname !== '0.0.0.0') return url.href;
    return new URL(`${url.pathname}${url.search}${url.hash}`, baseUrl).href;
  } catch { return null; }
}

const LOOPBACK_HOSTS: ReadonlySet<string> = new Set(['localhost', '127.0.0.1', '0.0.0.0', '::1']);
const LOOPBACK_PREFIX_PATTERN = /^(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1?\])(?::|\/|$)/i;

export function isLoopbackHost(host: string): boolean {
  return LOOPBACK_HOSTS.has(host) || host === '[::1]';
}

export type PreviewUrlNormalizationReason = 'empty' | 'parse' | 'unsupported-protocol';

export class PreviewUrlNormalizationError extends Error {
  readonly _tag = 'PreviewUrlNormalizationError';
  readonly inputLength: number;
  readonly reason: PreviewUrlNormalizationReason;
  readonly protocol?: string;
  constructor(input: { inputLength: number; reason: PreviewUrlNormalizationReason; protocol?: string; cause?: unknown }) {
    const protocol = input.protocol === undefined ? '' : `: ${input.protocol}`;
    super(`Invalid preview URL (${input.reason}${protocol}; input length ${input.inputLength}).`);
    this.name = 'PreviewUrlNormalizationError';
    this.inputLength = input.inputLength;
    this.reason = input.reason;
    if (input.protocol !== undefined) this.protocol = input.protocol;
    // Only a parse failure keeps its cause (the URL parser's own error), as the reference's schema does.
    if ('cause' in input) Object.defineProperty(this, 'cause', { value: input.cause, enumerable: false, writable: true, configurable: true });
  }
}

export const isPreviewUrlNormalizationError = (value: unknown): value is PreviewUrlNormalizationError => value instanceof PreviewUrlNormalizationError;

function previewUrlProtocol(rawUrl: string): string | undefined {
  return /^([A-Za-z][A-Za-z\d+.-]*):/.exec(rawUrl)?.[1]?.toLowerCase().concat(':');
}

/**
 * A free-form URL as a fully qualified `http(s)://` URL: bare loopback hosts (`localhost:5173`) get
 * `http://`, bare public hosts (`example.com`) `https://`, qualified URLs are validated and returned
 * as `URL.href`. Throws `PreviewUrlNormalizationError` for empty, unparseable or non-http(s) input.
 */
export function normalizePreviewUrl(rawUrl: string): string {
  const trimmed = rawUrl.trim();
  if (trimmed.length === 0) throw new PreviewUrlNormalizationError({ inputLength: rawUrl.length, reason: 'empty' });
  const useHttp = LOOPBACK_PREFIX_PATTERN.test(trimmed);
  const candidate = URL_SCHEME_WITH_AUTHORITY_PATTERN.test(trimmed) ? trimmed : `${useHttp ? 'http' : 'https'}://${trimmed}`;
  let parsed: URL;
  try {
    parsed = new URL(candidate);
  } catch (cause) {
    const protocol = previewUrlProtocol(candidate);
    throw new PreviewUrlNormalizationError({ inputLength: rawUrl.length, reason: 'parse', ...(protocol === undefined ? {} : { protocol }), cause });
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new PreviewUrlNormalizationError({ inputLength: rawUrl.length, reason: 'unsupported-protocol', protocol: parsed.protocol });
  }
  return parsed.href;
}

/** A bare host that is always an address: localhost, an IPv4 literal, or a bracketed IPv6 one. */
const ADDRESS_HOST_PATTERN = /^(?:localhost|\d{1,3}(?:\.\d{1,3}){3}|\[[\da-f:.]+\])$/i;
const BARE_HOST_PORT_PATTERN = /^[^/?#:@]+:\d+(?:[/?#]|$)/;
/** A scheme at the start, so `example.com/?next=https://example.org` is a bare host whose query holds another URL. */
const URL_SCHEME_WITH_AUTHORITY_PATTERN = /^[a-z][a-z\d+.-]*:\/\//i;
/** Registered schemes that are never a bare host, so `ftp:21` or `tel:5551234` is refused rather than read as host and port. */
const KNOWN_NON_WEB_SCHEMES: ReadonlySet<string> = new Set(['about:', 'blob:', 'chrome:', 'data:', 'file:', 'ftp:', 'javascript:', 'mailto:', 'sms:', 'ssh:', 'tel:', 'view-source:', 'ws:', 'wss:']);

/** contracts settings.ts BrowserSearchEngine: what the address bar searches with ("custom" uses the custom URL). */
export const BROWSER_SEARCH_ENGINES = ['duckduckgo', 'google', 'bing', 'brave', 'kagi', 'ecosia', 'custom'] as const;
export type BrowserSearchEngine = (typeof BROWSER_SEARCH_ENGINES)[number];
export const DEFAULT_BROWSER_SEARCH_ENGINE: BrowserSearchEngine = 'duckduckgo';
export const BROWSER_CUSTOM_SEARCH_URL_MAX_LENGTH = 2048;

/** Address bar search URL templates; `%s` is replaced with the encoded query. */
const BROWSER_SEARCH_URL_TEMPLATES: Record<Exclude<BrowserSearchEngine, 'custom'>, string> = {
  duckduckgo: 'https://duckduckgo.com/?q=%s',
  google: 'https://www.google.com/search?q=%s',
  bing: 'https://www.bing.com/search?q=%s',
  brave: 'https://search.brave.com/search?q=%s',
  kagi: 'https://kagi.com/search?q=%s',
  ecosia: 'https://www.ecosia.org/search?q=%s',
};

const fillSearchUrlTemplate = (template: string, query: string): string => template.split('%s').join(encodeURIComponent(query));

/**
 * Whether a custom search URL is an http(s) URL with `%s` where the query goes. `%s` in the host or credentials is
 * rejected: the query would never reach a search page, and a query with spaces would make the host unreachable.
 */
export function isValidSearchUrlTemplate(template: string): boolean {
  if (!template.includes('%s')) return false;
  try {
    // Two queries land on different authorities only when `%s` sits before the path.
    const authority = (url: URL) => `${url.username}:${url.password}@${url.host}`;
    const url = new URL(fillSearchUrlTemplate(template, 'a')), other = new URL(fillSearchUrlTemplate(template, 'b'));
    return (url.protocol === 'http:' || url.protocol === 'https:') && authority(url) === authority(other);
  } catch {
    return false;
  }
}

/** The search URL template for the chosen engine. An unusable custom template falls back to DuckDuckGo, so a query
 *  typed in the address bar always searches. */
export function resolveSearchUrlTemplate(settings: { readonly browserSearchEngine: BrowserSearchEngine; readonly browserCustomSearchUrl: string }): string {
  if (settings.browserSearchEngine !== 'custom') return BROWSER_SEARCH_URL_TEMPLATES[settings.browserSearchEngine];
  const custom = settings.browserCustomSearchUrl.trim();
  return isValidSearchUrlTemplate(custom) ? custom : BROWSER_SEARCH_URL_TEMPLATES.duckduckgo;
}

/**
 * What a person typed in the address bar as the URL to open: an address as `normalizePreviewUrl` reads it, or else a
 * web search for the text (`searchUrlTemplate`, DuckDuckGo by default). Text is an address when it has a scheme, or no spaces and a host that has a dot, a port, or
 * is localhost or an IP. Throws only for empty input or an explicit unsupported scheme (`ftp://`, `mailto:`, `data:`).
 */
export function resolveAddressBarInput(rawInput: string, searchUrlTemplate: string = BROWSER_SEARCH_URL_TEMPLATES.duckduckgo): string {
  const trimmed = rawInput.trim();
  if (URL_SCHEME_WITH_AUTHORITY_PATTERN.test(trimmed) || trimmed.length === 0) return normalizePreviewUrl(trimmed);
  if (!/\s/.test(trimmed)) {
    // `mailto:x` or `data:...` is an explicit scheme; `example.com:8080` or `devbox:8080` is a bare host and port.
    // `normalizePreviewUrl` would read `mailto:alice@example.com` as credentials on a bare host, so refuse here.
    const protocol = previewUrlProtocol(trimmed);
    if (protocol !== undefined && (KNOWN_NON_WEB_SCHEMES.has(protocol) || !BARE_HOST_PORT_PATTERN.test(trimmed))) {
      if (protocol !== 'http:' && protocol !== 'https:') throw new PreviewUrlNormalizationError({ inputLength: rawInput.length, reason: 'unsupported-protocol', protocol });
      return normalizePreviewUrl(trimmed);
    }
    const authority = trimmed.split(/[/?#]/, 1)[0] ?? '';
    const host = authority.replace(/^[^@]*@/, '').replace(/:\d+$/, '');
    const hasPort = host.length < authority.replace(/^[^@]*@/, '').length;
    if (hasPort || ADDRESS_HOST_PATTERN.test(host) || /^[^.]+(?:\.[^.]+)+\.?$/.test(host)) {
      try { return normalizePreviewUrl(trimmed); } catch { /* not a usable address after all: search for it */ }
    }
  }
  return fillSearchUrlTemplate(searchUrlTemplate, trimmed);
}

/** PREVIEW_ERROR_CODE_MESSAGES: the Chromium error names the load-failed page words in plain language. */
export const PREVIEW_ERROR_CODE_MESSAGES: Readonly<Record<string, string>> = Object.freeze({
  ERR_NAME_NOT_RESOLVED: 'DNS address could not be found',
  ERR_NAME_RESOLUTION_FAILED: 'DNS address could not be found',
  ERR_CONNECTION_REFUSED: 'Connection refused',
  ERR_CONNECTION_RESET: 'Connection was reset',
  ERR_CONNECTION_CLOSED: 'Connection was closed',
  ERR_CONNECTION_TIMED_OUT: 'Connection timed out',
  ERR_INTERNET_DISCONNECTED: 'No internet connection',
  ERR_TIMED_OUT: 'Connection timed out',
  ERR_CERT_AUTHORITY_INVALID: 'Certificate authority is not trusted',
  ERR_CERT_COMMON_NAME_INVALID: 'Certificate hostname mismatch',
  ERR_CERT_DATE_INVALID: 'Certificate is expired or not yet valid',
  ERR_TOO_MANY_REDIRECTS: 'Too many redirects',
});

/** describePreviewError: the friendly words for an error name, else the name itself, else "Network error". */
export function describePreviewError(description: string): string {
  const friendly = PREVIEW_ERROR_CODE_MESSAGES[description];
  if (friendly) return friendly;
  if (description.length > 0) return description;
  return 'Network error';
}

/** PreviewUnreachable's host line: the URL's host, else the URL as given. */
export function previewHost(url: string): string {
  try { return new URL(url).host || url; } catch { return url; }
}

/** PreviewUnreachable's error label: the error name, else `ERR_<code>`, else `ERR_FAILED`. */
export const previewErrorLabel = (code: number, description: string): string =>
  description.length > 0 ? description : `ERR_${Math.abs(code) || 'FAILED'}`;
