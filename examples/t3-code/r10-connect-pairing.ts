// Lane r10-connect: Add Environment's Host field takes a full pairing URL and fills both fields
// (ConnectionsSettings.tsx handleSavedBackendHostChange → parsePairingUrlFields, T3 Code MIT, see
// LICENSE-T3): a hosted pairing link (`?host=…` with a token) gives its host, any other URL with a
// token gives its origin; the token is the pairing code. `source` is the text this answer is for,
// so the dialog uses it only while the field still holds that text.

import { escapedUrl } from './protocol';

export type PairingFields = { source: string; host: string; code: string };

/**
 * parsePairingUrlFields: `host` is '' when `input` is not a pairing URL. A link whose host has spaces, which URL refuses,
 * is split as the reference's Chromium URL splits it: the host escaped (`https://not%20a%20url`), the token read (escapedUrl).
 */
export function pairingFields(input: string): PairingFields {
  const none = { source: input, host: '', code: '' };
  const trimmed = input.trim();
  if (!trimmed) return none;
  const address = /^[a-zA-Z][a-zA-Z\d+.-]*:\/\//u.test(trimmed) || trimmed.startsWith('//') ? trimmed : `https://${trimmed}`;
  let url: { origin: string; query: URLSearchParams; fragment: URLSearchParams } | null;
  try {
    const parsed = new URL(address, 'http://localhost');
    url = { origin: parsed.origin, query: parsed.searchParams, fragment: new URLSearchParams(parsed.hash.replace(/^#/, '')) };
  } catch { url = escapedUrl(address); }
  if (!url) return none;
  const token = url.fragment.get('token')?.trim() || (url.query.get('token')?.trim() ?? '');
  if (!token) return none;
  const hosted = url.query.get('host')?.trim() ?? '';
  return { source: input, host: hosted || url.origin, code: token };
}
