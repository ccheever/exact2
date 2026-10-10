// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/r10-connect-pairing.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Lane r10-connect: Add Environment's Host field takes a full pairing URL and fills both fields
// (ConnectionsSettings.tsx handleSavedBackendHostChange → parsePairingUrlFields, T3 Code MIT, see
// LICENSE-T3): a hosted pairing link (`?host=…` with a token) gives its host, any other URL with a
// token gives its origin; the token is the pairing code. `source` is the text this answer is for,
// so the dialog uses it only while the field still holds that text.

export type PairingFields = { source: string; host: string; code: string };

/** parsePairingUrlFields: `host` is '' when `input` is not a pairing URL. */
export function pairingFields(input: string): PairingFields {
  const none = { source: input, host: '', code: '' };
  const trimmed = input.trim();
  if (!trimmed) return none;
  try {
    const url = new URL(/^[a-zA-Z][a-zA-Z\d+.-]*:\/\//u.test(trimmed) || trimmed.startsWith('//') ? trimmed : `https://${trimmed}`, 'http://localhost');
    const hashToken = new URLSearchParams(url.hash.replace(/^#/, '')).get('token')?.trim() ?? '';
    const token = hashToken || (url.searchParams.get('token')?.trim() ?? '');
    if (!token) return none;
    const hosted = url.searchParams.get('host')?.trim() ?? '';
    return { source: input, host: hosted || url.origin, code: token };
  } catch { return none; }
}
