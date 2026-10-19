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

/** The app directory `argv` names, as resolveApp (app.mjs) reads it: `--app`'s
 * path, else EXACT_APP_DIR, else `--app`'s name under apps/; null for none.
 * `at` is `--app`'s value's index when it is a path (which resolveApp would
 * take over EXACT_APP_DIR). */
function namedDir(argv) {
  const flag = argv.indexOf('--app'), app = flag < 0 ? null : argv[flag + 1];
  if (app && /[\\/]|^\.\.?$/.test(app)) return { dir: resolve(app), at: flag + 1 };
  if (process.env.EXACT_APP_DIR) return { dir: resolve(process.env.EXACT_APP_DIR) };
  return app ? { dir: resolve(ROOT, 'apps', app.replace(/-(web|apple|ios|macos|linux|windows|gpu)$/, '')) } : null;
}

/** `argv` (a `verb`'s: agent or verify, its host first) for a drive of this
 * checkout's design tool: EXACT_TOOL_PORT set (the checkout's unless named),
 * EXACT_APP_DIR and EXACT_WEB_DIST the stage's, and each `--test` file mapped
 * into it. Any other app, EXACT_TOOL_PORT=7740 or a request for help is left
 * as it is. */
export async function toolDrive(verb, argv) {
  if (argv.includes('--help') || argv.includes('-h')) return argv;
  const named = namedDir(argv), tool = resolve(ROOT, 'apps/design-tool');
  if (!named || !existsSync(named.dir) || !existsSync(tool) || realpathSync(named.dir) !== realpathSync(tool)) return argv;
  process.env.EXACT_TOOL_PORT ??= String(checkoutPort(ROOT));
  const { stageToolTests, testFiles } = await import('./tamber-tool-port.mjs');
  const tested = testFiles(verb, argv, (await import('./help.mjs')).knownFlags(verb));
  const staged = stageToolTests(named.dir, argv, i => tested.has(i));
  if (!staged) return argv;
  process.env.EXACT_APP_DIR = staged.dir;
  // A path would name the tool's own directory again (resolveApp takes it
  // over EXACT_APP_DIR): its name, which the stage's directory answers.
  return named.at === undefined ? staged.args : staged.args.map((a, i) => (i === named.at ? 'design-tool' : a));
}
