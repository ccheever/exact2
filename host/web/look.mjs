// The web's neutral presentation, composed after the shared browser resets.
// @ref LLP 1115.000 D4 part B
import { readFileSync } from 'node:fs';

/** The shell inputs recorded by a web build's freshness receipt. */
export const SHELL_SOURCES = ['host/web/index.html', 'host/web/neutral.css', 'host/web/look.mjs'];

/** Append the neutral sheet before the build minifies the shell's styles. */
export function withLook(shell) {
  const at = shell.indexOf('</style>');
  if (at < 0) throw new Error('the shell has no <style> to carry the neutral look');
  return shell.slice(0, at) + readFileSync(new URL('./neutral.css', import.meta.url), 'utf8') + shell.slice(at);
}
