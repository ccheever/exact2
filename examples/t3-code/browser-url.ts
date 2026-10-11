// browser-surface: the preview URL rules (T3 Code 1e2ecbd975, MIT, see LICENSE-T3:
// packages/shared/src/preview.ts `isLoopbackHost`, `normalizePreviewUrl`,
// `PreviewUrlNormalizationError`; apps/web/src/components/preview/errorCodeMessages.ts and
// previewConstants.ts `PREVIEW_ERROR_CODE_MESSAGES`). The URL field ("Search or enter URL") takes
// a bare loopback host as http, a bare public host as https, and validates everything else. The
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
