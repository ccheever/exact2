// The name a model is said by (T3 Code 7c3b3b3605ef, MIT; see LICENSE-T3): packages/provider-core/src/modelShortName.ts,
// client-runtime providerClients.ts resolveModelShortName and the drivers' modelFamilies, and chat/providerIconUtils.ts
// getTriggerDisplayModelName. The sidebar's copy until the composer's model label takes the driver.
import { str, type Obj } from '../client/domain';

const FAMILY_PREFIXES = ['Claude ', 'GPT-', 'GPT ', 'Gemini ', 'Muse ', 'Grok '] as const;
/** Each driver's own model families (its client's `modelFamilies`); fx, OpenCode, Pi and ACP agents name none. */
const MODEL_FAMILIES: Record<string, readonly string[]> = {
  codex: ['GPT-', 'GPT '], claudeAgent: ['Claude '], cursor: ['Grok '], grok: ['Grok '], antigravity: ['Gemini '], muse: ['Muse '],
};
// Words that describe a variant within a family rather than name a model. Off the
// family's native provider there is no icon saying which family "4 Pro" belongs to.
const VARIANT_WORDS = new Set(['pro', 'flash', 'mini', 'max', 'lite', 'nano', 'ultra', 'plus', 'fast', 'turbo', 'preview', 'latest',
  'thinking', 'reasoning', 'build', 'code', 'image', 'vision', 'audio', 'live', 'custom', 'tools', 'exp', 'experimental', 'chat', 'tts',
  'stt', 'non', 'transcribe']);
const VERSION = /^v?\d+(?:\.\d+)*[a-z]?$/i;
const VENDOR_PREFIX = /^[A-Z][\w.&\- ]{0,24}: (?=\S)/;

const nonVersionWords = (text: string) => text.replace(/\(.*?\)/g, ' ').split(/[\s-]+/).filter(word => word.length > 0 && !VERSION.test(word));
const namesModel = (text: string) => nonVersionWords(text).some(word => !VARIANT_WORDS.has(word.toLowerCase()));
function withoutFamily(name: string, nativeFamilies: readonly string[]): string | undefined {
  const prefix = FAMILY_PREFIXES.find(candidate => name.startsWith(candidate));
  if (!prefix) return undefined;
  const rest = name.slice(prefix.length).trim();
  if (!(nativeFamilies.includes(prefix) ? nonVersionWords(rest).length > 0 : namesModel(rest))) return undefined;
  return /\s/.test(rest) ? rest : rest.replaceAll('-', ' ');
}
export function modelShortName(name: string, nativeFamilies: readonly string[]): string | undefined {
  const unprefixed = name.replace(VENDOR_PREFIX, '');
  const short = withoutFamily(unprefixed, nativeFamilies) ?? (namesModel(unprefixed) ? unprefixed : name);
  return short === name ? undefined : short;
}
/** resolveModelShortName: the server's short name, else the one the name gives under the driver's families. */
export function resolveModelShortName(model: Obj, driver: string): string | undefined {
  return str(model.shortName) || modelShortName(str(model.name), MODEL_FAMILIES[driver] ?? []);
}
/** stripLeadingQualifier: a sub-provider's name leading the label ("OpenRouter: …") goes. */
export function stripQualifier(value: string, qualifier: unknown): string {
  const trimmed = str(qualifier).trim();
  if (!trimmed) return value;
  return value.replace(new RegExp(`^${trimmed.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(?:\\s*[.:/-]\\s*|\\s+)`, 'iu'), '').trim() || value;
}
/** getTriggerDisplayModelName. */
export function triggerDisplayModelName(model: Obj, driver: string): string {
  return stripQualifier(resolveModelShortName(model, driver) || str(model.name), model.subProvider);
}
