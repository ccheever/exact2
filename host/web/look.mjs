// The web's neutral presentation, composed after the shared browser resets.
// @ref LLP 1115.000 D4 part B
import { readFileSync } from 'node:fs';

/** The shell inputs recorded by a web build's freshness receipt. */
export const SHELL_SOURCES = ['host/web/index.html', 'host/web/neutral.css', 'host/web/look.mjs', 'kernel/tables/schema.json'];

/** Append the neutral sheet before the build minifies the shell's styles. */
export function withLook(shell) {
  const at = shell.indexOf('</style>');
  if (at < 0) throw new Error('the shell has no <style> to carry the neutral look');
  return shell.slice(0, at) + readFileSync(new URL('./neutral.css', import.meta.url), 'utf8') + appleColors() + shell.slice(at);
}

/** Exact's colour roles in Apple's palette, in every browser (LLP 1115.000 D4): the schema's pairs, measured from
 * UIKit, and in Safari the system's own where WebKit names one (`-apple-system-*`). Each is held as
 * `--exact-apple-<role>`, which an app's theme (LLP 1095 D4) leaves alone, for the glue to report to the kernel;
 * `--exact-apple-roles` names them. */
export function appleColors() {
  const { colors } = JSON.parse(readFileSync(new URL('../../kernel/tables/schema.json', import.meta.url), 'utf8'));
  // Exact's roles (CSS's system colours are the browser's), less the tint's, which the accent draws
  const roles = colors.filter(([name, , ios]) => /^[a-z]/.test(name) && !ios.startsWith('@tint'));
  const pairs = roles.map(([name, , , , light, dark]) => `--exact-apple-${name}: light-dark(${light}, ${dark}); --exact-${name}: var(--exact-apple-${name});`).join(' ');
  // The backgrounds are UIKit's: WebKit on a Mac resolves all six to one colour, so a card would not stand out
  const own = roles.filter(([name, alias]) => alias && !name.endsWith('background')).map(([name, alias]) => `  @supports (color: ${alias}) { :root { --exact-apple-${name}: ${alias}; } }\n`);
  return `  :root { ${pairs} --exact-apple-roles: "${roles.map(([name]) => name).join(' ')}"; }\n${own.join('')}`;
}
