import { expect, test } from 'bun:test';
import { presentProviderGoal } from '../../src/composer/provider-goal';

test('goal tokens that round to a thousand thousands read as millions', () => {
  const usage = (tokensUsed: number) => presentProviderGoal({ objective: 'Ship', status: 'active', tokensUsed }, true).usage;
  expect([999, 999_499, 999_500, 999_999, 1_000_000, 1_250_000].map(usage))
    .toEqual(['999 tokens', '999k tokens', '1m tokens', '1m tokens', '1m tokens', '1.3m tokens']);
});
