const DISCORD_EPOCH = 1420070400000;
const TIME_SHIFT = 4194304;

/** Compare canonical decimal snowflakes without losing their low bits. */
export function cmpId(a: string, b: string): number {
  return a.length === b.length ? (a < b ? -1 : a > b ? 1 : 0) : a.length - b.length;
}

/** Return the first snowflake in an integer millisecond since the Unix epoch. */
export function snowflakeFromMs(ms: number): string {
  if (!Number.isSafeInteger(ms) || ms < DISCORD_EPOCH) throw new RangeError('Invalid snowflake time');
  return (BigInt(ms - DISCORD_EPOCH) << 22n).toString();
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
