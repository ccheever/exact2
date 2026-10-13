// The web's default look (LLP 1115.000 D4 part B): which designed look a web
// build draws where the author says nothing. `neutral` is no platform's:
// `neutral.css` after the shell's sheet, and the role table's neutral column
// (kernel/tables/schema.json `colors`). `apple` is LLP 1116 D1's, the shell's
// own stylesheet (index.html), which imitates iOS.
import { readFileSync } from 'node:fs';

// The switch, thrown on 2026-10-12 (Charlie, on seeing the two side by side:
// "switch to the neutral look"). `EXACT_WEB_LOOK=apple` still builds the look
// it replaced, byte for byte. No host draws that one since the hosts that
// paint their own controls took the neutral look (LLP 1115.000 D17), so it
// can go: QUEUE.md says what goes with it.
export const DEFAULT_LOOK = 'neutral';
export const LOOK = process.env.EXACT_WEB_LOOK || DEFAULT_LOOK;
if (!['apple', 'neutral'].includes(LOOK)) throw new Error(`EXACT_WEB_LOOK=${LOOK}: the looks are apple and neutral`);

/** What a build's shell is made of, for a receipt that says when the build is stale (a wasm bake's, scripts/app.mjs):
 * its sources as repository paths, and the look it was built in where that is not the default one, which is a
 * setting, so no file's digest says it. */
export const SHELL_SOURCES = ['host/web/index.html', 'host/web/neutral.css', 'host/web/look.mjs'];
export const LOOK_SETTING = LOOK === DEFAULT_LOOK ? {} : { EXACT_WEB_LOOK: LOOK };

/** The shell (index.html's text) as `look` builds it: unchanged for `apple`, and for `neutral` with the neutral sheet
 * at the end of the shell's own `<style>`, where each build's minifier takes it with the rest. The sheet's first rule
 * marks the page (`--exact-look: neutral` on the root), which is how a page says which look it is: a wasm page's glue
 * reads it at boot and its kernel shows the neutral column (navigation.js `foldBits`), as the JS target's compiler
 * does from this build's `EXACT_WEB_LOOK` (host/web-js/src/main.rs), and the render host from the shell it is given. */
export function withLook(shell, look = LOOK) {
  if (look !== 'neutral') return shell;
  const at = shell.indexOf('</style>');
  if (at < 0) throw new Error('the shell has no <style> to carry the neutral look');
  return shell.slice(0, at) + readFileSync(new URL('./neutral.css', import.meta.url), 'utf8') + shell.slice(at);
}
