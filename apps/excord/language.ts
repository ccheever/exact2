// A message's language for Translate (LLP 1133.027 §10): the device's language
// detector, made once and kept, read when a message's menu opens. A language
// under 0.7 confidence, or `und`, is none, so Translate is not offered for a
// message the detector is unsure of.
import { createLanguageDetector, detectLanguage, languageDetectorAvailability } from '@exact/translation';

let detector: string | null = null;

/** English names for the languages people most often see here, where the runtime has no `Intl.DisplayNames`. */
const NAMES: Record<string, string> = {
  ar: 'Arabic', de: 'German', en: 'English', es: 'Spanish', fr: 'French', hi: 'Hindi', it: 'Italian', ja: 'Japanese',
  ko: 'Korean', pt: 'Portuguese', ru: 'Russian', zh: 'Chinese', 'zh-Hant': 'Chinese (Traditional)',
};

function nameOf(tag: string): string {
  try {
    const name = new Intl.DisplayNames(undefined, { type: 'language' }).of(tag);
    if (name && name !== tag) return name;
  } catch {
    // No `Intl.DisplayNames` here: the table, then the tag.
  }
  return NAMES[tag] ?? NAMES[tag.split('-')[0]] ?? tag;
}

export async function messageLanguage(text: string): Promise<{ language: string; name: string; detector: string }> {
  const none = (availability: string) => ({ language: '', name: '', detector: availability });
  const { availability } = await languageDetectorAvailability({});
  if (availability !== 'available' || text.trim() === '') return none(availability);
  try {
    detector ??= (await createLanguageDetector({})).id;
    const [top] = (await detectLanguage({ detector, input: text })).results;
    if (!top || top.detectedLanguage === 'und' || top.confidence < 0.7) return none(availability);
    return { language: top.detectedLanguage, name: nameOf(top.detectedLanguage), detector: availability };
  } catch {
    // A detector the platform dropped is made again on the next message.
    detector = null;
    return none(availability);
  }
}
