// Supported source365aa87982 context payloads shared by draft ownership and capture.
// @ref llp/1109.005-composer-and-transcript.decision.md#foreground-capture-facts
import { obj, str, type Obj } from './shared/domain';
import { mobileImportedContextRecordValid } from './composer-context-record-admission';

const bounded = (value: unknown, max: number) => typeof value === 'string' && value.length <= max;
const nonempty = (value: unknown, max = Infinity) => bounded(value, max) && String(value).trim() === value && String(value).length > 0;
const integer = (value: unknown) => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
const id = (value: unknown) => nonempty(value, 128) && /^[a-z0-9_-]+$/i.test(String(value));

/** Canonical source records supported by producers and decoded clipboard imports.
 * Payload validity grants no captured editor or attachment ownership. */
export function mobileContextRecordValid(record: Obj): boolean {
  if (record.version !== 1 || !id(record.contextId) || !bounded(record.label, 200)) return false;
  switch (record.kind) {
    case 'image': case 'file': return id(record.attachmentId) && nonempty(record.name, 255) &&
      nonempty(record.mimeType, 100) && integer(record.sizeBytes);
    case 'thread': return nonempty(record.environmentId) && nonempty(record.threadId) && bounded(record.title, 200);
    case 'terminal': return nonempty(record.terminalId, 255) && nonempty(record.terminalLabel, 255) &&
      integer(record.lineStart) && integer(record.lineEnd) && Number(record.lineEnd) >= Number(record.lineStart) && bounded(record.text, 64000);
    case 'mention': return nonempty(record.path, 2048);
    case 'skill': return nonempty(record.name, 255);
    case 'review-comment': {
      const pr = record.pullRequest === undefined ? null : obj(record.pullRequest);
      return nonempty(record.sectionId, 255) && bounded(record.sectionTitle, 2048) && nonempty(record.filePath, 2048) &&
        integer(record.startIndex) && integer(record.endIndex) && Number(record.endIndex) >= Number(record.startIndex) &&
        bounded(record.rangeLabel, 2048) && bounded(record.text, 16000) && bounded(record.diff, 32000) &&
        (record.fenceLanguage === undefined || bounded(record.fenceLanguage, 64)) && (!pr || integer(pr.number) && Number(pr.number) > 0 &&
          ['title', 'url', 'headBranch', 'baseBranch'].every(key => bounded(pr[key], 2048)) &&
          ['open', 'closed', 'merged'].includes(str(pr.state)) && typeof pr.isDraft === 'boolean');
    }
    default: return mobileImportedContextRecordValid(record);
  }
}
