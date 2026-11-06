// A model's short name, the one you would say out loud (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3:
// packages/provider-core/src/modelShortName.ts, packages/client-runtime/src/providerClients.ts
// `resolveModelShortName` and each provider client's `modelFamilies`). The server's `shortName` wins; otherwise
// a family prefix the provider's own icon already shows ("Claude " on Claude) is dropped while a model word is
// left, and off its native provider only a name that still names a model sheds its family.

const FAMILY_PREFIXES = ['Claude ', 'GPT-', 'GPT ', 'Gemini ', 'Muse ', 'Grok '];

/** Model-name families each driver's icon already shows (the provider clients' `modelFamilies`). */
export const MODEL_FAMILIES: Record<string, readonly string[]> = {
  codex: ['GPT-', 'GPT '], claudeAgent: ['Claude '], cursor: ['Grok '], grok: ['Grok '], antigravity: ['Gemini '], muse: ['Muse '],
};

// Words that describe a variant within a family rather than name a model. Off the family's native provider there is
// no icon saying which family "4 Pro" belongs to.
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
  const keep = nativeFamilies.includes(prefix) ? nonVersionWords(rest).length > 0 : namesModel(rest);
  if (!keep) return undefined;
  return /\s/.test(rest) ? rest : rest.replaceAll('-', ' ');
}

/** modelShortName: undefined when the short name would be the name itself. */
export function modelShortName(name: string, nativeFamilies: readonly string[]): string | undefined {
  const unprefixed = name.replace(VENDOR_PREFIX, '');
  const short = withoutFamily(unprefixed, nativeFamilies) ?? (namesModel(unprefixed) ? unprefixed : name);
  return short === name ? undefined : short;
}

/** resolveModelShortName: the server's `shortName`, else the spoken name on `driver`'s icon. */
export function resolveModelShortName(model: { name?: unknown; shortName?: unknown }, driver: string): string | undefined {
  if (typeof model.shortName === 'string' && model.shortName) return model.shortName;
  return modelShortName(typeof model.name === 'string' ? model.name : '', MODEL_FAMILIES[driver] ?? []);
}
