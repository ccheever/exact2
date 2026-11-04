/** A round expires only on a newer app clock, strictly past its budget. */
export function staleRound(since: number | undefined, now: number | undefined, staleRoundMs: number): boolean {
  return since !== undefined && now !== undefined && Number.isFinite(since) && Number.isFinite(now)
    && now > since && now - since > staleRoundMs;
}
