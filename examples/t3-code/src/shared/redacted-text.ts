// RedactedSensitiveText, T3 Code 1e2ecbd975 (MIT, see LICENSE-T3):
// apps/web/src/components/settings/RedactedSensitiveText.tsx:6-25. The hidden
// text is a deterministic string of the value's shape: an FNV-1a seed from the
// value, one mixed letter per character from the alphabet, `@ . - _` kept.
// Changes: the placeholder is computed here and handed to Contract's
// RedactedText (redacted-text.contract), which owns the reveal flag.
const REDACTED_TEXT_ALPHABET = 'abcdefghjkmnpqrstuvwxyz23456789';

export function redactedPlaceholder(value: string): string {
  let state = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    state ^= value.charCodeAt(index);
    state = Math.imul(state, 0x01000193);
  }
  const nextChar = () => {
    state = Math.imul(state ^ (state >>> 13), 0x85ebca6b);
    state = Math.imul(state ^ (state >>> 16), 0xc2b2ae35);
    return REDACTED_TEXT_ALPHABET[Math.abs(state) % REDACTED_TEXT_ALPHABET.length] ?? 'x';
  };
  return Array.from(value, char => (char === '@' || char === '.' || char === '-' || char === '_' ? char : nextChar())).join('');
}

/** What RedactedText draws: the trimmed value and its placeholder; both '' when there is no value (the reference renders nothing). */
export function redactedValue(value: unknown): { value: string; placeholder: string } {
  const text = typeof value === 'string' ? value.trim() : '';
  return { value: text, placeholder: text ? redactedPlaceholder(text) : '' };
}
