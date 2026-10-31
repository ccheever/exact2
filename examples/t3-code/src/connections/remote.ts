// packages/shared/src/remote.ts at T3 Code 6d450a4284: readPairingAppLink and the pairingUrl branch of
// resolveRemotePairingTarget. Exact uses ClientError instead of Effect's tagged
// errors and returns the native bridge's origin/credential pair. Settings keeps
// its separate host-plus-code parser in protocol.ts.
import { ClientError } from '../client/protocol';

const supportedProtocols = new Set(['http:', 'https:', 'ws:', 'wss:']);
const APP_LINK_PROTOCOL = /^t3code(?:-[a-z]+)?:$/;

/** The pairing URL a `t3code://pair?pairingUrl=…` link carries, on any app channel scheme; null for any other link. */
export function readPairingAppLink(value: string): string | null {
  let url: URL;
  try { url = new URL(value); } catch { return null; }
  if (!APP_LINK_PROTOCOL.test(url.protocol) || url.host !== 'pair') return null;
  if (url.pathname !== '' && url.pathname !== '/') return null;
  return url.searchParams.get('pairingUrl')?.trim() || null;
}

export function resolveRemotePairingTarget(input: { pairingUrl: string }): { origin: string; credential: string } {
  let url: URL;
  try { url = new URL(input.pairingUrl.trim()); }
  catch { throw new ClientError('Pairing URL is invalid.'); }
  if (!supportedProtocols.has(url.protocol)) throw new ClientError('Pairing URL is invalid.');

  const hash = new URLSearchParams(url.hash.replace(/^#/, ''));
  const credential = hash.get('token')?.trim() || url.searchParams.get('token')?.trim() || '';
  if (!credential) throw new ClientError('Pairing URL is missing its token.');

  const host = url.searchParams.get('host')?.trim();
  if (host) {
    const normalized = host.replace(/^\/+/, '');
    try { url = new URL(/^[a-zA-Z][a-zA-Z\d+-]*:\/\//.test(normalized) ? normalized : `https://${normalized}`); }
    catch { throw new ClientError('Backend URL is invalid.'); }
    if (!supportedProtocols.has(url.protocol)) throw new ClientError('Backend URL is invalid.');
  }
  // The transport takes only an origin, never URL userinfo or paths.
  url.protocol = url.protocol === 'ws:' ? 'http:' : url.protocol === 'wss:' ? 'https:' : url.protocol;
  return { origin: url.origin, credential };
}
