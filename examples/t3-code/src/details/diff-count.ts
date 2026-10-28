// formatDiffCount (T3 Code 7c3b3b3605ef, MIT, see LICENSE-T3: packages/client-runtime/src/diffCount.ts): a +/- or file
// count as the runtime's default locale groups it. The packaged app ships only the en-US locale
// (scripts/build-desktop-artifact.ts DESKTOP_ELECTRON_LANGUAGES), so that default is en-US on every Mac: RUNTIME_LOCALE,
// not the Mac's region, which this runtime's default follows (shared/timestamp-format.ts).
import { RUNTIME_LOCALE } from '../shared/timestamp-format';

let format: Intl.NumberFormat | undefined; // made at the first count, not as the module loads
export const formatDiffCount = (count: number): string => (format ??= new Intl.NumberFormat(RUNTIME_LOCALE)).format(count);
