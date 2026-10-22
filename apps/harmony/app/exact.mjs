// `bun exact.mjs <verb>` for Harmony, as an app `exact new --snapback` made has
// it (scripts/app-cli.mjs `run`), on this checkout; `bun exact.mjs help` lists
// the verbs. The backend is the directory above (LLP 1086).
import { resolve } from 'node:path';

const EXACT2 = resolve(import.meta.dir, '../../..');
const { run } = await import(resolve(EXACT2, 'scripts/app-cli.mjs'));
process.exit(await run({ dir: import.meta.dir, name: 'app', exact2: EXACT2, args: process.argv.slice(2) }));
