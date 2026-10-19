// A drive or verify of the design tool straight from scripts/agent.mjs or
// scripts/verify.mjs (`--app design-tool`, not through its exact.mjs): on the
// checkout's own port, as app-cli.mjs runs it, from the copy staged with that
// origin and its stub (tamber-tool-port.mjs `stageToolTests`), so its stub
// never binds the 7740 a person's tool holds. EXACT_TOOL_PORT names another
// port; EXACT_TOOL_PORT=7740 runs it in place. Kept apart from
// tamber-tool-port.mjs, whose port is read as it loads: this sets it first.
import { existsSync, realpathSync } from 'node:fs';
import { resolve } from 'node:path';
import { checkoutPort } from './port-lock.mjs';

const ROOT = resolve(new URL('..', import.meta.url).pathname);

/** The app directory `argv` names, as resolveApp (app.mjs) reads it:
 * EXACT_APP_DIR, else `--app`'s path or name under apps/; null for none. */
function namedDir(argv) {
  if (process.env.EXACT_APP_DIR) return resolve(process.env.EXACT_APP_DIR);
  const at = argv.findIndex(a => a === '--app' || a.startsWith('--app='));
  const app = at < 0 ? null : argv[at].startsWith('--app=') ? argv[at].slice(6) : argv[at + 1];
  if (!app) return null;
  return /[\\/]|^\.\.?$/.test(app) ? resolve(app) : resolve(ROOT, 'apps', app.replace(/-(web|apple|ios|macos|linux|windows|gpu)$/, ''));
}

/** `argv` (a `verb`'s: agent or verify, its host first) for a drive of this
 * checkout's design tool: EXACT_TOOL_PORT set (the checkout's unless named),
 * EXACT_APP_DIR and EXACT_WEB_DIST the stage's, and each `--test` file mapped
 * into it. Any other app, or EXACT_TOOL_PORT=7740, is left as it is. */
export async function toolDrive(verb, argv) {
  const dir = namedDir(argv), tool = resolve(ROOT, 'apps/design-tool');
  if (!dir || !existsSync(dir) || !existsSync(tool) || realpathSync(dir) !== realpathSync(tool)) return argv;
  process.env.EXACT_TOOL_PORT ??= String(checkoutPort(ROOT));
  const { stageToolTests, testFiles } = await import('./tamber-tool-port.mjs');
  const tested = testFiles(verb, argv, (await import('./help.mjs')).knownFlags(verb));
  const staged = stageToolTests(dir, argv, i => tested.has(i));
  if (!staged) return argv;
  process.env.EXACT_APP_DIR = staged.dir;
  return staged.args;
}
