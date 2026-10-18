// The preview URL rules (T3 Code 9c161462c9, MIT, see LICENSE-T3:
// packages/shared/src/preview.ts `isLoopbackHost`, `normalizePreviewUrl`, `resolveAddressBarInput`,
// `PreviewUrlNormalizationError`; apps/web/src/components/preview/errorCodeMessages.ts and
// previewConstants.ts `PREVIEW_ERROR_CODE_MESSAGES`). The URL field ("Search or enter URL") takes
// a bare loopback host as http, a bare public host as https, searches DuckDuckGo for text that is
// not an address, and refuses only empty input and an explicit unsupported scheme. The
// error is the reference's Effect TaggedError as a plain Error: the same fields and message, and
// never the raw URL (it can carry credentials or tokens).

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
  const candidate = trimmed.includes('://') ? trimmed : `${useHttp ? 'http' : 'https'}://${trimmed}`;
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

const SEARCH_URL = 'https://duckduckgo.com/?q=';
/** A bare host that is always an address: localhost, an IPv4 literal, or a bracketed IPv6 one. */
const ADDRESS_HOST_PATTERN = /^(?:localhost|\d{1,3}(?:\.\d{1,3}){3}|\[[\da-f:.]+\])$/i;
const BARE_HOST_PORT_PATTERN = /^[^/?#:@]+:\d+(?:[/?#]|$)/;
/** Registered schemes that are never a bare host, so `ftp:21` or `tel:5551234` is refused rather than read as host and port. */
const KNOWN_NON_WEB_SCHEMES: ReadonlySet<string> = new Set(['about:', 'blob:', 'chrome:', 'data:', 'file:', 'ftp:', 'javascript:', 'mailto:', 'sms:', 'ssh:', 'tel:', 'view-source:', 'ws:', 'wss:']);

/**
 * What a person typed in the address bar as the URL to open: an address as `normalizePreviewUrl` reads it, or else a
 * web search for the text. Text is an address when it has a scheme, or no spaces and a host that has a dot, a port, or
 * is localhost or an IP. Throws only for empty input or an explicit unsupported scheme (`ftp://`, `mailto:`, `data:`).
 */
export function resolveAddressBarInput(rawInput: string): string {
  const trimmed = rawInput.trim();
  if (/^[a-z][a-z\d+.-]*:\/\//i.test(trimmed) || trimmed.length === 0) return normalizePreviewUrl(trimmed);
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
  return `${SEARCH_URL}${encodeURIComponent(trimmed)}`;
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
