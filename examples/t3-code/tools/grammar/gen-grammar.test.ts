import { expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

// shiki-residuals: the committed grammar data is what the generator makes from the pinned packages.
// The app build never needs them: without `bun install --frozen-lockfile` in tools/grammar this skips.
const installed = existsSync(join(import.meta.dir, 'node_modules', 'oniguruma-to-es'));
test.skipIf(!installed)('the generator reproduces the committed grammar data byte for byte', async () => {
  const { generate } = await import('./gen-grammar.mjs');
  for (const { file, text } of await generate()) expect(text === readFileSync(file, 'utf8')).toBe(true);
}, 120_000);

test('the generator pins the six packages at the integrity values the reference locks', () => {
  const lock = readFileSync(join(import.meta.dir, 'bun.lock'), 'utf8');
  for (const [name, integrity] of [
    ['@shikijs/langs@4.2.0', 'bwrVRlJ0wUhZxAbVdvBbv2TTC9yLsh4C/IO5Ofz0T8MQntgDvyVnkbjw9vi50r1kx7RCIJdnJnjZAwmAsXFLZQ=='],
    ['@pierre/theme@1.1.0', 'GC2OWTAfTIIWWYhPCygwG8t2EtePQkRfON4MI2rwIkJylmiyqIttJID2dCL8sUD8cNdEvYkEyfEHHKMeCiDLoQ=='],
    ['oniguruma-to-es@4.3.6', 'csuQ9x3Yr0cEIs/Zgx/OEt9iBw9vqIunAPQkx19R/fiMq2oGVTgcMqO/V3Ybqefr1TBvosI6jU539ksaBULJyA=='],
    ['oniguruma-parser@0.12.2', '6HVa5oIrgMC6aA6WF6XyyqbhRPJrKR02L20+2+zpDtO5QAzGHAUGw5TKQvwi5vctNnRHkJYmjAhRVQF2EKdTQw=='],
    ['regex@6.1.0', '6VwtthbV4o/7+OaAF9I5L5V3llLEsoPyq9P1JVXkedTP33c7MfCG0/5NOPcSJn0TzXcG9YUrR0gQSWioew3LDg=='],
    ['regex-recursion@6.0.2', '0YCaSCq2VRIebiaUviZNs0cBz1kg5kVS2UKUfNIx8YVs1cN3AV7NTctO5FOKBA+UT2BPJIWZauYHPqJODG50cg=='],
  ]) {
    const entry = lock.split('\n').find(line => line.includes(`["${name}"`)) ?? '';
    expect(entry).toContain(`sha512-${integrity}`);
  }
});
