// Isolated CLI transport for subprocess tests. Production has one fixed state root.
import { main } from './coordination.mjs';
try {
  const result = await main(process.argv.slice(2), {
    root: process.env.COORDINATION_FIXTURE_ROOT,
    freeBytes: process.env.COORDINATION_FIXTURE_FREE ? () => Number(process.env.COORDINATION_FIXTURE_FREE) : undefined,
  });
  console.log(JSON.stringify(result));
  if (result?.code !== undefined) process.exitCode = result.code ?? 1;
} catch (error) { console.error(error.message); process.exitCode = 1; }
