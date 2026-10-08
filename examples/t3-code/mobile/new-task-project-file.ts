// Pinned365aa87982 shared/schemaJson.ts + contracts/t3ProjectFile.ts.
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
import type { Obj } from './shared/domain';

const object = (value: unknown): value is Obj => value !== null && typeof value === 'object' && !Array.isArray(value);
const has = (value: Obj, key: string) => Object.prototype.hasOwnProperty.call(value, key);
const icons = ['play', 'test', 'lint', 'configure', 'build', 'debug'];
function trimmed(value: unknown, max = Infinity): string {
  // Source validates both encoded and decoded strings, so padding counts toward
  // iconPath's limit even though the returned path is trimmed.
  if (typeof value !== 'string' || !value.length || value.length > max || !value.trim()) throw new Error('Invalid project string.');
  return value.trim();
}
function literal(value: unknown, choices: readonly string[]): string {
  if (typeof value !== 'string' || !choices.includes(value)) throw new Error('Invalid project value.');
  return value;
}
function script(value: unknown): Obj {
  if (!object(value)) throw new Error('Invalid project script.');
  const result: Obj = { name: trimmed(value.name), command: trimmed(value.command) };
  if (has(value, 'icon')) result.icon = literal(value.icon, icons);
  if (has(value, 'previewUrl')) result.previewUrl = trimmed(value.previewUrl);
  for (const key of ['runOnWorktreeCreate', 'async', 'autoOpenPreview']) if (has(value, key)) {
    if (typeof value[key] !== 'boolean') throw new Error('Invalid project script flag.');
    result[key] = value[key];
  }
  return result;
}

/** Source JSONC decoding and whole-file schema validation. Unknown keys are
 * stripped and absent optional fields stay absent; settings resolution owns
 * defaults. The caller owns filesystem size and truncated-read checks. */
export function mobileParseProjectFile(contents: string): Obj | null {
  try {
    // Keep the pinned transformation order and quoted-string alternations. This
    // deliberately follows its JSONC behavior rather than a broader JSON5 parser.
    let stripped = contents.replace(/("(?:[^"\\]|\\.)*")|\/\/[^\n]*/g,
      (match, quoted: string | undefined) => quoted ? match : '');
    stripped = stripped.replace(/("(?:[^"\\]|\\.)*")|\/\*[\s\S]*?\*\//g,
      (match, quoted: string | undefined) => quoted ? match : '');
    stripped = stripped.replace(/("(?:[^"\\]|\\.)*")|,(\s*[}\]])/g,
      (match, quoted: string | undefined, bracket: string | undefined) => quoted ? match : bracket ?? '');
    const value: unknown = JSON.parse(stripped);
    if (!object(value)) return null;
    const result: Obj = {};
    if (has(value, '$schema')) {
      if (typeof value.$schema !== 'string') return null;
      result.$schema = value.$schema;
    }
    if (has(value, 'iconPath')) result.iconPath = trimmed(value.iconPath, 512);
    if (has(value, 'defaultThreadEnvMode')) result.defaultThreadEnvMode = literal(value.defaultThreadEnvMode, ['local', 'worktree']);
    if (has(value, 'worktreeSubmodules')) result.worktreeSubmodules = literal(value.worktreeSubmodules, ['recursive', 'top-level', 'none']);
    if (has(value, 'scripts')) {
      if (!Array.isArray(value.scripts) || value.scripts.length > 50) return null;
      result.scripts = value.scripts.map(script);
    }
    return result;
  } catch { return null; }
}
