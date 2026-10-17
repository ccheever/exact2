import { expect, test } from 'bun:test';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// The generated sources are built, not committed: the ones on disk are what the generator makes from
// the pinned packages and the committed inputs (`generate.mjs` writes them; this fails until it has).
test('the generated sources on disk are what the generator makes', () => {
  const run = spawnSync(process.execPath, [join(import.meta.dir, 'generate.mjs'), '--check'], { encoding: 'utf8' });
  expect(run.stderr.split('\n').filter(line => /^(missing|differs|stale): /.test(line))).toEqual([]);
  expect(run.status).toBe(0);
}, 120_000);

// Built, not committed (rules/RULES.md): .gitignore lists every file a generator writes, and git tracks none.
test('git ignores every generated source and tracks none', () => {
  const list = spawnSync(process.execPath, ['-e', `const { outputs } = await import(${JSON.stringify(join(import.meta.dir, 'generate.mjs'))});
console.log((await outputs()).files.map(({ file }) => file).join('\\n'));`], { encoding: 'utf8' });
  expect(list.stderr).toBe('');
  const files = list.stdout.trim().split('\n');
  expect(files.length).toBeGreaterThan(20);
  const git = (...args: string[]) => spawnSync('git', args, { cwd: import.meta.dir, encoding: 'utf8' }).stdout.trim().split('\n').filter(Boolean);
  expect(git('check-ignore', '--no-index', ...files).length).toBe(files.length);
  expect(git('ls-files', '--', ...files)).toEqual([]);
}, 120_000);

test('the generator pins its packages at the integrity values the reference locks', () => {
  const lock = readFileSync(join(import.meta.dir, 'bun.lock'), 'utf8');
  for (const [name, integrity] of [
    ['@shikijs/langs@4.2.0', 'bwrVRlJ0wUhZxAbVdvBbv2TTC9yLsh4C/IO5Ofz0T8MQntgDvyVnkbjw9vi50r1kx7RCIJdnJnjZAwmAsXFLZQ=='],
    ['@pierre/theme@1.1.0', 'GC2OWTAfTIIWWYhPCygwG8t2EtePQkRfON4MI2rwIkJylmiyqIttJID2dCL8sUD8cNdEvYkEyfEHHKMeCiDLoQ=='],
    ['oniguruma-to-es@4.3.6', 'csuQ9x3Yr0cEIs/Zgx/OEt9iBw9vqIunAPQkx19R/fiMq2oGVTgcMqO/V3Ybqefr1TBvosI6jU539ksaBULJyA=='],
    ['oniguruma-parser@0.12.2', '6HVa5oIrgMC6aA6WF6XyyqbhRPJrKR02L20+2+zpDtO5QAzGHAUGw5TKQvwi5vctNnRHkJYmjAhRVQF2EKdTQw=='],
    ['regex@6.1.0', '6VwtthbV4o/7+OaAF9I5L5V3llLEsoPyq9P1JVXkedTP33c7MfCG0/5NOPcSJn0TzXcG9YUrR0gQSWioew3LDg=='],
    ['regex-recursion@6.0.2', '0YCaSCq2VRIebiaUviZNs0cBz1kg5kVS2UKUfNIx8YVs1cN3AV7NTctO5FOKBA+UT2BPJIWZauYHPqJODG50cg=='],
    ['lucide@0.564.0', 'FasyXKHWon773WIl3HeCQpd5xS6E0aLjqxiQStlHNKktni+HDncc1sqY+6vRUbCfmDsIaKQz43EEQLAUDLZO0g=='],
    ['lucide-react@0.564.0', 'JJ8GVTQqFwuliifD48U6+h7DXEHdkhJ/E87kksGByII3qHxtPciVb8T8woQONHBQgHVOl7rSMrrip3SeVNy7Fg=='],
    ['@pierre/trees@1.0.0-beta.4', 'OfT1yk9ne8Te5+GB5zUY8yqE6B8BqjBHQJleH4lu8ltwNpoocZl4vXt1AzlEExpxI/pp+AFX5QG+lR3JjtTEag=='],
    ['morphicons@1.7.1', 'q5ylxy5/d7vBg0OAzanlooXf05PekovMYDuuQVpr6vAQZxl99lrJbaIi+jJ32PXQf9WEEaDO2pbNBsx1ZhEnFQ=='],
  ]) {
    const entry = lock.split('\n').find(line => line.includes(`["${name}"`)) ?? '';
    expect(entry).toContain(`sha512-${integrity}`);
  }
});
