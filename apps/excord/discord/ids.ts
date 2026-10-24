const DISCORD_EPOCH = 1420070400000;
const TIME_SHIFT = 4194304;

/** Compare canonical decimal snowflakes without losing their low bits. */
export function cmpId(a: string, b: string): number {
  return a.length === b.length ? (a < b ? -1 : a > b ? 1 : 0) : a.length - b.length;
}

/** Encode an integer Unix millisecond and the snowflake's low 22 bits (zero by default). */
export function snowflakeFromMs(ms: number, lowBits = 0): string {
  if (!Number.isSafeInteger(ms) || ms < DISCORD_EPOCH) throw new RangeError('Invalid snowflake time');
  if (!Number.isInteger(lowBits) || lowBits < 0 || lowBits >= TIME_SHIFT) throw new RangeError('Invalid snowflake low bits');
  return ((BigInt(ms - DISCORD_EPOCH) << 22n) | BigInt(lowBits)).toString();
}

/** Initialize once per session as { lastMs: null, sequence: 0 }; share across its sends. */
export interface NonceState { lastMs: number | null; sequence: number }

/** Allocate for a new send; the caller keeps this nonce with the pending send for every retry. */
export function makeNonce(nowMs: number, state: NonceState): string {
  if (!Number.isSafeInteger(nowMs) || nowMs < DISCORD_EPOCH) throw new RangeError('Invalid snowflake time');
  let ms = Math.max(nowMs, state.lastMs ?? nowMs);
  let sequence = ms === state.lastMs ? state.sequence + 1 : 0;
  // Keep logical time when the clock steps back; carry a full 22-bit sequence into the next ms.
  if (sequence >= TIME_SHIFT) {
    ms++;
    sequence = 0;
  }
  const nonce = snowflakeFromMs(ms, sequence);
  state.lastMs = ms;
  state.sequence = sequence;
  return nonce;
}

/** Read a snowflake's Unix millisecond timestamp without rounding its decimal id. */
export function msFromSnowflake(id: string): number {
  if (!/^\d+$/.test(id)) throw new RangeError('Invalid snowflake');
  let remainder = 0;
  let quotient = 0;
  for (const digit of id) {
    const value = remainder * 10 + Number(digit);
    quotient = quotient * 10 + Math.floor(value / TIME_SHIFT);
    remainder = value % TIME_SHIFT;
  }
  const ms = quotient + DISCORD_EPOCH;
  if (!Number.isSafeInteger(ms)) throw new RangeError('Snowflake timestamp exceeds integer precision');
  return ms;
}
