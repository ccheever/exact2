// packages/shared/src/remote.ts at T3 Code 1e2ecbd975: the pairingUrl branch of
// resolveRemotePairingTarget. Exact uses ClientError instead of Effect's tagged
// errors and returns the native bridge's origin/credential pair. Settings keeps
// its separate host-plus-code parser in protocol.ts.
import { ClientError } from './protocol';

const supportedProtocols = new Set(['http:', 'https:', 'ws:', 'wss:']);

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
  // The native transport takes only an origin, never URL userinfo or paths.
  url.protocol = url.protocol === 'ws:' ? 'http:' : url.protocol === 'wss:' ? 'https:' : url.protocol;
  return { origin: url.origin, credential };
}
